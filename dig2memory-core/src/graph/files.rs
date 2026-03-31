use crate::types::{FileEdge, FileEdgeKind};
use std::path::Path;
use tree_sitter::{Node, Tree};

/// Extract file-level edges from a parsed tree.
///
/// - `use_declaration` nodes → `UseDecl` edges (resolved to real `.rs` paths)
/// - `mod_item` without body → `ModDecl` edges
///
/// `root_path` is the absolute workspace root (e.g. `c:/Users/.../nemo`).
/// All stored file paths are relative to `root_path`; `root_path` is only
/// used for filesystem existence checks during indexing.
pub fn extract_file_edges(
    tree: &Tree,
    src: &[u8],
    workspace_id: &str,
    file_path: &str,
    root_path: &str,
) -> Vec<FileEdge> {
    let mut edges = Vec::new();
    walk_for_edges(
        tree.root_node(),
        src,
        workspace_id,
        file_path,
        root_path,
        &mut edges,
    );
    edges
}

fn node_text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn has_body(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "declaration_list" {
            return true;
        }
    }
    false
}

fn walk_for_edges(
    node: Node<'_>,
    src: &[u8],
    workspace_id: &str,
    file_path: &str,
    root_path: &str,
    edges: &mut Vec<FileEdge>,
) {
    match node.kind() {
        "use_declaration" => {
            let text = node_text(node, src);
            // Resolve the use path to a real .rs file path.
            if let Some(to) = resolve_use_to_path(text, file_path, root_path) {
                edges.push(FileEdge {
                    workspace_id: workspace_id.to_owned(),
                    from_file: file_path.to_owned(),
                    to_file: to,
                    kind: FileEdgeKind::UseDecl,
                });
            }
        }
        "mod_item" if !has_body(node) => {
            // mod foo; — resolve to foo.rs or foo/mod.rs
            if let Some(name_node) = node.child_by_field_name("name") {
                let mod_name = node_text(name_node, src);
                let resolved = resolve_mod_decl(file_path, mod_name, root_path);
                edges.push(FileEdge {
                    workspace_id: workspace_id.to_owned(),
                    from_file: file_path.to_owned(),
                    to_file: resolved,
                    kind: FileEdgeKind::ModDecl,
                });
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_for_edges(cursor.node(), src, workspace_id, file_path, root_path, edges);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Resolve a `mod foo;` declaration to a file path relative to the workspace.
///
/// `declaring_file` is relative to the workspace root.
/// `root_path` is used only for filesystem existence checks.
/// The returned path is always relative (no `root_path` prefix).
fn resolve_mod_decl(declaring_file: &str, mod_name: &str, root_path: &str) -> String {
    let declaring = Path::new(declaring_file);
    let parent = declaring.parent().unwrap_or(Path::new("."));

    let candidate_rs_rel = parent.join(format!("{mod_name}.rs"));
    let candidate_mod_rel = parent.join(mod_name).join("mod.rs");

    let root = Path::new(root_path);
    let candidate_rs_abs = root.join(&candidate_rs_rel);
    let candidate_mod_abs = root.join(&candidate_mod_rel);

    // Return the canonical candidate; prefer foo.rs if both could exist.
    // Existence checks use the absolute path; returned value is relative.
    if candidate_rs_abs.exists() {
        candidate_rs_rel.to_string_lossy().replace('\\', "/")
    } else if candidate_mod_abs.exists() {
        candidate_mod_rel.to_string_lossy().replace('\\', "/")
    } else {
        // Default: return foo.rs path.
        candidate_rs_rel.to_string_lossy().replace('\\', "/")
    }
}

/// Resolve a `use` statement text to an actual `.rs` file path.
///
/// Handles `crate::`, `super::`, and `self::` prefixes.  Strips trailing
/// item names (structs, enums, functions) that live inside a file.
/// Returns `None` if the path is external or cannot be resolved on disk.
///
/// `current_file` is relative to the workspace root.
/// `root_path` is used only for filesystem existence checks.
/// The returned path is always relative (no `root_path` prefix).
fn resolve_use_to_path(use_text: &str, current_file: &str, root_path: &str) -> Option<String> {
    // Strip "use " prefix, optional "pub " modifier, and trailing ";".
    // Also handle brace groups by taking only the base path before '{'.
    let raw = use_text
        .trim()
        .trim_start_matches("pub ")
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim();

    // Strip any brace group — we only care about the leading module path.
    let raw = if let Some(brace_pos) = raw.find('{') {
        raw[..brace_pos].trim_end_matches(':').trim()
    } else {
        raw
    };

    // Only handle intra-crate paths.
    let (prefix, rest) = if let Some(r) = raw.strip_prefix("crate::") {
        ("crate", r)
    } else if let Some(r) = raw.strip_prefix("super::") {
        ("super", r)
    } else if let Some(r) = raw.strip_prefix("self::") {
        ("self", r)
    } else {
        return None;
    };

    // Drop trailing `*` glob and any trailing `::`.
    let rest = rest.trim_end_matches('*').trim_end_matches(':').trim();

    // Determine the start directory for the walk.
    // `current_file` is relative, so combine with root_path for any filesystem ops.
    let current_path = Path::new(current_file);
    let current_dir = current_path.parent().unwrap_or(Path::new(""));

    let start_dir: std::path::PathBuf = match prefix {
        "crate" => {
            // Walk up from current_file to find the crate source root.
            // find_src_dir works in relative space but uses root_path for checks.
            find_src_dir(current_dir, root_path)?
        }
        "super" => {
            // `super::` = parent of current file's directory.
            current_dir
                .parent()
                .unwrap_or(current_dir)
                .to_path_buf()
        }
        _ => {
            // `self::` = same directory as current file.
            current_dir.to_path_buf()
        }
    };

    // Walk segments, trying to find the deepest `.rs` file on disk.
    let segments: Vec<&str> = if rest.is_empty() {
        vec![]
    } else {
        rest.split("::").collect()
    };

    resolve_segments(&start_dir, &segments, root_path)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// Walk up the directory tree (in relative path space) to find the crate
/// source root for `crate::` resolution.
///
/// `start` is a path relative to the workspace root.
/// `root_path` is prepended for all filesystem existence checks.
/// Returns the found directory as a relative path.
///
/// Resolution order at each level:
/// 1. If a `src/` subdirectory exists → return it (standard crate layout).
/// 2. If a `Cargo.toml` exists → return the directory itself (flat crate layout).
/// 3. Keep walking up.
///
/// Returns `None` only if we exhaust the entire directory tree without finding
/// either anchor.
fn find_src_dir(start: &Path, root_path: &str) -> Option<std::path::PathBuf> {
    let root = Path::new(root_path);
    let mut current = start.to_path_buf();
    loop {
        let src_candidate_abs = root.join(&current).join("src");
        if src_candidate_abs.is_dir() {
            return Some(current.join("src"));
        }
        let cargo_abs = root.join(&current).join("Cargo.toml");
        if cargo_abs.exists() {
            return Some(current);
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => return None,
        }
    }
}

/// Given a base directory (relative) and path segments, walk the filesystem
/// to find the deepest `.rs` file that matches.
///
/// `base` is relative to the workspace root.
/// `root_path` is prepended for all filesystem existence checks.
/// The returned path is always relative (no `root_path` prefix).
///
/// Strategy: for each segment, check if `dir/segment.rs` exists (file wins),
/// then check `dir/segment/mod.rs`, then step into `dir/segment/` for the next
/// segment.  Stop as soon as a `.rs` file is found (remaining segments are
/// items inside it).
fn resolve_segments(
    base: &Path,
    segments: &[&str],
    root_path: &str,
) -> Option<std::path::PathBuf> {
    let root = Path::new(root_path);

    // If no segments, check for mod.rs in the base directory.
    if segments.is_empty() {
        let mod_rs_rel = base.join("mod.rs");
        return if root.join(&mod_rs_rel).exists() {
            Some(mod_rs_rel)
        } else {
            None
        };
    }

    let mut current_dir = base.to_path_buf();

    for (i, segment) in segments.iter().enumerate() {
        // Try segment.rs
        let rs_file_rel = current_dir.join(format!("{segment}.rs"));
        if root.join(&rs_file_rel).exists() {
            return Some(rs_file_rel);
        }
        // Try segment/mod.rs
        let mod_file_rel = current_dir.join(segment).join("mod.rs");
        if root.join(&mod_file_rel).exists() {
            // If this is the last segment, return mod.rs; otherwise step in.
            if i == segments.len() - 1 {
                return Some(mod_file_rel);
            } else {
                current_dir = current_dir.join(segment);
                continue;
            }
        }
        // Try stepping into segment/ directory (no mod.rs yet — keep walking).
        let sub_dir_rel = current_dir.join(segment);
        if root.join(&sub_dir_rel).is_dir() {
            current_dir = sub_dir_rel;
        } else {
            // Dead end — the remaining segments are likely item names inside
            // a file we already passed.  Return the closest .rs candidate.
            let lib_rs_rel = current_dir.join("lib.rs");
            if root.join(&lib_rs_rel).exists() {
                return Some(lib_rs_rel);
            }
            return None;
        }
    }

    // All segments consumed by directory walking — try mod.rs.
    let mod_rs_rel = current_dir.join("mod.rs");
    if root.join(&mod_rs_rel).exists() {
        return Some(mod_rs_rel);
    }
    None
}
