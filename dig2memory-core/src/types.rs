use serde::{Deserialize, Serialize};

/// A workspace represents a root directory being indexed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub root_path: String,
    pub name: String,
    pub indexed_at: Option<i64>,
}

/// A symbol extracted from a Rust source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    /// 0 = placeholder; DB assigns real id via autoincrement.
    pub id: i64,
    pub workspace_id: String,
    pub file_path: String,
    pub name: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub line: u32,
    pub col: u32,
    pub parent_name: Option<String>,
    pub signature: Option<String>,
}

/// The kind of a Rust symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    AsyncFunction,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    TypeAlias,
    Macro,
    Module,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SymbolKind::Function => "function",
            SymbolKind::AsyncFunction => "async_function",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Const => "const",
            SymbolKind::TypeAlias => "type_alias",
            SymbolKind::Macro => "macro",
            SymbolKind::Module => "module",
        };
        write!(f, "{s}")
    }
}

impl SymbolKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "function" => Some(SymbolKind::Function),
            "async_function" => Some(SymbolKind::AsyncFunction),
            "struct" => Some(SymbolKind::Struct),
            "enum" => Some(SymbolKind::Enum),
            "trait" => Some(SymbolKind::Trait),
            "impl" => Some(SymbolKind::Impl),
            "const" => Some(SymbolKind::Const),
            "type_alias" => Some(SymbolKind::TypeAlias),
            "macro" => Some(SymbolKind::Macro),
            "module" => Some(SymbolKind::Module),
            _ => None,
        }
    }
}

/// Visibility of a Rust symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Crate,
    Super,
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Visibility::Public => "public",
            Visibility::Private => "private",
            Visibility::Crate => "crate",
            Visibility::Super => "super",
        };
        write!(f, "{s}")
    }
}

impl Visibility {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(Visibility::Public),
            "private" => Some(Visibility::Private),
            "crate" => Some(Visibility::Crate),
            "super" => Some(Visibility::Super),
            _ => None,
        }
    }
}

/// An edge representing a function/method call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdge {
    pub workspace_id: String,
    pub caller_file: String,
    /// 0 = placeholder before symbol is resolved.
    pub caller_symbol_id: i64,
    pub callee_name: String,
    pub line: u32,
}

/// An edge representing a file-level dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdge {
    pub workspace_id: String,
    pub from_file: String,
    pub to_file: String,
    pub kind: FileEdgeKind,
}

/// The kind of a file-level edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileEdgeKind {
    UseDecl,
    ModDecl,
}

impl std::fmt::Display for FileEdgeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FileEdgeKind::UseDecl => "use_decl",
            FileEdgeKind::ModDecl => "mod_decl",
        };
        write!(f, "{s}")
    }
}

impl FileEdgeKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "use_decl" => Some(FileEdgeKind::UseDecl),
            "mod_decl" => Some(FileEdgeKind::ModDecl),
            _ => None,
        }
    }
}

/// A crate in the cargo workspace graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateNode {
    pub workspace_id: String,
    pub name: String,
    pub version: String,
    pub manifest_path: String,
    /// Relative directory containing the crate's src/ (e.g. "zengeld-terminal/crates/app").
    pub source_dir: String,
    /// True if this package is a direct workspace member.
    pub is_workspace_member: bool,
}

/// A dependency edge between crates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepEdge {
    pub workspace_id: String,
    pub from_crate: String,
    pub to_crate: String,
    pub is_path_dep: bool,
    /// Resolved relative path for path deps, None for registry deps.
    pub dep_path: Option<String>,
}

/// Metadata record for an indexed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub workspace_id: String,
    pub file_path: String,
    pub mtime: i64,
    pub size: i64,
    pub symbol_count: u32,
}

/// Request to index a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRequest {
    pub workspace_id: String,
    pub root_path: String,
    pub workspace_name: String,
    /// Force re-index even if mtime unchanged.
    pub force: bool,
}

/// Result of an indexing run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexResult {
    pub workspace_id: String,
    pub files_scanned: u32,
    pub files_indexed: u32,
    pub symbols_extracted: u32,
    pub errors: u32,
    pub elapsed_ms: u64,
}

/// A search hit from a fuzzy query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub symbol: Symbol,
    pub score: f32,
}

/// Health status of the indexer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub ok: bool,
    pub db_path: String,
    pub workspace_count: u32,
    pub symbol_count: u32,
    pub file_count: u32,
}

/// Aggregate statistics for a workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStat {
    pub workspace_id: String,
    pub file_count: u32,
    pub symbol_count: u32,
    pub crate_count: u32,
}
