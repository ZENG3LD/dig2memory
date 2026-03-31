use crate::types::{CallEdge, Symbol};
use tree_sitter::{Node, Tree};

/// Extract all call edges from a parsed tree.
///
/// `file_symbols` is accepted for API consistency but is not used here — caller
/// symbol id resolution happens in `indexer.rs` after DB ids have been assigned.
/// The parameter is kept so that future refinements (e.g. narrowing scope using
/// symbol end lines) can use it without a signature change.
pub fn extract_calls(tree: &Tree, src: &[u8], file_path: &str, _file_symbols: &[Symbol]) -> Vec<CallEdge> {
    let mut edges = Vec::new();
    walk_for_calls(tree.root_node(), src, file_path, &mut edges);
    edges
}

fn node_text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn walk_for_calls(
    node: Node<'_>,
    src: &[u8],
    file_path: &str,
    edges: &mut Vec<CallEdge>,
) {
    match node.kind() {
        "call_expression" => {
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
                    let call_line = pos.row as u32 + 1;
                    edges.push(CallEdge {
                        workspace_id: String::new(), // set by indexer
                        caller_file: file_path.to_owned(),
                        caller_symbol_id: 0, // resolved by indexer after DB insert
                        callee_name,
                        line: call_line,
                    });
                }
            }
        }
        "impl_item" => {
            // When someone writes `impl SomeTrait for SomeType`, record an edge
            // from the enclosing context to the trait name.  This makes trait
            // implementors show up in `/ast/callers?sym=SomeTrait`.
            //
            // Tree-sitter grammar:
            //   impl_item
            //     "impl"
            //     trait: type_identifier / generic_type   <- trait name (optional)
            //     "for"
            //     type: type_identifier                   <- concrete type
            //
            // When there is no "for" keyword it's a plain `impl SomeType { ... }`
            // and we skip it (no trait reference).
            if let Some(trait_node) = node.child_by_field_name("trait") {
                let trait_name = extract_type_name(trait_node, src);
                if !trait_name.is_empty() {
                    let pos = node.start_position();
                    let call_line = pos.row as u32 + 1;
                    edges.push(CallEdge {
                        workspace_id: String::new(), // set by indexer
                        caller_file: file_path.to_owned(),
                        caller_symbol_id: 0, // resolved by indexer after DB insert
                        callee_name: trait_name,
                        line: call_line,
                    });
                }
            }
        }
        _ => {}
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

/// Extract the plain name from a type node (handles generic_type by looking at
/// the inner type_identifier).
fn extract_type_name<'a>(node: Node<'_>, src: &'a [u8]) -> String {
    match node.kind() {
        "type_identifier" | "identifier" => node_text(node, src).to_owned(),
        "generic_type" => {
            // generic_type -> type: type_identifier
            if let Some(inner) = node.child_by_field_name("type") {
                node_text(inner, src).to_owned()
            } else {
                // Fallback: first type_identifier child
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "type_identifier" {
                        return node_text(child, src).to_owned();
                    }
                }
                String::new()
            }
        }
        "scoped_type_identifier" => {
            // e.g. some::path::Trait — take last segment
            let full = node_text(node, src);
            full.rsplit("::").next().unwrap_or(full).to_owned()
        }
        _ => String::new(),
    }
}
