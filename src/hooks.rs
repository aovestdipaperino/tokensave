//! Hook handlers for Claude Code, Kiro, and Factory Droid integrations.
//!
//! These functions are invoked by Claude Code's hook system to intercept
//! tool calls, redirect exploration work to tokensave MCP tools, and
//! track per-session token savings. Kiro and Factory Droid invoke their own
//! handlers with hook events on stdin and expect blocking decisions through
//! process exit codes rather than Claude's stdout JSON decision.

use std::borrow::Cow;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

const TOKENSAVE_RESEARCH_BLOCK_REASON: &str = "STOP: Use tokensave MCP tools \
(tokensave_context, tokensave_search, tokensave_callees, tokensave_callers, \
tokensave_impact, tokensave_files, tokensave_affected) instead of agents for \
code research. Tokensave is faster and more precise for symbol relationships, \
call paths, and code structure. Only use agents for code exploration if you \
have already tried tokensave and it cannot answer the question.";

/// Maximum pattern length we'll classify. Beyond this we always pass through —
/// long patterns are almost certainly regex sweeps, not symbol lookups.
const MAX_PATTERN_LEN: usize = 200;

/// File extensions tokensave indexes (across all language feature tiers).
const CODE_EXTENSIONS: &[&str] = &[
    // Lite tier
    "rs", "go", "java", "scala", "sc", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "py",
    "pyi", "pyw", "c", "h", "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "inl", "ipp",
    "tcc", "kt", "kts", "cs", "csx", "swift", // Medium tier
    "dart", "pas", "pp", "dpr", "php", "phtml", "rb", "rake", "gemspec", "sh", "bash", "zsh",
    "proto", "ps1", "psm1", "psd1", "nix", "vb", "vbs", // Full tier
    "lua", "zig", "m", "mm", "pl", "pm", "bat", "cmd", "f", "f90", "f95", "f03", "for", "ftn",
    "cbl", "cob", "cpy", "bas", // HDL
    "v", "vh", "sv", "svh",
];

/// Directory basenames that we treat as "code roots" when a grep target has no
/// file extension (e.g. `src/`, `crates/`).
const CODE_DIRS: &[&str] = &[
    "src", "lib", "tests", "test", "crates", "app", "internal", "pkg", "cmd", "include",
];

/// `type` filter values (ripgrep `--type`) we treat as code-language scoped.
const CODE_TYPE_FILTERS: &[&str] = &[
    "rust",
    "go",
    "py",
    "python",
    "ts",
    "typescript",
    "js",
    "javascript",
    "java",
    "scala",
    "kt",
    "kotlin",
    "c",
    "cpp",
    "cxx",
    "swift",
    "cs",
    "csharp",
    "dart",
    "rb",
    "ruby",
    "php",
    "lua",
    "zig",
    "perl",
    "pascal",
    "vb",
    "vbnet",
    "nix",
    "bash",
    "sh",
    "shell",
    "proto",
    "powershell",
    "ps1",
    "fortran",
    "cobol",
    "objc",
    "objective-c",
    "basic",
];

/// Runtime environment for hook decisions.
///
/// Fields capture every piece of process state the decision logic needs, so
/// the rest of the module can stay a pure function of `(tool_input, env)`.
/// `from_runtime()` reads the real environment; tests construct an instance
/// directly.
#[derive(Debug, Clone, Default)]
pub struct HookEnv {
    /// `true` when the working directory is inside a usable tokensave index
    /// (`.tokensave/tokensave.db` in it or in an ancestor). Without an index
    /// there is nothing to redirect to, so the hook always passes through.
    pub in_tokensave_project: bool,

    /// `true` when the user has opted out for this invocation via
    /// `TOKENSAVE_DISABLE_GREP_HOOK=1`.
    pub disable_grep_hook: bool,

    /// The indexed project root the working directory belongs to. Kept so a
    /// target spelled as an absolute (or `~`-rooted) path can be recognized as
    /// the whole project instead of an unknown directory. `None` and
    /// `in_tokensave_project` always agree; both come from one discovery walk.
    pub project_root: Option<PathBuf>,
}

impl HookEnv {
    /// Snapshot the real environment.
    pub fn from_runtime() -> Self {
        Self::from_runtime_at(std::env::current_dir().ok().as_deref())
    }

    /// [`HookEnv::from_runtime`] with an explicit working directory, so a
    /// harness that reports its own `cwd` can be honored and tests can stay
    /// hermetic. Discovers the project the same way `serve`, `sync`, and
    /// `status` do, by walking ancestors.
    pub fn from_runtime_at(cwd: Option<&Path>) -> Self {
        let project_root = cwd.and_then(crate::config::discover_project_root);
        let disable_grep_hook = std::env::var("TOKENSAVE_DISABLE_GREP_HOOK")
            .is_ok_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"));
        Self {
            in_tokensave_project: project_root.is_some(),
            disable_grep_hook,
            project_root,
        }
    }

    /// Re-resolve the project from a hook event's top-level `cwd`, the working
    /// directory of the session that issued the tool call. The hook process
    /// itself can be spawned anywhere. The opt-out belongs to that process, so
    /// it survives the override, and an event without a usable `cwd` leaves the
    /// environment untouched.
    fn for_event(&self, event: &Value) -> Self {
        let Some(cwd) = event_cwd(event) else {
            return self.clone();
        };
        let project_root = crate::config::discover_project_root(&cwd);
        Self {
            in_tokensave_project: project_root.is_some(),
            disable_grep_hook: self.disable_grep_hook,
            project_root,
        }
    }
}

/// Shape of a grep pattern that is safe to redirect to a tokensave tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatternShape {
    /// Single bare identifier (e.g. `handle_request`).
    BareSymbol,
    /// `\bsymbol\b` — a word-boundary symbol lookup.
    WordBoundary,
    /// Multiple identifiers joined by `|` (or `\|` in BRE).
    Alternation,
    /// A definition-anchored spelling of a single symbol — a leading
    /// `def`/`class`/`fn`/… keyword, a trailing `(`, or both (#452). The
    /// grepper is looking for where the symbol is *declared*, which is
    /// precisely `tokensave_search`.
    Definition,
}

/// `PreToolUse` hook handler for Claude Code's Agent / Grep / Bash matchers.
///
/// Claude Code delivers the hook event as JSON on **stdin** with the tool
/// arguments nested under `tool_input`; it sets no `TOOL_INPUT` env var.
/// Reads stdin, inspects the input, and prints a JSON decision to stdout.
/// Blocks Explore agents, exploration-style prompts, and symbol-shaped
/// grep/Grep calls against indexed code files — directing Claude to use
/// tokensave MCP tools instead. Falls back to the `TOOL_INPUT` env var when
/// stdin is empty.
pub fn hook_pre_tool_use() {
    let raw = read_stdin_to_string();
    let decision = if raw.trim().is_empty() {
        evaluate_hook_decision(&std::env::var("TOOL_INPUT").unwrap_or_default())
    } else {
        evaluate_claude_pre_tool_use(&raw)
    };
    // Cursor's permission-gating `preToolUse` hook treats any stdout that lacks
    // a `permission` field as fail-closed and reports `Hook ... returned invalid
    // JSON`, silently blocking every Grep/Shell call. An empty decision means
    // "allow", so emit the explicit allow object; Claude Code ignores the
    // unknown flat field and falls through to its normal permission flow.
    if decision.is_empty() {
        println!("{}", build_allow_message());
    } else {
        println!("{decision}");
    }
}

/// Parse Claude Code's `PreToolUse` stdin JSON and return the decision string.
///
/// Unwraps the nested `tool_input` object before delegating to
/// [`evaluate_hook_decision`]. If the payload isn't the expected wrapper shape,
/// falls back to treating `raw` as a flat tool-input object.
pub fn evaluate_claude_pre_tool_use(raw: &str) -> String {
    evaluate_claude_pre_tool_use_with_env(raw, &HookEnv::from_runtime())
}

/// [`evaluate_claude_pre_tool_use`] with an explicit environment snapshot.
///
/// The snapshot is a base: when the event reports its own `cwd`, the project is
/// re-discovered from there.
pub fn evaluate_claude_pre_tool_use_with_env(raw: &str, env: &HookEnv) -> String {
    let event = serde_json::from_str::<serde_json::Value>(raw).ok();
    let env = event
        .as_ref()
        .map_or_else(|| env.clone(), |event| env.for_event(event));
    let tool_input = event
        .and_then(|v| v.get("tool_input").cloned())
        .map_or_else(|| raw.to_string(), |ti| ti.to_string());
    evaluate_hook_decision_with_env(&tool_input, &env)
}

/// Pure decision logic for the `PreToolUse` hook, using the real process
/// environment.
///
/// Takes the raw `TOOL_INPUT` JSON string and returns the JSON decision
/// string to print to stdout. An empty string means "allow".
pub fn evaluate_hook_decision(tool_input: &str) -> String {
    evaluate_hook_decision_with_env(tool_input, &HookEnv::from_runtime())
}

/// Pure decision logic for the `PreToolUse` hook with an explicit environment
/// snapshot. Tests use this to avoid touching the real process state.
pub fn evaluate_hook_decision_with_env(tool_input: &str, env: &HookEnv) -> String {
    match evaluate_hook_decision_core(tool_input, env) {
        Some(reason) => build_block_message(&reason),
        // Empty string = no output -> Claude Code implicitly allows the tool call.
        None => String::new(),
    }
}

/// Shared decision core behind every `PreToolUse`-style hook (Claude's
/// stdout-JSON path, and the exit-code path used by Kiro and Factory Droid).
/// Returns `Some(reason)` when the call should be redirected to tokensave MCP
/// tools, `None` to allow it through unchanged. Per-agent adapters only
/// differ in how they deliver the event and how they signal the decision —
/// the classification logic here is identical for all of them.
fn evaluate_hook_decision_core(tool_input: &str, env: &HookEnv) -> Option<String> {
    let parsed: serde_json::Value =
        serde_json::from_str(tool_input).unwrap_or_else(|_| serde_json::json!({}));

    // Agent/Task redirection is gated the same way as the Grep/Bash paths:
    // without a `.tokensave` index there are no MCP tools to redirect to, and
    // the opt-out gives a user who deliberately wants to delegate an explicit
    // override instead of a hard wall.
    if env.in_tokensave_project && !env.disable_grep_hook {
        // A blank `subagent_type` is treated as absent: a caller that
        // initializes the field to "" is no more a deliberate typed delegation
        // than one that omits it, so it must not slip past both the Explore
        // check and the untyped-prompt check below.
        let subagent = parsed
            .get("subagent_type")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        // Block Claude Code's built-in `Explore` research agent outright.
        // Droid normalizes its exact lowercase built-in type in its adapter.
        if subagent == Some("Explore") {
            return Some(TOKENSAVE_RESEARCH_BLOCK_REASON.to_string());
        }

        // Only steer *untyped* Agent/Task calls by prompt shape: an untyped call
        // may still be an Explore-style research fan-out. An explicitly typed
        // non-Explore agent (`general-purpose`, an implementer, a custom agent,
        // or another harness's own task/subagent type) is a deliberate
        // delegation and must not be blocked on prompt text — the caller chose a
        // specific worker, and prompt keywords cannot tell research from
        // implementation.
        if subagent.is_none() {
            if let Some(prompt) = parsed.get("prompt").and_then(|v| v.as_str()) {
                if is_code_research_prompt(prompt) {
                    return Some(TOKENSAVE_RESEARCH_BLOCK_REASON.to_string());
                }
            }
        }
    }

    // Grep tool — `pattern` is the discriminating field.
    if parsed.get("pattern").is_some() {
        if let Some(reason) = evaluate_grep_tool_input(&parsed, env) {
            return Some(reason);
        }
        // Glob shares the `pattern` field with Grep and is told apart by the
        // fields it lacks (#294).
        if let Some(reason) = evaluate_glob_tool_input(&parsed, env) {
            return Some(reason);
        }
    }

    // Bash/Execute tool — `command` is the discriminating field.
    if let Some(command) = parsed.get("command").and_then(|v| v.as_str()) {
        if let Some(reason) = evaluate_bash_command(command, env) {
            return Some(reason);
        }
        if let Some(reason) = evaluate_find_command(command, env) {
            return Some(reason);
        }
    }

    None
}

/// Cross-harness "allow" decision for the stdout `PreToolUse` contract.
///
/// Cursor gates the tool on the flat `permission` field and treats a missing one
/// as a fail-closed block; Claude Code ignores the unknown field and falls
/// through to its normal permission flow. One object therefore allows the call
/// under Cursor without changing Claude's behaviour.
fn build_allow_message() -> String {
    serde_json::json!({ "permission": "allow" }).to_string()
}

fn build_block_message(reason: &str) -> String {
    // Cursor-native fields (`permission` + user/agent messages) gate the tool
    // and surface the reason without any Claude-compat mapping; the nested
    // `hookSpecificOutput` keeps Claude Code (and the hook tests) working.
    serde_json::json!({
        "permission": "deny",
        "user_message": reason,
        "agent_message": reason,
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
    .to_string()
}

/// Inspect a `Grep` tool input. Returns `Some(reason)` to redirect.
fn evaluate_grep_tool_input(parsed: &Value, env: &HookEnv) -> Option<String> {
    if !env.in_tokensave_project || env.disable_grep_hook {
        return None;
    }
    let pattern = parsed.get("pattern").and_then(|v| v.as_str())?;
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
        return None;
    }
    // Both harnesses default an omitted mode to a cheap path-only result.
    // Redirect only explicit content searches; missing, malformed, cheap, or
    // unknown modes fail open.
    if parsed.get("output_mode").and_then(|v| v.as_str()) != Some("content") {
        return None;
    }
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    // Claude names this field `glob`; Droid names it `glob_pattern`.
    let glob = parsed
        .get("glob")
        .or_else(|| parsed.get("glob_pattern"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let ty = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !target_looks_like_code(path, &[glob], ty, env) {
        return None;
    }
    let shape = classify_symbol_pattern(pattern)?;
    Some(redirect_message("Grep", pattern, shape))
}

/// Inspect a `Bash` tool command. Returns `Some(reason)` to redirect.
/// Commands whose only effect is output. Anything not listed is treated as
/// side-effecting, so an unrecognized command means the batch is left alone.
///
/// `before_search` distinguishes the two positions `true` and `:` appear in.
/// Ahead of the search they carry nothing, so `true; grep -n Sym src/lib.rs`
/// is a batched search. *After* it they are consuming the search's exit
/// status — `grep -n Sym src/lib.rs || true` is the error-suppression idiom
/// #475/#476 pin as pass-through, so they are not inert there.
fn is_inert_command(segment: &str, before_search: bool) -> bool {
    // Deliberately short and conservative: anything unrecognised counts as
    // having side effects, so the unknown case allows rather than eating work.
    const INERT: [&str; 5] = ["echo", "pwd", "ls", "cat", "printf"];
    const EXIT_STATUS_ONLY: [&str; 2] = ["true", ":"];
    let rest = strip_command_prefixes(segment.trim()).rest;
    let head = rest.split_whitespace().next().unwrap_or("");
    INERT.contains(&head) || (before_search && EXIT_STATUS_ONLY.contains(&head))
}

/// Split a command on top-level `&&`, `||` and `;`, keeping each segment
/// verbatim so it can be re-classified on its own.
///
/// Returns `None` for a single segment, for unbalanced quotes, and for the
/// shapes this hook deliberately does not model: subshells, command
/// substitution, newlines, pipes, redirects and background jobs. Not modeled means not
/// blocked — the caller falls through and allows.
fn split_top_level_segments(command: &str) -> Option<Vec<&str>> {
    // Command substitution is unmodeled wherever it sits, and quoting does not
    // make it inert: `echo "$(curl -X POST …)"` runs the POST. The scan below
    // skips quoted spans, so this has to be caught before it starts.
    // A newline separates commands just like `;` does, but the segment scan
    // below does not split on it, so an embedded newline would hide real work
    // inside a segment that looks inert from its first word.
    if command.contains("$(") || command.contains('`') || command.contains('\n') {
        return None;
    }

    let mut segments: Vec<&str> = Vec::new();
    let mut start = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_double = false;
            }
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            // Matches has_chained_command: on Windows a backslash is a path separator,
            // not an escape, so consuming the next char there would desynchronise the
            // two scanners on the same line.
            '\\' if !cfg!(windows) => {
                chars.next();
            }
            '(' | ')' | '`' | '<' | '>' => return None,
            // A lone `&` backgrounds and a lone `|` pipes; only the doubled
            // forms are sequencing operators.
            '&' | '|' => {
                if chars.peek().map(|&(_, next)| next) != Some(c) {
                    return None;
                }
                chars.next();
                segments.push(&command[start..i]);
                start = i + 2;
            }
            ';' => {
                segments.push(&command[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    if in_single || in_double {
        return None;
    }
    segments.push(&command[start..]);

    let segments: Vec<&str> = segments
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if segments.len() < 2 {
        None
    } else {
        Some(segments)
    }
}

fn evaluate_bash_command(command: &str, env: &HookEnv) -> Option<String> {
    if !env.in_tokensave_project || env.disable_grep_hook {
        return None;
    }
    // The whole command first. This is the only path that models a leading
    // `cd`, which segment splitting would otherwise read as a side-effecting
    // command and allow.
    if let Some(reason) = evaluate_bash_segment(command, env) {
        return Some(reason);
    }
    // #451: batching a search behind other commands is an ordinary shape, not
    // an attempt to slip past the hook. Redirect the search only when every
    // other segment is inert, so a denial can never discard real work.
    let segments = split_top_level_segments(command)?;
    let mut reason = None;
    for segment in &segments {
        if let Some(found) = evaluate_bash_segment(segment, env) {
            reason.get_or_insert(found);
        } else if !is_inert_command(segment, reason.is_none()) {
            return None;
        }
    }
    reason
}

/// Classify one command that carries no top-level sequencing operators.
fn evaluate_bash_segment(command: &str, env: &HookEnv) -> Option<String> {
    // An explicit inline `TOKENSAVE_DISABLE_GREP_HOOK=<truthy>` opts out too, so
    // the deliberate bypass is honored rather than stripped and then blocked.
    let stripped = strip_command_prefixes(command.trim());
    if stripped.disables_hook {
        return None;
    }
    let inv = extract_grep_invocation(command)?;
    if inv.pattern.is_empty() || inv.pattern.len() > MAX_PATTERN_LEN {
        return None;
    }
    let target = inv.targets.first().map_or("", String::as_str);
    let target =
        if let (Some(cd_path), Some(root)) = (stripped.cd_target, env.project_root.as_deref()) {
            // The grep runs after a `cd`: resolve the target relative to the
            // cd'd directory, not the session cwd. If the cd takes us outside
            // the indexed project, the hook does not apply. An unresolvable cd
            // path is treated as unknown so the ordinary classification rules can
            // fall back to the session cwd rather than silently allowing.
            let cd_path = unquote(cd_path);
            let cd_path = unescape_shell_backslashes(cd_path);
            let cd_path = expand_home_prefix(&cd_path, crate::agents::home_dir().as_deref())
                .unwrap_or_else(|| PathBuf::from(cd_path.as_ref()));
            match classify_path_within_project(&cd_path.to_string_lossy(), Some(root)) {
                Containment::Outside => return None,
                Containment::Inside => {
                    // Canonicalize the cd'd directory so a symlink to a code directory
                    // is resolved to the real path and classified by its basename.
                    let cd_base = root
                        .join(&cd_path)
                        .canonicalize()
                        .unwrap_or_else(|_| root.join(&cd_path));
                    let effective = if target.is_empty() || target == "." || target == "./" {
                        cd_base
                    } else {
                        cd_base.join(target)
                    };
                    Cow::Owned(effective.to_string_lossy().into_owned())
                }
                Containment::Unknown => Cow::Borrowed(target),
            }
        } else {
            Cow::Borrowed(target)
        };
    let globs: Vec<&str> = inv.globs.iter().map(String::as_str).collect();
    if !target_looks_like_code(
        target.as_ref(),
        &globs,
        inv.ty.as_deref().unwrap_or(""),
        env,
    ) {
        return None;
    }
    let shape = classify_symbol_pattern(&inv.pattern)?;
    Some(redirect_message("Bash grep", &inv.pattern, shape))
}

/// Inspect a `Bash` `find`/`fd` command. Returns `Some(reason)` to redirect.
///
/// Path-shaped discovery has a graph answer now that non-code artifacts are
/// tracked too (#323); before that, `tokensave_files` was lossy and redirecting
/// here would have traded a working command for an empty result (#294).
fn evaluate_find_command(command: &str, env: &HookEnv) -> Option<String> {
    if !env.in_tokensave_project || env.disable_grep_hook {
        return None;
    }
    let stripped = strip_command_prefixes(command.trim());
    if stripped.disables_hook {
        return None;
    }
    let inv = extract_find_invocation(command)?;

    // Re-root find roots the same way grep targets are re-rooted after a
    // leading `cd`. If the cd leaves the project, the hook does not apply.
    let targets =
        if let (Some(cd_path), Some(root)) = (stripped.cd_target, env.project_root.as_deref()) {
            let cd_path = unquote(cd_path);
            let cd_path = unescape_shell_backslashes(cd_path);
            let cd_path = expand_home_prefix(&cd_path, crate::agents::home_dir().as_deref())
                .unwrap_or_else(|| PathBuf::from(cd_path.as_ref()));
            match classify_path_within_project(&cd_path.to_string_lossy(), Some(root)) {
                Containment::Outside => return None,
                Containment::Inside => {
                    let cd_base = root
                        .join(&cd_path)
                        .canonicalize()
                        .unwrap_or_else(|_| root.join(&cd_path));
                    if inv.targets.is_empty() {
                        vec![cd_base.to_string_lossy().into_owned()]
                    } else {
                        inv.targets
                            .iter()
                            .map(|t| {
                                if t.is_empty() || t == "." || t == "./" {
                                    cd_base.to_string_lossy().into_owned()
                                } else {
                                    cd_base.join(t).to_string_lossy().into_owned()
                                }
                            })
                            .collect()
                    }
                }
                Containment::Unknown => inv.targets,
            }
        } else {
            inv.targets
        };

    // Every root must be code-ish. A search spanning an unindexed tree is one
    // `tokensave_files` cannot answer, and a partial answer is worse than none.
    if !targets.is_empty()
        && !targets
            .iter()
            .all(|target| target_looks_like_code(target, &[], "", env))
    {
        return None;
    }

    // Likewise every name glob: `find . -name '*.rs' -o -name '*.bin'` is only
    // redirectable if the whole thing is.
    if !inv
        .globs
        .iter()
        .all(|glob| classify_glob_target(glob) == Some(true))
    {
        return None;
    }

    Some(files_redirect_message("Bash find", &inv.globs.join(", ")))
}

/// Inspect a `Glob` tool call. Returns `Some(reason)` to redirect.
fn evaluate_glob_tool_input(parsed: &Value, env: &HookEnv) -> Option<String> {
    if !env.in_tokensave_project || env.disable_grep_hook {
        return None;
    }
    // `Grep` and `Glob` both carry `pattern`; only `Grep` carries these. Without
    // the check a content search would be misread as a path search, which is
    // the one mistake that would send a caller to a tool that cannot help.
    if parsed.get("output_mode").is_some()
        || parsed.get("glob").is_some()
        || parsed.get("glob_pattern").is_some()
        || parsed.get("type").is_some()
    {
        return None;
    }
    let pattern = parsed.get("pattern").and_then(|v| v.as_str())?;
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
        return None;
    }
    // A glob without a wildcard is indistinguishable from a `Grep` pattern that
    // happens to contain a dot, so require the wildcard before claiming this is
    // a path search at all.
    if !pattern.contains('*') && !pattern.contains('?') {
        return None;
    }
    if classify_glob_target(pattern) != Some(true) {
        return None;
    }
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if !target_looks_like_code(path, &[], "", env) {
        return None;
    }
    Some(files_redirect_message("Glob", pattern))
}

/// Redirect text for path-shaped discovery, which `tokensave_files` answers.
fn files_redirect_message(tool_label: &str, pattern: &str) -> String {
    format!(
        "STOP: This {tool_label} searches a tokensave-indexed project for files matching \
         `{pattern}`. Use tokensave_files(pattern=\"{pattern}\") instead — it answers from the \
         index, honors the project's ignore rules, and covers non-code artifacts (specs, \
         schemas, fixtures) as well as source. To override for this one call, set \
         TOKENSAVE_DISABLE_GREP_HOOK=1 in the shell."
    )
}

fn redirect_message(tool_label: &str, pattern: &str, shape: PatternShape) -> String {
    let suggestion = match shape {
        PatternShape::BareSymbol | PatternShape::WordBoundary => {
            "tokensave_search (definition) or tokensave_callers_for (usages)"
        }
        PatternShape::Definition => "tokensave_search (definition)",
        PatternShape::Alternation => {
            "tokensave_signature_search (multiple names at once) or repeated tokensave_search calls"
        }
    };
    format!(
        "STOP: This {tool_label} targets a code file in a tokensave-indexed project and the \
         pattern `{pattern}` looks like a symbol name. Use {suggestion} instead — symbol-indexed \
         lookups are faster and more accurate than text grep. To override for this one call, set \
         TOKENSAVE_DISABLE_GREP_HOOK=1 in the shell."
    )
}

/// Classify the pattern. Returns `None` for anything that contains regex
/// metacharacters we don't understand — the caller passes those through.
fn classify_symbol_pattern(pattern: &str) -> Option<PatternShape> {
    let mut p = pattern;
    let mut had_wb = false;
    if let Some(rest) = p.strip_prefix("\\b") {
        if let Some(rest2) = rest.strip_suffix("\\b") {
            p = rest2;
            had_wb = true;
        }
    }

    // Normalize BRE `\|` to ERE `|` so we can split uniformly. Anything that
    // still looks like a regex escape (e.g. `\.`, `\(`, `\d`) leaves a `\`
    // behind, which `is_pure_identifier` will reject.
    let normalized = p.replace("\\|", "|");
    let parts: Vec<&str> = normalized.split('|').collect();
    if !parts.iter().all(|s| is_pure_identifier(s)) {
        // Not a bare identifier (or alternation of them). Before passing it
        // through, check whether it is the idiomatic way to grep for a
        // *definition* — `def foo`, `class MyError`, `foo(` (#452). Those are
        // the highest-value redirects, not the least: the intent is exactly a
        // declaration lookup.
        return classify_definition_pattern(p);
    }

    match (parts.len(), had_wb) {
        (1, true) => Some(PatternShape::WordBoundary),
        (1, false) => Some(PatternShape::BareSymbol),
        _ => Some(PatternShape::Alternation),
    }
}

/// Definition-anchor keywords that may precede a symbol name in a grep pattern.
/// Deliberately short and language-idiomatic: each one is a *declaration*
/// keyword, so what follows it is a name being defined, never arbitrary prose.
const DEFINITION_KEYWORDS: &[&str] = &[
    "def",
    "class",
    "fn",
    "func",
    "function",
    "struct",
    "enum",
    "trait",
    "interface",
    "impl",
    "type",
    "module",
    "package",
];

/// Recognize a definition-anchored spelling of a single symbol: an optional
/// leading declaration keyword, the identifier, and an optional trailing `(`.
///
/// Conservative by construction — exactly one identifier may survive the strip,
/// and anything else left over (extra words, regex metacharacters, a trailing
/// `)`, an argument list) returns `None` so the call passes through.
fn classify_definition_pattern(pattern: &str) -> Option<PatternShape> {
    let mut rest = pattern.trim();
    // Anchors are noise for this purpose: `^def foo` is the same intent.
    rest = rest.strip_prefix('^').unwrap_or(rest);
    rest = rest.trim_start();

    let mut had_keyword = false;
    let mut had_paren = false;
    for kw in DEFINITION_KEYWORDS {
        if let Some(tail) = rest.strip_prefix(kw) {
            // Require real separation, so `defaults` is not read as `def aults`.
            if tail.starts_with(|c: char| c.is_whitespace()) {
                rest = tail.trim_start();
                had_keyword = true;
                break;
            }
        }
    }

    // A trailing `(` (bare or escaped) marks a call or definition site.
    if let Some(head) = rest.strip_suffix('(') {
        rest = head.strip_suffix('\\').unwrap_or(head);
        had_paren = true;
    }
    rest = rest.trim_end();

    // Without an anchor there is nothing here the bare-identifier path did not
    // already reject.
    if !(had_keyword || had_paren) || !is_pure_identifier(rest) {
        return None;
    }
    // A declaration keyword pins the intent to the definition. A bare trailing
    // paren does not — `foo(` is as often a hunt for call sites — so it gets
    // the same both-ways suggestion a bare identifier gets.
    if had_keyword {
        Some(PatternShape::Definition)
    } else {
        Some(PatternShape::BareSymbol)
    }
}

fn is_pure_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
}

/// Does the grep target point at a code file/directory/glob?
///
/// Conservative: when the answer is ambiguous we return `false` so the call
/// passes through unchanged.
fn target_looks_like_code(path: &str, globs: &[&str], ty: &str, env: &HookEnv) -> bool {
    // When a concrete path is provided, it must resolve inside the indexed
    // project for the guardrail to apply. Greps targeting other directories
    // (e.g. /tmp, another repo) are not tokensave's concern.
    // Skip when project_root is unknown (tests, edge cases): fall through to
    // the original classification rules.
    // Set when the target is known to resolve inside the indexed tree. That is
    // a stronger signal than any name-based rule, so it overrides the
    // directory-basename fallback further down (#452).
    let mut known_inside = false;
    if !path.is_empty() && env.project_root.is_some() {
        let raw = path.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
        match classify_path_within_project(raw, env.project_root.as_deref()) {
            Containment::Outside => return false,
            // Inside the tree is not the same as inside the index. A path
            // under one of the project's own `exclude` globs — `node_modules`,
            // `vendor`, `build`, `target` — is never indexed, so the tools
            // this guardrail redirects to cannot answer for it, and blocking
            // the grep costs a round-trip through the opt-out for a query
            // nothing else can serve (#448). Same shape as #435, which fixed
            // the out-of-tree half; the indexer and the hook were reading two
            // different notions of "in scope".
            Containment::Inside => {
                if path_is_config_excluded(raw, env.project_root.as_deref()) {
                    return false;
                }
                known_inside = true;
            }
            // Unknown: keep the existing extension / directory rules.
            Containment::Unknown => {}
        }
    }

    if !ty.is_empty() {
        return CODE_TYPE_FILTERS.contains(&ty.to_ascii_lowercase().as_str());
    }

    // A search narrowed to a file set is answered by that set, not by the root
    // the walk starts from. When several globs are given, one non-code member
    // is enough to pass the whole search through: the graph cannot answer for
    // it, so blocking costs a round-trip through the opt-out for nothing.
    let glob_verdicts: Vec<bool> = globs
        .iter()
        .filter_map(|g| classify_glob_target(g))
        .collect();
    if glob_verdicts.iter().any(|is_code| !is_code) {
        return false;
    }
    if !glob_verdicts.is_empty() {
        return true;
    }

    let raw = if path.is_empty() {
        globs.first().copied().unwrap_or("")
    } else {
        path
    };
    let trimmed = raw.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if trimmed.is_empty() || trimmed == "." || trimmed == "./" {
        return true;
    }

    // Ahead of the rules below because they only ever see the basename of an
    // absolute directory.
    if path_is_project_root(trimmed, env.project_root.as_deref()) {
        return true;
    }

    // Extension path: only block when the extension is in our supported list.
    // Look at the last path component only, otherwise a parent directory with a
    // dot (e.g. a temporary path like `.tmp123/project/src`) is misread as a
    // non-code extension and the directory rule never runs.
    let file_part = trimmed
        .trim_end_matches(std::path::is_separator)
        .rsplit(std::path::is_separator)
        .next()
        .unwrap_or(trimmed);
    if let Some(idx) = file_part.rfind('.') {
        let after_dot = &file_part[idx + 1..];
        let ext: String = after_dot
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '+')
            .collect::<String>()
            .to_ascii_lowercase();
        if !ext.is_empty() {
            return CODE_EXTENSIONS.contains(&ext.as_str());
        }
    }

    // No extension — treat as a directory. When the path already resolved
    // inside the indexed tree and survived the exclude globs, that *is* the
    // answer: the directory holds indexed files whatever it is called, and a
    // name list can only get it wrong (#452 — `mypkg/`, `core/`, `api/` are
    // ordinary source roots). The basename list stays as the fallback for a
    // target we could not resolve, where a name is all we have.
    if known_inside && dir_holds_code_files(trimmed) {
        return true;
    }
    let last = trimmed
        .trim_end_matches(std::path::is_separator)
        .rsplit(std::path::is_separator)
        .next()
        .unwrap_or("");
    CODE_DIRS.contains(&last)
}

/// How many directory entries `dir_holds_code_files` will look at before
/// giving up. A hook runs on every tool call, so the walk has to be bounded;
/// exhausting the budget answers "no" and the caller falls through to the
/// name-based rule, which is the pre-existing behaviour.
const CODE_FILE_SCAN_BUDGET: usize = 2_000;

/// Does this directory actually contain source files the index would hold?
///
/// The name of a directory is a poor proxy for what is in it (#452): `mypkg/`,
/// `core/` and `api/` are ordinary source roots, while `docs/` inside the same
/// project is not. Answer from the contents instead, breadth-first and
/// bounded, stopping at the first file with a known code extension. Hidden
/// directories are skipped — they are not indexed, and descending into `.git`
/// would burn the whole budget for nothing.
fn dir_holds_code_files(path: &str) -> bool {
    let start = PathBuf::from(path);
    if start.is_file() {
        return true;
    }
    let mut queue = std::collections::VecDeque::from([start]);
    let mut seen = 0usize;
    while let Some(dir) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > CODE_FILE_SCAN_BUDGET {
                return false;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => queue.push_back(entry.path()),
                Ok(ft) if ft.is_file() => {
                    let ext = name
                        .rsplit_once('.')
                        .map(|(_, e)| e.to_ascii_lowercase())
                        .unwrap_or_default();
                    if !ext.is_empty() && CODE_EXTENSIONS.contains(&ext.as_str()) {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// Result of asking whether a path lies inside the indexed project root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Containment {
    /// The path canonicalizes to a location inside (or equal to) the root.
    Inside,
    /// The path canonicalizes to a location outside the root.
    Outside,
    /// Cannot be resolved (path does not exist, root unknown, unexpanded home,
    /// etc.). The caller decides the policy.
    Unknown,
}

/// Classify whether `raw` points inside, outside, or somewhere undecidable
/// relative to `project_root`. Relative paths are resolved against the root.
/// Symlinks and `..` are followed via `canonicalize`. The only spells treated
/// as inside without hitting the filesystem are `.`, `./`, and an exact root
/// spelling; everything unresolvable is `Unknown` so the caller can fall back
/// to its own conservative rules.
fn classify_path_containment_with_home(
    raw: &str,
    project_root: Option<&Path>,
    home: Option<&Path>,
) -> Containment {
    let Some(root) = project_root else {
        return Containment::Unknown;
    };
    let Some(target) = expand_home_prefix(raw, home) else {
        return Containment::Unknown;
    };
    if target.as_os_str().is_empty() || target == Path::new(".") || target == Path::new("./") {
        return Containment::Inside;
    }
    let resolved = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    match (resolved.canonicalize(), root.canonicalize()) {
        (Ok(target), Ok(root)) => {
            if target.starts_with(&root) {
                Containment::Inside
            } else {
                Containment::Outside
            }
        }
        _ => Containment::Unknown,
    }
}

/// True when `raw` resolves to a path the project's own `.tokensave/config.json`
/// excludes from indexing (#448).
///
/// Read straight from the project's config so the hook and the indexer share
/// one definition of what is in scope, rather than the hook keeping a second
/// list that can drift. A config that cannot be read answers `false`, leaving
/// the caller's existing rules in charge — the same fail-open policy
/// [`classify_path_containment_with_home`] applies to a path it cannot
/// resolve.
fn path_is_config_excluded(raw: &str, project_root: Option<&Path>) -> bool {
    let Some(root) = project_root else {
        return false;
    };
    let Ok(config) = crate::config::load_config(root) else {
        return false;
    };
    path_is_config_excluded_with(raw, root, &config, crate::agents::home_dir().as_deref())
}

/// [`path_is_config_excluded`] with the config and home directory injected, so
/// tests need neither a config file on disk nor a mutated environment.
fn path_is_config_excluded_with(
    raw: &str,
    root: &Path,
    config: &crate::config::TokenSaveConfig,
    home: Option<&Path>,
) -> bool {
    let Some(target) = expand_home_prefix(raw, home) else {
        return false;
    };
    let resolved = if target.is_absolute() {
        target
    } else {
        root.join(target)
    };
    let (Ok(target), Ok(root)) = (resolved.canonicalize(), root.canonicalize()) else {
        return false;
    };
    let Ok(relative) = target.strip_prefix(&root) else {
        return false;
    };
    // Exclude globs are written with forward slashes regardless of platform,
    // matching how the scanner spells the paths it tests them against.
    let relative = relative.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        return false;
    }
    // A grep target is as likely to be a directory as a file, and the two
    // glob spellings (`vendor/**` and `**/vendor`) are matched by different
    // helpers, so ask both.
    crate::config::is_excluded(&relative, config)
        || crate::config::is_excluded_dir(&relative, config)
}

fn classify_path_within_project(raw: &str, project_root: Option<&Path>) -> Containment {
    classify_path_containment_with_home(raw, project_root, crate::agents::home_dir().as_deref())
}

#[cfg(test)]
fn path_is_within_project_with_home(
    raw: &str,
    project_root: Option<&Path>,
    home: Option<&Path>,
) -> bool {
    matches!(
        classify_path_containment_with_home(raw, project_root, home),
        Containment::Inside
    )
}

/// Is `raw` an absolute spelling of the indexed project root itself?
///
/// Compares canonical paths, so a symlinked or `..`-laden spelling still
/// matches while a sibling that merely shares the root's string prefix does
/// not. Deliberately exact: nothing below the root is treated as the whole
/// project. A relative target, an unknown root, an unknown home directory, or
/// any path that cannot be canonicalized answers `false`, leaving the caller's
/// remaining rules in charge.
fn path_is_project_root(raw: &str, project_root: Option<&Path>) -> bool {
    path_is_project_root_with_home(raw, project_root, crate::agents::home_dir().as_deref())
}

fn path_is_project_root_with_home(
    raw: &str,
    project_root: Option<&Path>,
    home: Option<&Path>,
) -> bool {
    let Some(root) = project_root else {
        return false;
    };
    let Some(target) = expand_home_prefix(raw, home) else {
        return false;
    };
    // Relative targets stay with the rules below: they cost two `canonicalize`
    // calls per hook invocation and `.`, the only relative whole-project
    // spelling in practice, is already handled by the caller.
    if !target.is_absolute() {
        return false;
    }
    match (target.canonicalize(), root.canonicalize()) {
        (Ok(target), Ok(root)) => target == root,
        _ => false,
    }
}

/// Strip surrounding quotes/whitespace and expand an exact `~` or `~/…` prefix.
/// `~user` is left alone: only the current user's home is known here.
fn expand_home_prefix(raw: &str, home: Option<&Path>) -> Option<PathBuf> {
    let trimmed = raw.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "~" {
        return home.map(Path::to_path_buf);
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home.map(|h| h.join(rest));
    }
    Some(PathBuf::from(trimmed))
}

fn classify_glob_target(glob: &str) -> Option<bool> {
    let trimmed = glob.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }

    let file_glob = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let extensions = if file_glob.ends_with('}') {
        let brace_start = file_glob.rfind(".{")?;
        let values = &file_glob[brace_start + 2..file_glob.len() - 1];
        let extensions = values
            .split(',')
            .map(str::trim)
            .map(|ext| {
                (!ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric() || c == '+'))
                    .then(|| ext.to_ascii_lowercase())
            })
            .collect::<Option<Vec<_>>>()?;
        extensions
    } else {
        let idx = file_glob.rfind('.')?;
        let ext = file_glob[idx + 1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '+')
            .collect::<String>()
            .to_ascii_lowercase();
        if ext.is_empty() {
            return None;
        }
        vec![ext]
    };

    (!extensions.is_empty()).then(|| {
        extensions
            .iter()
            .all(|ext| CODE_EXTENSIONS.contains(&ext.as_str()))
    })
}

#[derive(Debug, Default)]
struct GrepInvocation {
    pattern: String,
    targets: Vec<String>,
    /// File globs the search was narrowed to: grep's `--include`, rg/ag's
    /// `-g`/`--glob`/`--iglob`. An include glob is more specific evidence than
    /// the search root, exactly as the native `Grep` tool's `glob` field is.
    globs: Vec<String>,
    /// rg/ag's `-t`/`--type` file-type filter, if given.
    ty: Option<String>,
}

/// A `find`/`fd` invocation reduced to what the policy needs: the name globs
/// it is searching for and the directories it is searching under.
#[derive(Debug, Default)]
struct FindInvocation {
    /// Name patterns, normalized to glob form (`*.rs`), in command order.
    globs: Vec<String>,
    /// Search roots, in command order. Empty means the default (`.`).
    targets: Vec<String>,
}

/// True when `command` carries work beyond the search itself — a top-level
/// `&&`, `||`, `;`, `|`, backgrounding `&`, a command substitution (`$(…)` or
/// backticks), or an output redirect — outside of single quotes.
///
/// A search is only ever a *suggestion* to use the graph instead, so it must
/// never be allowed to veto a command it does not fully model. When the line
/// carries anything else, denying it discards that other work — a
/// `./deploy.sh` that never runs is a far worse failure than a grep that was
/// not redirected (#475). The parsers that follow assume the whole line
/// belongs to the search they matched, so anything else on it makes that
/// assumption false and the command passes through untouched.
///
/// `2>`/`2>>` is the exception: it only discards the search's own stderr, so
/// the line still *is* just the search and stays deniable (#480).
fn has_chained_command(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            }
        } else if in_double {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_double = false;
            } else if c == '`' || (c == '$' && chars.peek() == Some(&'(')) {
                // Substitutions still run inside double quotes.
                return true;
            }
        } else {
            match c {
                '\'' => in_single = true,
                '"' => in_double = true,
                '\\' if !cfg!(windows) => {
                    chars.next();
                }
                '$' if chars.peek() == Some(&'(') => return true,
                // Consume `2>` / `2>>` before the redirect arm sees it: routing
                // the search's own stderr away leaves nothing for a denial to
                // discard, so it is still fully modeled.
                '2' if chars.peek() == Some(&'>') => {
                    chars.next();
                    if chars.peek() == Some(&'>') {
                        chars.next();
                    }
                }
                '&' | '|' | ';' | '`' | '>' => return true,
                _ => {}
            }
        }
    }
    false
}

/// Parse a bash command that *starts* with `find` or `fd`, after the same
/// leading-noise stripping `extract_grep_invocation` applies.
///
/// Only the name-matching forms are recognized, because only those have a
/// `tokensave_files` equivalent: `-name`/`-iname` for `find`, and
/// `-e`/`--extension`/`-g`/`--glob` for `fd`. A `find` predicate this does not
/// understand (`-mtime`, `-size`, `-exec`, …) is not a discovery-by-name call
/// and is deliberately left alone, as is `fd`'s default regex form — a regex is
/// not a glob, and guessing at one would block calls we cannot actually serve.
fn extract_find_invocation(command: &str) -> Option<FindInvocation> {
    let rest = strip_command_prefixes(command.trim()).rest;
    if has_chained_command(rest) {
        return None;
    }
    let (is_find, after_tool) = rest
        .strip_prefix("find ")
        .map(|after| (true, after))
        .or_else(|| rest.strip_prefix("fd ").map(|after| (false, after)))?;

    let mut inv = FindInvocation::default();
    let mut iter = shell_split(after_tool).into_iter().peekable();
    while let Some(tok) = iter.next() {
        match tok.as_str() {
            // `find`: -name/-iname take the glob as the next token.
            "-name" | "-iname" if is_find => {
                let Some(glob) = iter.next() else { continue };
                inv.globs.push(glob);
            }
            // `fd`: an extension is given bare, so restore the glob form the
            // rest of the policy already knows how to classify.
            "-e" | "--extension" if !is_find => {
                let Some(ext) = iter.next() else { continue };
                inv.globs.push(format!("*.{ext}"));
            }
            "-g" | "--glob" if !is_find => {
                let Some(glob) = iter.next() else { continue };
                inv.globs.push(glob);
            }
            // Flags that only narrow the result set, and so cannot change what
            // the command is asking for. `find . -type f -name '*.py'` is the
            // ordinary spelling; without these the common case would bail out
            // as unmodelled and the redirect would almost never fire.
            "-type" | "-maxdepth" | "-mindepth" if is_find => {
                iter.next();
            }
            "-print" | "-print0" | "-follow" if is_find => {}
            "-t" | "--type" | "-d" | "--max-depth" | "-E" | "--exclude" if !is_find => {
                iter.next();
            }
            // A predicate we do not model can change what the command means —
            // `-delete` and `-exec` most of all — so stop claiming to
            // understand the invocation rather than redirect it.
            _ if is_find && tok.starts_with('-') => return None,
            // `fd`'s remaining flags (`--hidden`, `-t f`, …) narrow the result
            // set without changing what is being matched, so they are ignored.
            _ if tok.starts_with('-') => {}
            // `find` takes its roots before the predicates; `fd` takes the
            // pattern first and the path after. Either way a bare token that
            // is not a name glob is a search root.
            _ => inv.targets.push(tok),
        }
    }

    // `fd PATTERN [PATH]` with no flags: the first bare token is a regex, not
    // a path. Treating it as a search root would misread the command.
    if !is_find && !inv.targets.is_empty() && inv.globs.is_empty() {
        return None;
    }

    (!inv.globs.is_empty()).then_some(inv)
}

/// Parse a bash command that *starts* with `grep`, `rg`, or `ag` after leading
/// noise is stripped (see `strip_command_prefixes`: `rtk`/`sudo`/`time`/`nice`
/// wrappers, `NAME=value` env assignments, and a leading `cd … &&`/`cd … ;`).
/// Returns `None` for anything else, including piped commands like
/// `ls | grep foo`: piping another command's output through grep is not a code
/// search, so it deliberately passes through.
fn extract_grep_invocation(command: &str) -> Option<GrepInvocation> {
    let rest = strip_command_prefixes(command.trim()).rest;
    // A search chained to other work is not this command's whole story, and a
    // denial would throw that other work away (#475).
    if has_chained_command(rest) {
        return None;
    }

    // Identify the tool. `git grep` is intentionally excluded — it searches
    // git history, which tokensave does not index.
    let after_tool = ["grep ", "rg ", "ag "]
        .iter()
        .find_map(|prefix| rest.strip_prefix(prefix))?;

    let tokens = shell_split(after_tool);
    let mut pattern: Option<String> = None;
    let mut targets: Vec<String> = Vec::new();
    let mut globs: Vec<String> = Vec::new();
    let mut ty: Option<String> = None;
    let mut iter = tokens.into_iter().peekable();
    while let Some(tok) = iter.next() {
        if tok.starts_with('-') {
            if (tok == "-e" || tok == "--regexp") && pattern.is_none() {
                if let Some(p) = iter.next() {
                    pattern = Some(p);
                }
            } else if let Some(p) = tok.strip_prefix("--regexp=") {
                if pattern.is_none() {
                    pattern = Some(p.to_string());
                }
            // An include glob narrows the search to a file set, which is
            // stronger evidence about what is being searched than the root the
            // walk starts from: `grep -rn foo --include='*.md' .` is a docs
            // search whatever `.` contains. Both spellings, and both the
            // separate-token and `=`-joined forms.
            } else if tok == "--include" || tok == "-g" || tok == "--glob" || tok == "--iglob" {
                if let Some(g) = iter.next() {
                    globs.push(g);
                }
            } else if let Some(g) = tok
                .strip_prefix("--include=")
                .or_else(|| tok.strip_prefix("--glob="))
                .or_else(|| tok.strip_prefix("--iglob="))
            {
                globs.push(g.to_string());
            } else if tok == "-t" || tok == "--type" {
                // First filter wins, mirroring the single `type` field the
                // native `Grep` tool carries.
                if let Some(t) = iter.next() {
                    ty.get_or_insert(t);
                }
            } else if let Some(t) = tok.strip_prefix("--type=") {
                ty.get_or_insert(t.to_string());
            // Value-taking flags whose argument is not a glob. Consuming the
            // value keeps it from being misread as the pattern or a target.
            } else if tok == "--exclude"
                || tok == "--exclude-dir"
                || tok == "-T"
                || tok == "--type-not"
            {
                iter.next();
            }
            continue;
        }
        if pattern.is_none() {
            pattern = Some(tok);
        } else {
            targets.push(tok);
        }
    }

    Some(GrepInvocation {
        pattern: pattern?,
        targets,
        globs,
        ty,
    })
}

/// Result of peeling leading noise off a command. `rest` is the command with
/// `rtk`/`sudo`/`time`/`nice` wrappers, `NAME=value` assignments, and a leading
/// `cd … &&`/`cd … ;` removed. `disables_hook` is true when one of those leading
/// assignments explicitly set `TOKENSAVE_DISABLE_GREP_HOOK` to a truthy value,
/// so the caller can honor a deliberate inline opt-out.
struct StrippedCommand<'a> {
    rest: &'a str,
    disables_hook: bool,
    /// The effective working directory after a leading `cd` (if any). Only the
    /// first leading `cd` is tracked; subsequent `cd` prefixes are stripped as
    /// noise but ignored because the hook does not model the shell's cwd.
    cd_target: Option<&'a str>,
}

/// Peel leading noise that hides a code search: the `rtk`/`sudo`/`time`/`nice`
/// wrappers, `NAME=value` environment assignments, and a leading `cd … &&` /
/// `cd … ;` prefix. Applied repeatedly so combinations unwrap (for example
/// `cd src && FOO=1 grep …`). A pipeline (`… | grep`) is intentionally NOT
/// unwrapped, so piped grep still passes through. An inline
/// `TOKENSAVE_DISABLE_GREP_HOOK=<truthy>` is recorded rather than treated as
/// ordinary noise, so an explicit inline opt-out is honored exactly like the
/// exported one instead of being stripped and then blocked.
fn strip_command_prefixes(command: &str) -> StrippedCommand<'_> {
    let mut rest = command.trim_start();
    let mut disables_hook = false;
    let mut cd_target: Option<&str> = None;
    loop {
        let mut advanced = false;

        for prefix in ["rtk ", "sudo ", "time ", "nice "] {
            if let Some(after) = rest.strip_prefix(prefix) {
                rest = after.trim_start();
                advanced = true;
            }
        }

        if let Some((name, value, after)) = parse_leading_env_assignment(rest) {
            // Mirror the shell's "last assignment wins": a later reassignment of
            // the opt-out var overrides an earlier one in either direction.
            if name == "TOKENSAVE_DISABLE_GREP_HOOK" {
                disables_hook = disable_value_is_truthy(unquote(value));
            }
            rest = after.trim_start();
            advanced = true;
        }

        if let Some((cd_arg, after)) = strip_leading_cd(rest) {
            // Only the first leading cd is modeled. A later cd is stripped as
            // prefix noise but ignored, so we do not pretend to know the shell's
            // effective cwd after a directory change sequence.
            if cd_target.is_none() {
                cd_target = Some(cd_arg);
            }
            rest = after.trim_start();
            advanced = true;
        }

        if !advanced {
            return StrippedCommand {
                rest,
                disables_hook,
                cd_target,
            };
        }
    }
}

/// Mirror `HookEnv::from_runtime`'s truthiness for `TOKENSAVE_DISABLE_GREP_HOOK`:
/// set, non-empty, not `0`, not `false` (case-insensitive).
fn disable_value_is_truthy(value: &str) -> bool {
    !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
}

/// Strip one layer of matching surrounding single or double quotes.
fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'\'' || b[0] == b'"') && b[b.len() - 1] == b[0] {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Strip backslash escapes used by a shell to protect the following character
/// (typically a space or glob metacharacter). On Windows a backslash is a path
/// separator, not an escape, so this is a no-op there.
fn unescape_shell_backslashes(s: &str) -> Cow<'_, str> {
    if cfg!(windows) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut changed = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
                changed = true;
                continue;
            }
        }
        out.push(c);
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(s)
    }
}

/// If `s` begins with a `NAME=value` assignment followed by another token,
/// return `(name, value, remainder)`. `value` may be single/double quoted and
/// is returned verbatim. Returns `None` when there is no trailing command
/// (nothing to search), so a bare `FOO=bar` is left alone.
fn parse_leading_env_assignment(s: &str) -> Option<(&str, &str, &str)> {
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut eq_pos = None;
    for (idx, c) in chars {
        if c == '=' {
            eq_pos = Some(idx);
            break;
        }
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
    }
    let eq = eq_pos?;
    let value_start = eq + 1;

    let mut in_single = false;
    let mut in_double = false;
    for (idx, c) in s[value_start..].char_indices() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                let value = &s[value_start..value_start + idx];
                return Some((&s[..eq], value, &s[value_start + idx..]));
            }
            _ => {}
        }
    }
    None
}

/// If `s` begins with a `cd …` command terminated by a top-level `&&` or `;`,
/// return the remainder after that separator. Returns `None` when the first
/// top-level separator is a pipe (so `cd x && ls | grep` still passes through)
/// or when there is no separator at all.
fn strip_leading_cd(s: &str) -> Option<(&str, &str)> {
    let after = s.strip_prefix("cd")?;
    if !after.starts_with(char::is_whitespace) {
        return None;
    }

    let mut in_single = false;
    let mut in_double = false;
    let mut iter = s.char_indices().peekable();
    while let Some((idx, c)) = iter.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            }
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            '|' => return None,
            ';' => {
                let cd_arg = extract_cd_argument(s)?;
                return Some((cd_arg, &s[idx + c.len_utf8()..]));
            }
            '&' => {
                if let Some(&(idx2, '&')) = iter.peek() {
                    let cd_arg = extract_cd_argument(s)?;
                    return Some((cd_arg, &s[idx2 + 1..]));
                }
                return None;
            }
            _ => {}
        }
    }
    None
}
/// Extract the path argument from a `cd <path>` command string.
/// Returns the raw (unexpanded) path, or `None` if the cd has no argument.
fn extract_cd_argument(s: &str) -> Option<&str> {
    let after = s.strip_prefix("cd")?.trim_start();
    if after.is_empty() {
        return None;
    }
    // Find the end of the path: up to the first unquoted `&&` or `;`.
    let mut in_single = false;
    let mut in_double = false;
    let mut iter = after.char_indices().peekable();
    let mut last_end = after.len();
    while let Some((idx, c)) = iter.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            }
            continue;
        }
        match c {
            '\'' => in_single = true,
            '"' => in_double = true,
            ';' | '|' => {
                last_end = idx;
                break;
            }
            '&' => {
                if let Some(&(_, '&')) = iter.peek() {
                    last_end = idx;
                    break;
                }
            }
            _ => {}
        }
    }
    let path = after[..last_end].trim_end();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Minimal shell tokenizer covering single/double quotes and backslash
/// escapes. Stops at unquoted pipe / semicolon / redirect / background — the
/// pattern always appears before any of those in a normal grep invocation.
///
/// On Windows, `\` is a path separator (not an escape) in the unquoted context,
/// so absolute targets like `C:\Users\me\project` survive intact for the
/// project-root classifier. Inside double quotes `\` still escapes only before
/// `"`, `\`, `$`, and `` ` `` on every platform.
fn shell_split(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
        } else if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                if let Some(&next) = chars.peek() {
                    if matches!(next, '"' | '\\' | '$' | '`') {
                        chars.next();
                        cur.push(next);
                        continue;
                    }
                }
                cur.push(c);
            } else {
                cur.push(c);
            }
        } else {
            match c {
                '\'' => in_single = true,
                '"' => in_double = true,
                '\\' => {
                    if cfg!(windows) {
                        // On Windows `\` is a path separator, not an escape;
                        // treating it as one would strip every backslash from
                        // an absolute target and break the root classifier.
                        cur.push(c);
                    } else if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                }
                '|' | ';' | '&' | '>' | '<' => break,
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                c => cur.push(c),
            }
        }
    }

    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn is_code_research_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let exploration_patterns = [
        "explore",
        "codebase structure",
        "codebase architecture",
        "codebase overview",
        "source files contents",
        "read every",
        "full contents",
        "entire codebase",
        "architecture and structure",
        "call graph",
        "call path",
        "call chain",
        "symbol relat",
        "symbol lookup",
        "who calls",
        "callers of",
        "callees of",
    ];
    exploration_patterns.iter().any(|pat| lower.contains(pat))
}

/// Kiro `preToolUse` hook handler.
///
/// Kiro sends the hook event JSON on stdin. Returning exit code 2 blocks the
/// tool call and sends stderr back to the model. This is intentionally separate
/// from Claude's hook handler because Claude expects a JSON decision on stdout.
pub fn hook_kiro_pre_tool_use() -> i32 {
    let event = read_stdin_to_string();
    if let Some(reason) = evaluate_kiro_pre_tool_use_with_env(&event, &HookEnv::from_runtime()) {
        eprintln!("{reason}");
        2
    } else {
        0
    }
}

/// Pure decision logic for Kiro `preToolUse` hook events.
///
/// Returns a block reason only for Kiro delegation/subagent tool calls whose
/// task text looks like codebase research that tokensave MCP tools should
/// answer first.
pub fn evaluate_kiro_pre_tool_use(event_json: &str) -> Option<&'static str> {
    evaluate_kiro_pre_tool_use_with_env(event_json, &HookEnv::from_runtime())
}

/// [`evaluate_kiro_pre_tool_use`] with an explicit environment snapshot.
///
/// Gated like the Claude agent path: no `.tokensave` index means there is
/// nothing to redirect to, and the opt-out env var suppresses the block. The
/// index is resolved from the event's `cwd` when it reports one, matching the
/// `postToolUse` sync hook.
pub fn evaluate_kiro_pre_tool_use_with_env(
    event_json: &str,
    env: &HookEnv,
) -> Option<&'static str> {
    if env.disable_grep_hook {
        return None;
    }
    let parsed: Value = serde_json::from_str(event_json).ok()?;
    if !env.for_event(&parsed).in_tokensave_project {
        return None;
    }
    let tool_name = parsed.get("tool_name").and_then(Value::as_str)?;
    if !is_kiro_delegation_tool(tool_name) {
        return None;
    }

    if kiro_event_has_research_text(parsed.get("tool_input").unwrap_or(&Value::Null)) {
        Some(TOKENSAVE_RESEARCH_BLOCK_REASON)
    } else {
        None
    }
}

fn is_kiro_delegation_tool(tool_name: &str) -> bool {
    matches!(tool_name, "delegate" | "subagent" | "use_subagent")
}

/// Factory Droid `PreToolUse` hook handler.
///
/// Droid delivers the hook event as JSON on stdin with the tool payload
/// nested under `tool_input` — the same envelope shape Claude Code uses —
/// but blocks a tool call via **exit code 2 + stderr**, the same mechanism
/// Kiro uses (not Claude's stdout JSON decision). The install side registers
/// this hook for the `^(Execute|Grep|Task)$` matcher, so symbol-shaped shell
/// searches (`Execute`), native content searches (`Grep`), and typed subagent
/// launches (`Task`) reach the shared classifier. `Read`/`LS`/`Glob` remain
/// excluded, and this hook fails open for anything it isn't told to inspect.
pub fn hook_droid_pre_tool_use() -> i32 {
    let event = read_stdin_to_string();
    if let Some(reason) = evaluate_droid_pre_tool_use(&event) {
        eprintln!("{reason}");
        2
    } else {
        0
    }
}

/// Pure decision logic for Droid `PreToolUse` hook events, using the real
/// process environment.
pub fn evaluate_droid_pre_tool_use(raw: &str) -> Option<String> {
    evaluate_droid_pre_tool_use_with_env(raw, &HookEnv::from_runtime())
}

/// Pure decision logic for Droid `PreToolUse` hook events with an explicit
/// environment snapshot. Tests use this to avoid touching the real process
/// state.
///
/// Unwraps the nested `tool_input` object (falling back to treating the whole
/// payload as the tool input if it isn't wrapped), normalizes Droid's exact
/// lowercase built-in `explorer` type to the shared core's `Explore` sentinel,
/// and delegates to the same [`evaluate_hook_decision_core`] the Claude and
/// Kiro adapters share. Other `Task` subagent types pass without entering the
/// generic research-prompt classifier.
/// Returns the raw block reason text for the caller to print to stderr —
/// Droid's channel is exit code + stderr, not a stdout decision object.
pub fn evaluate_droid_pre_tool_use_with_env(raw: &str, env: &HookEnv) -> Option<String> {
    let Ok(event) = serde_json::from_str::<Value>(raw) else {
        return evaluate_hook_decision_core(raw, env);
    };
    let is_task = event.get("tool_name").and_then(Value::as_str) == Some("Task");
    let env = env.for_event(&event);
    let mut tool_input = event.get("tool_input").cloned().unwrap_or(event);
    let subagent_type = tool_input.get("subagent_type").and_then(Value::as_str);

    if is_task && subagent_type != Some("explorer") {
        return None;
    }
    if subagent_type == Some("explorer") {
        tool_input["subagent_type"] = Value::String("Explore".to_string());
    }

    evaluate_hook_decision_core(&tool_input.to_string(), &env)
}

fn kiro_event_has_research_text(value: &Value) -> bool {
    let mut text = Vec::new();
    collect_kiro_task_strings(value, &mut text);
    if text.is_empty() {
        collect_strings(value, &mut text);
    }
    text.iter().any(|s| is_code_research_prompt(s))
}

fn collect_kiro_task_strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let key = key.to_ascii_lowercase();
                if key.contains("prompt")
                    || key.contains("task")
                    || key.contains("query")
                    || key.contains("instruction")
                    || key.contains("message")
                    || key.contains("description")
                {
                    collect_strings(child, out);
                } else {
                    collect_kiro_task_strings(child, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_kiro_task_strings(item, out);
            }
        }
        Value::String(s) => out.push(s),
        _ => {}
    }
}

fn collect_strings<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
    match value {
        Value::String(s) => out.push(s),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        }
        Value::Object(map) => {
            for child in map.values() {
                collect_strings(child, out);
            }
        }
        _ => {}
    }
}

/// `UserPromptSubmit` hook handler: resets the per-session local counter.
///
/// Token savings are now reported inline in each MCP tool response,
/// so this hook only needs to reset the counter for the new turn.
pub async fn hook_prompt_submit() {
    let project_path = crate::config::resolve_path(None);
    if let Ok(cg) = crate::tokensave::TokenSave::open(&project_path).await {
        let _ = cg.reset_local_counter().await;
    }
}

/// Kiro `userPromptSubmit` hook handler.
///
/// Kiro adds successful hook stdout to context, so this handler stays silent.
pub async fn hook_kiro_prompt_submit() -> i32 {
    let event = read_stdin_to_string();
    reset_counter_for_kiro_event(&event).await;
    0
}

/// Kiro `postToolUse` hook handler used to keep the graph fresh after writes.
///
/// The installed Kiro agent maps this to `fs_write`. The hook discovers the
/// nearest initialized tokensave project from Kiro's `cwd` field and runs a
/// silent incremental sync. Missing indexes and concurrent syncs are no-ops.
pub async fn hook_kiro_post_tool_use() -> i32 {
    let event = read_stdin_to_string();
    match sync_for_kiro_event(&event).await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tokensave sync failed: {e}");
            1
        }
    }
}

async fn reset_counter_for_kiro_event(event_json: &str) {
    let Some(project_root) = kiro_project_root(event_json) else {
        return;
    };
    if let Ok(cg) = crate::tokensave::TokenSave::open(&project_root).await {
        let _ = cg.reset_local_counter().await;
    }
}

async fn sync_for_kiro_event(event_json: &str) -> crate::errors::Result<()> {
    let Some(project_root) = kiro_project_root(event_json) else {
        return Ok(());
    };
    let cg = crate::tokensave::TokenSave::open(&project_root).await?;
    match cg.sync().await {
        Ok(_) | Err(crate::errors::TokenSaveError::SyncLock { .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

fn kiro_project_root(event_json: &str) -> Option<PathBuf> {
    let cwd = kiro_event_cwd(event_json).or_else(|| std::env::current_dir().ok())?;
    crate::config::discover_project_root(&cwd)
}

fn kiro_event_cwd(event_json: &str) -> Option<PathBuf> {
    event_cwd(&serde_json::from_str::<Value>(event_json).ok()?)
}

/// The session working directory a harness reports at the top level of its
/// hook event. Kiro, Claude Code, and Factory Droid all use the `cwd` key.
///
/// The value is agent-supplied, so only an absolute path to an existing
/// directory is accepted; anything else yields `None` and leaves the caller on
/// the hook process's own directory. A relative or stale `cwd` would otherwise
/// start the ancestor walk somewhere neither the agent nor the user meant.
fn event_cwd(event: &Value) -> Option<PathBuf> {
    let path = Path::new(event.get("cwd").and_then(Value::as_str)?.trim());
    (path.is_absolute() && path.is_dir()).then(|| path.to_path_buf())
}

fn read_stdin_to_string() -> String {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    input
}

/// `Stop` hook handler: ingests new session data and prints a cost receipt.
///
/// Parses any new JSONL lines from Claude Code sessions, inserts them into
/// the global DB, and prints a one-line summary to stderr showing the
/// session cost, tokens saved, and efficiency ratio.
pub async fn hook_stop() {
    let Some(gdb) = crate::global_db::GlobalDb::open().await else {
        return;
    };

    let stats = crate::accounting::parser::ingest_claude_only(&gdb).await;
    if stats.turns_inserted == 0 {
        return;
    }

    // Read tokens saved for efficiency calculation
    let project_path = crate::config::resolve_path(None);
    let tokens_saved = if let Ok(cg) = crate::tokensave::TokenSave::open(&project_path).await {
        cg.get_tokens_saved().await.unwrap_or(0)
    } else {
        0
    };

    let efficiency = if tokens_saved + stats.tokens_consumed > 0 {
        (tokens_saved as f64 / (tokens_saved + stats.tokens_consumed) as f64) * 100.0
    } else {
        0.0
    };

    let saved_str = crate::display::format_token_count(tokens_saved);

    // Print to stderr so it appears in the terminal but doesn't interfere
    // with stdout (which Claude Code may parse).
    if stats.cost_usd >= 0.001 {
        eprintln!(
            "\x1b[36mSession: ${:.2} spent | {saved_str} saved | {efficiency:.0}% efficiency\x1b[0m",
            stats.cost_usd
        );
    }
}

#[cfg(test)]
mod cursor_decision_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::{build_allow_message, build_block_message};
    use serde_json::Value;

    #[test]
    fn allow_message_carries_cursor_permission_field() {
        // Cursor gates the tool on the flat `permission` field; a payload
        // without it fails closed and reports "returned invalid JSON".
        let v: Value = serde_json::from_str(&build_allow_message()).unwrap();
        assert_eq!(v["permission"].as_str(), Some("allow"));
    }

    #[test]
    fn block_message_is_cross_harness() {
        let v: Value = serde_json::from_str(&build_block_message("use tokensave instead")).unwrap();
        // Cursor-native gate + surfaced reason.
        assert_eq!(v["permission"].as_str(), Some("deny"));
        assert_eq!(v["user_message"].as_str(), Some("use tokensave instead"));
        assert_eq!(v["agent_message"].as_str(), Some("use tokensave instead"));
        // Claude Code's nested contract stays intact.
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecision"].as_str(),
            Some("deny")
        );
        assert_eq!(
            v["hookSpecificOutput"]["permissionDecisionReason"].as_str(),
            Some("use tokensave instead")
        );
    }
}

#[cfg(test)]
mod project_root_target_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::{path_is_project_root_with_home, path_is_within_project_with_home};

    fn indexed_project() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        (tmp, root)
    }

    #[test]
    fn tilde_alone_resolves_to_the_project_root() {
        let (_tmp, root) = indexed_project();
        assert!(path_is_project_root_with_home(
            "~",
            Some(&root),
            Some(&root)
        ));
    }

    #[test]
    fn tilde_subpath_resolves_to_the_project_root() {
        let (tmp, root) = indexed_project();
        assert!(path_is_project_root_with_home(
            "~/project",
            Some(&root),
            Some(tmp.path())
        ));
    }

    #[test]
    fn tilde_subpath_below_the_root_is_not_the_root() {
        let (tmp, root) = indexed_project();
        std::fs::create_dir_all(root.join("src")).unwrap();
        assert!(!path_is_project_root_with_home(
            "~/project/src",
            Some(&root),
            Some(tmp.path())
        ));
    }

    #[test]
    fn tilde_without_a_known_home_fails_open() {
        let (_tmp, root) = indexed_project();
        assert!(!path_is_project_root_with_home("~", Some(&root), None));
    }

    #[test]
    fn other_users_home_is_not_expanded() {
        let (tmp, root) = indexed_project();
        assert!(!path_is_project_root_with_home(
            "~someone/project",
            Some(&root),
            Some(tmp.path())
        ));
    }

    #[test]
    fn quoted_absolute_root_still_matches() {
        let (_tmp, root) = indexed_project();
        let quoted = format!("\"{}\"", root.display());
        assert!(path_is_project_root_with_home(
            &quoted,
            Some(&root),
            Some(&root)
        ));
    }

    #[test]
    fn relative_targets_are_left_to_the_other_rules() {
        let (tmp, root) = indexed_project();
        assert!(!path_is_project_root_with_home(
            "project",
            Some(&root),
            Some(tmp.path())
        ));
    }

    // --- #435: path_is_within_project tests ---

    #[test]
    fn within_project_absolute_root_matches() {
        let (_tmp, root) = indexed_project();
        assert!(path_is_within_project_with_home(
            root.to_str().unwrap(),
            Some(&root),
            None
        ));
    }

    #[test]
    fn within_project_absolute_subdir_matches() {
        let (_tmp, root) = indexed_project();
        std::fs::create_dir_all(root.join("src")).unwrap();
        let subdir = root.join("src");
        assert!(path_is_within_project_with_home(
            subdir.to_str().unwrap(),
            Some(&root),
            None
        ));
    }

    #[test]
    fn within_project_absolute_outside_does_not_match() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        assert!(!path_is_within_project_with_home(
            other.to_str().unwrap(),
            Some(&root),
            None
        ));
    }

    #[test]
    fn within_project_relative_subdir_resolves_against_root() {
        let (_tmp, root) = indexed_project();
        std::fs::create_dir_all(root.join("src")).unwrap();
        assert!(path_is_within_project_with_home("src", Some(&root), None));
    }

    #[test]
    fn within_project_relative_outside_does_not_match() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("project");
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        // "../other" resolves outside the project root
        assert!(!path_is_within_project_with_home(
            "../other",
            Some(&root),
            None
        ));
    }

    #[test]
    fn within_project_tilde_resolves_to_project() {
        let (tmp, root) = indexed_project();
        assert!(path_is_within_project_with_home(
            "~/project",
            Some(&root),
            Some(tmp.path())
        ));
    }

    #[test]
    fn within_project_unresolvable_path_returns_false() {
        let (_tmp, root) = indexed_project();
        // A path that doesn't exist and cannot be canonicalized
        assert!(!path_is_within_project_with_home(
            "/nonexistent/path/that/does/not/exist",
            Some(&root),
            None
        ));
    }

    #[test]
    fn within_project_no_root_returns_false() {
        let (_tmp, _root) = indexed_project();
        assert!(!path_is_within_project_with_home("/anywhere", None, None));
    }

    #[test]
    fn within_project_other_users_home_is_not_expanded() {
        let (tmp, root) = indexed_project();
        // `~someone` is not expanded; it is treated as a literal path relative
        // to the root, which does not exist and therefore cannot be inside.
        assert!(!path_is_within_project_with_home(
            "~someone/project",
            Some(&root),
            Some(tmp.path())
        ));
    }
}
