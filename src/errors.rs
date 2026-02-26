use thiserror::Error;

/// Errors that can occur during code graph operations.
#[derive(Error, Debug)]
pub enum CodeGraphError {
    #[error("file error: {message} (path: {path})")]
    File { message: String, path: String },

    #[error("parse error: {message} (path: {path}, line: {line:?})")]
    Parse {
        message: String,
        path: String,
        line: Option<u32>,
    },

    #[error("database error: {message} (operation: {operation})")]
    Database { message: String, operation: String },

    #[error("search error: {message} (query: {query})")]
    Search { message: String, query: String },

    #[error("config error: {message}")]
    Config { message: String },

    #[error("vector error: {message}")]
    Vector { message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias for results using `CodeGraphError`.
pub type Result<T> = std::result::Result<T, CodeGraphError>;
