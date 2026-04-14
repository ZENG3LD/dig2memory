use crate::error::CoreError;
use crate::search::trigram::trigrams;
use crate::types::{CallEdge, CrateNode, DepEdge, FileEdge, FileRecord, Symbol, Workspace};
use rusqlite::{params, Connection};
use std::collections::HashSet;

/// Insert or update a workspace record.
pub fn upsert_workspace(conn: &Connection, workspace: &Workspace) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO workspaces (id, root_path, name, indexed_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
             root_path  = excluded.root_path,
             name       = excluded.name,
             indexed_at = excluded.indexed_at",
        params![
            workspace.id,
            workspace.root_path,
            workspace.name,
            workspace.indexed_at,
        ],
    )?;
    Ok(())
}

/// Insert or update a file record.
pub fn upsert_file_record(conn: &Connection, rec: &FileRecord) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO file_records (workspace_id, file_path, mtime, size, symbol_count)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace_id, file_path) DO UPDATE SET
             mtime        = excluded.mtime,
             size         = excluded.size,
             symbol_count = excluded.symbol_count",
        params![
            rec.workspace_id,
            rec.file_path,
            rec.mtime,
            rec.size,
            rec.symbol_count,
        ],
    )?;
    Ok(())
}

/// Delete all symbols for a given file (cascade removes trigrams and call edges via FK).
pub fn delete_file_symbols(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
) -> Result<(), CoreError> {
    conn.execute(
        "DELETE FROM symbols WHERE workspace_id = ?1 AND file_path = ?2",
        params![workspace_id, file_path],
    )?;
    conn.execute(
        "DELETE FROM call_edges WHERE workspace_id = ?1 AND caller_file = ?2",
        params![workspace_id, file_path],
    )?;
    Ok(())
}

/// Insert a symbol and return its new rowid.
pub fn insert_symbol(conn: &Connection, sym: &Symbol) -> Result<i64, CoreError> {
    conn.execute(
        "INSERT INTO symbols (workspace_id, file_path, name, kind, visibility, line, col, parent_name, signature)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            sym.workspace_id,
            sym.file_path,
            sym.name,
            sym.kind.to_string(),
            sym.visibility.to_string(),
            sym.line,
            sym.col,
            sym.parent_name,
            sym.signature,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Compute trigrams for a name and insert them into symbol_trigrams.
pub fn insert_symbol_trigrams(
    conn: &Connection,
    symbol_id: i64,
    name: &str,
) -> Result<(), CoreError> {
    for tgram in trigrams(name) {
        conn.execute(
            "INSERT OR IGNORE INTO symbol_trigrams (symbol_id, trigram) VALUES (?1, ?2)",
            params![symbol_id, tgram],
        )?;
    }
    Ok(())
}

/// Insert a call edge.
pub fn insert_call_edge(conn: &Connection, edge: &CallEdge) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO call_edges (workspace_id, caller_file, caller_symbol_id, callee_name, line)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            edge.workspace_id,
            edge.caller_file,
            edge.caller_symbol_id,
            edge.callee_name,
            edge.line,
        ],
    )?;
    Ok(())
}

/// Insert or update a file edge.
pub fn upsert_file_edge(conn: &Connection, edge: &FileEdge) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO file_edges (workspace_id, from_file, to_file, kind)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(workspace_id, from_file, to_file, kind) DO NOTHING",
        params![
            edge.workspace_id,
            edge.from_file,
            edge.to_file,
            edge.kind.to_string(),
        ],
    )?;
    Ok(())
}

/// Insert or update a crate node.
pub fn upsert_crate_node(conn: &Connection, node: &CrateNode) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO crate_nodes (workspace_id, name, version, manifest_path, source_dir, is_workspace_member)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(workspace_id, name) DO UPDATE SET
             version             = excluded.version,
             manifest_path       = excluded.manifest_path,
             source_dir          = excluded.source_dir,
             is_workspace_member = excluded.is_workspace_member",
        params![
            node.workspace_id,
            node.name,
            node.version,
            node.manifest_path,
            node.source_dir,
            node.is_workspace_member as i32,
        ],
    )?;
    Ok(())
}

/// Insert or update a crate dependency edge.
pub fn upsert_crate_dep_edge(conn: &Connection, edge: &DepEdge) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO crate_dep_edges (workspace_id, from_crate, to_crate, is_path_dep, dep_path)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace_id, from_crate, to_crate) DO UPDATE SET
             is_path_dep = excluded.is_path_dep,
             dep_path    = excluded.dep_path",
        params![
            edge.workspace_id,
            edge.from_crate,
            edge.to_crate,
            edge.is_path_dep as i32,
            edge.dep_path,
        ],
    )?;
    Ok(())
}

/// Delete file records (and cascade-delete their symbols) for files no longer on disk.
pub fn delete_stale_files(
    conn: &Connection,
    workspace_id: &str,
    current_files: &HashSet<String>,
) -> Result<(), CoreError> {
    let mut stmt = conn.prepare(
        "SELECT file_path FROM file_records WHERE workspace_id = ?1",
    )?;
    let stored: Vec<String> = stmt
        .query_map(params![workspace_id], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for path in stored {
        if !current_files.contains(&path) {
            conn.execute(
                "DELETE FROM file_records WHERE workspace_id = ?1 AND file_path = ?2",
                params![workspace_id, path],
            )?;
        }
    }
    Ok(())
}

