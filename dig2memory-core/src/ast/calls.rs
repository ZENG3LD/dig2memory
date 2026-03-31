use crate::types::CallEdge;
use tree_sitter::{Node, Tree};

/// Extract all call edges from a parsed tree.
pub fn extract_calls(tree: &Tree, src: &[u8], file_path: &str) -> Vec<CallEdge> {
    let mut edges = Vec::new();
    walk_for_calls(tree.root_node(), src, file_path, &mut edges);
    edges
}

fn node_text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn walk_for_calls(node: Node<'_>, src: &[u8], file_path: &str, edges: &mut Vec<CallEdge>) {
    if node.kind() == "call_expression" {
        if let Some(func_node) = node.child_by_field_name("function") {
            let callee_name = match func_node.kind() {
                "identifier" => node_text(func_node, src).to_owned(),
                "field_expression" => {
                    // method call: expr.method(...)
                    if let Some(field) = func_node.child_by_field_name("field") {
                        node_text(field, src).to_owned()
                    } else {
                        String::new()
                    }
                }
                "scoped_identifier" => {
                    // qualified path: crate::mod::func
                    let full = node_text(func_node, src);
                    full.rsplit("::").next().unwrap_or(full).to_owned()
                }
                _ => node_text(func_node, src).to_owned(),
            };

            if !callee_name.is_empty() {
                let pos = node.start_position();
                edges.push(CallEdge {
                    workspace_id: String::new(), // will be set by indexer
                    caller_file: file_path.to_owned(),
                    caller_symbol_id: 0,
                    callee_name,
                    line: pos.row as u32 + 1,
                });
            }
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_for_calls(cursor.node(), src, file_path, edges);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
