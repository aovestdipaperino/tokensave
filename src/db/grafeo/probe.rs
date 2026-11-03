//! Lightweight read-only probes.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

/// Not ported yet. The grep hook treats an error as "the name may be
/// indexed" and keeps its pre-#686 deny, so this fails closed.
pub async fn any_name_indexed(_db_path: &Path, _names: &[String]) -> Result<bool> {
    Err(TokenSaveError::Database {
        message: "`any_name_indexed` is not supported by the grafeo storage backend yet"
            .to_string(),
        operation: "any_name_indexed".to_string(),
    })
}
