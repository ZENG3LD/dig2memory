use crate::error::CoreError;
use crate::types::{CallEdge, FileEdge, Symbol};
use tree_sitter::Tree;

/// Trait for language-specific AST extraction.
///
/// Each implementation wraps a tree-sitter parser configured for a specific
/// language and provides symbol/call/edge extraction from a parsed tree.
pub trait LanguageSupport: Send {
    /// Parse source bytes into a tree-sitter Tree.
    fn parse(&mut self, src: &[u8]) -> Result<Tree, CoreError>;

    /// Extract symbol definitions from the parsed tree.
    fn extract_symbols(
        &self,
        tree: &Tree,
        src: &[u8],
        workspace_id: &str,
        file_path: &str,
    ) -> Vec<Symbol>;

    /// Extract call edges from the parsed tree.
    fn extract_calls(
        &self,
        tree: &Tree,
        src: &[u8],
        file_path: &str,
        file_symbols: &[Symbol],
    ) -> Vec<CallEdge>;

    /// Extract file-level edges (imports, module declarations).
    ///
    /// `root_path` is used for filesystem resolution checks.
    fn extract_file_edges(
        &self,
        tree: &Tree,
        src: &[u8],
        workspace_id: &str,
        file_path: &str,
        root_path: &str,
    ) -> Vec<FileEdge>;
}
