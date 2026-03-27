//! Background daemon that watches all tracked tokensave projects for file
//! changes and runs incremental syncs automatically.

use std::path::PathBuf;
use std::time::Duration;

use daemon_kit::{Daemon, DaemonConfig};

use crate::errors::TokenSaveError;

/// Parse a human-readable duration string like "15s" or "1m" into a Duration.
pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.trim().parse::<u64>().ok().map(Duration::from_secs)
    } else if let Some(mins) = s.strip_suffix('m') {
        mins.trim().parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    } else {
        s.parse::<u64>().ok().map(Duration::from_secs)
    }
}

/// Build the daemon-kit Daemon instance with tokensave paths.
pub fn build_daemon() -> std::result::Result<Daemon, TokenSaveError> {
    let home = dirs::home_dir().ok_or_else(|| TokenSaveError::Config {
        message: "cannot determine home directory".to_string(),
    })?;
    let ts_dir = home.join(".tokensave");
    let bin = crate::agents::which_tokensave().unwrap_or_else(|| "tokensave".to_string());

    let config = DaemonConfig::new("tokensave-daemon")
        .pid_dir(&ts_dir)
        .log_file(ts_dir.join("daemon.log"))
        .executable(PathBuf::from(bin))
        .service_args(vec!["daemon".to_string(), "--foreground".to_string()])
        .description("tokensave file watcher daemon");

    Ok(Daemon::new(config))
}

/// Returns the PID of the running daemon, or None.
pub fn running_daemon_pid() -> Option<u32> {
    build_daemon().ok()?.running_pid()
}

/// Returns true if an autostart service is installed.
pub fn is_autostart_enabled() -> bool {
    build_daemon().ok().is_some_and(|d| d.is_service_installed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration("15s"), Some(Duration::from_secs(15)));
        assert_eq!(parse_duration("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration(" 5s "), Some(Duration::from_secs(5)));
    }

    #[test]
    fn parse_duration_minutes() {
        assert_eq!(parse_duration("1m"), Some(Duration::from_secs(60)));
        assert_eq!(parse_duration("2m"), Some(Duration::from_secs(120)));
    }

    #[test]
    fn parse_duration_bare_number() {
        assert_eq!(parse_duration("10"), Some(Duration::from_secs(10)));
    }

    #[test]
    fn parse_duration_invalid() {
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("1h"), None);
    }
}
