use std::time::Duration;

// Shared with scripts/upgrade-windows.ps1.
pub(super) const LOCK_FILE: &str = "tokensave.cargo-upgrade.lock";
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(250);
