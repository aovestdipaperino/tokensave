//! Named values for agent tool-approval management (#1067).
//!
//! `manage_permissions = false` in `~/.tokensave/config.toml` hands every
//! agent's tool approvals to the user; see
//! [`permissions_management_enabled`](super::permissions_management_enabled).

/// Key of the setting in `~/.tokensave/config.toml`. The serde field
/// `UserConfig::manage_permissions` carries the same name.
pub const MANAGE_PERMISSIONS_KEY: &str = "manage_permissions";

/// Environment variable that overrides [`MANAGE_PERMISSIONS_KEY`] for one run.
pub const MANAGE_PERMISSIONS_ENV: &str = "TOKENSAVE_MANAGE_PERMISSIONS";

/// Install/uninstall note printed when an integration leaves its tool
/// approvals alone.
pub const PERMISSIONS_USER_MANAGED_NOTE: &str =
    "  Tool approvals are user-managed (manage_permissions = false), leaving them alone";

/// Doctor line for an integration whose tool approvals the user manages.
pub const PERMISSIONS_USER_MANAGED_DOCTOR: &str =
    "Tool approvals are user-managed (manage_permissions = false); not checked";
