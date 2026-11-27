//! Named values for the `doctor` MCP handshake probe (#587).

use std::time::Duration;

/// MCP revision the probe requests. Codex and other current clients send a
/// 2025 revision; the server echoes any revision it supports.
pub(super) const PROBE_PROTOCOL_VERSION: &str = "2025-06-18";

/// Client name the probe reports in `initialize`.
pub(super) const PROBE_CLIENT_NAME: &str = "tokensave-doctor";

/// JSON-RPC id of the probe's `initialize` request.
pub(super) const INITIALIZE_ID: u64 = 1;

/// JSON-RPC id of the probe's `tools/list` request.
pub(super) const TOOLS_LIST_ID: u64 = 2;

/// Most stdout bytes the probe reads from the server. A `tools/list` answer
/// is far smaller; the cap only bounds a misbehaving process.
pub(super) const MAX_PROBE_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;

/// How long the server gets to exit after its stdin closes before the probe
/// kills it, so it can unregister itself as a normal shutdown would.
pub(super) const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Interval between exit checks during [`SHUTDOWN_GRACE`].
pub(super) const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Upper bound on any probe, whatever timeout the agent configures.
pub const MAX_PROBE_DURATION: Duration = Duration::from_secs(120);
