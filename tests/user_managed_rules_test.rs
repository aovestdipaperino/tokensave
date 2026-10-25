//! #603: a team keeps its own `~/.claude/rules/tokensave.md`. Without an
//! opt-out, every install and upgrade resync replaced it again, and `doctor`
//! prescribed the very command that did. `manage_rules = false` in
//! `~/.tokensave/config.toml` hands the file to the user.
//!
//! The negative control runs the same install without the setting and fails
//! if the file is *not* replaced, so the test cannot pass by the install
//! silently doing nothing.

use std::path::Path;
use std::process::{Command, Output};

const MINE: &str = "# our team's rules, not tokensave's\n";

fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home)
        .env("LOCALAPPDATA", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("TOKENSAVE_SKIP_AGENT_MAINTENANCE", "1")
        .env_remove("TOKENSAVE_MANAGE_RULES")
        .output()
        .expect("run tokensave")
}

/// Install the Claude integration over a hand-written rules file and return
/// the file's contents afterwards, plus doctor's output.
fn install_over_own_file(manage_rules: Option<bool>) -> (String, String) {
    let home = tempfile::tempdir().expect("temp home");
    let rules = home.path().join(".claude").join("rules");
    std::fs::create_dir_all(&rules).unwrap();
    let rules_file = rules.join("tokensave.md");
    std::fs::write(&rules_file, MINE).unwrap();
    if let Some(value) = manage_rules {
        let dir = home.path().join(".tokensave");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), format!("manage_rules = {value}\n")).unwrap();
    }

    run(home.path(), &["install", "--agent", "claude"]);
    let after = std::fs::read_to_string(&rules_file).unwrap_or_default();
    let doctor = run(home.path(), &["doctor", "--agent", "claude"]);
    let doctor = format!(
        "{}{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );
    (after, doctor)
}

#[test]
fn manage_rules_false_keeps_the_users_rules_file() {
    let (after, doctor) = install_over_own_file(Some(false));
    assert_eq!(
        after, MINE,
        "install must leave a user-managed rules file alone"
    );
    assert!(doctor.contains("user-managed"), "doctor output: {doctor}");
    assert!(
        !doctor.contains("rules text drifted"),
        "doctor output: {doctor}"
    );
}

#[test]
fn without_the_setting_install_takes_the_file_over() {
    let (after, _) = install_over_own_file(None);
    assert_ne!(
        after, MINE,
        "negative control: install should replace an unowned file"
    );
}
