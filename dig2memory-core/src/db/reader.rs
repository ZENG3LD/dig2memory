use crate::error::CoreError;
use crate::types::{
    CrateNode, DepEdge, FileEdge, FileEdgeKind, Symbol, SymbolKind, Visibility, Workspace,
};
use rusqlite::{params, Connection};

fn row_to_symbol(row: &rusqlite::Row<'_>) -> rusqlite::Result<Symbol> {
    let kind_str: String = row.get(4)?;
    let vis_str: String = row.get(5)?;
    Ok(Symbol {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        file_path: row.get(2)?,
        name: row.get(3)?,
        kind: SymbolKind::from_str(&kind_str).unwrap_or(SymbolKind::Function),
        visibility: Visibility::from_str(&vis_str).unwrap_or(Visibility::Private),
        line: row.get(6)?,
        col: row.get(7)?,
        parent_name: row.get(8)?,
        signature: row.get(9)?,
    })
}

/// Get the stored mtime for a file, or None if not indexed.
pub fn get_file_mtime(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
) -> Result<Option<i64>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT mtime FROM file_records WHERE workspace_id = ?1 AND file_path = ?2",
    )?;
    let result = stmt
        .query_map(params![workspace_id, file_path], |row| row.get(0))?
        .next()
        .transpose()?;
    Ok(result)
}

/// Get all symbols defined in a file.
pub fn get_symbols_in_file(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
) -> Result<Vec<Symbol>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, workspace_id, file_path, name, kind, visibility, line, col, parent_name, signature
         FROM symbols WHERE workspace_id = ?1 AND file_path = ?2",
    )?;
    let symbols = stmt
        .query_map(params![workspace_id, file_path], row_to_symbol)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(symbols)
}

/// Get all symbols that call a given callee name.
pub fn get_callers_of(
    conn: &Connection,
    callee_name: &str,
    workspace_id: Option<&str>,
) -> Result<Vec<Symbol>, CoreError> {
    match workspace_id {
        Some(ws) => {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT s.id, s.workspace_id, s.file_path, s.name, s.kind, s.visibility,
                        s.line, s.col, s.parent_name, s.signature
                 FROM call_edges ce
                 JOIN symbols s ON s.workspace_id = ce.workspace_id AND s.file_path = ce.caller_file
                 WHERE ce.callee_name = ?1 AND ce.workspace_id = ?2",
            )?;
            let result = stmt
                .query_map(params![callee_name, ws], row_to_symbol)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT s.id, s.workspace_id, s.file_path, s.name, s.kind, s.visibility,
                        s.line, s.col, s.parent_name, s.signature
                 FROM call_edges ce
                 JOIN symbols s ON s.workspace_id = ce.workspace_id AND s.file_path = ce.caller_file
                 WHERE ce.callee_name = ?1",
            )?;
            let result = stmt
                .query_map(params![callee_name], row_to_symbol)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
    }
}

/// Get all file edges originating from a file.
pub fn get_deps_of_file(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
) -> Result<Vec<FileEdge>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT workspace_id, from_file, to_file, kind
         FROM file_edges WHERE workspace_id = ?1 AND from_file = ?2",
    )?;
    let result = stmt
        .query_map(params![workspace_id, file_path], |row| {
            let kind_str: String = row.get(3)?;
            Ok(FileEdge {
                workspace_id: row.get(0)?,
                from_file: row.get(1)?,
                to_file: row.get(2)?,
                kind: FileEdgeKind::from_str(&kind_str).unwrap_or(FileEdgeKind::UseDecl),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(result)
}

fn row_to_crate_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<CrateNode> {
    let is_member: i32 = row.get(5)?;
    Ok(CrateNode {
        workspace_id: row.get(0)?,
        name: row.get(1)?,
        version: row.get(2)?,
        manifest_path: row.get(3)?,
        source_dir: row.get(4)?,
        is_workspace_member: is_member != 0,
    })
}

fn row_to_dep_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<DepEdge> {
    let is_path: i32 = row.get(3)?;
    Ok(DepEdge {
        workspace_id: row.get(0)?,
        from_crate: row.get(1)?,
        to_crate: row.get(2)?,
        is_path_dep: is_path != 0,
        dep_path: row.get(4)?,
    })
}

/// List all crate nodes, optionally filtered by workspace.
pub fn list_crate_nodes(
    conn: &Connection,
    workspace_id: Option<&str>,
) -> Result<Vec<CrateNode>, CoreError> {
    match workspace_id {
        Some(ws) => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, name, version, manifest_path, source_dir, is_workspace_member
                 FROM crate_nodes WHERE workspace_id = ?1",
            )?;
            let result = stmt
                .query_map(params![ws], row_to_crate_node)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, name, version, manifest_path, source_dir, is_workspace_member
                 FROM crate_nodes",
            )?;
            let result = stmt
                .query_map([], row_to_crate_node)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
    }
}

/// List all crate dependency edges, optionally filtered by workspace.
pub fn list_crate_deps(
    conn: &Connection,
    workspace_id: Option<&str>,
) -> Result<Vec<DepEdge>, CoreError> {
    match workspace_id {
        Some(ws) => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, from_crate, to_crate, is_path_dep, dep_path
                 FROM crate_dep_edges WHERE workspace_id = ?1",
            )?;
            let result = stmt
                .query_map(params![ws], row_to_dep_edge)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, from_crate, to_crate, is_path_dep, dep_path
                 FROM crate_dep_edges",
            )?;
            let result = stmt
                .query_map([], row_to_dep_edge)?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
    }
}

/// Given a symbol name, find which crate(s) define it.
///
/// Joins symbols with crate_nodes by matching the symbol's file_path against
/// each crate's source_dir prefix.  Returns all (symbol, crate) pairs where
/// the symbol name matches exactly.
pub fn resolve_symbol_to_crate(
    conn: &Connection,
    workspace_id: &str,
    symbol_name: &str,
) -> Result<Vec<(Symbol, CrateNode)>, CoreError> {
    // Pull all matching symbols first.
    let mut sym_stmt = conn.prepare(
        "SELECT id, workspace_id, file_path, name, kind, visibility, line, col, parent_name, signature
         FROM symbols
         WHERE workspace_id = ?1 AND name = ?2",
    )?;
    let symbols: Vec<Symbol> = sym_stmt
        .query_map(params![workspace_id, symbol_name], row_to_symbol)?
        .filter_map(|r| r.ok())
        .collect();

    if symbols.is_empty() {
        return Ok(vec![]);
    }

    // Pull all workspace member crate nodes for this workspace.
    let mut cn_stmt = conn.prepare(
        "SELECT workspace_id, name, version, manifest_path, source_dir, is_workspace_member
         FROM crate_nodes
         WHERE workspace_id = ?1",
    )?;
    let crate_nodes: Vec<CrateNode> = cn_stmt
        .query_map(params![workspace_id], row_to_crate_node)?
        .filter_map(|r| r.ok())
        .collect();

    let mut results = Vec::new();
    for sym in symbols {
        // Normalise the file path (forward slashes, no leading slash).
        let file_norm = sym.file_path.replace('\\', "/");
        let file_norm = file_norm.trim_start_matches('/');

        // Find the crate whose source_dir is the longest prefix of the file path.
        // This handles nested crates correctly.
        let best_crate = crate_nodes
            .iter()
            .filter(|cn| {
                // Empty source_dir means the crate is at the workspace root.
                if cn.source_dir.is_empty() {
                    return true;
                }
                let dir = cn.source_dir.trim_start_matches('/');
                file_norm.starts_with(dir)
                    && file_norm[dir.len()..].starts_with('/')
            })
            .max_by_key(|cn| cn.source_dir.len());

        if let Some(cn) = best_crate {
            results.push((sym, cn.clone()));
        }
    }

    Ok(results)
}

/// List file edges, optionally filtered by workspace and source file.
pub fn list_file_edges(
    conn: &Connection,
    workspace_id: &str,
    from_file: Option<&str>,
) -> Result<Vec<FileEdge>, CoreError> {
    match from_file {
        Some(ff) => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, from_file, to_file, kind
                 FROM file_edges WHERE workspace_id = ?1 AND from_file = ?2",
            )?;
            let result = stmt
                .query_map(params![workspace_id, ff], |row| {
                    let kind_str: String = row.get(3)?;
                    Ok(FileEdge {
                        workspace_id: row.get(0)?,
                        from_file: row.get(1)?,
                        to_file: row.get(2)?,
                        kind: FileEdgeKind::from_str(&kind_str).unwrap_or(FileEdgeKind::UseDecl),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT workspace_id, from_file, to_file, kind
                 FROM file_edges WHERE workspace_id = ?1",
            )?;
            let result = stmt
                .query_map(params![workspace_id], |row| {
                    let kind_str: String = row.get(3)?;
                    Ok(FileEdge {
                        workspace_id: row.get(0)?,
                        from_file: row.get(1)?,
                        to_file: row.get(2)?,
                        kind: FileEdgeKind::from_str(&kind_str).unwrap_or(FileEdgeKind::UseDecl),
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
    }
}

/// Get files that depend on a given file (reverse file edges).
pub fn get_reverse_file_deps(
    conn: &Connection,
    workspace_id: &str,
    file_path: &str,
) -> Result<Vec<String>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT from_file FROM file_edges WHERE workspace_id = ?1 AND to_file = ?2",
    )?;
    let result = stmt
        .query_map(params![workspace_id, file_path], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(result)
}

/// Get the most depended-upon files (by in-degree of file edges).
pub fn get_hotspots(
    conn: &Connection,
    workspace_id: Option<&str>,
    limit: u32,
) -> Result<Vec<(String, u32)>, CoreError> {
    match workspace_id {
        Some(ws) => {
            let mut stmt = conn.prepare(
                "SELECT to_file, COUNT(*) as cnt
                 FROM file_edges WHERE workspace_id = ?1
                 GROUP BY to_file ORDER BY cnt DESC LIMIT ?2",
            )?;
            let result = stmt
                .query_map(params![ws, limit], |row| {
                    let count: i64 = row.get(1)?;
                    Ok((row.get(0)?, count as u32))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT to_file, COUNT(*) as cnt
                 FROM file_edges
                 GROUP BY to_file ORDER BY cnt DESC LIMIT ?1",
            )?;
            let result = stmt
                .query_map(params![limit], |row| {
                    let count: i64 = row.get(1)?;
                    Ok((row.get(0)?, count as u32))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(result)
        }
    }
}

/// Count total symbols, optionally filtered by workspace.
pub fn count_symbols(conn: &Connection, workspace_id: Option<&str>) -> Result<u32, CoreError> {
    match workspace_id {
        Some(ws) => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM symbols WHERE workspace_id = ?1",
                params![ws],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        }
        None => {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
            Ok(count as u32)
        }
    }
}

/// Count total indexed files, optionally filtered by workspace.
pub fn count_files(conn: &Connection, workspace_id: Option<&str>) -> Result<u32, CoreError> {
    match workspace_id {
        Some(ws) => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM file_records WHERE workspace_id = ?1",
                params![ws],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        }
        None => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM file_records",
                [],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        }
    }
}

/// Get a workspace by id.
pub fn get_workspace(conn: &Connection, id: &str) -> Result<Option<Workspace>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, root_path, name, indexed_at FROM workspaces WHERE id = ?1",
    )?;
    let result = stmt
        .query_map(params![id], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                root_path: row.get(1)?,
                name: row.get(2)?,
                indexed_at: row.get(3)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(result)
}

/// List all workspaces.
pub fn list_workspaces(conn: &Connection) -> Result<Vec<Workspace>, CoreError> {
    let mut stmt =
        conn.prepare("SELECT id, root_path, name, indexed_at FROM workspaces")?;
    let result = stmt
        .query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                root_path: row.get(1)?,
                name: row.get(2)?,
                indexed_at: row.get(3)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(result)
}
