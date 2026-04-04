use crate::types::{Symbol, SymbolKind, Visibility};
use tree_sitter::{Node, Tree};

/// Extract all symbols from a parsed tree.
pub fn extract_symbols(
    tree: &Tree,
    src: &[u8],
    workspace_id: &str,
    file_path: &str,
) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut cursor = tree.walk();
    // Stack of enclosing context names (impl type or mod name).
    let mut parent_stack: Vec<String> = Vec::new();
    walk_node(
        &mut cursor,
        src,
        workspace_id,
        file_path,
        &mut parent_stack,
        &mut symbols,
    );
    symbols
}

fn node_text<'a>(node: Node<'_>, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn get_name(node: Node<'_>, src: &[u8]) -> Option<String> {
    // Try "name" field first.
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(node_text(name_node, src).to_owned());
    }
    // Fallback: any identifier or type_identifier child.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(node_text(child, src).to_owned());
        }
    }
    None
}

fn get_impl_name(node: Node<'_>, src: &[u8]) -> Option<String> {
    // For impl nodes: look for type_identifier child (the type being implemented).
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            return Some(node_text(child, src).to_owned());
        }
    }
    None
}

fn get_visibility(node: Node<'_>, src: &[u8]) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = node_text(child, src);
            if text.contains("crate") {
                return Visibility::Crate;
            }
            if text.contains("super") {
                return Visibility::Super;
            }
            return Visibility::Public;
        }
    }
    Visibility::Private
}

fn is_async_function(node: Node<'_>, src: &[u8]) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "async" {
            return true;
        }
        if child.kind() == "function_modifiers" {
            let text = node_text(child, src);
            if text.contains("async") {
                return true;
            }
        }
    }
    false
}

fn has_body(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "declaration_list" || child.kind() == "block" {
            return true;
        }
    }
    false
}

fn walk_node(
    cursor: &mut tree_sitter::TreeCursor<'_>,
    src: &[u8],
    workspace_id: &str,
    file_path: &str,
    parent_stack: &mut Vec<String>,
    symbols: &mut Vec<Symbol>,
) {
    let node = cursor.node();
    let node_kind = node.kind();

    let pushed = match node_kind {
        "function_item" => {
            if let Some(name) = get_name(node, src) {
                let kind = if is_async_function(node, src) {
                    SymbolKind::AsyncFunction
                } else {
                    SymbolKind::Function
                };
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                parent_stack.push(name);
                true
            } else {
                false
            }
        }
        "struct_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind: SymbolKind::Struct,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                parent_stack.push(name);
                true
            } else {
                false
            }
        }
        "enum_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind: SymbolKind::Enum,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                parent_stack.push(name);
                true
            } else {
                false
            }
        }
        "trait_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind: SymbolKind::Trait,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                parent_stack.push(name);
                true
            } else {
                false
            }
        }
        "impl_item" => {
            if let Some(name) = get_impl_name(node, src) {
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind: SymbolKind::Impl,
                    visibility: Visibility::Private,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                parent_stack.push(name);
                true
            } else {
                false
            }
        }
        "const_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name,
                    kind: SymbolKind::Const,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
            }
            false
        }
        "type_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name,
                    kind: SymbolKind::TypeAlias,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
            }
            false
        }
        "macro_definition" => {
            // macro_rules! foo — name is identifier child
            let mut cursor2 = node.walk();
            let mut macro_name: Option<String> = None;
            for child in node.children(&mut cursor2) {
                if child.kind() == "identifier" {
                    macro_name = Some(node_text(child, src).to_owned());
                    break;
                }
            }
            if let Some(name) = macro_name {
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name,
                    kind: SymbolKind::Macro,
                    visibility: Visibility::Private,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
            }
            false
        }
        "mod_item" => {
            if let Some(name) = get_name(node, src) {
                let visibility = get_visibility(node, src);
                let pos = node.start_position();
                let parent_name = parent_stack.last().cloned();
                symbols.push(Symbol {
                    id: 0,
                    workspace_id: workspace_id.to_owned(),
                    file_path: file_path.to_owned(),
                    name: name.clone(),
                    kind: SymbolKind::Module,
                    visibility,
                    line: pos.row as u32 + 1,
                    col: pos.column as u32,
                    parent_name,
                    signature: None,
                });
                if has_body(node) {
                    parent_stack.push(name);
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        _ => false,
    };

    if cursor.goto_first_child() {
        loop {
            walk_node(cursor, src, workspace_id, file_path, parent_stack, symbols);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }

    if pushed {
        parent_stack.pop();
    }
}

