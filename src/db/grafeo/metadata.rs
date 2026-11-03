//! Key/value metadata.
use super::{db_error, get_opt_str, text, Database, PropMap, META};
use crate::errors::Result;

impl Database {
    /// Returns the value stored under `key`.
    pub async fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let Some(gid) = self
            .find("meta_key", key, "get_metadata")?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        Ok(self
            .graph_node(gid, "get_metadata")?
            .and_then(|n| get_opt_str(&n, "meta_value")))
    }

    /// Stores `value` under `key`, replacing any previous value.
    pub async fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        if let Some(gid) = self
            .find("meta_key", key, "set_metadata")?
            .into_iter()
            .next()
        {
            return self
                .db
                .set_node_property(gid, "meta_value", text(value))
                .map_err(|e| db_error("set_metadata", e));
        }
        let props = PropMap::from([
            ("meta_key".into(), text(key)),
            ("meta_value".into(), text(value)),
        ]);
        self.db
            .create_node_with_props(&[META], props)
            .map_err(|e| db_error("set_metadata", e))?;
        Ok(())
    }
}
