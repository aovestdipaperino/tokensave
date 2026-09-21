//! #575: `cargo test --workspace` rewrote the developer's real
//! `~/.claude/rules/tokensave.md`.
//!
//! A test that spawns the freshly built binary carries a `CARGO_PKG_VERSION`
//! ahead of whatever the machine last installed, which is the signal the
//! silent agent resync watches for external upgrades (`brew upgrade`,
//! `cargo install`). The resync then refreshed the managed rules file — in
//! the developer's own home, because the write and the marker that gates it
//! resolve the home directory two different ways.
//!
//! `TOKENSAVE_SKIP_AGENT_MAINTENANCE` turns the whole startup maintenance
//! block off, and `.cargo/config.toml` sets it for every cargo process. The
//! test below is the guard on that: the negative control runs the same
//! command with the variable removed and fails if the write does *not*
//! happen, so a change that quietly stops honouring the variable cannot pass
//! by making both halves silent.

use std::path::Path;
use std::process::Command;

/// Run `tokensave status` against a throwaway home holding a managed rules
/// file, and report whether the file was rewritten.
fn rules_file_rewritten(skip_maintenance: bool) -> bool {
    let home = tempfile::tempdir().expect("temp home");
    let project = tempfile::tempdir().expect("temp project");

    // A managed rules file whose content is *not* the canonical text, so a
    // refresh is a visible change rather than a no-op. This stands in for the
    // reporter's hand-maintained file.
    let rules = home.path().join(".claude").join("rules");
    std::fs::create_dir_all(&rules).expect("create rules dir");
    let rules_file = rules.join("tokensave.md");
    let mine = "# my own notes, not tokensave's\n";
    std::fs::write(&rules_file, mine).expect("write rules file");

    let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
    command
        .arg("status")
        .current_dir(project.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("APPDATA", home.path())
        .env("LOCALAPPDATA", home.path())
        .env("XDG_CONFIG_HOME", home.path().join(".config"));
    if skip_maintenance {
        command.env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1");
    } else {
        command.env_remove("TOKENSAVE_SKIP_AGENT_MAINTENANCE");
    }
    command.output().expect("run tokensave status");

    let after = std::fs::read_to_string(&rules_file).unwrap_or_default();
    let backup_exists = backup_beside(&rules_file);
    after != mine || backup_exists
}

/// `write_managed_rules_file` leaves a `.bak` beside the file it overwrites.
/// Either signal counts as "the file was touched".
fn backup_beside(rules_file: &Path) -> bool {
    let Some(dir) = rules_file.parent() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .starts_with("tokensave.md.bak")
    })
}

#[test]
fn the_env_var_keeps_the_managed_rules_file_untouched() {
    assert!(
        !rules_file_rewritten(true),
        "TOKENSAVE_SKIP_AGENT_MAINTENANCE=1 must leave ~/.claude/rules/tokensave.md alone"
    );
}

/// Negative control, so the assertion above cannot pass vacuously.
///
/// Unix only. Redirecting `HOME` moves both the write and the version marker
/// there, which is what makes the resync observable in a sandbox. On Windows
/// the marker goes through `dirs::home_dir` → `SHGetKnownFolderPath`, which
/// ignores `HOME` and `USERPROFILE`: the marker in the *real* home is already
/// advanced, the resync no-ops, and this control would fail for a reason that
/// has nothing to do with the variable. That divergence is the same one that
/// let the original bug escape a sandboxed run.
#[cfg(unix)]
#[test]
fn without_the_env_var_the_resync_still_refreshes_the_file() {
    assert!(
        rules_file_rewritten(false),
        "negative control: with the variable removed the resync should refresh \
         the managed rules file — if it no longer does, this test has stopped \
         guarding anything"
    );
}
