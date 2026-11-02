//! Compiled identity shared by CLI output, release packaging and OS resources.

pub const PRODUCT_NAME: &str = env!("TOKENSAVE_PRODUCT_NAME");
pub const CLI_NAME: &str = env!("TOKENSAVE_CLI_NAME");
pub const VERSION: &str = env!("TOKENSAVE_BUILD_VERSION");
pub const APPLICATION_ID: &str = env!("TOKENSAVE_APPLICATION_ID");

pub fn is_beta() -> bool {
    env!("TOKENSAVE_BUILD_BETA") == "true"
}

/// Name inside a release archive, based on the downloaded channel.
pub fn archive_executable(beta: bool, windows: bool) -> &'static str {
    match (beta, windows) {
        (true, true) => env!("TOKENSAVE_BETA_WINDOWS_EXECUTABLE"),
        (true, false) => env!("TOKENSAVE_BETA_UNIX_EXECUTABLE"),
        (false, true) => env!("TOKENSAVE_STABLE_WINDOWS_EXECUTABLE"),
        (false, false) => env!("TOKENSAVE_STABLE_UNIX_EXECUTABLE"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_share_canonical_archive_names() {
        assert_eq!(archive_executable(true, true), "tokensave.exe");
        assert_eq!(archive_executable(true, false), "tokensave");
        assert_eq!(archive_executable(false, true), "tokensave.exe");
        assert_eq!(archive_executable(false, false), "tokensave");
    }

    #[test]
    fn compiled_identity_matches_channel() {
        if is_beta() {
            assert_eq!(PRODUCT_NAME, "TokenSave Beta");
            assert_eq!(CLI_NAME, "tokensave");
            assert_eq!(APPLICATION_ID, "TokenSave.TokenSave.Beta");
            assert!(VERSION.contains('-'));
        } else {
            assert_eq!(PRODUCT_NAME, "TokenSave");
            assert_eq!(CLI_NAME, "tokensave");
            assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        }
    }
}
