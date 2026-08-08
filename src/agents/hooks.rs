// ---------------------------------------------------------------------------
// Git post-commit hook
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};
use clap::ValueEnum;
use crate::agents::home_dir;
use crate::agents::expand_tilde;

/// Whether `tokensave install` should install the global git `post-commit`
/// hook, and if so, whether to ask the user interactively or act
/// non-interactively. The `Default` variant preserves the previous
/// behavior: prompt on a TTY, silently skip on a non-TTY.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GitHookMode {
    /// Preserve today's behavior — prompt on a TTY, silently skip otherwise.
    Default,
    /// Install the hook without asking, even on a TTY.
    Yes,
    /// Skip the hook install entirely, without asking.
    No,
}

/// The marker comment used to identify tokensave's section in a hook script.
const HOOK_MARKER: &str = "# tokensave: auto-sync";

/// Marker comment identifying tokensave's section in the post-checkout hook.
const HOOK_MARKER_CHECKOUT: &str = "# tokensave: auto-init";

/// Marker comment identifying the repo-hook chaining preamble (issue #164).
const HOOK_MARKER_CHAIN: &str = "# tokensave: chain-repo-hook";

/// Preamble that forwards a global hook to the repository's own hook.
///
/// A global `core.hooksPath` makes git ignore every repository's
/// `.git/hooks/` — including hooks copied there by `init.templateDir` —
/// so a tokensave-owned global hook must delegate to the repo's hook or
/// pre-existing user hooks silently stop running (issue #164). Uses
/// `git rev-parse --git-dir` (not `--git-path hooks`, which resolves
/// through `core.hooksPath` and would re-enter this very script).
fn chain_repo_hook_snippet(hook_name: &str) -> String {
    format!(
        "{HOOK_MARKER_CHAIN}\n\
         repo_hook=\"$(git rev-parse --git-dir 2>/dev/null)/hooks/{hook_name}\"\n\
         if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
         \t\"$repo_hook\" \"$@\"\n\
         fi\n"
    )
}

/// Client-side git hooks that tokensave does **not** itself install, but whose
/// per-repository copies would be silently disabled the moment tokensave claims
/// a global `core.hooksPath` (issue #164 follow-up).
///
/// A global `core.hooksPath` makes git resolve **every** hook type from that one
/// directory, with no fallback to `.git/hooks/`. The #164 fix only re-chained
/// the two hooks tokensave owns (`post-commit`, `post-checkout`), so a repo's
/// own `pre-commit`, `pre-push`, `commit-msg`, … (as delivered by
/// `init.templateDir`, husky, pre-commit, lefthook, …) still stopped running.
/// tokensave drops a pure forwarder for each of these so they keep firing.
///
/// `post-commit`/`post-checkout` are intentionally excluded — they are written
/// separately with the chaining preamble **plus** tokensave's own action. The
/// list is the client-side set from `githooks(5)`; server-side hooks
/// (`pre-receive`, `update`, `post-receive`, `post-update`, `proc-receive`) and
/// the config-driven `fsmonitor-watchman` are omitted.
const FORWARDED_REPO_HOOKS: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "pre-rebase",
    "post-merge",
    "pre-push",
    "post-rewrite",
    "pre-auto-gc",
    "post-index-change",
    "push-to-checkout",
    "sendemail-validate",
    "reference-transaction",
];

/// Install pure forwarders for every [`FORWARDED_REPO_HOOKS`] hook so that a
/// repository's own hooks of those types keep running after tokensave claims a
/// global `core.hooksPath`.
///
/// Only acts when tokensave owns the global hooks directory (it is claiming
/// `core.hooksPath` right now, or the configured dir is tokensave's default),
/// mirroring [`should_chain_repo_hooks`]; a user-managed `core.hooksPath` is
/// left untouched. Each forwarder is written only when no file of that name
/// already exists, so a hook the user placed in the directory — or a forwarder
/// from a previous run — is never clobbered.
fn install_repo_hook_forwarders(
    hooks_dir: &Path,
    claiming_hookspath: bool,
    hooks_dir_is_default: bool,
) {
    if !(claiming_hookspath || hooks_dir_is_default) {
        return;
    }
    for name in FORWARDED_REPO_HOOKS {
        let path = hooks_dir.join(name);
        if path.exists() {
            continue;
        }
        write_global_hook(&path, &chain_repo_hook_snippet(name));
    }
}

/// Whether the chaining preamble should be added to a global hook file.
///
/// Chain only when tokensave owns the global hooks directory — either it
/// is claiming `core.hooksPath` right now, or the configured hooks dir is
/// tokensave's default and the hook file is absent or tokensave-created.
/// A user-managed `core.hooksPath` setup is left alone: the user may
/// deliberately not forward to per-repo hooks.
fn should_chain_repo_hooks(
    claiming_hookspath: bool,
    hooks_dir_is_default: bool,
    existing_contents: Option<&str>,
) -> bool {
    if existing_contents.is_some_and(|c| c.contains(HOOK_MARKER_CHAIN)) {
        return false;
    }
    claiming_hookspath
        || (hooks_dir_is_default
        && existing_contents
        .is_none_or(|c| c.contains(HOOK_MARKER) || c.contains(HOOK_MARKER_CHECKOUT)))
}

/// The hook snippet appended to (or written as) the post-commit script.
fn post_commit_snippet(tokensave_bin: &str) -> String {
    let bin = tokensave_bin.replace('\\', "/");
    format!(
        "{HOOK_MARKER}\n\
         {bin} sync >/dev/null 2>&1 &\n"
    )
}

/// The hook snippet appended to (or written as) the post-checkout script.
///
/// Runs `tokensave init` in the background on the initial checkout of a fresh
/// clone — git passes the all-zeros sentinel as the previous HEAD in that case.
/// On an ordinary **branch** checkout (git passes flag `$3 == 1`) it runs
/// `tokensave branch add` to transparently track the just-checked-out branch;
/// that is a no-op when the branch is already tracked or is the default branch.
/// File checkouts (`$3 == 0`) trigger nothing.
fn post_checkout_snippet(tokensave_bin: &str) -> String {
    let bin = tokensave_bin.replace('\\', "/");
    format!(
        "{HOOK_MARKER_CHECKOUT}\n\
         if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
         \t{bin} init >/dev/null 2>&1 &\n\
         elif [ \"$3\" = \"1\" ]; then\n\
         \t{bin} branch add >/dev/null 2>&1 &\n\
         fi\n"
    )
}

/// Append `snippet` to an existing hook file (creating it with a `#!/bin/sh`
/// shebang first if absent), then mark it executable on Unix. Prints an error
/// and returns `false` on any I/O failure. Idempotency (skipping when the
/// tokensave marker is already present) is the caller's responsibility.
fn write_global_hook(hook_path: &Path, snippet: &str) -> bool {
    if hook_path.exists() {
        use std::io::Write;
        let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(hook_path) else {
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to open {} for writing",
                hook_path.display()
            );
            return false;
        };
        if write!(f, "\n{snippet}").is_err() {
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to write to {}",
                hook_path.display()
            );
            return false;
        }
    } else {
        let contents = format!("#!/bin/sh\n{snippet}");
        if std::fs::write(hook_path, contents).is_err() {
            eprintln!(
                "  \x1b[31m✘\x1b[0m Failed to create {}",
                hook_path.display()
            );
            return false;
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(hook_path, std::fs::Permissions::from_mode(0o755));
    }

    true
}

/// Action decided by [`decide_hook_action`]: what the caller should do
/// given the user-supplied mode and the current state of the global
/// `post-commit` hook file.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HookAction {
    /// Hook is already installed (marker present) — nothing to do. The
    /// caller may still print an informational message.
    AlreadyInstalled,
    /// Skip the install entirely (mode `No`, or default-mode non-TTY).
    Skip,
    /// Show the interactive prompt and act on the answer.
    Prompt,
    /// Install the hook now (no prompt).
    Install,
}

/// Pure decision: figure out what to do for the global post-commit hook
/// given the requested mode and the hook file's current contents (`None`
/// when the file does not exist or could not be read). The caller
/// handles all I/O.
pub(crate) fn decide_hook_action(mode: GitHookMode, hook_contents: Option<&str>) -> HookAction {
    if hook_contents.is_some_and(|c| c.contains(HOOK_MARKER)) {
        return HookAction::AlreadyInstalled;
    }

    match mode {
        GitHookMode::Default if atty_stdin() => HookAction::Prompt,
        GitHookMode::Default | GitHookMode::No => HookAction::Skip,
        GitHookMode::Yes => HookAction::Install,
    }
}

/// If a global git `post-commit` hook is not already set up for tokensave,
/// interactively asks the user whether to install one. Silently succeeds if
/// the hook is already present, if stdin is not a terminal, or if the user
/// declines. The `mode` argument lets the caller pre-decide the answer so
/// scripted installs do not have to drive an interactive prompt.
pub fn offer_git_post_commit_hook(tokensave_bin: &str, mode: GitHookMode) {
    let Some(home) = home_dir() else { return };

    // Determine the global hooks directory by reading core.hooksPath from
    // the global gitconfig file(s). Falls back to ~/.config/git/hooks/.
    let hooks_dir = read_global_hooks_path(&home);

    let default_hooks_dir = home.join(".config").join("git").join("hooks");
    let (hooks_dir, need_set_hookspath) = match hooks_dir {
        Some(dir) => (dir, false),
        None => (default_hooks_dir.clone(), true),
    };
    let hooks_dir_is_default = hooks_dir == default_hooks_dir;

    // Issue #164: a global core.hooksPath makes git ignore every repo's
    // .git/hooks/, where init.templateDir hooks are copied. tokensave's
    // hooks chain to the repo's own hooks (below) so nothing stops
    // running, but if we're about to claim core.hooksPath and the user
    // relies on a hook template, say so up front.
    if need_set_hookspath {
        let template_dir = [
            home.join(".gitconfig"),
            home.join(".config").join("git").join("config"),
        ]
            .iter()
            .find_map(|p| parse_gitconfig_value(p, "init", "templatedir"));
        if let Some(dir) = template_dir {
            eprintln!(
                "  \x1b[33m⚠\x1b[0m git init.templateDir is set ({dir}). Installing sets a global \
                 core.hooksPath, which makes git skip each repository's .git/hooks/. tokensave's \
                 global hooks forward to the repository's own hooks so they keep running."
            );
        }
    }

    let hook_path = hooks_dir.join("post-commit");

    // Read existing contents once so the decision is pure and the
    // install path can append without re-reading.
    let existing_contents: Option<String> = if hook_path.exists() {
        std::fs::read_to_string(&hook_path).ok()
    } else {
        None
    };

    // Whether to (re)write the post-commit hook. The post-checkout hook is
    // installed alongside it under the same opt-in, with its own marker, so a
    // pre-existing post-commit install still gains post-checkout on the next run.
    let install_post_commit = match decide_hook_action(mode, existing_contents.as_deref()) {
        HookAction::AlreadyInstalled => {
            eprintln!("  Global git post-commit hook already contains tokensave, skipping");
            false
        }
        HookAction::Skip => {
            // Mode `No` (or default-mode non-TTY). Stay quiet — script
            // callers asked for no output here.
            return;
        }
        HookAction::Prompt => {
            // TTY + default mode: ask, and bail entirely if the user declines.
            eprintln!();
            eprint!(
                "Install global git \x1b[1mpost-commit\x1b[0m + \x1b[1mpost-checkout\x1b[0m hooks to auto-run \x1b[1mtokensave sync\x1b[0m after each commit and \x1b[1mtokensave init\x1b[0m after a fresh clone? [y/N] "
            );
            let mut answer = String::new();
            if std::io::stdin().read_line(&mut answer).is_err() {
                return;
            }
            if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
                eprintln!("  Skipped git hooks");
                return;
            }
            true
        }
        HookAction::Install => true,
    };

    // Create the hooks directory if needed.
    if let Err(e) = std::fs::create_dir_all(&hooks_dir) {
        eprintln!(
            "  \x1b[31m✘\x1b[0m Failed to create {}: {e}",
            hooks_dir.display()
        );
        return;
    }

    // If no global hooksPath was configured, set it in ~/.gitconfig.
    if need_set_hookspath {
        let gitconfig_path = home.join(".gitconfig");
        if let Err(msg) = set_global_hooks_path(&gitconfig_path, &hooks_dir) {
            eprintln!("  \x1b[31m✘\x1b[0m {msg} — hook not installed");
            return;
        }
        eprintln!(
            "\x1b[32m✔\x1b[0m Set git core.hooksPath to {}",
            hooks_dir.display()
        );
    }

    // Issue #164: chain to the repo's own hook before tokensave's snippet
    // so hooks in .git/hooks/ (e.g. from init.templateDir) keep running.
    // Also retrofits tokensave-owned hook files from earlier versions.
    if should_chain_repo_hooks(
        need_set_hookspath,
        hooks_dir_is_default,
        existing_contents.as_deref(),
    ) {
        write_global_hook(&hook_path, &chain_repo_hook_snippet("post-commit"));
    }

    if install_post_commit && write_global_hook(&hook_path, &post_commit_snippet(tokensave_bin)) {
        eprintln!(
            "\x1b[32m✔\x1b[0m Installed global git post-commit hook at {}",
            hook_path.display()
        );
    }

    // Install the post-checkout hook so a fresh clone auto-initializes. Its
    // marker is independent of post-commit's, so this is skipped only when the
    // post-checkout hook itself is already present.
    let checkout_path = hooks_dir.join("post-checkout");
    let checkout_contents = std::fs::read_to_string(&checkout_path).ok();
    if should_chain_repo_hooks(
        need_set_hookspath,
        hooks_dir_is_default,
        checkout_contents.as_deref(),
    ) {
        write_global_hook(&checkout_path, &chain_repo_hook_snippet("post-checkout"));
    }
    let checkout_present = checkout_contents.is_some_and(|c| c.contains(HOOK_MARKER_CHECKOUT));
    if !checkout_present && write_global_hook(&checkout_path, &post_checkout_snippet(tokensave_bin))
    {
        eprintln!(
            "\x1b[32m✔\x1b[0m Installed global git post-checkout hook at {}",
            checkout_path.display()
        );
    }

    // Issue #164 follow-up: claiming a global core.hooksPath disables *every*
    // hook type in each repo's .git/hooks/, not just the two tokensave owns.
    // Drop pure forwarders for the remaining client-side hooks so a repo's own
    // pre-commit / pre-push / commit-msg / … keep running.
    install_repo_hook_forwarders(&hooks_dir, need_set_hookspath, hooks_dir_is_default);
}

/// Reads `core.hooksPath` from the global gitconfig files.
///
/// Checks `~/.gitconfig` first, then `~/.config/git/config` (the XDG
/// location). Returns the resolved absolute path, or `None` if the key
/// is absent from both files.
fn read_global_hooks_path(home: &Path) -> Option<PathBuf> {
    let candidates = [
        home.join(".gitconfig"),
        home.join(".config").join("git").join("config"),
    ];
    for path in &candidates {
        if let Some(value) = parse_gitconfig_value(path, "core", "hookspath") {
            let expanded = expand_tilde(&value, home);
            let p = PathBuf::from(&expanded);
            if p.is_absolute() {
                return Some(p);
            }
            // Relative paths in gitconfig are relative to the home dir.
            return Some(home.join(p));
        }
    }
    None
}

/// Minimal gitconfig parser: finds the value of `key` under `[section]`.
///
/// Key matching is case-insensitive (git config keys are case-insensitive).
/// Handles `key = value`, `key=value`, and quoted values.
fn parse_gitconfig_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    let section_lower = section.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();

    let mut in_section = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Parse section header: [core], [core "subsection"], etc.
            let header = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            let section_name = header.split_whitespace().next().unwrap_or("");
            in_section = section_name.eq_ignore_ascii_case(&section_lower);
            continue;
        }
        if !in_section {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        // Parse key = value
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().to_ascii_lowercase() == key_lower {
                let v = v.trim();
                // Strip surrounding quotes if present.
                let v = v
                    .strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .unwrap_or(v);
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Appends `core.hooksPath` to the global gitconfig file, creating it if
/// necessary. Appends to an existing `[core]` section if one exists,
/// otherwise adds a new one at the end of the file.
fn set_global_hooks_path(
    gitconfig_path: &Path,
    hooks_dir: &Path,
) -> std::result::Result<(), String> {
    let hooks_str = hooks_dir.to_string_lossy().replace('\\', "/");
    let contents = if gitconfig_path.exists() {
        std::fs::read_to_string(gitconfig_path)
            .map_err(|e| format!("Failed to read {}: {e}", gitconfig_path.display()))?
    } else {
        String::new()
    };

    let new_contents = insert_gitconfig_value(&contents, "core", "hooksPath", &hooks_str);

    if let Some(parent) = gitconfig_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(gitconfig_path, new_contents)
        .map_err(|e| format!("Failed to write {}: {e}", gitconfig_path.display()))?;
    Ok(())
}

/// Inserts `key = value` under `[section]` in gitconfig content.
/// If the section exists, appends the key after the last line of that section.
/// Otherwise appends a new section at the end.
fn insert_gitconfig_value(contents: &str, section: &str, key: &str, value: &str) -> String {
    let section_lower = section.to_ascii_lowercase();
    let lines: Vec<&str> = contents.lines().collect();
    let mut result = Vec::with_capacity(lines.len() + 3);
    let entry = format!("\t{key} = {value}");

    // Find the target section and the line index just before the next section.
    let mut section_end: Option<usize> = None;
    let mut in_section = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_section {
                // We've hit the next section — insert before it.
                section_end = Some(i);
                break;
            }
            let header = trimmed
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or("")
                .trim();
            let name = header.split_whitespace().next().unwrap_or("");
            if name.eq_ignore_ascii_case(&section_lower) {
                in_section = true;
            }
        }
    }
    if in_section && section_end.is_none() {
        // Section runs to end of file.
        section_end = Some(lines.len());
    }

    if let Some(insert_at) = section_end {
        for (i, line) in lines.iter().enumerate() {
            if i == insert_at {
                result.push(entry.as_str());
            }
            result.push(line);
        }
        // If inserting at end-of-file.
        if insert_at == lines.len() {
            result.push(&entry);
        }
    } else {
        // Section doesn't exist — append it.
        for line in &lines {
            result.push(line);
        }
        if !contents.is_empty() && !contents.ends_with('\n') {
            result.push("");
        }
        let section_header = format!("[{section}]");
        // We need to own these strings for the result.
        // Re-build as a String directly instead.
        let mut out = result.join("\n");
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&section_header);
        out.push('\n');
        out.push_str(&entry);
        out.push('\n');
        return out;
    }

    let mut out = result.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Returns true if stdin is connected to a terminal.
fn atty_stdin() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod git_hook_tests {
    use super::*;
    use crate::agents::*;
    use std::path::Path;

    #[test]
    fn parse_hookspath_basic() {
        let config = "[core]\n\thooksPath = /home/user/.git-hooks\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/home/user/.git-hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_quoted() {
        let config = "[core]\n\thooksPath = \"/home/user/my hooks\"\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/home/user/my hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_case_insensitive() {
        let config = "[Core]\n\tHooksPath = /tmp/hooks\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            Some("/tmp/hooks".to_string())
        );
    }

    #[test]
    fn parse_hookspath_missing() {
        let config = "[core]\n\tautocrlf = true\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            None
        );
    }

    #[test]
    fn parse_hookspath_wrong_section() {
        let config = "[user]\n\thooksPath = /nope\n[core]\n\tautocrlf = true\n";
        assert_eq!(
            parse_gitconfig_value_from_str(config, "core", "hookspath"),
            None
        );
    }

    #[test]
    fn insert_into_existing_section() {
        let config = "[user]\n\tname = Test\n[core]\n\tautocrlf = true\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("\thooksPath = /tmp/hooks"));
        assert!(result.contains("[core]"));
        assert!(result.contains("autocrlf = true"));
    }

    #[test]
    fn insert_new_section() {
        let config = "[user]\n\tname = Test\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("[core]\n\thooksPath = /tmp/hooks"));
    }

    #[test]
    fn insert_into_empty_file() {
        let result = insert_gitconfig_value("", "core", "hooksPath", "/tmp/hooks");
        assert!(result.contains("[core]\n\thooksPath = /tmp/hooks"));
    }

    #[test]
    fn insert_before_next_section() {
        let config = "[core]\n\tautocrlf = true\n[user]\n\tname = Test\n";
        let result = insert_gitconfig_value(config, "core", "hooksPath", "/tmp/hooks");
        // hooksPath should appear after autocrlf but before [user]
        let hooks_pos = result.find("hooksPath").unwrap();
        let user_pos = result.find("[user]").unwrap();
        let autocrlf_pos = result.find("autocrlf").unwrap();
        assert!(hooks_pos > autocrlf_pos);
        assert!(hooks_pos < user_pos);
    }

    #[test]
    fn expand_tilde_with_slash() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("~/hooks", home), "/home/test/hooks");
    }

    #[test]
    fn expand_tilde_bare() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("~", home), "/home/test");
    }

    #[test]
    fn expand_tilde_no_tilde() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde("/abs/path", home), "/abs/path");
    }

    #[test]
    fn decide_hook_action_yes_installs_when_file_missing() {
        assert_eq!(
            decide_hook_action(GitHookMode::Yes, None),
            HookAction::Install
        );
    }

    #[test]
    fn decide_hook_action_yes_installs_when_file_exists_without_marker() {
        let contents = "#!/bin/sh\necho hello\n";
        assert_eq!(
            decide_hook_action(GitHookMode::Yes, Some(contents)),
            HookAction::Install
        );
    }

    #[test]
    fn decide_hook_action_yes_reports_already_installed_when_marker_present() {
        let contents = "#!/bin/sh\n# tokensave: auto-sync\n/usr/bin/tokensave sync\n";
        assert_eq!(
            decide_hook_action(GitHookMode::Yes, Some(contents)),
            HookAction::AlreadyInstalled
        );
    }

    #[test]
    fn decide_hook_action_no_skips_even_when_file_missing() {
        assert_eq!(decide_hook_action(GitHookMode::No, None), HookAction::Skip);
    }

    #[test]
    fn post_checkout_snippet_inits_only_on_fresh_clone() {
        let s = post_checkout_snippet("/usr/local/bin/tokensave");
        assert!(
            s.contains(HOOK_MARKER_CHECKOUT),
            "must carry its idempotency marker, got: {s}"
        );
        assert!(
            s.contains("/usr/local/bin/tokensave init"),
            "must run `init` with the resolved binary, got: {s}"
        );
        assert!(
            s.contains("0000000000000000000000000000000000000000"),
            "must guard on the fresh-clone sentinel so branch switches re-route to branch add, got: {s}"
        );
        assert!(
            s.contains("elif [ \"$3\" = \"1\" ]")
                && s.contains("/usr/local/bin/tokensave branch add"),
            "must transparently track the branch on a branch checkout (flag $3==1), got: {s}"
        );
    }

    #[test]
    fn write_global_hook_creates_with_shebang_then_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("post-checkout");

        assert!(write_global_hook(&path, "FIRST\n"));
        let after_create = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_create.starts_with("#!/bin/sh\n"),
            "new hook file must get a shebang, got: {after_create}"
        );
        assert!(after_create.contains("FIRST"));

        assert!(write_global_hook(&path, "SECOND\n"));
        let after_append = std::fs::read_to_string(&path).unwrap();
        assert!(
            after_append.contains("FIRST") && after_append.contains("SECOND"),
            "second write must append, not clobber, got: {after_append}"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "hook must be executable");
        }
    }

    #[test]
    fn bare_name_resolves_through_injected_path() {
        // A bare `tokensave` that resolves via PATH must survive reinstall
        // (issue #161).
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("tokensave"), "").unwrap();
        let path_var = dir.path().to_string_lossy().to_string();
        assert!(command_resolves_to_tokensave_in(
            "tokensave",
            Some(&path_var)
        ));
        assert!(!command_resolves_to_tokensave_in(
            "tokensave",
            Some("/nonexistent")
        ));
        assert!(!command_resolves_to_tokensave_in("tokensave", None));
        // Foreign bare names never match regardless of PATH.
        assert!(!command_resolves_to_tokensave_in(
            "othertool",
            Some(&path_var)
        ));
    }

    #[test]
    fn preserve_mcp_command_replaces_stale_or_foreign_commands() {
        // Nonexistent absolute path: replace.
        assert_eq!(
            preserve_mcp_command_str(Some("/nonexistent/dir/tokensave"), "/new/tokensave"),
            "/new/tokensave"
        );
        // Not a tokensave binary at all: replace.
        assert_eq!(
            preserve_mcp_command_str(Some("/bin/sh"), "/new/tokensave"),
            "/new/tokensave"
        );
        // No previous entry: use the new path.
        assert_eq!(
            preserve_mcp_command_str(None, "/new/tokensave"),
            "/new/tokensave"
        );
    }

    #[test]
    fn preserve_mcp_command_reads_string_and_array_shapes() {
        let dir = tempfile::TempDir::new().unwrap();
        let abs = dir.path().join("tokensave");
        std::fs::write(&abs, "").unwrap();
        let abs = abs.to_string_lossy().to_string();

        let string_shape = serde_json::json!(abs);
        assert_eq!(preserve_mcp_command(Some(&string_shape), "/new/bin"), abs);

        let array_shape = serde_json::json!([abs, "serve"]);
        assert_eq!(preserve_mcp_command(Some(&array_shape), "/new/bin"), abs);
    }

    #[test]
    fn forwarded_hooks_cover_common_types_but_not_tokensave_owned() {
        // The two hooks tokensave installs itself carry the chain preamble
        // plus tokensave's action, so they must NOT be in the pure-forwarder
        // list (that would double-write / conflict).
        assert!(!FORWARDED_REPO_HOOKS.contains(&"post-commit"));
        assert!(!FORWARDED_REPO_HOOKS.contains(&"post-checkout"));
        // The high-value client-side hooks must be forwarded — these are where
        // husky / pre-commit / lefthook live.
        for h in ["pre-commit", "pre-push", "commit-msg", "prepare-commit-msg"] {
            assert!(
                FORWARDED_REPO_HOOKS.contains(&h),
                "{h} must be forwarded or a global hooksPath silently disables it"
            );
        }
        // Server-side hooks are irrelevant to a client `core.hooksPath` and
        // must not be written.
        for h in ["pre-receive", "update", "post-receive", "proc-receive"] {
            assert!(!FORWARDED_REPO_HOOKS.contains(&h));
        }
    }

    #[test]
    fn install_repo_hook_forwarders_writes_when_claiming_and_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        // A hook the user already placed in the dir must be preserved verbatim.
        let user_pre_commit = dir.path().join("pre-commit");
        std::fs::write(&user_pre_commit, "#!/bin/sh\n# user's own\n").unwrap();

        install_repo_hook_forwarders(dir.path(), true, true);

        // Existing file untouched.
        assert_eq!(
            std::fs::read_to_string(&user_pre_commit).unwrap(),
            "#!/bin/sh\n# user's own\n",
            "an existing hook must never be clobbered"
        );
        // A forwarder was created for a type that had no file, and it chains
        // to the repo's own hook of the same name.
        let created = std::fs::read_to_string(dir.path().join("pre-push")).unwrap();
        assert!(created.starts_with("#!/bin/sh\n"));
        assert!(created.contains(HOOK_MARKER_CHAIN));
        assert!(created.contains("/hooks/pre-push"));
        assert!(created.contains("git rev-parse --git-dir"));
    }

    #[test]
    fn install_repo_hook_forwarders_noop_for_user_managed_hookspath() {
        // Not claiming, and the dir is not tokensave's default → user-managed
        // core.hooksPath. tokensave must not write anything into it.
        let dir = tempfile::tempdir().unwrap();
        install_repo_hook_forwarders(dir.path(), false, false);
        assert!(
            !dir.path().join("pre-commit").exists(),
            "must leave a user-managed hooksPath directory untouched"
        );
    }

    #[test]
    fn chain_snippet_forwards_to_repo_hook_via_git_dir() {
        let s = chain_repo_hook_snippet("post-checkout");
        assert!(s.contains(HOOK_MARKER_CHAIN));
        // Must use --git-dir, not --git-path hooks: the latter resolves
        // through core.hooksPath and would re-enter the global hook.
        assert!(s.contains("git rev-parse --git-dir"));
        assert!(!s.contains("--git-path"));
        assert!(s.contains("/hooks/post-checkout"));
        // Args must be forwarded (post-checkout receives old/new/flag).
        assert!(s.contains("\"$@\""));
    }

    #[test]
    fn should_chain_when_claiming_hookspath() {
        assert!(should_chain_repo_hooks(true, true, None));
        assert!(should_chain_repo_hooks(true, false, None));
    }

    #[test]
    fn should_chain_retrofits_tokensave_owned_default_dir() {
        // Existing tokensave-created hook in the default dir gains chaining.
        assert!(should_chain_repo_hooks(
            false,
            true,
            Some("#!/bin/sh\n# tokensave: auto-sync\ntokensave sync &\n")
        ));
        // Absent file in the default dir also chains.
        assert!(should_chain_repo_hooks(false, true, None));
    }

    #[test]
    fn should_not_chain_user_managed_hookspath_or_twice() {
        // User configured their own core.hooksPath with their own hook.
        assert!(!should_chain_repo_hooks(
            false,
            false,
            Some("#!/bin/sh\nmy-own-hook\n")
        ));
        // Non-tokensave hook file in the default dir is user content too.
        assert!(!should_chain_repo_hooks(
            false,
            true,
            Some("#!/bin/sh\nmy-own-hook\n")
        ));
        // Already chained: never append a second preamble.
        assert!(!should_chain_repo_hooks(
            true,
            true,
            Some("#!/bin/sh\n# tokensave: chain-repo-hook\n")
        ));
    }

    #[test]
    fn decide_hook_action_no_still_reports_already_installed() {
        // The user explicitly opted out of changes, but we should still
        // report that the hook is already in place rather than silently
        // skipping. Caller prints the message.
        let contents = "# tokensave: auto-sync\nfoo\n";
        assert_eq!(
            decide_hook_action(GitHookMode::No, Some(contents)),
            HookAction::AlreadyInstalled
        );
    }

    #[test]
    fn decide_hook_action_default_skips_when_file_missing() {
        // On a non-TTY the default mode silently skips. We cannot
        // guarantee whether `atty_stdin()` is true or false in a test
        // process, so assert that the result is one of the two valid
        // outcomes.
        let action = decide_hook_action(GitHookMode::Default, None);
        assert!(matches!(action, HookAction::Skip | HookAction::Prompt));
    }

    #[test]
    fn decide_hook_action_default_already_installed_wins_over_tty() {
        let contents = "# tokensave: auto-sync\nfoo\n";
        assert_eq!(
            decide_hook_action(GitHookMode::Default, Some(contents)),
            HookAction::AlreadyInstalled
        );
    }

    /// Helper: parse from a string directly (avoids file I/O in tests).
    fn parse_gitconfig_value_from_str(contents: &str, section: &str, key: &str) -> Option<String> {
        let section_lower = section.to_ascii_lowercase();
        let key_lower = key.to_ascii_lowercase();
        let mut in_section = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                let header = trimmed
                    .trim_start_matches('[')
                    .split(']')
                    .next()
                    .unwrap_or("")
                    .trim();
                let section_name = header.split_whitespace().next().unwrap_or("");
                in_section = section_name.eq_ignore_ascii_case(&section_lower);
                continue;
            }
            if !in_section {
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim().to_ascii_lowercase() == key_lower {
                    let v = v.trim();
                    let v = v
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .unwrap_or(v);
                    return Some(v.to_string());
                }
            }
        }
        None
    }
}
