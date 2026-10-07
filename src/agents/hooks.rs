// ---------------------------------------------------------------------------
// Git post-commit hook
// ---------------------------------------------------------------------------

use crate::agents::expand_tilde;
use crate::agents::home_dir;
use clap::ValueEnum;
use std::path::{Path, PathBuf};

/// Whether `tokensave install` should install the git
/// `post-commit`/`post-checkout`/`post-merge` hooks, where to put them, and
/// whether to ask first.
///
/// `Yes` and `Default` install into the **current repository** since #506.
/// They used to claim a global `core.hooksPath`, which is a single
/// machine-wide slot: taking it takes it from every other tool that installs
/// hooks, and `git lfs install --local` then fails with exit 2 in every
/// repository on the machine. `init` has always installed per repository for
/// the same reason. `Global` keeps the old behavior for anyone who wants one
/// hook directory for every repo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GitHookMode {
    /// Prompt on a TTY, silently skip otherwise. Installs into the current
    /// repository when accepted.
    Default,
    /// Install into the current repository without asking, even on a TTY.
    Yes,
    /// Skip the hook install entirely, without asking.
    No,
    /// Install the global hooks, claiming `core.hooksPath` machine-wide.
    Global,
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

/// Marker comment closing tokensave's section in the post-commit hook (#624).
///
/// The section used to be the marker plus one command line with nothing after
/// it, so it could be appended but never found again with confidence, and a
/// moved binary left the line pointing at a path that no longer existed. The
/// end marker makes it a fence like post-checkout's, so it is rewritten in
/// place. The unfenced shape is still recognised by
/// [`replace_legacy_sync_line`] and migrated.
const HOOK_MARKER_END: &str = "# tokensave: end auto-sync";

/// Marker comment closing tokensave's section in the post-merge hook (#624).
/// See [`HOOK_MARKER_END`].
const HOOK_MARKER_MERGE_END: &str = "# tokensave: end auto-sync (post-merge)";

/// Marker comment closing tokensave's section in the post-checkout hook.
///
/// Written since #391 so that a migration can replace the section body in
/// place instead of having to pattern-match the shapes that shipped in 6.4.3
/// and 7.3.0 (both of which end in a bare `fi`). The ownership-aware
/// migration policy from #342 Q1 rewrites only tokensave's fenced section.
const HOOK_MARKER_CHECKOUT_END: &str = "# tokensave: end auto-init";

/// Marker comment identifying the repo-hook chaining preamble (issue #164).
const HOOK_MARKER_CHAIN: &str = "# tokensave: chain-repo-hook";

/// Version stamp carried on the post-checkout fence's opening line (#342 Q1).
///
/// Bumped whenever [`post_checkout_snippet`]'s body changes, so an install
/// carrying an older stamp is recognised as stale and rewritten in place
/// rather than left running the shape it was installed with. v2 replaced the
/// inline `$1`/`$3` branching with a single delegating line — see
/// [`post_checkout_snippet`]. v3 shell-quotes the binary path (#636).
const HOOK_CHECKOUT_VERSION: u32 = 3;

/// Outcome of writing tokensave's block into a hook file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookWrite {
    /// No tokensave block was present; one was added.
    Installed,
    /// A tokensave block was present with different content; it was replaced
    /// in place and everything outside the fence preserved byte-for-byte.
    Migrated,
    /// The block already matches; the file was not touched.
    UpToDate,
    Failed,
}

/// Replaces the fenced tokensave block in `contents` with `block`.
///
/// The fence is the first line starting with `begin_prefix` through the first
/// subsequent line starting with `end_marker`. Everything outside that span is
/// preserved byte-for-byte — that is the whole point of the fence, since the
/// file may carry a user's own hook code that tokensave has no claim on.
///
/// Returns `None` when no complete fence is present (the caller appends
/// instead), and `Some(unchanged)` when the block already matches, which the
/// caller turns into a no-op rather than a rewrite.
fn replace_fenced_block(
    contents: &str,
    begin_prefix: &str,
    end_marker: &str,
    block: &str,
) -> Option<String> {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start().starts_with(begin_prefix))?;
    let end = lines
        .iter()
        .skip(start)
        .position(|l| l.trim_start().starts_with(end_marker))
        .map(|offset| start + offset)?;

    let mut out = String::with_capacity(contents.len() + block.len());
    for line in &lines[..start] {
        out.push_str(line);
    }
    out.push_str(block);
    // The block always ends in a newline; if the closing marker line did not
    // (end of file, no trailing newline), do not invent one beyond the block.
    for line in &lines[end + 1..] {
        out.push_str(line);
    }
    Some(out)
}

/// Replaces an unfenced legacy post-checkout block in `contents` with `block`.
///
/// 6.4.3 and 7.3.0 wrote the block with no closing marker, so
/// [`replace_fenced_block`] declines it and the caller used to append a second
/// block beside it (#580). Both blocks then ran on every checkout, and the
/// legacy one called `tokensave init` directly, bypassing everything
/// [`crate::commands::hook_post_checkout`] decides (the #569 temp-dir skip
/// included).
///
/// The extent is not a guess: both shipped shapes are the bare marker line, an
/// `if` on the fresh-checkout sentinel, and a single closing `fi`. Anything
/// that departs from that shape before the `fi` (a blank line, another
/// tokensave marker, a nested `if`) is declined, as is a block whose `fi` is
/// followed by the end marker, which is a fenced block and belongs to
/// [`replace_fenced_block`].
///
/// An install that already went through the append path carries a complete
/// fenced block further down as well. That duplicate is removed along with the
/// blank separator [`write_global_hook`] put in front of it, so the file ends
/// up with one block. Everything else is preserved byte-for-byte.
fn replace_legacy_checkout_block(
    contents: &str,
    begin_marker: &str,
    end_marker: &str,
    block: &str,
) -> Option<String> {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let start = lines.iter().position(|l| l.trim() == begin_marker)?;
    let guard = format!("if [ \"$1\" = \"{FRESH_CHECKOUT_SENTINEL}\" ]");
    if !lines.get(start + 1)?.trim_start().starts_with(&guard) {
        return None;
    }
    let mut fi = None;
    for (i, line) in lines.iter().enumerate().skip(start + 2) {
        let trimmed = line.trim();
        if trimmed == "fi" {
            fi = Some(i);
            break;
        }
        if trimmed.is_empty() || is_tokensave_marker(line) || trimmed.starts_with("if ") {
            return None;
        }
    }
    let fi = fi?;
    if lines
        .get(fi + 1)
        .is_some_and(|l| l.trim_start().starts_with(end_marker))
    {
        return None;
    }

    let rest = &lines[fi + 1..];
    let duplicate = rest
        .iter()
        .position(|l| l.trim_start().starts_with(begin_marker))
        .and_then(|dup_start| {
            rest.iter()
                .skip(dup_start)
                .position(|l| l.trim_start().starts_with(end_marker))
                .map(|offset| {
                    let separator = dup_start > 0 && rest[dup_start - 1].trim().is_empty();
                    (dup_start - usize::from(separator))..=(dup_start + offset)
                })
        });

    let mut out = String::with_capacity(contents.len() + block.len());
    for line in &lines[..start] {
        out.push_str(line);
    }
    out.push_str(block);
    for (i, line) in rest.iter().enumerate() {
        if !duplicate.as_ref().is_some_and(|d| d.contains(&i)) {
            out.push_str(line);
        }
    }
    Some(out)
}

/// Replaces an unfenced post-commit/post-merge section with `block` (#624).
///
/// Every release before the fence wrote the marker line and one command line,
/// `<bin> sync >/dev/null 2>&1 &`, with nothing closing it. The extent is not
/// a guess: the marker must be a whole line, the next line must be that
/// command ([`is_legacy_sync_command`]), and a section already followed by the
/// end marker is fenced and belongs to [`replace_fenced_block`]. Everything
/// else is preserved byte-for-byte.
fn replace_legacy_sync_line(
    contents: &str,
    begin_marker: &str,
    end_marker: &str,
    block: &str,
) -> Option<String> {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let start = lines.iter().position(|l| l.trim() == begin_marker)?;
    if !is_legacy_sync_command(lines.get(start + 1)?) {
        return None;
    }
    if lines
        .get(start + 2)
        .is_some_and(|l| l.trim_start().starts_with(end_marker))
    {
        return None;
    }
    let mut out = String::with_capacity(contents.len() + block.len());
    for line in &lines[..start] {
        out.push_str(line);
    }
    out.push_str(block);
    for line in &lines[start + 2..] {
        out.push_str(line);
    }
    Some(out)
}

/// Writes `contents` to `path` atomically, preserving the existing mode.
///
/// A hook is executed by git, so a partially written file is a broken hook
/// rather than a corrupted data file: the write goes to a sibling temp file
/// and is renamed into place, which is atomic within a directory. The mode is
/// copied from the original so a rewrite cannot silently drop the executable
/// bit that makes the hook run at all.
fn write_file_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::Builder::new()
        .prefix(".tokensave-hook-")
        .tempfile_in(dir)?;
    {
        use std::io::Write;
        tmp.write_all(contents.as_bytes())?;
        tmp.flush()?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path).map_or(0o755, |m| m.permissions().mode());
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode))?;
    }

    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// `contents` with tokensave's section replaced by `block`, or `None` when no
/// section of a recognised shape is present.
fn rewrite_section(
    contents: &str,
    begin_prefix: &str,
    end_marker: &str,
    block: &str,
) -> Option<String> {
    // Legacy first (#580): in a file that holds both an unfenced legacy block
    // and an appended fenced one, the fenced search would pair the legacy
    // begin marker with the later end marker and replace everything between.
    replace_legacy_checkout_block(contents, begin_prefix, end_marker, block)
        .or_else(|| replace_legacy_sync_line(contents, begin_prefix, end_marker, block))
        .or_else(|| replace_fenced_block(contents, begin_prefix, end_marker, block))
}

/// Rewrites tokensave's section of a hook file in place, never appending.
///
/// The refresh path (#624): a file with no section of a recognised shape is
/// left untouched and reported [`HookWrite::UpToDate`], since adding a second
/// block beside one tokensave cannot delimit would run both.
fn migrate_block(hook_path: &Path, begin_prefix: &str, end_marker: &str, block: &str) -> HookWrite {
    let Ok(existing) = std::fs::read_to_string(hook_path) else {
        return HookWrite::UpToDate;
    };
    match rewrite_section(&existing, begin_prefix, end_marker, block) {
        Some(updated) if updated != existing => match write_file_atomically(hook_path, &updated) {
            Ok(()) => HookWrite::Migrated,
            Err(e) => {
                eprintln!(
                    "  \x1b[31m✘\x1b[0m Failed to update {}: {e}",
                    hook_path.display()
                );
                HookWrite::Failed
            }
        },
        _ => HookWrite::UpToDate,
    }
}

/// Installs a sync section (post-commit, post-merge) or rewrites the one
/// already there (#624).
///
/// Before the fence these hooks were skipped whenever tokensave's marker was
/// present, so the line was never updated. A present section is now rewritten
/// in place; one tokensave cannot delimit is still left alone rather than
/// gaining a duplicate.
fn ensure_sync_section(
    hook_path: &Path,
    begin_prefix: &str,
    end_marker: &str,
    block: &str,
) -> HookWrite {
    if std::fs::read_to_string(hook_path).is_ok_and(|c| has_section(&c, begin_prefix)) {
        migrate_block(hook_path, begin_prefix, end_marker, block)
    } else if write_global_hook(hook_path, block) {
        HookWrite::Installed
    } else {
        HookWrite::Failed
    }
}

/// Installs or migrates tokensave's fenced block in a hook file.
///
/// This is the (A) branch of #342 Q1: an ownership-aware in-place rewrite.
/// tokensave replaces only the region between its own markers, so it can now
/// *update* its block on upgrade — and, just as importantly, could remove it
/// cleanly, which an append-only writer never could for a hook that also
/// carries a user's code.
fn install_or_migrate_block(
    hook_path: &Path,
    begin_prefix: &str,
    end_marker: &str,
    block: &str,
) -> HookWrite {
    let Ok(existing) = std::fs::read_to_string(hook_path) else {
        return if write_global_hook(hook_path, block) {
            HookWrite::Installed
        } else {
            HookWrite::Failed
        };
    };

    match rewrite_section(&existing, begin_prefix, end_marker, block) {
        Some(updated) if updated == existing => HookWrite::UpToDate,
        Some(updated) => match write_file_atomically(hook_path, &updated) {
            Ok(()) => HookWrite::Migrated,
            Err(e) => {
                eprintln!(
                    "  \x1b[31m✘\x1b[0m Failed to update {}: {e}",
                    hook_path.display()
                );
                HookWrite::Failed
            }
        },
        // No complete fence and no recognisable legacy block: either no
        // tokensave block at all, or an unterminated one in a shape that
        // never shipped. Append, which leaves it alone rather than guessing
        // its extent.
        None => {
            if write_global_hook(hook_path, block) {
                HookWrite::Installed
            } else {
                HookWrite::Failed
            }
        }
    }
}

/// Preamble that forwards a global hook to the repository's own hook.
///
/// A global `core.hooksPath` makes git ignore every repository's
/// `.git/hooks/` — including hooks copied there by `init.templateDir` —
/// so a tokensave-owned global hook must delegate to the repo's hook or
/// pre-existing user hooks silently stop running (issue #164). Uses
/// `git rev-parse --git-common-dir` (not `--git-path hooks`, which resolves
/// through `core.hooksPath` and would re-enter this very script). The common
/// directory is required for linked worktrees, where `--git-dir` points at
/// `.git/worktrees/<name>` rather than the repository's hooks directory.
fn chain_repo_hook_snippet(hook_name: &str) -> String {
    format!(
        "{HOOK_MARKER_CHAIN}\n\
         repo_hook=\"$(git rev-parse --git-common-dir 2>/dev/null)/hooks/{hook_name}\"\n\
         if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
         \t\"$repo_hook\" \"$@\"\n\
         fi\n"
    )
}

/// Replace an old chain preamble in an already-installed hook.
///
/// Global hooks are intentionally not overwritten wholesale, so changing the
/// generated snippet alone would leave existing installations on the buggy
/// `--git-dir` path forever. Preserve all unrelated hook content and make a
/// second migration a no-op.
fn migrate_chain_repo_hook(contents: &str, hook_name: &str) -> Option<String> {
    if !contents.contains(HOOK_MARKER_CHAIN) {
        return None;
    }
    let marker_start = contents.find(HOOK_MARKER_CHAIN)?;
    let section = &contents[marker_start..];
    let mut section_end = 0;
    let mut found_fi = false;
    for line in section.split_inclusive('\n') {
        section_end += line.len();
        if line.trim() == "fi" {
            found_fi = true;
            break;
        }
    }
    if !found_fi || !section[..section_end].contains("git rev-parse --git-dir") {
        return None;
    }
    let replacement_body = chain_repo_hook_snippet(hook_name);
    Some(format!(
        "{}{}{}",
        &contents[..marker_start],
        replacement_body,
        &section[section_end..],
    ))
}

/// Which global hook files [`migrate_global_chain_hooks`] rewrote, and which
/// it could not.
#[derive(Debug, Default)]
struct ChainMigration {
    migrated: Vec<&'static str>,
    failed: Vec<&'static str>,
}

fn migrate_global_chain_hooks(hooks_dir: &Path) -> ChainMigration {
    let owned = ["post-commit", "post-checkout", "post-merge"];
    let names = owned.iter().copied().chain(
        FORWARDED_REPO_HOOKS
            .iter()
            .copied()
            .filter(|name| !owned.contains(name)),
    );
    let mut result = ChainMigration::default();
    for name in names {
        let path = hooks_dir.join(name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(migrated) = migrate_chain_repo_hook(&contents, name) else {
            continue;
        };
        // Atomic, not an in-place truncate: after an upgrade the silent
        // resync can run from a `tokensave sync` this very hook launched,
        // and `sh` reads a script as it executes it (#624).
        if write_file_atomically(&path, &migrated).is_err() {
            result.failed.push(name);
        } else {
            result.migrated.push(name);
        }
    }
    result
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

/// Quotes `value` as a single POSIX shell word.
///
/// The binary path is interpolated into `sh` hook scripts, so a space, `$`,
/// backtick or `;` in the install path must not split the word or run a
/// command (#636). Single quotes disable every expansion; an embedded `'`
/// closes the quote, emits an escaped `\'`, and reopens it.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// The hook snippet appended to (or written as) the post-commit script.
///
/// Fenced by [`HOOK_MARKER`] and [`HOOK_MARKER_END`] since #624, so a changed
/// binary path is rewritten in place.
fn post_commit_snippet(tokensave_bin: &str) -> String {
    let bin = shell_quote(&tokensave_bin.replace('\\', "/"));
    format!(
        "{HOOK_MARKER}\n\
         {bin} sync >/dev/null 2>&1 &\n\
         {HOOK_MARKER_END}\n"
    )
}

/// The hook snippet appended to (or written as) the post-merge script.
///
/// Fenced like [`post_commit_snippet`].
fn post_merge_snippet(tokensave_bin: &str) -> String {
    let bin = shell_quote(&tokensave_bin.replace('\\', "/"));
    format!(
        "{HOOK_MARKER_MERGE}\n\
         {bin} sync >/dev/null 2>&1 &\n\
         {HOOK_MARKER_MERGE_END}\n"
    )
}

/// Is `line` the command an unfenced post-commit/post-merge section carried?
///
/// Every release before #624 wrote `<bin> sync >/dev/null 2>&1 &`; the bare
/// `sync` and `sync &` forms are accepted too so a hand-trimmed copy of the
/// line is still recognised. It is only consulted for the line directly after
/// tokensave's own marker.
fn is_legacy_sync_command(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && !t.starts_with('#')
        && (t.ends_with(" sync >/dev/null 2>&1 &")
            || t.ends_with(" sync &")
            || t.ends_with(" sync"))
}

/// The hook files tokensave writes a section into, with each section's
/// opening marker prefix, closing marker, and current body.
fn owned_hook_sections(
    tokensave_bin: &str,
) -> [(&'static str, &'static str, &'static str, String); 3] {
    [
        (
            "post-checkout",
            HOOK_MARKER_CHECKOUT,
            HOOK_MARKER_CHECKOUT_END,
            post_checkout_snippet(tokensave_bin),
        ),
        (
            "post-commit",
            HOOK_MARKER,
            HOOK_MARKER_END,
            post_commit_snippet(tokensave_bin),
        ),
        (
            "post-merge",
            HOOK_MARKER_MERGE,
            HOOK_MARKER_MERGE_END,
            post_merge_snippet(tokensave_bin),
        ),
    ]
}

/// Does `contents` carry a section opened by `begin_prefix`?
fn has_section(contents: &str, begin_prefix: &str) -> bool {
    contents
        .lines()
        .any(|l| l.trim_start().starts_with(begin_prefix))
}

/// Rewrites each tokensave section already present in `hooks_dir` to the
/// current shape, and installs nothing (#624).
///
/// Returns the hook names rewritten and the hook names whose rewrite failed.
fn migrate_present_sections(
    hooks_dir: &Path,
    tokensave_bin: &str,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut migrated = Vec::new();
    let mut failed = Vec::new();
    for (name, begin, end, block) in owned_hook_sections(tokensave_bin) {
        let path = hooks_dir.join(name);
        if !std::fs::read_to_string(&path).is_ok_and(|c| has_section(&c, begin)) {
            continue;
        }
        match migrate_block(&path, begin, end, &block) {
            HookWrite::Migrated => migrated.push(name),
            HookWrite::Failed => failed.push(name),
            HookWrite::Installed | HookWrite::UpToDate => {}
        }
    }
    (migrated, failed)
}

/// What a `post-checkout` event should trigger.
///
/// Kept as a pure decision, separate from the effects in
/// [`crate::commands::hook_post_checkout`], because this is exactly where #391
/// went wrong: two correct-looking commands sat in mutually exclusive arms, so
/// a fresh worktree was indexed but never had its branch tracked. That is a
/// property of the *decision*, and it is only cheap to test if the decision
/// has no side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckoutAction {
    /// A fresh clone or new worktree: index, then track the branch — in that
    /// order, because tracking copies the index `init` creates.
    InitThenTrack,
    /// An ordinary branch switch: track without re-indexing.
    TrackOnly,
    /// A file checkout: do nothing.
    Nothing,
}

/// git's all-zeros previous HEAD, marking the initial checkout of a fresh
/// clone or of a newly added worktree.
pub const FRESH_CHECKOUT_SENTINEL: &str = "0000000000000000000000000000000000000000";

/// Classifies a `post-checkout` event from git's `$1` and `$3`.
///
/// The fresh-checkout sentinel wins over the branch flag: such a checkout is
/// *also* a branch checkout, and it can land on a branch that is not the
/// default one (`git clone -b feature`, `git worktree add -b feature`), so it
/// needs both actions rather than either one.
pub fn classify_checkout(prev_head: Option<&str>, branch_flag: Option<&str>) -> CheckoutAction {
    if prev_head == Some(FRESH_CHECKOUT_SENTINEL) {
        return CheckoutAction::InitThenTrack;
    }
    if branch_flag == Some("1") {
        return CheckoutAction::TrackOnly;
    }
    CheckoutAction::Nothing
}

/// Whether `path` sits under `system_temp`.
///
/// Under a `Global` [`GitHookMode`] install, `core.hooksPath` applies to
/// every `git clone`/`checkout` on the machine, including ones done by
/// unrelated tools that stage work in a throwaway system temp directory and
/// expect to own it exclusively until they delete or overwrite it — `CocoaPods`
/// fetching a git-sourced pod is one example (#569). A backgrounded
/// `tokensave init` racing that kind of cleanup can lose a file mid-copy out
/// from under the other tool, e.g. `rsync --delete` failing with `No such
/// file or directory` on `.tokensave/tokensave.db-wal`.
///
/// Kept as a pure prefix check, separate from resolving the two paths to
/// compare, so it is cheap to test without touching the filesystem. Callers
/// must canonicalize both arguments first: on macOS `/tmp` is a symlink to
/// `/private/tmp`, and comparing the un-resolved forms would silently never
/// match.
pub fn is_under_temp_dir(path: &Path, system_temp: &Path) -> bool {
    path.starts_with(system_temp)
}

/// The hook snippet written into the post-checkout script.
///
/// **v2 delegates instead of branching in shell.** The block is one line that
/// hands git's arguments to the binary:
///
/// ```sh
/// # tokensave: auto-init (v3)
/// '/path/to/tokensave' hook post-checkout "$@" >/dev/null 2>&1 &
/// # tokensave: end auto-init
/// ```
///
/// v3 only adds the single-quoting of the binary path ([`shell_quote`]).
///
/// The `$1`-is-all-zeros / `$3 == 1` branching that used to live here now
/// lives in [`crate::commands::hook_post_checkout`]. That means one in-place
/// rewrite of the file in a project's lifetime: every later change to hook
/// *behaviour* ships with the binary and needs no file surgery at all. It also
/// makes the behaviour unit-testable in Rust rather than in shell.
///
/// Backgrounding stays in the shell (`&`) so git is never blocked waiting on
/// tokensave, which is the one property that has to survive the move.
///
/// The section is fenced by [`HOOK_MARKER_CHECKOUT`] and
/// [`HOOK_MARKER_CHECKOUT_END`], and the opening line carries
/// [`HOOK_CHECKOUT_VERSION`]. Since #342 Q1 an install carrying an older body
/// is rewritten in place on install/reinstall, preserving anything outside the
/// fence byte-for-byte.
fn post_checkout_snippet(tokensave_bin: &str) -> String {
    let bin = shell_quote(&tokensave_bin.replace('\\', "/"));
    format!(
        "{HOOK_MARKER_CHECKOUT} (v{HOOK_CHECKOUT_VERSION})\n\
         {bin} hook post-checkout \"$@\" >/dev/null 2>&1 &\n\
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
        // `--git-hook global` is an explicit request for the machine-wide
        // slot, so it does not prompt.
        GitHookMode::Global => HookAction::Install,
        // Since #506 `Yes` and `Default` install into the current repository
        // instead, handled by the caller before it reaches this path.
        GitHookMode::Default | GitHookMode::No | GitHookMode::Yes => HookAction::Skip,
    }
}

/// A warning for a user whose machine still has tokensave's global
/// `core.hooksPath` claimed, or `None` when it does not.
///
/// `core.hooksPath` is a single machine-wide slot. Taking it takes it from
/// every other tool that installs hooks: `git lfs install --local` finds
/// tokensave's `pre-push` forwarder sitting there and fails with exit 2 in
/// **every** repository on the machine, including a brand-new empty one with
/// no LFS hook anywhere near it (#506). husky, pre-commit and lefthook
/// collide the same way.
///
/// This only reports. Unsetting `core.hooksPath` here would stop the user's
/// syncs running with no warning, and the global setup is a legitimate choice
/// for anyone who wants one hook directory for every repo — so the remedy is
/// named and left to them.
pub fn global_hookspath_conflict_warning() -> Option<String> {
    if !global_git_hooks_installed() {
        return None;
    }
    let home = home_dir()?;
    // Only warn when tokensave is the one claiming the slot. A hooks dir the
    // user points at themselves is theirs to manage.
    read_global_hooks_path(&home)?;
    Some(
        "  \x1b[33m⚠\x1b[0m tokensave holds the global `core.hooksPath`, which is a single \
         machine-wide slot.\n     Other hook installers fail while it is taken — \
         `git lfs install --local` exits 2 in every repository.\n     To switch this repo to \
         its own hooks: `tokensave githooks on --local`\n     Then release the slot: \
         `tokensave githooks off` and `git config --global --unset core.hooksPath`"
            .to_string(),
    )
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
        HookAction::AlreadyInstalled => false,
        HookAction::Skip => {
            // Mode `No` (or default-mode non-TTY). Stay quiet — script
            // callers asked for no output here.
            return Ok(());
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

    // Migrate existing tokensave-owned chain preambles before writing any
    // other hook sections. This repairs linked-worktree forwarding without
    // touching unrelated user content.
    let mut failed: Vec<&str> = migrate_global_chain_hooks(&hooks_dir).failed;

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
    //
    // An existing section is rewritten in place rather than skipped (#624), so
    // a moved binary or an unfenced pre-#624 line is brought up to date.
    let commit_snippet = post_commit_snippet(tokensave_bin);
    let commit_write = if install_post_commit {
        ensure_sync_section(&hook_path, HOOK_MARKER, HOOK_MARKER_END, &commit_snippet)
    } else {
        migrate_block(&hook_path, HOOK_MARKER, HOOK_MARKER_END, &commit_snippet)
    };
    match commit_write {
        HookWrite::Installed => eprintln!(
            "\x1b[32m✔\x1b[0m Installed global git post-commit hook at {}",
            hook_path.display()
        ),
        HookWrite::Migrated => eprintln!(
            "\x1b[32m✔\x1b[0m Updated global git post-commit hook at {}",
            hook_path.display()
        ),
        HookWrite::UpToDate => {
            eprintln!("  Global git post-commit hook already contains tokensave, skipping");
        }
        HookWrite::Failed => failed.push("post-commit"),
    }

    // Install the post-checkout hook so a fresh clone or worktree
    // auto-initializes and tracks its branch. Its marker is independent of
    // post-commit's, so this is skipped only when the post-checkout hook
    // itself is already present. The fenced migration below updates existing
    // installs without touching unrelated hook content (#342 Q1).
    let checkout_path = hooks_dir.join("post-checkout");
    let checkout_contents = std::fs::read_to_string(&checkout_path).ok();
    if should_chain_repo_hooks(
        need_set_hookspath,
        hooks_dir_is_default,
        checkout_contents.as_deref(),
    ) {
        write_global_hook(&checkout_path, &chain_repo_hook_snippet("post-checkout"));
    }
    // #342 Q1: rewrite our own fenced block in place when it is stale, rather
    // than skipping because *some* tokensave block is present. A version bump
    // in the snippet now reaches installs that already have the hook.
    let checkout_write = install_or_migrate_block(
        &checkout_path,
        HOOK_MARKER_CHECKOUT,
        HOOK_MARKER_CHECKOUT_END,
        &post_checkout_snippet(tokensave_bin),
    );
    match checkout_write {
        HookWrite::Installed => eprintln!(
            "\x1b[32m✔\x1b[0m Installed global git post-checkout hook at {}",
            checkout_path.display()
        ),
        HookWrite::Migrated => eprintln!(
            "\x1b[32m✔\x1b[0m Updated global git post-checkout hook at {} (v{HOOK_CHECKOUT_VERSION})",
            checkout_path.display()
        ),
        HookWrite::UpToDate => {}
        HookWrite::Failed => failed.push("post-checkout"),
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
    let merge_snippet = post_merge_snippet(tokensave_bin);
    let merge_write = if merge_contents.is_some_and(|c| c.contains(HOOK_MARKER_MERGE)) {
        migrate_block(
            &merge_path,
            HOOK_MARKER_MERGE,
            HOOK_MARKER_MERGE_END,
            &merge_snippet,
        )
    } else if write_global_hook(&merge_path, &merge_snippet) {
        HookWrite::Installed
    } else {
        HookWrite::Failed
    };
    match merge_write {
        HookWrite::Installed => eprintln!(
            "\x1b[32m✔\x1b[0m Installed global git post-merge hook at {}",
            merge_path.display()
        ),
        HookWrite::Migrated => eprintln!(
            "\x1b[32m✔\x1b[0m Updated global git post-merge hook at {}",
            merge_path.display()
        ),
        HookWrite::UpToDate => {}
        HookWrite::Failed => failed.push("post-merge"),
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
    /// Hooks whose tokensave block was stale and was rewritten in place,
    /// preserving anything outside the fence (#342 Q1). Reported rather than
    /// silent: the rewrite is automatic, but a file in the user's repository
    /// changed and they are entitled to be told.
    pub migrated: Vec<String>,
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

/// Rewrites a stale tokensave block in this repository's hooks, without
/// installing anything that is not already there (#342 Q1).
///
/// Called on the path where hooks are already present, which previously
/// returned early and so never updated the block it had written. Migration
/// needs no prompt: the region between tokensave's markers is tokensave's
/// own, and everything outside it is preserved byte-for-byte. Installing a
/// hook that is *absent* still asks, because that is a file the user has not
/// agreed to yet.
///
/// Since #624 this covers post-commit and post-merge as well as post-checkout,
/// so a moved binary is followed in every hook.
///
/// Returns the hook names that were rewritten, for the caller to report.
pub fn migrate_local_hook_blocks(repo: &Path, tokensave_bin: &str) -> Vec<String> {
    let Some(hooks_dir) = repo_hooks_dir(repo) else {
        return Vec::new();
    };
    migrate_present_sections(&hooks_dir, tokensave_bin)
        .0
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// What [`refresh_installed_git_hooks`] rewrote, so the caller can report it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HookRefresh {
    /// Hook files whose tokensave section was rewritten in place.
    pub updated: Vec<PathBuf>,
    /// Hook files whose rewrite was attempted and failed. The reason was
    /// printed at the point of failure.
    pub failed: Vec<PathBuf>,
}

impl HookRefresh {
    /// Adds the outcome of [`migrate_present_sections`] in `dir`, listing a
    /// file once however many of its sections changed.
    fn record(&mut self, dir: &Path, (migrated, failed): (Vec<&str>, Vec<&str>)) {
        for name in migrated {
            let path = dir.join(name);
            if !self.updated.contains(&path) {
                self.updated.push(path);
            }
        }
        for name in failed {
            let path = dir.join(name);
            if !self.failed.contains(&path) {
                self.failed.push(path);
            }
        }
    }
}

/// Brings tokensave's sections of already-installed git hooks up to the shape
/// this binary writes, for `tokensave reinstall` and the silent resync after
/// an upgrade (#624). That includes the binary path every section names, so a
/// moved binary is followed.
///
/// Covers the global hook directory, when tokensave's global hooks are
/// installed, and `repo`'s own hook directory, when tokensave's local hooks
/// are installed there. It is a refresh, not an install: a hook that does not
/// already carry tokensave's marker is left alone, no new hook file is
/// created, and `core.hooksPath` is never set. A user who never opted into
/// hooks gets none from a reinstall.
///
/// As with [`migrate_local_hook_blocks`], no prompt is needed (#342 Q1): only
/// tokensave's own sections are rewritten, and everything outside them is
/// preserved byte-for-byte.
pub fn refresh_installed_git_hooks(repo: &Path, tokensave_bin: &str) -> HookRefresh {
    let mut result = HookRefresh::default();

    if let Some(home) = home_dir().filter(|_| global_git_hooks_installed()) {
        let hooks_dir = read_global_hooks_path(&home)
            .unwrap_or_else(|| home.join(".config").join("git").join("hooks"));

        let chain = migrate_global_chain_hooks(&hooks_dir);
        result.record(&hooks_dir, (chain.migrated, chain.failed));
        result.record(
            &hooks_dir,
            migrate_present_sections(&hooks_dir, tokensave_bin),
        );
    }

    if let Some(local_dir) = repo_hooks_dir(repo) {
        result.record(
            &local_dir,
            migrate_present_sections(&local_dir, tokensave_bin),
        );
    }

    result
}

/// Hook files whose tokensave block is present but out of date (#342 Q1).
///
/// A read-only probe for `doctor`: the rewrite itself happens on
/// install/reinstall, and reporting it here means an automatic change to a
/// file in the user's repository is at least visible somewhere they can ask.
pub fn stale_hook_blocks(repo: &Path, tokensave_bin: &str) -> Vec<PathBuf> {
    let global_dir = home_dir().map(|home| {
        read_global_hooks_path(&home)
            .unwrap_or_else(|| home.join(".config").join("git").join("hooks"))
    });
    [repo_hooks_dir(repo), global_dir]
        .into_iter()
        .flatten()
        .flat_map(|dir| stale_sections_in(&dir, tokensave_bin))
        .collect()
}

/// The hooks in `hooks_dir` whose tokensave section a refresh would rewrite.
///
/// A post-checkout section is stale unless it is fenced and current. A
/// post-commit or post-merge section is stale when it is fenced and names a
/// different binary, or still has the unfenced pre-#624 shape; any other line
/// after the marker is not tokensave's to rewrite, so it is not reported.
fn stale_sections_in(hooks_dir: &Path, tokensave_bin: &str) -> Vec<PathBuf> {
    let mut stale = Vec::new();
    for (name, begin, end, want) in owned_hook_sections(tokensave_bin) {
        let path = hooks_dir.join(name);
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !contents.contains(begin) {
            continue;
        }
        let is_stale = match replace_fenced_block(&contents, begin, end, &want) {
            Some(updated) => updated != contents,
            None if name == "post-checkout" => true,
            None => replace_legacy_sync_line(&contents, begin, end, &want).is_some(),
        };
        if is_stale {
            stale.push(path);
        }
    }
    stale
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

    // post-checkout carries a versioned fence, so a stale block is rewritten
    // in place here too (#342 Q1) rather than skipped for carrying *some*
    // tokensave marker. Since #624 post-commit and post-merge are fenced too,
    // so an existing section of theirs is rewritten in place as well.
    for (name, begin, end, block) in owned_hook_sections(tokensave_bin) {
        let path = hooks_dir.join(name);
        let write = if name == "post-checkout" {
            install_or_migrate_block(&path, begin, end, &block)
        } else {
            ensure_sync_section(&path, begin, end, &block)
        };
        let bucket = match write {
            HookWrite::Installed => &mut result.installed,
            HookWrite::Migrated => &mut result.migrated,
            HookWrite::UpToDate => &mut result.already_present,
            HookWrite::Failed => &mut result.failed,
        };
        bucket.push(name.to_string());
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

/// True for a line that closes one of tokensave's fenced sections
/// ([`HOOK_MARKER_CHECKOUT_END`], [`HOOK_MARKER_END`],
/// [`HOOK_MARKER_MERGE_END`]).
fn is_tokensave_end_marker(line: &str) -> bool {
    line.trim_start().starts_with("# tokensave: end ")
}

/// Remove every tokensave-owned section from a hook script.
///
/// A section runs from its `# tokensave:` marker to the first blank line, to
/// the section's own end marker, or to end of file — whichever comes first.
/// That rule covers every shape that has shipped: the `post-commit` and
/// `post-merge` sections, a marker plus one command, unfenced before #624 and
/// fenced since; the fenced `post-checkout` section written since #391; and the unfenced
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
        // A stray end marker is dropped on its own; it opens nothing.
        if is_tokensave_end_marker(line) {
            continue;
        }
        // Consume the section body. An end marker is consumed with it and
        // closes the section, so a user's line directly after it survives; a
        // blank line is the boundary and is left for the blank-run collapse
        // below.
        while let Some(next) = lines.peek() {
            if next.trim().is_empty() {
                break;
            }
            if is_tokensave_end_marker(next) {
                lines.next();
                break;
            }
            if is_tokensave_marker(next) {
                break;
            }
            lines.next();
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod git_hook_tests {
    use super::*;
    use crate::agents::*;
    use std::path::Path;
    use std::process::Command;

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

    /// #506 changed what `yes` means. It used to install the *global* hooks,
    /// claiming `core.hooksPath` — a single machine-wide slot whose capture
    /// makes `git lfs install --local` fail with exit 2 in every repository on
    /// the machine. It now installs into the current repository, so this
    /// decision function (which governs the global path only) must decline it.
    #[test]
    fn decide_hook_action_yes_no_longer_claims_the_global_slot() {
        assert_eq!(decide_hook_action(GitHookMode::Yes, None), HookAction::Skip);

        let contents = "#!/bin/sh\necho hello\n";
        assert_eq!(
            decide_hook_action(GitHookMode::Yes, Some(contents)),
            HookAction::Skip
        );
    }

    /// `global` is the explicit opt-in that replaces it, and being explicit it
    /// installs rather than prompting.
    #[test]
    fn decide_hook_action_global_installs_without_prompting() {
        assert_eq!(
            decide_hook_action(GitHookMode::Global, None),
            HookAction::Install
        );

        let contents = "#!/bin/sh\necho hello\n";
        assert_eq!(
            decide_hook_action(GitHookMode::Global, Some(contents)),
            HookAction::Install
        );
    }

    /// An existing tokensave global install is still recognised as such
    /// whichever mode asks, so `--git-hook global` on a machine that already
    /// has it does not rewrite the hook.
    #[test]
    fn decide_hook_action_global_reports_already_installed() {
        let contents = format!("#!/bin/sh\n{HOOK_MARKER}\ntokensave sync\n");
        assert_eq!(
            decide_hook_action(GitHookMode::Global, Some(&contents)),
            HookAction::AlreadyInstalled
        );
    }

    /// `default` must never take the machine-wide slot on its own, on a TTY or
    /// off it — that was the path #506's reporter hit without asking for it.
    #[test]
    fn decide_hook_action_default_never_claims_the_global_slot() {
        assert_eq!(
            decide_hook_action(GitHookMode::Default, None),
            HookAction::Skip
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
            s.contains("'/usr/local/bin/tokensave' sync"),
            "must run `sync` with the resolved binary so a `git pull` reindexes teammates' changes, got: {s}"
        );
    }

    #[test]
    fn hook_snippets_quote_a_binary_path_containing_a_space() {
        // A binary installed under a path with a space (e.g. a user directory
        // named "Enzo Lombardi") must be quoted in the shell snippet; an
        // unquoted path would split into two words and silently disable the
        // hook. Regression for the unquoted `{bin}` interpolation.
        let bin = "/Users/Enzo Lombardi/bin/tokensave";
        let commit = post_commit_snippet(bin);
        let merge = post_merge_snippet(bin);
        let checkout = post_checkout_snippet(bin);
        assert!(
            commit.contains("'/Users/Enzo Lombardi/bin/tokensave' sync"),
            "post-commit snippet must quote the binary path, got: {commit}"
        );
        assert!(
            merge.contains("'/Users/Enzo Lombardi/bin/tokensave' sync"),
            "post-merge snippet must quote the binary path, got: {merge}"
        );
        assert!(
            checkout.contains("'/Users/Enzo Lombardi/bin/tokensave' hook post-checkout \"$@\""),
            "post-checkout snippet must quote the binary path, got: {checkout}"
        );
    }

    #[test]
    fn hook_snippets_neutralise_shell_metacharacters_in_the_binary_path() {
        // Double quotes would still expand `$(...)` and backticks; the path
        // must reach `sh` as one inert word, including an embedded `'` (#636).
        let bin = "/opt/my;tools/$(touch pwned)/`id`/it's/tokensave";
        let quoted = r"'/opt/my;tools/$(touch pwned)/`id`/it'\''s/tokensave'";
        assert_eq!(shell_quote(bin), quoted);
        assert!(post_commit_snippet(bin).contains(&format!("{quoted} sync")));
        assert!(post_merge_snippet(bin).contains(&format!("{quoted} sync")));
        assert!(post_checkout_snippet(bin).contains(&format!("{quoted} hook post-checkout")));
    }

    #[test]
    fn post_checkout_snippet_delegates_to_the_binary() {
        let s = post_checkout_snippet("/usr/local/bin/tokensave");
        assert!(
            s.contains(HOOK_MARKER_CHECKOUT),
            "must carry its idempotency marker, got: {s}"
        );
        assert!(
            s.contains("'/usr/local/bin/tokensave' hook post-checkout \"$@\""),
            "must forward git's arguments to the binary verbatim, got: {s}"
        );
        assert!(
            !s.contains(FRESH_CHECKOUT_SENTINEL) && !s.contains("elif"),
            "v2 carries no branching: that moved into the binary so behaviour \
             changes ship with it rather than needing another file rewrite, got: {s}"
        );
        assert!(
            s.trim_end().ends_with(HOOK_MARKER_CHECKOUT_END),
            "the section must be fenced so a migration can replace it in place, got: {s}"
        );
    }

    /// The #391 regression, now asserted against the decision rather than the
    /// shell. The bug was that two correct-looking commands sat in mutually
    /// exclusive arms, so a fresh worktree was indexed but never had its
    /// branch tracked — a property of the classification, which is why the
    /// classification is a pure function.
    #[test]
    fn a_fresh_worktree_or_clone_is_both_indexed_and_tracked() {
        const SHA: &str = "1111111111111111111111111111111111111111";

        // git passes the all-zeros previous HEAD *and* flag 1, and the branch
        // checked out need not be the default one.
        assert_eq!(
            classify_checkout(Some(FRESH_CHECKOUT_SENTINEL), Some("1")),
            CheckoutAction::InitThenTrack,
            "a fresh worktree/clone must be indexed AND have its branch tracked"
        );
        // The sentinel wins even if git reported it without the branch flag.
        assert_eq!(
            classify_checkout(Some(FRESH_CHECKOUT_SENTINEL), Some("0")),
            CheckoutAction::InitThenTrack
        );
        // An ordinary branch switch tracks without re-indexing.
        assert_eq!(
            classify_checkout(Some(SHA), Some("1")),
            CheckoutAction::TrackOnly,
            "a branch switch must not re-run init"
        );
        // A file checkout triggers nothing at all.
        assert_eq!(
            classify_checkout(Some(SHA), Some("0")),
            CheckoutAction::Nothing,
            "a file checkout must not run tokensave"
        );
        // Missing arguments must not be read as a branch checkout.
        assert_eq!(classify_checkout(None, None), CheckoutAction::Nothing);
    }

    /// #569: a clone/checkout under the system temp dir must be recognised so
    /// the hook can skip it — `CocoaPods` and similar tools stage throwaway
    /// clones there and race a backgrounded `tokensave init` against their
    /// own cleanup.
    #[test]
    fn temp_dir_paths_are_recognised_as_ephemeral() {
        let system_temp = Path::new("/private/var/folders/xy/abc123/T");

        assert!(is_under_temp_dir(
            &system_temp.join("d20260917-1234-abcdef"),
            system_temp
        ));
        assert!(
            is_under_temp_dir(system_temp, system_temp),
            "the temp root itself counts as ephemeral"
        );
    }

    #[test]
    fn real_project_paths_are_not_ephemeral() {
        let system_temp = Path::new("/private/var/folders/xy/abc123/T");

        assert!(!is_under_temp_dir(
            Path::new("/Users/dev/Documents/GitHub/my_pillsbox"),
            system_temp
        ));
        // A prefix collision on the directory name alone must not match —
        // only a real path-component prefix counts.
        assert!(!is_under_temp_dir(
            Path::new("/private/var/folders-backup/xy"),
            system_temp
        ));
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
    /// Executed rather than substring-matched, for the same reason the #391
    /// test was: what the shell still owns is passing git's arguments through
    /// untouched and backgrounding the call. A `"$@"` that loses or reorders
    /// an argument would make the binary's classification correct and the
    /// outcome wrong anyway.
    #[cfg(unix)]
    #[test]
    fn the_snippet_forwards_gits_arguments_verbatim() {
        const ZERO: &str = FRESH_CHECKOUT_SENTINEL;
        const SHA: &str = "1111111111111111111111111111111111111111";

        for args in [
            vec![ZERO, SHA, "1"],
            vec![SHA, SHA, "1"],
            vec![SHA, SHA, "0"],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let invoked = run_post_checkout_snippet(dir.path(), &args);
            assert_eq!(
                invoked,
                vec![format!("hook post-checkout {}", args.join(" "))],
                "the hook must hand git's arguments to the binary unchanged"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn global_forwarder_runs_repository_hook_from_linked_worktree() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::tempdir().unwrap();
        let run_git = |dir: &Path, args: &[&str], hooks_path: Option<&Path>| {
            let mut command = Command::new("git");
            command
                .args(args)
                .current_dir(dir)
                .env("HOME", repo.path())
                .env("XDG_CONFIG_HOME", repo.path().join(".config"));
            if let Some(hooks_path) = hooks_path {
                command
                    .env("GIT_CONFIG_COUNT", "1")
                    .env("GIT_CONFIG_KEY_0", "core.hooksPath")
                    .env("GIT_CONFIG_VALUE_0", hooks_path);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };

        run_git(repo.path(), &["init", "-q", "-b", "main"], None);
        run_git(
            repo.path(),
            &["config", "user.name", "TokenSave Test"],
            None,
        );
        run_git(
            repo.path(),
            &["config", "user.email", "tokensave@example.com"],
            None,
        );
        std::fs::write(repo.path().join("README"), "main\n").unwrap();
        run_git(repo.path(), &["add", "README"], None);
        run_git(repo.path(), &["commit", "-qm", "initial"], None);

        let sentinel = repo.path().join("hook-fired");
        let repo_hook = repo.path().join(".git/hooks/pre-commit");
        std::fs::write(
            &repo_hook,
            format!("#!/bin/sh\nprintf fired > '{}'\n", sentinel.display()),
        )
        .unwrap();
        std::fs::set_permissions(&repo_hook, std::fs::Permissions::from_mode(0o755)).unwrap();

        let global_hooks = tempfile::tempdir().unwrap();
        let global_hook = global_hooks.path().join("pre-commit");
        std::fs::write(
            &global_hook,
            format!("#!/bin/sh\n{}", chain_repo_hook_snippet("pre-commit")),
        )
        .unwrap();
        std::fs::set_permissions(&global_hook, std::fs::Permissions::from_mode(0o755)).unwrap();

        let worktree = repo.path().join("worktree");
        run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature",
                worktree.to_str().unwrap(),
            ],
            None,
        );
        std::fs::write(worktree.join("feature"), "feature\n").unwrap();
        run_git(&worktree, &["add", "feature"], Some(global_hooks.path()));
        run_git(
            &worktree,
            &["commit", "-qm", "feature"],
            Some(global_hooks.path()),
        );

        assert_eq!(
            std::fs::read_to_string(&sentinel).unwrap(),
            "fired",
            "the global forwarder must reach the shared repository hook from a linked worktree"
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
        assert!(created.contains("git rev-parse --git-common-dir"));
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
    fn chain_snippet_forwards_to_repo_hook_via_common_git_dir() {
        let s = chain_repo_hook_snippet("post-checkout");
        assert!(s.contains(HOOK_MARKER_CHAIN));
        // Must use --git-common-dir, not --git-dir: in a linked worktree,
        // --git-dir points at .git/worktrees/<name>, not .git/hooks.
        assert!(s.contains("git rev-parse --git-common-dir"));
        assert!(!s.contains("--git-dir"));
        assert!(!s.contains("--git-path"));
        assert!(s.contains("/hooks/post-checkout"));
        // Args must be forwarded (post-checkout receives old/new/flag).
        assert!(s.contains("\"$@\""));
    }

    #[test]
    fn migrate_chain_snippet_repairs_existing_worktree_forwarder() {
        let old = "#!/bin/sh\n\
                   # user hook\n\
                   # tokensave: chain-repo-hook\n\
                   repo_hook=\"$(git rev-parse --git-dir 2>/dev/null)/hooks/pre-push\"\n\
                   if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
                   \t\"$repo_hook\" \"$@\"\n\
                   fi\n\
                   # user hook after tokensave\n\
                   USER_GIT_DIR=\"$(git rev-parse --git-dir)\"\n\
                   # tokensave: auto-sync\n\
                   tokensave sync >/dev/null 2>&1 &\n";
        let migrated = migrate_chain_repo_hook(old, "pre-push").unwrap();
        assert!(migrated.contains("# user hook"));
        assert!(migrated.contains("# tokensave: auto-sync"));
        assert!(migrated.contains("git rev-parse --git-common-dir"));
        assert!(migrated.contains("USER_GIT_DIR=\"$(git rev-parse --git-dir)\""));
        assert_eq!(
            migrate_chain_repo_hook(&migrated, "pre-push"),
            None,
            "a second migration must be a no-op"
        );
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
        // process, so the old assertion allowed either outcome. Since #506
        // `default` never takes the machine-wide slot at all, on a TTY or off
        // it, so the answer no longer depends on the terminal.
        let action = decide_hook_action(GitHookMode::Default, None);
        assert_eq!(action, HookAction::Skip);
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod fence_tests {
    use super::*;

    const BEGIN: &str = "# tokensave: auto-init";
    const END: &str = "# tokensave: end auto-init";
    const NEW_BLOCK: &str = "# tokensave: auto-init (v2)\ntokensave hook post-checkout \"$@\" >/dev/null 2>&1 &\n# tokensave: end auto-init\n";

    /// The property the whole fence exists for: a user's own hook code, on
    /// both sides of our block, must survive a rewrite byte-for-byte.
    #[test]
    fn a_rewrite_preserves_everything_outside_the_fence() {
        let existing = "#!/bin/sh\n\
                        # my own pre-step\n\
                        ./scripts/notify.sh \"$@\"\n\
                        \n\
                        # tokensave: auto-init\n\
                        if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
                        \ttokensave init >/dev/null 2>&1 &\n\
                        fi\n\
                        # tokensave: end auto-init\n\
                        \n\
                        # my own post-step\n\
                        exec ./scripts/after.sh\n";

        let updated = replace_fenced_block(existing, BEGIN, END, NEW_BLOCK).unwrap();

        assert!(updated.starts_with("#!/bin/sh\n# my own pre-step\n./scripts/notify.sh \"$@\"\n\n"));
        assert!(updated.ends_with("\n# my own post-step\nexec ./scripts/after.sh\n"));
        assert!(updated.contains("hook post-checkout"));
        // The old body is gone rather than accumulated.
        assert!(!updated.contains("0000000000000000000000000000000000000000"));
        assert_eq!(updated.matches(BEGIN).count(), 1);
        assert_eq!(updated.matches(END).count(), 1);
    }

    #[test]
    fn an_up_to_date_block_is_returned_unchanged() {
        // Drives the caller's no-op path: a second install must not rewrite a
        // file whose block already matches.
        let existing = format!("#!/bin/sh\n{NEW_BLOCK}");
        let updated = replace_fenced_block(&existing, BEGIN, END, NEW_BLOCK).unwrap();
        assert_eq!(updated, existing);
    }

    #[test]
    fn a_file_with_no_fence_is_declined() {
        // A pre-#391 block never wrote a closing marker, so the fence search
        // has nothing to pair the begin marker with. The shipped legacy shapes
        // are `replace_legacy_checkout_block`'s job; anything else is appended
        // to instead of guessing which lines to delete.
        let no_block = "#!/bin/sh\necho hello\n";
        assert!(replace_fenced_block(no_block, BEGIN, END, NEW_BLOCK).is_none());

        let unterminated = "#!/bin/sh\n# tokensave: auto-init\ntokensave init &\n";
        assert!(replace_fenced_block(unterminated, BEGIN, END, NEW_BLOCK).is_none());
        assert!(replace_legacy_checkout_block(unterminated, BEGIN, END, NEW_BLOCK).is_none());
    }

    /// The unfenced body 6.4.3 wrote.
    const LEGACY_6_4_3: &str = "# tokensave: auto-init\n\
                                if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
                                \ttokensave init >/dev/null 2>&1 &\n\
                                fi\n";
    /// The unfenced body 7.3.0 wrote.
    const LEGACY_7_3_0: &str = "# tokensave: auto-init\n\
                                if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
                                \ttokensave init >/dev/null 2>&1 &\n\
                                elif [ \"$3\" = \"1\" ]; then\n\
                                \ttokensave branch add >/dev/null 2>&1 &\n\
                                fi\n";
    const CHAIN: &str = "# tokensave: chain-repo-hook\n\
                         repo_hook=\"$(git rev-parse --git-common-dir 2>/dev/null)/hooks/post-checkout\"\n\
                         if [ -x \"$repo_hook\" ] && [ \"$repo_hook\" != \"$0\" ]; then\n\
                         \t\"$repo_hook\" \"$@\"\n\
                         fi\n";

    /// #580: both shipped unfenced shapes are replaced in place, and what
    /// surrounds them survives byte-for-byte.
    #[test]
    fn a_legacy_unfenced_block_is_replaced_in_place() {
        for legacy in [LEGACY_6_4_3, LEGACY_7_3_0] {
            let existing = format!("#!/bin/sh\n./mine.sh\n\n{legacy}\n{CHAIN}");
            let updated = replace_legacy_checkout_block(&existing, BEGIN, END, NEW_BLOCK).unwrap();
            assert_eq!(
                updated,
                format!("#!/bin/sh\n./mine.sh\n\n{NEW_BLOCK}\n{CHAIN}")
            );
        }
    }

    /// #580: a file that already went through the append path holds the legacy
    /// block *and* a fenced one. It must end up with a single block, and the
    /// chain section between the two must not be swallowed.
    #[test]
    fn a_legacy_block_with_an_appended_duplicate_collapses_to_one_block() {
        let existing = format!("#!/bin/sh\n{LEGACY_7_3_0}\n{CHAIN}\n{NEW_BLOCK}");
        let updated = replace_legacy_checkout_block(&existing, BEGIN, END, NEW_BLOCK).unwrap();
        assert_eq!(updated, format!("#!/bin/sh\n{NEW_BLOCK}\n{CHAIN}"));
    }

    #[test]
    fn a_fenced_or_unrecognised_block_is_not_treated_as_legacy() {
        // Fenced v1: same body, but closed — `replace_fenced_block` owns it.
        let fenced = format!("#!/bin/sh\n{LEGACY_6_4_3}{END}\n");
        assert!(replace_legacy_checkout_block(&fenced, BEGIN, END, NEW_BLOCK).is_none());

        // A body that is not the shipped shape: the first `fi` would not be
        // the end of it, so its extent is unknown.
        let nested = "#!/bin/sh\n\
                      # tokensave: auto-init\n\
                      if [ \"$1\" = \"0000000000000000000000000000000000000000\" ]; then\n\
                      \tif [ -n \"$X\" ]; then\n\
                      \t\ttokensave init &\n\
                      \tfi\n\
                      fi\n";
        assert!(replace_legacy_checkout_block(nested, BEGIN, END, NEW_BLOCK).is_none());

        let current = format!("#!/bin/sh\n{NEW_BLOCK}");
        assert!(replace_legacy_checkout_block(&current, BEGIN, END, NEW_BLOCK).is_none());
    }

    /// #580 end to end: the install path migrates a legacy file instead of
    /// appending to it, and a second run is a no-op.
    #[test]
    fn a_legacy_install_is_migrated_not_appended_to() {
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("post-checkout");
        std::fs::write(&hook, format!("#!/bin/sh\n{LEGACY_6_4_3}\n{CHAIN}")).unwrap();

        let block = post_checkout_snippet("tokensave");
        assert_eq!(
            install_or_migrate_block(&hook, BEGIN, END, &block),
            HookWrite::Migrated
        );
        let after = std::fs::read_to_string(&hook).unwrap();
        assert_eq!(after, format!("#!/bin/sh\n{block}\n{CHAIN}"));

        assert_eq!(
            install_or_migrate_block(&hook, BEGIN, END, &block),
            HookWrite::UpToDate
        );
        assert_eq!(std::fs::read_to_string(&hook).unwrap(), after);
    }

    #[test]
    fn a_rewrite_keeps_a_missing_trailing_newline_from_being_invented() {
        // The fence ends the file with no trailing newline after the marker.
        let existing = format!("#!/bin/sh\n{}", NEW_BLOCK.trim_end_matches('\n'));
        let updated = replace_fenced_block(&existing, BEGIN, END, NEW_BLOCK).unwrap();
        assert_eq!(updated, format!("#!/bin/sh\n{NEW_BLOCK}"));
    }

    #[test]
    fn the_installed_snippet_is_a_single_delegating_line() {
        // v2's reason for existing: the hook file carries no branching, so
        // behaviour changes ship with the binary instead of needing another
        // rewrite of a file in the user's repository.
        let snippet = post_checkout_snippet("/usr/local/bin/tokensave");
        assert!(snippet.contains(&format!("(v{HOOK_CHECKOUT_VERSION})")));
        assert!(snippet.contains("hook post-checkout \"$@\""));
        assert!(
            !snippet.contains("0000000000000000000000000000000000000000"),
            "the sentinel branch belongs in the binary now"
        );
        assert!(snippet.trim_end().ends_with(HOOK_MARKER_CHECKOUT_END));
        // Backgrounded, so git is never blocked on tokensave.
        assert!(snippet.contains("&\n"));
    }

    #[test]
    fn a_stale_install_is_migrated_and_an_current_one_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("post-checkout");
        std::fs::write(
            &hook,
            "#!/bin/sh\n./mine.sh\n# tokensave: auto-init\ntokensave init &\n# tokensave: end auto-init\n",
        )
        .unwrap();

        let block = post_checkout_snippet("tokensave");
        assert_eq!(
            install_or_migrate_block(&hook, BEGIN, END, &block),
            HookWrite::Migrated
        );
        let after = std::fs::read_to_string(&hook).unwrap();
        assert!(after.starts_with("#!/bin/sh\n./mine.sh\n"));
        assert!(after.contains("hook post-checkout"));

        // Second run is a no-op, so reinstall does not churn the file.
        assert_eq!(
            install_or_migrate_block(&hook, BEGIN, END, &block),
            HookWrite::UpToDate
        );
        assert_eq!(std::fs::read_to_string(&hook).unwrap(), after);
    }

    #[cfg(unix)]
    #[test]
    fn a_migration_keeps_the_executable_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("post-checkout");
        std::fs::write(
            &hook,
            "#!/bin/sh\n# tokensave: auto-init\ntokensave init &\n# tokensave: end auto-init\n",
        )
        .unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

        install_or_migrate_block(&hook, BEGIN, END, &post_checkout_snippet("tokensave"));

        // A hook that loses +x silently stops running, which is worse than
        // not migrating it at all.
        let mode = std::fs::metadata(&hook).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111, "hook must stay executable");
    }

    /// #624: doctor reports a post-commit or post-merge section that names a
    /// moved binary or still has the unfenced shape, as it does post-checkout.
    #[test]
    fn stale_sync_sections_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let bin = "/new/tokensave";
        std::fs::write(
            dir.path().join("post-commit"),
            format!("#!/bin/sh\n{}", post_commit_snippet("/old/tokensave")),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("post-merge"),
            "#!/bin/sh\n# tokensave: auto-sync (post-merge)\n/new/tokensave sync >/dev/null 2>&1 &\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("post-checkout"),
            format!("#!/bin/sh\n{}", post_checkout_snippet(bin)),
        )
        .unwrap();

        let mut stale = stale_sections_in(dir.path(), bin);
        stale.sort();
        assert_eq!(
            stale,
            vec![
                dir.path().join("post-commit"),
                dir.path().join("post-merge")
            ]
        );

        std::fs::write(
            dir.path().join("post-commit"),
            format!("#!/bin/sh\n{}", post_commit_snippet(bin)),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("post-merge"),
            format!("#!/bin/sh\n{}", post_merge_snippet(bin)),
        )
        .unwrap();
        assert!(stale_sections_in(dir.path(), bin).is_empty());
    }

    /// A line after the sync marker that is not ours is never rewritten, so
    /// doctor must not call it stale either.
    #[test]
    fn an_unrecognised_sync_section_is_not_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("post-commit"),
            "#!/bin/sh\n# tokensave: auto-sync\n./scripts/mine.sh\n",
        )
        .unwrap();
        assert!(stale_sections_in(dir.path(), "/new/tokensave").is_empty());
    }

    /// #624: post-commit and post-merge are fenced now, so tokensave's line can
    /// be found and rewritten without guessing.
    #[test]
    fn sync_snippets_are_fenced() {
        let commit = post_commit_snippet("/usr/bin/tokensave");
        assert!(commit.starts_with(HOOK_MARKER));
        assert!(commit.trim_end().ends_with(HOOK_MARKER_END));
        let merge = post_merge_snippet("/usr/bin/tokensave");
        assert!(merge.starts_with(HOOK_MARKER_MERGE));
        assert!(merge.trim_end().ends_with(HOOK_MARKER_MERGE_END));
    }

    /// #624: the unfenced marker-plus-one-line shape every release before the
    /// fence wrote is recognised, rewritten with the current binary path, and
    /// everything around it is kept byte-for-byte.
    #[test]
    fn a_legacy_post_commit_line_is_fenced_and_repointed() {
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("post-commit");
        let before = "#!/bin/sh\n\
                      ./scripts/mine.sh\n\
                      \n\
                      # tokensave: auto-sync\n\
                      C:/old/place/tokensave.exe sync >/dev/null 2>&1 &\n\
                      ./scripts/after.sh\n";
        std::fs::write(&hook, before).unwrap();

        let write = install_or_migrate_block(
            &hook,
            HOOK_MARKER,
            HOOK_MARKER_END,
            &post_commit_snippet("/new/bin/tokensave"),
        );
        assert_eq!(write, HookWrite::Migrated);
        let after = std::fs::read_to_string(&hook).unwrap();
        assert_eq!(
            after,
            format!(
                "#!/bin/sh\n./scripts/mine.sh\n\n{}./scripts/after.sh\n",
                post_commit_snippet("/new/bin/tokensave")
            )
        );

        // Second run is a no-op.
        let again = install_or_migrate_block(
            &hook,
            HOOK_MARKER,
            HOOK_MARKER_END,
            &post_commit_snippet("/new/bin/tokensave"),
        );
        assert_eq!(again, HookWrite::UpToDate);
    }

    #[test]
    fn a_legacy_post_merge_line_is_fenced_and_repointed() {
        let dir = tempfile::tempdir().unwrap();
        let hook = dir.path().join("post-merge");
        std::fs::write(
            &hook,
            "#!/bin/sh\n# tokensave: auto-sync (post-merge)\n/old/tokensave sync >/dev/null 2>&1 &\n",
        )
        .unwrap();

        let write = install_or_migrate_block(
            &hook,
            HOOK_MARKER_MERGE,
            HOOK_MARKER_MERGE_END,
            &post_merge_snippet("/new/tokensave"),
        );
        assert_eq!(write, HookWrite::Migrated);
        assert_eq!(
            std::fs::read_to_string(&hook).unwrap(),
            format!("#!/bin/sh\n{}", post_merge_snippet("/new/tokensave"))
        );
    }

    /// A fenced sync block whose binary path changed is rewritten in place.
    #[test]
    fn a_fenced_post_commit_block_follows_a_moved_binary() {
        let old = format!(
            "#!/bin/sh\n{}# mine\n",
            post_commit_snippet("/old/tokensave")
        );
        let updated = replace_fenced_block(
            &old,
            HOOK_MARKER,
            HOOK_MARKER_END,
            &post_commit_snippet("/new/tokensave"),
        )
        .unwrap();
        assert_eq!(
            updated,
            format!(
                "#!/bin/sh\n{}# mine\n",
                post_commit_snippet("/new/tokensave")
            )
        );
    }

    /// A line after the marker that is not a sync command is not ours to
    /// rewrite, so the legacy matcher declines it.
    #[test]
    fn an_unrecognised_line_after_the_sync_marker_is_left_alone() {
        let contents = "#!/bin/sh\n# tokensave: auto-sync\n./scripts/mine.sh\n";
        assert!(replace_legacy_sync_line(
            contents,
            HOOK_MARKER,
            HOOK_MARKER_END,
            &post_commit_snippet("/new/tokensave")
        )
        .is_none());
    }

    /// Removing a fenced sync block keeps a user's line that follows the end
    /// marker directly, with no blank line between them.
    #[test]
    fn strip_stops_at_the_sync_end_marker() {
        let contents = format!(
            "#!/bin/sh\n{}./scripts/after.sh\n",
            post_commit_snippet("/usr/bin/tokensave")
        );
        assert_eq!(
            strip_tokensave_sections(&contents).unwrap(),
            "#!/bin/sh\n./scripts/after.sh\n"
        );
    }
}
