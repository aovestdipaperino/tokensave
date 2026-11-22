//! Detection of a Windows process running below medium integrity (#1063).
//!
//! Windows starts a process at the lower of its parent's integrity level and
//! the mandatory label of its executable file. `cargo install` builds in
//! `%TEMP%`; when that directory carries an inherited Low mandatory label (as
//! some sandboxing tools apply), the installed `tokensave.exe` inherits it and
//! every run is a low-integrity process. Such a process cannot create files in
//! medium-labeled folders, so lockfiles, `.tokensave/` and the SQLite database
//! fail with "Access denied" (os error 5) even though the user's ACLs allow
//! the write. This module detects that state so the CLI can explain it.
//!
//! Detection runs `whoami /groups` instead of calling the token APIs directly,
//! which keeps the crate free of new `unsafe` code. It runs only on demand
//! (after a command fails, or in `doctor`) and its result is cached.

mod constants;

use std::sync::OnceLock;

/// Returns `true` when `whoami /groups /fo csv /nh` output lists a mandatory
/// label below medium (Untrusted or Low).
#[must_use]
pub fn groups_output_is_below_medium(output: &str) -> bool {
    output.lines().any(|line| {
        constants::BELOW_MEDIUM_LABEL_SIDS
            .iter()
            .any(|sid| line.contains(sid))
    })
}

/// Returns `true` when the current process runs below medium integrity.
///
/// Always `false` outside Windows, and `false` when the level cannot be
/// determined (for example when `whoami` is unavailable).
#[must_use]
pub fn process_is_below_medium() -> bool {
    static BELOW_MEDIUM: OnceLock<bool> = OnceLock::new();
    *BELOW_MEDIUM.get_or_init(detect_below_medium)
}

#[cfg(windows)]
fn detect_below_medium() -> bool {
    std::process::Command::new(constants::WHOAMI_PROGRAM)
        .args(constants::WHOAMI_GROUP_ARGS)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|out| {
            out.status.success()
                && groups_output_is_below_medium(&String::from_utf8_lossy(&out.stdout))
        })
}

#[cfg(not(windows))]
fn detect_below_medium() -> bool {
    false
}

/// Explains a low-integrity process and how to fix it, naming `exe` (the
/// running binary) in the suggested commands.
#[must_use]
pub fn low_integrity_hint(exe: &std::path::Path) -> String {
    let exe = exe.display();
    format!(
        "tokensave is running at low integrity level, so Windows denies writes to \
         ordinary (medium-integrity) folders such as this project's .tokensave \
         directory and .git\\hooks, whatever the folder ACLs allow. This usually \
         means the executable carries a Low mandatory label, for example inherited \
         from a labeled %TEMP% during `cargo install`.\n  \
         Check the label:   icacls \"{exe}\"\n  \
         Reset it to medium: icacls \"{exe}\" /setintegritylevel medium\n  \
         If %TEMP% itself is labeled Low (`icacls \"%TEMP%\"`), later \
         `cargo install` runs will reapply it."
    )
}

/// The low-integrity hint for the running binary, or `None` when the process
/// runs at medium integrity or above (or outside Windows).
#[must_use]
pub fn current_low_integrity_hint() -> Option<String> {
    if !process_is_below_medium() {
        return None;
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| "tokensave.exe".into());
    Some(low_integrity_hint(&exe))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_low_label_in_whoami_csv() {
        let output = "\"Everyone\",\"Well-known group\",\"S-1-1-0\",\"Mandatory group\"\n\
                      \"Mandatory Label\\Low Mandatory Level\",\"Label\",\"S-1-16-4096\",\"\"\n";
        assert!(groups_output_is_below_medium(output));
    }

    #[test]
    fn detects_untrusted_label() {
        assert!(groups_output_is_below_medium(
            "\"Mandatory Label\\Untrusted Mandatory Level\",\"Label\",\"S-1-16-0\",\"\"\n"
        ));
    }

    #[test]
    fn medium_and_high_labels_are_not_below_medium() {
        let output = "\"Everyone\",\"Well-known group\",\"S-1-1-0\",\"Mandatory group\"\n\
                      \"Etichetta obbligatoria\\Livello obbligatorio medio\",\"Etichetta\",\"S-1-16-8192\",\"\"\n\
                      \"Mandatory Label\\High Mandatory Level\",\"Label\",\"S-1-16-12288\",\"\"\n";
        assert!(!groups_output_is_below_medium(output));
    }

    #[test]
    fn empty_output_is_not_below_medium() {
        assert!(!groups_output_is_below_medium(""));
    }

    #[test]
    fn hint_names_the_binary_and_the_fix() {
        let hint = low_integrity_hint(std::path::Path::new(r"C:\bin\tokensave.exe"));
        assert!(hint.contains(r#"icacls "C:\bin\tokensave.exe" /setintegritylevel medium"#));
        assert!(hint.contains("low integrity"));
    }
}
