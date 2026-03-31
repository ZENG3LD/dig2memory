use crate::types::{FileEdge, FileEdgeKind};
use std::path::Path;
use tree_sitter::{Node, Tree};

/// Extract file-level edges from a parsed tree.
///
/// - `use_declaration` nodes → `UseDecl` edges
/// - `mod_item` without body → `ModDecl` edges
pub fn extract_file_edges(
    tree: &Tree,
    src: &[u8],
    workspace_id: &str,
    file_path: &str,
) -> Vec<FileEdge> {
    let mut edges = Vec::new();
    walk_for_edges(
        tree.root_node(),
        src,
        workspace_id,
        file_path,
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
    edges: &mut Vec<FileEdge>,
) {
    match node.kind() {
        "use_declaration" => {
            let text = node_text(node, src);
            // Extract the path from the use statement.
            let to_file = resolve_use_to_path(text);
            if let Some(to) = to_file {
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
                let resolved = resolve_mod_decl(file_path, mod_name);
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
            walk_for_edges(cursor.node(), src, workspace_id, file_path, edges);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// Resolve a `mod foo;` declaration to a file path relative to the workspace.
fn resolve_mod_decl(declaring_file: &str, mod_name: &str) -> String {
    let declaring = Path::new(declaring_file);
    let parent = declaring.parent().unwrap_or(Path::new("."));

    let candidate_rs = parent.join(format!("{mod_name}.rs"));
    let candidate_mod = parent.join(mod_name).join("mod.rs");

    // Return the canonical candidate; prefer foo.rs if both could exist.
    // We return both as a combined hint using the .rs form as primary.
    if candidate_rs.exists() {
        candidate_rs.to_string_lossy().replace('\\', "/")
    } else if candidate_mod.exists() {
        candidate_mod.to_string_lossy().replace('\\', "/")
    } else {
        // Default: return foo.rs path.
        candidate_rs.to_string_lossy().replace('\\', "/")
    }
}

/// Attempt to extract a simple file path hint from a use statement text.
/// Returns None if the path is external (no leading crate:: or self::).
fn resolve_use_to_path(use_text: &str) -> Option<String> {
    // Strip "use " prefix and trailing ";".
    let path = use_text
        .trim()
        .trim_start_matches("use ")
        .trim_end_matches(';')
        .trim();

    // Only track internal paths (self::, super::, crate::).
    let internal = path.starts_with("self::")
        || path.starts_with("super::")
        || path.starts_with("crate::");

    if internal {
        Some(path.to_owned())
    } else {
        None
    }
}
