use crate::ast::{extract_calls, extract_symbols, RustParser};
use crate::db::{reader, writer};
use crate::error::CoreError;
use crate::graph::{crates::parse_cargo_workspace, files::extract_file_edges};
use crate::types::{FileRecord, IndexRequest, IndexResult, Workspace};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Incremental Rust workspace indexer.
pub struct Indexer<'conn> {
    conn: &'conn Connection,
    parser: RustParser,
}

impl<'conn> Indexer<'conn> {
    /// Create a new indexer bound to a database connection.
    pub fn new(conn: &'conn Connection) -> Result<Self, CoreError> {
        let parser = RustParser::new()?;
        Ok(Self { conn, parser })
    }

    /// Run an incremental index pass for the given request.
    pub fn run(&mut self, req: &IndexRequest) -> Result<IndexResult, CoreError> {
        let start = std::time::Instant::now();

        // Upsert workspace record.
        let workspace = Workspace {
            id: req.workspace_id.clone(),
            root_path: req.root_path.clone(),
            name: req.workspace_name.clone(),
            indexed_at: None,
        };
        writer::upsert_workspace(self.conn, &workspace)?;

        let root = Path::new(&req.root_path);
        let workspace_id = &req.workspace_id;

        let mut files_scanned: u32 = 0;
        let mut files_indexed: u32 = 0;
        let mut symbols_extracted: u32 = 0;
        let mut errors: u32 = 0;
        let mut current_files: HashSet<String> = HashSet::new();

        // Walk all .rs files, skipping common build/vendor dirs.
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_excluded(e.path()))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!("walkdir error: {err}");
                    errors += 1;
                    continue;
                }
            };

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            files_scanned += 1;

            let file_path = path.to_string_lossy().replace('\\', "/");
            current_files.insert(file_path.clone());

            // Check mtime.
            let mtime = match path.metadata() {
                Ok(m) => file_mtime(&m),
                Err(err) => {
                    tracing::warn!("metadata error for {file_path}: {err}");
                    errors += 1;
                    continue;
                }
            };

            let file_size = path.metadata().map(|m| m.len() as i64).unwrap_or(0);

            if !req.force {
                if let Some(stored_mtime) =
                    reader::get_file_mtime(self.conn, workspace_id, &file_path)?
                {
                    if stored_mtime == mtime {
                        continue;
                    }
                }
            }

            // Read and parse the file.
            let src = match std::fs::read(path) {
                Ok(b) => b,
                Err(err) => {
                    tracing::warn!("read error for {file_path}: {err}");
                    errors += 1;
                    continue;
                }
            };

            let tree = match self.parser.parse_bytes(&src) {
                Ok(t) => t,
                Err(err) => {
                    tracing::warn!("parse error for {file_path}: {err}");
                    errors += 1;
                    continue;
                }
            };

            // Delete old data for this file.
            writer::delete_file_symbols(self.conn, workspace_id, &file_path)?;

            // Upsert file record (without symbol_count yet).
            let file_rec = FileRecord {
                workspace_id: workspace_id.clone(),
                file_path: file_path.clone(),
                mtime,
                size: file_size,
                symbol_count: 0,
            };
            writer::upsert_file_record(self.conn, &file_rec)?;

            // Extract and insert symbols.
            let syms = extract_symbols(&tree, &src, workspace_id, &file_path);
            let sym_count = syms.len() as u32;

            for sym in &syms {
                match writer::insert_symbol(self.conn, sym) {
                    Ok(sym_id) => {
                        if let Err(err) =
                            writer::insert_symbol_trigrams(self.conn, sym_id, &sym.name)
                        {
                            tracing::warn!("trigram insert error: {err}");
                        }
                    }
                    Err(err) => {
                        tracing::warn!("symbol insert error: {err}");
                    }
                }
            }

            // Update symbol_count in file record.
            let updated_rec = FileRecord {
                symbol_count: sym_count,
                ..file_rec
            };
            writer::upsert_file_record(self.conn, &updated_rec)?;

            // Extract and insert call edges.
            let mut calls = extract_calls(&tree, &src, &file_path);
            for edge in &mut calls {
                edge.workspace_id = workspace_id.clone();
                if let Err(err) = writer::insert_call_edge(self.conn, edge) {
                    tracing::warn!("call edge insert error: {err}");
                }
            }

            // Extract and insert file edges.
            let file_edges = extract_file_edges(&tree, &src, workspace_id, &file_path);
            for edge in &file_edges {
                if let Err(err) = writer::upsert_file_edge(self.conn, edge) {
                    tracing::warn!("file edge insert error: {err}");
                }
            }

            symbols_extracted += sym_count;
            files_indexed += 1;

            tracing::info!(
                "indexed {file_path}: {sym_count} symbols, {} calls, {} file edges",
                calls.len(),
                file_edges.len()
            );
        }

        // Remove stale files.
        writer::delete_stale_files(self.conn, workspace_id, &current_files)?;

        // Parse cargo workspace for crate graph.
        match parse_cargo_workspace(root, workspace_id) {
            Ok((nodes, edges)) => {
                for node in &nodes {
                    if let Err(err) = writer::upsert_crate_node(self.conn, node) {
                        tracing::warn!("crate node insert error: {err}");
                    }
                }
                for edge in &edges {
                    if let Err(err) = writer::upsert_crate_dep_edge(self.conn, edge) {
                        tracing::warn!("crate dep edge insert error: {err}");
                    }
                }
            }
            Err(err) => {
                tracing::warn!("crate graph parse error: {err}");
            }
        }

        // Update workspace indexed_at.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let updated_ws = Workspace {
            id: workspace_id.clone(),
            root_path: req.root_path.clone(),
            name: req.workspace_name.clone(),
            indexed_at: Some(now),
        };
        writer::upsert_workspace(self.conn, &updated_ws)?;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            "index complete for workspace={workspace_id}: \
             scanned={files_scanned}, indexed={files_indexed}, \
             symbols={symbols_extracted}, errors={errors}, elapsed={elapsed_ms}ms"
        );

        Ok(IndexResult {
            workspace_id: workspace_id.clone(),
            files_scanned,
            files_indexed,
            symbols_extracted,
            errors,
            elapsed_ms,
        })
    }
}

fn is_excluded(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("target") | Some(".git") | Some("node_modules") | Some(".cargo")
        )
    })
}

fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
