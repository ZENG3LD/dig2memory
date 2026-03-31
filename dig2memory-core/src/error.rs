use thiserror::Error;

/// Core error type for dig2memory-core.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("parse error in {file}: {reason}")]
    Parse { file: String, reason: String },

    #[error("IO error at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("manifest error at {path}: {source}")]
    Manifest {
        path: String,
        source: toml::de::Error,
    },

    #[error("workspace not found: {id}")]
    WorkspaceNotFound { id: String },
}
