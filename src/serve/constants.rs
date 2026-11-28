use std::time::Duration;

pub(super) const AUTO_INIT_LOCK_FILENAME: &str = "mcp-auto-init.lock";
pub(super) const AUTO_INIT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const AUTO_INIT_LOCK_POLL: Duration = Duration::from_millis(50);
