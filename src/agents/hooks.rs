// ---------------------------------------------------------------------------
// Git post-commit hook
// ---------------------------------------------------------------------------

use crate::agents::expand_tilde;
use crate::agents::home_dir;
use clap::ValueEnum;
use std::path::{Path, PathBuf};

/// Whether `tokensave install` should install the global git
/// `post-commit`/`post-checkout`/`post-merge` hooks, and if so, whether to
/// ask the user interactively or act non-interactively. The `Default`
/// variant preserves the previous behavior: prompt on a TTY, silently skip
/// on a non-TTY.
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

/// Marker comment identifying tokensave's section in the post-merge hook.
///
/// `git pull` on a fast-forward or non-rebase merge fires `post-merge`, not
/// `post-commit` or `post-checkout` — so without this hook, pulling a
/// teammate's changes left the index stale until the next local commit
/// (`post-commit`) or branch switch (`post-checkout`). The section shares
/// [`post_commit_snippet`]'s body (an unconditional background `sync`),
/// since `post-merge`'s arguments (`$1` squash flag) don't change what
/// tokensave needs to do.
const HOOK_MARKER_MERGE: &str = "# tokensave: auto-sync (post-merge)";

/// Marker comment closing tokensave's section in the post-checkout hook.
///
/// Written since #391 so that a migration can replace the section body in
/// place instead of having to pattern-match the shapes that shipped in 6.4.3
/// and 7.3.0 (both of which end in a bare `fi`). Whether such a migration runs
/// automatically or only on prompt is the open policy question in #342 Q1.
const HOOK_MARKER_CHECKOUT_END: &str = "# tokensave: end auto-init";

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
/// the three hooks tokensave owns (`post-commit`, `post-checkout`,
/// `post-merge`), so a repo's own `pre-commit`, `pre-push`, `commit-msg`, …
/// (as delivered by `init.templateDir`, husky, pre-commit, lefthook, …) still
/// stopped running. tokensave drops a pure forwarder for each of these so
/// they keep firing.
///
/// `post-commit`/`post-checkout`/`post-merge` are intentionally excluded —
/// they are written separately with the chaining preamble **plus**
/// tokensave's own action. The list is the client-side set from
/// `githooks(5)`; server-side hooks (`pre-receive`, `update`,
/// `post-receive`, `post-update`, `proc-receive`) and the config-driven
/// `fsmonitor-watchman` are omitted.
const FORWARDED_REPO_HOOKS: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "pre-rebase",
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
) -> Vec<&'static str> {
    let mut failed = Vec::new();
    if !(claiming_hookspath || hooks_dir_is_default) {
        return failed;
    }
    for name in FORWARDED_REPO_HOOKS {
        let path = hooks_dir.join(name);
        if path.exists() {
            continue;
        }
        if !write_global_hook(&path, &chain_repo_hook_snippet(name)) {
            failed.push(*name);
        }
    }
    failed
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

/// The hook snippet appended to (or written as) the post-merge script.
fn post_merge_snippet(tokensave_bin: &str) -> String {
    let bin = tokensave_bin.replace('\\', "/");
    format!(
        "{HOOK_MARKER_MERGE}\n\
         {bin} sync >/dev/null 2>&1 &\n"
    )
}

/// The hook snippet appended to (or written as) the post-checkout script.
///
/// git reports the initial checkout of a fresh clone — and of every new
/// `git worktree add` — by passing the all-zeros sentinel as the previous
/// HEAD. That checkout is **also** a branch checkout (git passes flag
/// `$3 == 1`) and it can land on a branch that is not the default one:
/// `git worktree add -b feature` and `git clone -b feature` both do. So the
/// sentinel arm runs `tokensave init` **and then**
/// `tokensave branch add --if-enabled`:
/// sequentially, because `branch add` copies the index that `init` creates,
/// and inside a single background job, because two independent background
/// jobs would race (#391).
///
/// Any other branch checkout (`$3 == 1` with a real previous HEAD) runs
/// `tokensave branch add --if-enabled` alone to transparently track the
/// just-checked-out branch; that is a no-op when the branch is already
/// tracked, is the default branch, or when `auto_track` is off (#397 — the
/// flag is what makes that knob authoritative on the hook path rather than
/// only inside `TokenSave::open`). File checkouts (`$3 == 0`) trigger nothing.
///
/// The section is fenced by [`HOOK_MARKER_CHECKOUT`] and
/// [`HOOK_MARKER_CHECKOUT_END`]. Changing this body does not reach an install
/// that already has the hook: the installer skips a post-checkout file that
/// already carries the marker, and [`write_global_hook`] never replaces
/// existing content. Migrating those installs is the open policy question in
/// #342 Q1.
fn post_checkout_snippet(tokensave_bin: &str) -> String {
    let bin = tokensave_bin.replace('\\', "/");
    format!(
        "{HOOK_MARKER_CHECKOUT}\n\
         if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
         \t(\n\
         \t\t{bin} init >/dev/null 2>&1 || exit 0\n\
         \t\t{bin} branch add --if-enabled >/dev/null 2>&1\n\
         \t) &\n\
         elif [ \"$3\" = \"1\" ]; then\n\
         \t{bin} branch add --if-enabled >/dev/null 2>&1 &\n\
         fi\n\
         {HOOK_MARKER_CHECKOUT_END}\n"
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
///
/// Returns `Err` only when the user asked for hooks and they could not be
/// written. Declining, or a hook that is already present, is `Ok` — those are
/// not failures. The specific reason is printed at the point of failure; the
/// returned message exists so an explicit `githooks on` can exit non-zero
/// instead of reporting a failed install as a success.
pub fn offer_git_post_commit_hook(tokensave_bin: &str, mode: GitHookMode) -> Result<(), String> {
    let Some(home) = home_dir() else {
        return Ok(());
    };

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
            return Ok(());
        }
        HookAction::Prompt => {
            // TTY + default mode: ask, and bail entirely if the user declines.
            eprintln!();
            eprint!(
                "Install global git \x1b[1mpost-commit\x1b[0m + \x1b[1mpost-checkout\x1b[0m + \x1b[1mpost-merge\x1b[0m hooks to auto-run \x1b[1mtokensave sync\x1b[0m after each commit and \x1b[1mgit pull\x1b[0m, and \x1b[1mtokensave init\x1b[0m after a fresh clone? [y/N] "
            );
            let mut answer = String::new();
            if std::io::stdin().read_line(&mut answer).is_err() {
                return Ok(());
            }
            if !matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
                eprintln!("  Skipped git hooks");
                return Ok(());
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
        return Err(format!("failed to create {}: {e}", hooks_dir.display()));
    }

    // If no global hooksPath was configured, set it in ~/.gitconfig.
    if need_set_hookspath {
        let gitconfig_path = home.join(".gitconfig");
        if let Err(msg) = set_global_hooks_path(&gitconfig_path, &hooks_dir) {
            eprintln!("  \x1b[31m✘\x1b[0m {msg} — hook not installed");
            return Err(format!("{msg} — hook not installed"));
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

    // Hooks the user asked for whose write failed. Collected rather than
    // returned early: this installs three hooks, and bailing on the first
    // would skip the other two the user also asked for.
    let mut failed: Vec<&str> = Vec::new();

    if install_post_commit {
        if write_global_hook(&hook_path, &post_commit_snippet(tokensave_bin)) {
            eprintln!(
                "\x1b[32m✔\x1b[0m Installed global git post-commit hook at {}",
                hook_path.display()
            );
        } else {
            failed.push("post-commit");
        }
    }

    // Install the post-checkout hook so a fresh clone or worktree
    // auto-initializes and tracks its branch. Its marker is independent of
    // post-commit's, so this is skipped only when the post-checkout hook
    // itself is already present — which also means a body change here never
    // reaches an existing install (#342 Q1).
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
    if !checkout_present {
        if write_global_hook(&checkout_path, &post_checkout_snippet(tokensave_bin)) {
            eprintln!(
                "\x1b[32m✔\x1b[0m Installed global git post-checkout hook at {}",
                checkout_path.display()
            );
        } else {
            failed.push("post-checkout");
        }
    }

    // Install the post-merge hook so `git pull` (fast-forward or a real
    // merge) reindexes teammates' changes instead of waiting for the next
    // local commit or branch switch. Same independent-marker treatment as
    // post-checkout above.
    let merge_path = hooks_dir.join("post-merge");
    let merge_contents = std::fs::read_to_string(&merge_path).ok();
    if should_chain_repo_hooks(
        need_set_hookspath,
        hooks_dir_is_default,
        merge_contents.as_deref(),
    ) {
        write_global_hook(&merge_path, &chain_repo_hook_snippet("post-merge"));
    }
    let merge_present = merge_contents.is_some_and(|c| c.contains(HOOK_MARKER_MERGE));
    if !merge_present {
        if write_global_hook(&merge_path, &post_merge_snippet(tokensave_bin)) {
            eprintln!(
                "\x1b[32m✔\x1b[0m Installed global git post-merge hook at {}",
                merge_path.display()
            );
        } else {
            failed.push("post-merge");
        }
    }

    // Issue #164 follow-up: claiming a global core.hooksPath disables *every*
    // hook type in each repo's .git/hooks/, not just the three tokensave owns.
    // Drop pure forwarders for the remaining client-side hooks so a repo's own
    // pre-commit / pre-push / commit-msg / … keep running.
    failed.extend(install_repo_hook_forwarders(
        &hooks_dir,
        need_set_hookspath,
        hooks_dir_is_default,
    ));

    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "could not install git hooks: {}",
            failed.join(", ")
        ))
    }
}

// ---------------------------------------------------------------------------
// Per-repository hooks (#455)
// ---------------------------------------------------------------------------

/// What [`install_local_git_hooks`] did, so the caller can report it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct LocalHookInstall {
    /// The repository hook directory that was written to.
    pub hooks_dir: PathBuf,
    /// Hooks installed by this run.
    pub installed: Vec<String>,
    /// Hooks that already carried tokensave's section.
    pub already_present: Vec<String>,
    /// Hooks whose write was attempted and failed. The reason was printed at
    /// the point of failure; this records it so the caller can exit non-zero
    /// rather than reporting a partial install as a success.
    pub failed: Vec<String>,
    /// Set when a `core.hooksPath` is in effect for this repository, which
    /// makes git resolve every hook from *that* directory and ignore the one
    /// written here.
    pub shadowed_by: Option<PathBuf>,
}

/// Run `git` in `repo` and return trimmed stdout on success.
fn git_output(repo: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The directory holding this repository's own hooks.
///
/// Resolved from `--git-common-dir` rather than `--git-dir` because linked
/// worktrees share one hook directory with the main checkout: reading
/// `--git-dir` from inside a worktree would write hooks to a per-worktree
/// directory git never consults.
pub fn repo_hooks_dir(repo: &Path) -> Option<PathBuf> {
    let common = git_output(repo, &["rev-parse", "--git-common-dir"])?;
    let common = PathBuf::from(&common);
    let common = if common.is_absolute() {
        common
    } else {
        repo.join(common)
    };
    Some(common.join("hooks"))
}

/// A `core.hooksPath` in effect for this repository, from any config scope.
///
/// When one is set, git resolves **every** hook from there and never looks at
/// the repository's own hook directory — so a hook installed there would be
/// silently inert. That is the whole complaint behind #455 pointed the other
/// way, and the caller says so rather than reporting a successful install of
/// something that will not run.
fn effective_hooks_path(repo: &Path) -> Option<PathBuf> {
    git_output(repo, &["config", "--get", "core.hooksPath"]).map(PathBuf::from)
}

/// Are tokensave's *global* hooks installed?
///
/// Checked before offering local ones: a repository covered by both would run
/// a sync twice per commit.
pub fn global_git_hooks_installed() -> bool {
    let Some(home) = home_dir() else {
        return false;
    };
    let hooks_dir = read_global_hooks_path(&home)
        .unwrap_or_else(|| home.join(".config").join("git").join("hooks"));
    [
        ("post-commit", HOOK_MARKER),
        ("post-checkout", HOOK_MARKER_CHECKOUT),
        ("post-merge", HOOK_MARKER_MERGE),
    ]
    .iter()
    .any(|(name, marker)| {
        std::fs::read_to_string(hooks_dir.join(name)).is_ok_and(|c| c.contains(*marker))
    })
}

/// Does this repository's hook directory already carry tokensave's sections?
pub fn local_git_hooks_present(repo: &Path) -> bool {
    let Some(dir) = repo_hooks_dir(repo) else {
        return false;
    };
    [
        ("post-commit", HOOK_MARKER),
        ("post-checkout", HOOK_MARKER_CHECKOUT),
        ("post-merge", HOOK_MARKER_MERGE),
    ]
    .iter()
    .any(|(name, marker)| {
        std::fs::read_to_string(dir.join(name)).is_ok_and(|c| c.contains(*marker))
    })
}

/// Install tokensave's three hooks into this repository's own hook directory,
/// leaving `core.hooksPath` alone (#455).
///
/// A global `core.hooksPath` is a single setting for every repository on the
/// machine, so claiming it forces one hook set on all of them — which is
/// wrong for anyone whose projects need different tooling. Per-repository
/// hooks are the git-native answer, and they need no global config at all.
///
/// Writing is additive in the same way the global path is: an existing hook
/// keeps everything it already has and gains a marked tokensave section, so a
/// repository's husky or pre-commit setup is not disturbed. There is
/// deliberately no chaining preamble here — the repository's own hook *is*
/// this file, and a forwarder would invoke itself.
pub fn install_local_git_hooks(
    repo: &Path,
    tokensave_bin: &str,
) -> Result<LocalHookInstall, String> {
    let hooks_dir = repo_hooks_dir(repo)
        .ok_or_else(|| format!("{} is not a git repository", repo.display()))?;
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("failed to create {}: {e}", hooks_dir.display()))?;

    let mut result = LocalHookInstall {
        hooks_dir: hooks_dir.clone(),
        shadowed_by: effective_hooks_path(repo),
        ..Default::default()
    };

    for (name, marker, snippet) in [
        (
            "post-commit",
            HOOK_MARKER,
            post_commit_snippet(tokensave_bin),
        ),
        (
            "post-checkout",
            HOOK_MARKER_CHECKOUT,
            post_checkout_snippet(tokensave_bin),
        ),
        (
            "post-merge",
            HOOK_MARKER_MERGE,
            post_merge_snippet(tokensave_bin),
        ),
    ] {
        let path = hooks_dir.join(name);
        let existing = std::fs::read_to_string(&path).ok();
        if existing.is_some_and(|c| c.contains(marker)) {
            result.already_present.push(name.to_string());
            continue;
        }
        if write_global_hook(&path, &snippet) {
            result.installed.push(name.to_string());
        } else {
            result.failed.push(name.to_string());
        }
    }
    Ok(result)
}

/// Remove tokensave's sections from this repository's own hooks.
///
/// Same conservative rule as the global removal: a hook holding anything
/// tokensave did not write keeps that content and loses only the marked
/// section; a file that is nothing but tokensave's own content is deleted. No
/// git config is touched, because installing never set any.
pub fn remove_local_git_hooks(repo: &Path) -> HookRemoval {
    let Some(hooks_dir) = repo_hooks_dir(repo) else {
        return HookRemoval::default();
    };
    let mut result = HookRemoval {
        hooks_dir: hooks_dir.clone(),
        ..Default::default()
    };
    for name in ["post-commit", "post-checkout", "post-merge"] {
        let path = hooks_dir.join(name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(stripped) = strip_tokensave_sections(&contents) else {
            continue;
        };
        if is_inert_hook(&stripped) {
            if std::fs::remove_file(&path).is_ok() {
                result.deleted.push(path);
            }
        } else if std::fs::write(&path, &stripped).is_ok() {
            result.cleaned.push(path);
        }
    }
    result
}

/// Report this repository's own hooks, for `tokensave githooks --local`.
pub fn describe_local_git_hooks(repo: &Path) -> Vec<String> {
    let Some(hooks_dir) = repo_hooks_dir(repo) else {
        return vec![format!("{} is not a git repository", repo.display())];
    };
    let mut out = vec![format!("repository hooks: {}", hooks_dir.display())];

    if let Some(path) = effective_hooks_path(repo) {
        out.push(format!(
            "core.hooksPath is set to {} — git reads hooks from there and ignores the directory above",
            path.display()
        ));
    }

    let mut acting = Vec::new();
    for (name, marker) in [
        ("post-commit", HOOK_MARKER),
        ("post-checkout", HOOK_MARKER_CHECKOUT),
        ("post-merge", HOOK_MARKER_MERGE),
    ] {
        if std::fs::read_to_string(hooks_dir.join(name)).is_ok_and(|c| c.contains(marker)) {
            acting.push(name);
        }
    }

    if acting.is_empty() {
        out.push("no tokensave hooks installed in this repository".to_string());
        out.push("install them with `tokensave githooks on --local`".to_string());
        return out;
    }
    for name in acting {
        out.push(format!("{name}: runs tokensave"));
    }
    out.push("remove them with `tokensave githooks off --local`".to_string());
    out
}

// ---------------------------------------------------------------------------
// Removal (#420)
// ---------------------------------------------------------------------------

/// What [`remove_git_hooks`] did, so the caller can report it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HookRemoval {
    /// The global hooks directory that was inspected.
    pub hooks_dir: PathBuf,
    /// Hook files that had a tokensave section removed but kept other content.
    pub cleaned: Vec<PathBuf>,
    /// Hook files deleted because nothing but tokensave's own content was left.
    pub deleted: Vec<PathBuf>,
    /// Whether `core.hooksPath` was unset from the global gitconfig.
    pub hooks_path_unset: bool,
    /// Set when the hooks directory was left in place because it still holds
    /// files tokensave did not write.
    pub dir_kept_for_foreign_files: bool,
}

impl HookRemoval {
    /// True when nothing tokensave-owned was found to remove.
    pub fn found_nothing(&self) -> bool {
        self.cleaned.is_empty() && self.deleted.is_empty() && !self.hooks_path_unset
    }
}

/// True for a line that opens or closes one of tokensave's hook sections.
fn is_tokensave_marker(line: &str) -> bool {
    line.trim_start().starts_with("# tokensave:")
}

/// Remove every tokensave-owned section from a hook script.
///
/// A section runs from its `# tokensave:` marker to the first blank line, to
/// the section's own end marker, or to end of file — whichever comes first.
/// That rule covers all three shapes that have shipped: `post-commit`, whose
/// section is a marker plus one command and has never had an end marker; the
/// fenced `post-checkout` section written since #391; and the unfenced
/// `post-checkout` bodies from 6.4.3 and 7.3.0, which end in a bare `fi`
/// followed by a blank line. [`write_global_hook`] separates every appended
/// snippet with a blank line, so the boundary is reliable.
///
/// Returns `None` when the script holds no tokensave section, so a hook file
/// the user wrote themselves is never rewritten.
pub(crate) fn strip_tokensave_sections(contents: &str) -> Option<String> {
    if !contents.contains("# tokensave:") {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    let mut lines = contents.lines().peekable();
    while let Some(line) = lines.next() {
        if !is_tokensave_marker(line) {
            out.push(line);
            continue;
        }
        // Consume the section body. An end marker is consumed with it; a blank
        // line is the boundary and is left for the blank-run collapse below.
        let fenced = line.trim() == HOOK_MARKER_CHECKOUT;
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                break;
            }
            let is_end = fenced && next.trim() == HOOK_MARKER_CHECKOUT_END;
            let stop_before = !is_end && is_tokensave_marker(next);
            if stop_before {
                break;
            }
            lines.next();
            if is_end {
                break;
            }
        }
    }
    // Collapse the blank runs the removals left behind.
    let mut cleaned: Vec<&str> = Vec::new();
    for line in out {
        if line.trim().is_empty() && cleaned.last().is_none_or(|p| p.trim().is_empty()) {
            continue;
        }
        cleaned.push(line);
    }
    while cleaned.last().is_some_and(|l| l.trim().is_empty()) {
        cleaned.pop();
    }
    if cleaned.is_empty() {
        return Some(String::new());
    }
    Some(format!("{}\n", cleaned.join("\n")))
}

/// True when what is left of a hook script would do nothing — empty, or a
/// shebang and whitespace only. Such a file is tokensave's to delete; anything
/// else is the user's and is kept with the tokensave section stripped out.
fn is_inert_hook(contents: &str) -> bool {
    contents
        .lines()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with("#!"))
}

/// Describe the global git hooks tokensave currently owns, one line per
/// finding, for `tokensave githooks` with no action.
///
/// A read-only query: it opens the hooks directory and the gitconfig and
/// writes neither (#419).
pub fn describe_git_hooks() -> Vec<String> {
    let Some(home) = home_dir() else {
        return vec!["could not determine home directory".to_string()];
    };
    let default_hooks_dir = home.join(".config").join("git").join("hooks");
    let configured = read_global_hooks_path(&home);
    let hooks_dir = configured
        .clone()
        .unwrap_or_else(|| default_hooks_dir.clone());

    let mut out = vec![match &configured {
        Some(d) => format!("core.hooksPath: {}", d.display()),
        None => "core.hooksPath: not set".to_string(),
    }];

    let owned = ["post-commit", "post-checkout", "post-merge"];
    let mut acting: Vec<&str> = Vec::new();
    let mut forwarders = 0usize;
    for name in owned.iter().copied().chain(
        FORWARDED_REPO_HOOKS
            .iter()
            .copied()
            .filter(|n| !owned.contains(n)),
    ) {
        let Ok(contents) = std::fs::read_to_string(hooks_dir.join(name)) else {
            continue;
        };
        if contents.contains(HOOK_MARKER)
            || contents.contains(HOOK_MARKER_CHECKOUT)
            || contents.contains(HOOK_MARKER_MERGE)
        {
            acting.push(name);
        } else if contents.contains(HOOK_MARKER_CHAIN) {
            forwarders += 1;
        }
    }

    if acting.is_empty() && forwarders == 0 {
        out.push(format!(
            "no tokensave git hooks installed in {}",
            hooks_dir.display()
        ));
        out.push("install them with `tokensave githooks on`".to_string());
        return out;
    }
    for name in &acting {
        out.push(format!("{name}: runs tokensave"));
    }
    if forwarders > 0 {
        out.push(format!(
            "{forwarders} forwarder(s) to each repository's own hooks"
        ));
    }
    out.push("remove them with `tokensave githooks off`".to_string());
    out
}

/// Remove tokensave's global git hooks: the `post-commit`, `post-checkout`,
/// and `post-merge` sections, the pure forwarders installed for every other client-side hook
/// (#164 follow-up), and `core.hooksPath` when tokensave's own default
/// directory is left empty.
///
/// The inverse of [`offer_git_post_commit_hook`], and the missing half of
/// `tokensave uninstall` (#420): before this, uninstalling from every agent
/// and deleting a project's index still left a global `post-commit` hook that
/// recreated the index on the next commit.
///
/// Conservative by construction. A hook file that holds anything tokensave did
/// not write keeps that content and loses only the marked section. A
/// `core.hooksPath` pointing anywhere other than tokensave's default is left
/// alone entirely — the user chose it — as is the default directory itself
/// while it still holds files tokensave did not write.
pub fn remove_git_hooks() -> HookRemoval {
    let Some(home) = home_dir() else {
        return HookRemoval::default();
    };
    let default_hooks_dir = home.join(".config").join("git").join("hooks");
    let hooks_dir = read_global_hooks_path(&home).unwrap_or_else(|| default_hooks_dir.clone());
    let mut result = HookRemoval {
        hooks_dir: hooks_dir.clone(),
        ..Default::default()
    };
    if !hooks_dir.exists() {
        return result;
    }

    // post-commit, post-checkout, and post-merge carry tokensave's own
    // actions; the rest of FORWARDED_REPO_HOOKS are pure forwarders. All are
    // handled by the same strip-then-delete-if-inert rule.
    let owned = ["post-commit", "post-checkout", "post-merge"];
    for name in owned.iter().copied().chain(
        FORWARDED_REPO_HOOKS
            .iter()
            .copied()
            .filter(|n| !owned.contains(n)),
    ) {
        let path = hooks_dir.join(name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(stripped) = strip_tokensave_sections(&contents) else {
            continue;
        };
        if is_inert_hook(&stripped) {
            if std::fs::remove_file(&path).is_ok() {
                result.deleted.push(path);
            }
        } else if std::fs::write(&path, &stripped).is_ok() {
            result.cleaned.push(path);
        }
    }

    // Only tokensave's own default directory is a candidate for teardown; a
    // path the user configured stays, and so does core.hooksPath pointing at it.
    if hooks_dir != default_hooks_dir {
        return result;
    }
    // And only when this run actually removed a tokensave hook. A user who set
    // core.hooksPath to this same default path themselves, with no tokensave
    // hooks in it, must not have their config edited by a command that found
    // nothing of ours to remove — the path alone cannot tell us who wrote it.
    // The cost is that an already-empty leftover directory is left behind; that
    // is inert, since git simply finds no hooks there.
    if result.cleaned.is_empty() && result.deleted.is_empty() {
        return result;
    }
    let empty = std::fs::read_dir(&hooks_dir).is_ok_and(|mut d| d.next().is_none());
    if !empty {
        result.dir_kept_for_foreign_files = true;
        return result;
    }
    let _ = std::fs::remove_dir(&hooks_dir);
    if unset_global_hooks_path(&home.join(".gitconfig"), &hooks_dir) {
        result.hooks_path_unset = true;
    }
    result
}

/// Remove a `hooksPath = <dir>` entry from the `[core]` section of a gitconfig,
/// and the now-empty `[core]` header with it. Returns true when the file
/// changed.
///
/// Only removes an entry pointing at `expected_dir`, so a value the user
/// repointed elsewhere between install and uninstall is left alone.
fn unset_global_hooks_path(gitconfig_path: &Path, expected_dir: &Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(gitconfig_path) else {
        return false;
    };
    let expected = expected_dir.to_string_lossy().replace('\\', "/");
    let mut out: Vec<&str> = Vec::new();
    let mut removed = false;
    for line in contents.lines() {
        let t = line.trim();
        if let Some((key, value)) = t.split_once('=') {
            if key.trim().eq_ignore_ascii_case("hookspath")
                && value.trim().replace('\\', "/") == expected
            {
                removed = true;
                continue;
            }
        }
        out.push(line);
    }
    if !removed {
        return false;
    }
    // Drop a `[core]` header the removal just emptied.
    let mut pruned: Vec<&str> = Vec::new();
    for (i, line) in out.iter().enumerate() {
        if line.trim().eq_ignore_ascii_case("[core]") {
            let next_meaningful = out[i + 1..]
                .iter()
                .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
            if next_meaningful.is_none_or(|l| l.trim_start().starts_with('[')) {
                continue;
            }
        }
        pruned.push(line);
    }
    let mut text = pruned.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    std::fs::write(gitconfig_path, text).is_ok()
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
    fn post_merge_snippet_runs_sync_unconditionally() {
        let s = post_merge_snippet("/usr/local/bin/tokensave");
        assert!(
            s.contains(HOOK_MARKER_MERGE),
            "must carry its idempotency marker, got: {s}"
        );
        assert!(
            s.contains("/usr/local/bin/tokensave sync"),
            "must run `sync` with the resolved binary so a `git pull` reindexes teammates' changes, got: {s}"
        );
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
                && s.contains("/usr/local/bin/tokensave branch add --if-enabled"),
            "must transparently track the branch on a branch checkout (flag $3==1), got: {s}"
        );
        assert!(
            s.trim_end().ends_with(HOOK_MARKER_CHECKOUT_END),
            "the section must be fenced so a migration can replace it in place, got: {s}"
        );
    }

    /// Runs the generated post-checkout snippet under `sh`, with a stub script
    /// standing in for the tokensave binary, and returns the commands it
    /// invoked in the order it invoked them.
    ///
    /// The snippet backgrounds its work — git does not wait for it — so the
    /// wrapper appends `wait` to make the observation deterministic.
    #[cfg(unix)]
    fn run_post_checkout_snippet(dir: &Path, args: &[&str]) -> Vec<String> {
        use std::os::unix::fs::PermissionsExt;

        let log = dir.join("calls.log");
        let stub = dir.join("tokensave-stub");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\necho \"$*\" >> \"{}\"\n", log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();

        let hook = dir.join("post-checkout");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\n{}\nwait\n",
                post_checkout_snippet(stub.to_str().unwrap())
            ),
        )
        .unwrap();

        // `sh` is required for this test; a missing shell is a broken
        // environment, not a skippable condition.
        let status = std::process::Command::new("sh")
            .arg(&hook)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "hook script failed: {status}");

        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(std::string::ToString::to_string)
            .collect()
    }

    /// The regression test for #391, and the reason it is written by executing
    /// the snippet rather than by matching substrings: the bug was that two
    /// correct-looking commands sat in mutually exclusive arms, which every
    /// `contains` assertion in the test above passes straight over.
    #[cfg(unix)]
    #[test]
    fn post_checkout_snippet_tracks_the_branch_of_a_fresh_worktree_or_clone() {
        const ZERO: &str = "0000000000000000000000000000000000000000";
        const SHA: &str = "1111111111111111111111111111111111111111";

        // A new worktree or a fresh clone: git passes the all-zeros previous
        // HEAD *and* flag 1, and the branch checked out need not be the
        // default one (`git worktree add -b`, `git clone -b`). Both commands
        // must run, `init` first, so `branch add` has an index to copy.
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            run_post_checkout_snippet(dir.path(), &[ZERO, SHA, "1"]),
            vec!["init".to_string(), "branch add --if-enabled".to_string()],
            "a fresh worktree/clone must be indexed AND have its branch tracked"
        );

        // An ordinary branch switch tracks the branch without re-indexing.
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            run_post_checkout_snippet(dir.path(), &[SHA, SHA, "1"]),
            vec!["branch add --if-enabled".to_string()],
            "a branch switch must not re-run init"
        );

        // A file checkout triggers nothing at all.
        let dir = tempfile::tempdir().unwrap();
        assert!(
            run_post_checkout_snippet(dir.path(), &[SHA, SHA, "0"]).is_empty(),
            "a file checkout must not run tokensave"
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
        // The three hooks tokensave installs itself carry the chain preamble
        // plus tokensave's action, so they must NOT be in the pure-forwarder
        // list (that would double-write / conflict).
        assert!(!FORWARDED_REPO_HOOKS.contains(&"post-commit"));
        assert!(!FORWARDED_REPO_HOOKS.contains(&"post-checkout"));
        assert!(!FORWARDED_REPO_HOOKS.contains(&"post-merge"));
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

    // -----------------------------------------------------------------------
    // #420: removal
    // -----------------------------------------------------------------------

    /// A post-commit hook as the installer writes it: shebang, chaining
    /// preamble, then tokensave's own section.
    fn installed_post_commit() -> String {
        format!(
            "#!/bin/sh\n{}\n{}",
            chain_repo_hook_snippet("post-commit"),
            post_commit_snippet("tokensave")
        )
    }

    #[test]
    fn strip_removes_every_tokensave_section_from_a_pure_tokensave_hook() {
        let stripped = strip_tokensave_sections(&installed_post_commit()).unwrap();
        assert!(
            is_inert_hook(&stripped),
            "nothing but a shebang should remain, got: {stripped:?}"
        );
        assert!(!stripped.contains("tokensave"));
    }

    #[test]
    fn strip_removes_the_post_merge_section_from_a_pure_tokensave_hook() {
        let installed = format!(
            "#!/bin/sh\n{}\n{}",
            chain_repo_hook_snippet("post-merge"),
            post_merge_snippet("tokensave")
        );
        let stripped = strip_tokensave_sections(&installed).unwrap();
        assert!(
            is_inert_hook(&stripped),
            "nothing but a shebang should remain, got: {stripped:?}"
        );
        assert!(!stripped.contains("tokensave"));
    }

    #[test]
    fn strip_keeps_a_users_own_hook_content() {
        let mixed = format!(
            "#!/bin/sh\necho \"my guard\"\nexit 0\n\n{}",
            chain_repo_hook_snippet("pre-push")
        );
        let stripped = strip_tokensave_sections(&mixed).unwrap();
        assert!(stripped.contains("echo \"my guard\""));
        assert!(stripped.contains("exit 0"));
        assert!(!stripped.contains("tokensave"));
        assert!(
            !is_inert_hook(&stripped),
            "a hook with the user's own commands must not be treated as deletable"
        );
    }

    #[test]
    fn strip_leaves_a_hook_with_no_tokensave_section_alone() {
        let theirs = "#!/bin/sh\nnpx lint-staged\n";
        assert_eq!(
            strip_tokensave_sections(theirs),
            None,
            "a file we never wrote to must not be rewritten at all"
        );
    }

    #[test]
    fn strip_handles_the_fenced_post_checkout_section() {
        let installed = format!(
            "#!/bin/sh\n{}\n{}",
            chain_repo_hook_snippet("post-checkout"),
            post_checkout_snippet("tokensave")
        );
        // The fenced body contains blank-line-free shell with its own `fi`, so
        // this is the case the end marker exists for.
        let stripped = strip_tokensave_sections(&installed).unwrap();
        assert!(is_inert_hook(&stripped), "got: {stripped:?}");
        assert!(!stripped.contains("end auto-init"));
    }

    #[test]
    fn strip_handles_the_unfenced_post_checkout_bodies_from_6_4_3_and_7_3_0() {
        // Those shapes end in a bare `fi` with no end marker, so the blank-line
        // boundary is what terminates the section.
        let legacy = "#!/bin/sh\n# tokensave: auto-init\nif [ \"$1\" = \"000\" ]; then\n\ttokensave init &\nfi\n\necho \"mine\"\n";
        let stripped = strip_tokensave_sections(legacy).unwrap();
        assert!(stripped.contains("echo \"mine\""));
        assert!(!stripped.contains("tokensave init"));
    }

    #[test]
    fn unset_hookspath_removes_only_our_entry_and_the_emptied_core_header() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("gitconfig");
        let hooks = Path::new("/home/u/.config/git/hooks");
        std::fs::write(
            &cfg,
            "[core]\n\thooksPath = /home/u/.config/git/hooks\n[user]\n\tname = T\n",
        )
        .unwrap();
        assert!(unset_global_hooks_path(&cfg, hooks));
        let after = std::fs::read_to_string(&cfg).unwrap();
        assert!(!after.contains("hooksPath"));
        assert!(
            !after.contains("[core]"),
            "emptied [core] should go: {after:?}"
        );
        assert!(
            after.contains("[user]"),
            "other sections must survive: {after:?}"
        );
    }

    #[test]
    fn unset_hookspath_keeps_a_core_section_that_still_has_settings() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("gitconfig");
        std::fs::write(
            &cfg,
            "[core]\n\thooksPath = /home/u/.config/git/hooks\n\teditor = vim\n",
        )
        .unwrap();
        assert!(unset_global_hooks_path(
            &cfg,
            Path::new("/home/u/.config/git/hooks")
        ));
        let after = std::fs::read_to_string(&cfg).unwrap();
        assert!(after.contains("[core]"));
        assert!(after.contains("editor = vim"));
        assert!(!after.contains("hooksPath"));
    }

    #[test]
    fn unset_hookspath_leaves_a_path_the_user_repointed_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("gitconfig");
        let original = "[core]\n\thooksPath = /somewhere/else\n";
        std::fs::write(&cfg, original).unwrap();
        assert!(
            !unset_global_hooks_path(&cfg, Path::new("/home/u/.config/git/hooks")),
            "a value pointing somewhere else is not ours to remove"
        );
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), original);
    }

    #[test]
    fn hook_removal_reports_nothing_found_when_it_did_nothing() {
        assert!(HookRemoval::default().found_nothing());
        let did = HookRemoval {
            deleted: vec![PathBuf::from("post-commit")],
            ..Default::default()
        };
        assert!(!did.found_nothing());
    }
}
