use crate::ast::{extract_calls, extract_symbols, RustParser};
use crate::db::{reader, writer};
use crate::error::CoreError;
use crate::graph::{crates::parse_cargo_workspace, files::extract_file_edges};
use crate::types::{FileRecord, IndexRequest, IndexResult, Workspace};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

/// Incremental Rust workspace indexer.
///
/// Designed for use inside `spawn_blocking`. Holds an `Arc<Mutex<Connection>>`
/// and releases the lock between batches so other requests can proceed.
pub struct Indexer {
    parser: RustParser,
}

impl Indexer {
    /// Create a new indexer.
    pub fn new() -> Result<Self, CoreError> {
        let parser = RustParser::new()?;
        Ok(Self { parser })
    }

    /// Run an incremental index pass for the given request.
    ///
    /// The DB mutex is locked only briefly per batch — never for the full walk/parse duration.
    pub fn run(
        &mut self,
        db: &Mutex<rusqlite::Connection>,
        req: &IndexRequest,
    ) -> Result<IndexResult, CoreError> {
        let start = std::time::Instant::now();
        let root = Path::new(&req.root_path);
        let workspace_id = &req.workspace_id;

        // Phase 1: Register/upsert workspace (brief lock).
        {
            let workspace = Workspace {
                id: workspace_id.clone(),
                root_path: req.root_path.clone(),
                name: req.workspace_name.clone(),
                indexed_at: None,
            };
            let conn = db.lock().unwrap();
            writer::upsert_workspace(&conn, &workspace)?;
        }

        // Phase 2: Collect all .rs file paths — no DB lock needed.
        let all_files = collect_rs_files(root);
        let files_scanned = all_files.len() as u32;
        tracing::info!("found {} .rs files in {:?}", files_scanned, root);

        // Phase 3: Filter to files that need (re)indexing.
        let files_to_index: Vec<(PathBuf, String, i64, i64)> = if req.force {
            // Force: wipe existing workspace data first (brief lock).
            let conn = db.lock().unwrap();
            conn.execute(
                "DELETE FROM symbols WHERE workspace_id = ?1",
                rusqlite::params![workspace_id],
            )?;
            conn.execute(
                "DELETE FROM call_edges WHERE workspace_id = ?1",
                rusqlite::params![workspace_id],
            )?;
            conn.execute(
                "DELETE FROM file_edges WHERE workspace_id = ?1",
                rusqlite::params![workspace_id],
            )?;
            conn.execute(
                "DELETE FROM file_records WHERE workspace_id = ?1",
                rusqlite::params![workspace_id],
            )?;
            drop(conn);
            all_files
        } else {
            // Incremental: check mtime for each file (brief lock).
            let conn = db.lock().unwrap();
            let filtered = all_files
                .into_iter()
                .filter(|(_, rel, mtime, _)| {
                    match reader::get_file_mtime(&conn, workspace_id, rel) {
                        Ok(Some(stored)) => stored != *mtime,
                        _ => true,
                    }
                })
                .collect();
            drop(conn);
            filtered
        };

        tracing::info!("{} files need indexing", files_to_index.len());

        // Phase 4: Parse + write in batches of 50.
        const BATCH_SIZE: usize = 50;
        let mut total_symbols: u32 = 0;
        let mut files_indexed: u32 = 0;
        let mut errors: u32 = 0;

        for chunk in files_to_index.chunks(BATCH_SIZE) {
            // Parse all files in this chunk — no DB lock.
            let mut parsed_batch: Vec<ParsedFile> = Vec::with_capacity(chunk.len());

            for (abs_path, rel_path, mtime, size) in chunk {
                let src = match std::fs::read(abs_path) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("read error {}: {}", rel_path, e);
                        errors += 1;
                        continue;
                    }
                };
                let tree = match self.parser.parse_bytes(&src) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("parse error {}: {}", rel_path, e);
                        errors += 1;
                        continue;
                    }
                };

                let symbols = extract_symbols(&tree, &src, workspace_id, rel_path);
                let calls = extract_calls(&tree, &src, rel_path);
                let file_edges = extract_file_edges(&tree, &src, workspace_id, rel_path);

                parsed_batch.push(ParsedFile {
                    rel_path: rel_path.clone(),
                    mtime: *mtime,
                    size: *size,
                    symbols,
                    calls,
                    file_edges,
                });
            }

            // Write entire batch inside a single brief transaction.
            {
                let conn = db.lock().unwrap();
                let tx = conn.unchecked_transaction()?;

                for pf in &parsed_batch {
                    // Delete stale data for this file.
                    writer::delete_file_symbols(&tx, workspace_id, &pf.rel_path)?;

                    let sym_count = pf.symbols.len() as u32;

                    // Upsert file record FIRST so the FK constraint on symbols is satisfied.
                    let file_rec = FileRecord {
                        workspace_id: workspace_id.clone(),
                        file_path: pf.rel_path.clone(),
                        mtime: pf.mtime,
                        size: pf.size,
                        symbol_count: sym_count,
                    };
                    writer::upsert_file_record(&tx, &file_rec)?;

                    // Insert symbols and trigrams.
                    for sym in &pf.symbols {
                        match writer::insert_symbol(&tx, sym) {
                            Ok(sym_id) => {
                                if let Err(e) =
                                    writer::insert_symbol_trigrams(&tx, sym_id, &sym.name)
                                {
                                    tracing::warn!("trigram insert error: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("symbol insert error: {}", e);
                            }
                        }
                    }

                    // Insert call edges.
                    for edge in &pf.calls {
                        let mut edge = edge.clone();
                        edge.workspace_id = workspace_id.clone();
                        if let Err(e) = writer::insert_call_edge(&tx, &edge) {
                            tracing::warn!("call edge insert error: {}", e);
                        }
                    }

                    // Insert file edges.
                    for edge in &pf.file_edges {
                        if let Err(e) = writer::upsert_file_edge(&tx, edge) {
                            tracing::warn!("file edge insert error: {}", e);
                        }
                    }

                    total_symbols += sym_count;
                    files_indexed += 1;
                }

                tx.commit()?;
            } // mutex released here

            tracing::info!(
                "batch written: {} files indexed so far",
                files_indexed
            );
        }

        // Phase 5: Remove stale files (brief lock).
        {
            let current_files: HashSet<String> = collect_rs_files(root)
                .into_iter()
                .map(|(_, rel, _, _)| rel)
                .collect();
            let conn = db.lock().unwrap();
            writer::delete_stale_files(&conn, workspace_id, &current_files)?;
        }

        // Phase 6: Parse cargo workspace for crate graph (brief lock).
        match parse_cargo_workspace(root, workspace_id) {
            Ok((nodes, edges)) => {
                let conn = db.lock().unwrap();
                for node in &nodes {
                    if let Err(e) = writer::upsert_crate_node(&conn, node) {
                        tracing::warn!("crate node insert error: {}", e);
                    }
                }
                for edge in &edges {
                    if let Err(e) = writer::upsert_crate_dep_edge(&conn, edge) {
                        tracing::warn!("crate dep edge insert error: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("crate graph parse error: {}", e);
            }
        }

        // Phase 7: Update workspace indexed_at (brief lock).
        {
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
            let conn = db.lock().unwrap();
            writer::upsert_workspace(&conn, &updated_ws)?;
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            "index complete for workspace={workspace_id}: \
             scanned={files_scanned}, indexed={files_indexed}, \
             symbols={total_symbols}, errors={errors}, elapsed={elapsed_ms}ms"
        );

        Ok(IndexResult {
            workspace_id: workspace_id.clone(),
            files_scanned,
            files_indexed,
            symbols_extracted: total_symbols,
            errors,
            elapsed_ms,
        })
    }
}

/// Intermediate parsed file data, assembled before the DB lock.
struct ParsedFile {
    rel_path: String,
    mtime: i64,
    size: i64,
    symbols: Vec<crate::types::Symbol>,
    calls: Vec<crate::types::CallEdge>,
    file_edges: Vec<crate::types::FileEdge>,
}

/// Collect all `.rs` files under `root`, returning `(abs_path, rel_path, mtime, size)`.
///
/// Paths are normalised to forward slashes so they are consistent across platforms.
fn collect_rs_files(root: &Path) -> Vec<(PathBuf, String, i64, i64)> {
    let mut result = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(is_not_excluded)
    {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("walkdir error: {}", e);
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

        let meta = match path.metadata() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("metadata error for {}: {}", path.display(), e);
                continue;
            }
        };

        let mtime = file_mtime(&meta);
        let size = meta.len() as i64;

        // Normalise to forward slashes for cross-platform consistency.
        let abs_str = path.to_string_lossy().replace('\\', "/");
        let root_str = root.to_string_lossy().replace('\\', "/");
        let rel = if abs_str.starts_with(&root_str) {
            abs_str[root_str.len()..].trim_start_matches('/').to_string()
        } else {
            abs_str.clone()
        };

        result.push((path.to_path_buf(), rel, mtime, size));
    }

    result
}

/// Returns `true` if a WalkDir entry should be skipped entirely.
fn is_not_excluded(entry: &walkdir::DirEntry) -> bool {
    !is_excluded(entry)
}

fn is_excluded(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_str().unwrap_or("");

    // Skip any hidden directory (starts with '.').
    if name.starts_with('.') && entry.file_type().is_dir() {
        return true;
    }

    // Named directories to skip unconditionally.
    matches!(
        name,
        "target" | "node_modules" | ".cargo" | ".git" | "build"
    )
}

fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
