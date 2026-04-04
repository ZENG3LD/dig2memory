use crate::ast::language::LanguageSupport;
use crate::ast::{calls, parser::RustParser, symbols};
use crate::error::CoreError;
use crate::graph::files;
use crate::types::{CallEdge, FileEdge, Symbol};
use tree_sitter::Tree;

/// `LanguageSupport` wrapper around the existing Rust AST extraction code.
pub struct RustLanguage {
    parser: RustParser,
}

impl RustLanguage {
    /// Create a new Rust language support instance.
    pub fn new() -> Result<Self, CoreError> {
        Ok(Self {
            parser: RustParser::new()?,
        })
    }
}

impl LanguageSupport for RustLanguage {
    fn parse(&mut self, src: &[u8]) -> Result<Tree, CoreError> {
        self.parser.parse_bytes(src)
    }

    fn extract_symbols(
        &self,
        tree: &Tree,
        src: &[u8],
        workspace_id: &str,
        file_path: &str,
    ) -> Vec<Symbol> {
        symbols::extract_symbols(tree, src, workspace_id, file_path)
    }

    fn extract_calls(
        &self,
        tree: &Tree,
        src: &[u8],
        file_path: &str,
        file_symbols: &[Symbol],
    ) -> Vec<CallEdge> {
        calls::extract_calls(tree, src, file_path, file_symbols)
    }

    fn extract_file_edges(
        &self,
        tree: &Tree,
        src: &[u8],
        workspace_id: &str,
        file_path: &str,
        root_path: &str,
    ) -> Vec<FileEdge> {
        files::extract_file_edges(tree, src, workspace_id, file_path, root_path)
    }
}
