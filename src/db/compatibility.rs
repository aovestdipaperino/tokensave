//! Database creation channel, independent of graph schema migrations.
//!
//! `application_id` is an additive SQLite header field ignored by old binaries.
//! Zero means legacy/unknown origin and is never retroactively labelled.
use std::path::Path;

use libsql::{Builder, Connection, OpenFlags};

use crate::errors::{Result, TokenSaveError};

const STABLE: i64 = 0x5453_5354; // TSST
const BETA: i64 = 0x5453_4254; // TSBT
pub const OPERATION: &str = "database_compatibility";

fn refusal(message: impl Into<String>) -> TokenSaveError {
    TokenSaveError::Database {
        message: message.into(),
        operation: OPERATION.to_string(),
    }
}

pub fn is_refusal(error: &TokenSaveError) -> bool {
    matches!(error, TokenSaveError::Database { operation, .. } if operation == OPERATION)
}

async fn scalar(conn: &Connection, sql: &str) -> Result<i64> {
    let mut rows = conn.query(sql, ()).await?;
    let row = rows
        .next()
        .await?
        .ok_or_else(|| refusal("missing database header"))?;
    Ok(row.get(0)?)
}

/// Validate before any write pragmas or migration. Zero is for the global DB,
/// which has its own table migrations and currently uses `user_version` zero.
pub async fn check(conn: &Connection, latest: u32) -> Result<()> {
    check_channel(conn, latest, crate::cloud::is_beta()).await
}

async fn check_channel(conn: &Connection, latest: u32, beta: bool) -> Result<()> {
    let origin = scalar(conn, "PRAGMA application_id").await?;
    match origin {
        0 | STABLE => {},
        BETA if beta => {},
        BETA => return Err(refusal(
            "database was created by beta; use a beta build to access it. \
             Stable requires its own index. Preserve this database; do not force a rebuild."
        )),
        other => return Err(refusal(format!(
            "unrecognized database application ID {other}; preserve this database and use a compatible build."
        ))),
    }
    let version = scalar(conn, "PRAGMA user_version").await?;
    if version < 0 || version > i64::from(latest) {
        return Err(refusal(format!(
            "database schema version {version} is newer than or unsupported by this build (maximum {latest}); \
             use a compatible newer build. Preserve this database; do not force a rebuild."
        )));
    }
    Ok(())
}

/// Inspect existing files using a read-only connection, before opening writable.
/// SQLite may need existing WAL/SHM sidecars to read an active WAL database.
pub async fn preflight(path: &Path, latest: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let result: Result<()> = async {
        let db = Builder::new_local(path)
            .flags(OpenFlags::SQLITE_OPEN_READ_ONLY)
            .build()
            .await?;
        let conn = db.connect()?;
        check(&conn, latest).await
    }
    .await;
    result.map_err(|error| match error {
        TokenSaveError::Libsql(error) => TokenSaveError::Database {
            message: format!("failed to inspect database: {error}"),
            operation: "database_preflight".to_string(),
        },
        other => other,
    })
}

/// Stamp only an empty, unmarked database, before schema creation. Repeated
/// initialization and migrations must preserve legacy and stable origins.
pub async fn mark_empty(conn: &Connection) -> Result<()> {
    let count = scalar(conn, "SELECT count(*) FROM sqlite_schema").await?;
    let origin = scalar(conn, "PRAGMA application_id").await?;
    if count == 0 && origin == 0 {
        let origin = if crate::cloud::is_beta() {
            BETA
        } else {
            STABLE
        };
        conn.execute(&format!("PRAGMA application_id = {origin}"), ())
            .await?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn channel_policy_preserves_origin() {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        for (origin, stable_ok, beta_ok) in [
            (0, true, true),
            (STABLE, true, true),
            (BETA, false, true),
            (17, false, false),
        ] {
            conn.execute(&format!("PRAGMA application_id = {origin}"), ())
                .await
                .unwrap();
            assert_eq!(check_channel(&conn, 18, false).await.is_ok(), stable_ok);
            assert_eq!(check_channel(&conn, 18, true).await.is_ok(), beta_ok);
            assert_eq!(
                scalar(&conn, "PRAGMA application_id").await.unwrap(),
                origin
            );
        }
    }
}
