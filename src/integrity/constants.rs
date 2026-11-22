//! Named values for Windows integrity-level detection (#1063).

/// Program that lists the current token's groups, including its mandatory
/// integrity label.
pub(super) const WHOAMI_PROGRAM: &str = "whoami";

/// `whoami` arguments: list groups as headerless CSV so the label SIDs can be
/// matched without depending on the localized column names.
pub(super) const WHOAMI_GROUP_ARGS: [&str; 4] = ["/groups", "/fo", "csv", "/nh"];

/// Mandatory label SIDs below the medium level. A process carrying one of them
/// cannot write to objects labeled medium, which includes ordinary user
/// folders such as the Desktop. Each value is quoted as in `whoami` CSV
/// output so that `S-1-16-0` does not match a prefix of another SID.
pub(super) const BELOW_MEDIUM_LABEL_SIDS: [&str; 2] = [
    // Untrusted Mandatory Level.
    "\"S-1-16-0\"",
    // Low Mandatory Level.
    "\"S-1-16-4096\"",
];
