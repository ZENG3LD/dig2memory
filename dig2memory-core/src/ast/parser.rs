use crate::error::CoreError;
use tree_sitter::Parser;

/// Wraps tree-sitter's Parser configured for Rust.
pub struct RustParser {
    inner: Parser,
}

impl RustParser {
    /// Creates a new parser configured for the Rust language.
    pub fn new() -> Result<Self, CoreError> {
        let mut inner = Parser::new();
        // tree-sitter-rust 0.23 exports LANGUAGE as a LanguageFn; .into() converts it.
        inner
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .map_err(|e| CoreError::Parse {
                file: String::new(),
                reason: format!("failed to set language: {e}"),
            })?;
        Ok(Self { inner })
    }

    /// Parse raw bytes and return the syntax tree.
    pub fn parse_bytes(&mut self, src: &[u8]) -> Result<tree_sitter::Tree, CoreError> {
        self.inner
            .parse(src, None)
            .ok_or_else(|| CoreError::Parse {
                file: String::new(),
                reason: "tree-sitter returned None (timeout or cancellation)".into(),
            })
    }
}
