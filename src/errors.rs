// Rust guideline compliant 2025-10-17
use thiserror::Error;

/// Errors that can occur during code graph operations.
#[derive(Error, Debug)]
pub enum TokenSaveError {
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

    #[error("sync lock: {message}")]
    SyncLock { message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("libsql error: {0}")]
    Libsql(#[from] libsql::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias for results using `TokenSaveError`.
pub type Result<T> = std::result::Result<T, TokenSaveError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_error_display_includes_message_and_path() {
        let err = TokenSaveError::File {
            message: "not found".to_string(),
            path: "/tmp/foo.rs".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("not found"), "message missing: {s}");
        assert!(s.contains("/tmp/foo.rs"), "path missing: {s}");
    }

    #[test]
    fn parse_error_display_includes_line() {
        let err = TokenSaveError::Parse {
            message: "unexpected token".to_string(),
            path: "src/main.rs".to_string(),
            line: Some(42),
        };
        let s = err.to_string();
        assert!(s.contains("unexpected token"), "{s}");
        assert!(s.contains("src/main.rs"), "{s}");
        assert!(s.contains("42"), "{s}");
    }

    #[test]
    fn parse_error_display_no_line() {
        let err = TokenSaveError::Parse {
            message: "eof".to_string(),
            path: "src/lib.rs".to_string(),
            line: None,
        };
        let s = err.to_string();
        assert!(s.contains("eof"), "{s}");
    }

    #[test]
    fn database_error_display_includes_operation() {
        let err = TokenSaveError::Database {
            message: "constraint violated".to_string(),
            operation: "INSERT".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("constraint violated"), "{s}");
        assert!(s.contains("INSERT"), "{s}");
    }

    #[test]
    fn search_error_display_includes_query() {
        let err = TokenSaveError::Search {
            message: "timeout".to_string(),
            query: "fn main".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("timeout"), "{s}");
        assert!(s.contains("fn main"), "{s}");
    }

    #[test]
    fn config_error_display() {
        let err = TokenSaveError::Config {
            message: "bad value".to_string(),
        };
        assert!(err.to_string().contains("bad value"));
    }

    #[test]
    fn sync_lock_error_display() {
        let err = TokenSaveError::SyncLock {
            message: "already running".to_string(),
        };
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn json_error_from_serde() {
        let serde_err = serde_json::from_str::<serde_json::Value>("bad json").unwrap_err();
        let err: TokenSaveError = serde_err.into();
        assert!(err.to_string().contains("json error"));
    }
}
