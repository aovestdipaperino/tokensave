//! Cooperation with the repository's Windows Cargo upgrade helper.

mod constants;

use std::fs::File;
use std::path::Path;

/// Reject new launches and route upgrade requests to the existing cooperative
/// cancellation path. Keep monitoring on the runtime while the CLI is alive.
pub(crate) fn watch() -> std::io::Result<()> {
    let marker = std::env::current_exe()?.with_file_name(constants::LOCK_FILE);
    if upgrade_active(&marker)? {
        return Err(std::io::Error::other(
            "Cargo upgrade in progress. Retry when it finishes.",
        ));
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(constants::POLL_INTERVAL);
        loop {
            interval.tick().await;
            if matches!(upgrade_active(&marker), Ok(true)) {
                tokensave::cancel::request();
                return;
            }
        }
    });
    Ok(())
}

fn upgrade_active(marker: &Path) -> std::io::Result<bool> {
    match File::open(marker) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error)
            if error.raw_os_error()
                == Some(windows_sys::Win32::Foundation::ERROR_SHARING_VIOLATION as i32) =>
        {
            Ok(true)
        }
        Err(error) => Err(error),
    }
}
