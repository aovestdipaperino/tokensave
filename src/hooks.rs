//! Hook handlers for Claude Code and Kiro integrations.
//!
//! These functions are invoked by Claude Code's hook system to intercept
//! tool calls, redirect exploration work to tokensave MCP tools, and
//! track per-session token savings. Kiro invokes its own handlers with hook
//! events on stdin and expects blocking decisions through process exit codes.

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
    "pyi", "pyw", "c", "h", "cpp", "cc", "cxx", "c++", "hpp", "hh", "hxx", "h++", "ipp", "tcc",
    "kt", "kts", "cs", "csx", "swift", // Medium tier
    "dart", "pas", "pp", "dpr", "php", "phtml", "rb", "rake", "gemspec", "sh", "bash", "zsh",
    "proto", "ps1", "psm1", "psd1", "nix", "vb", "vbs", // Full tier
    "lua", "zig", "m", "mm", "pl", "pm", "bat", "cmd", "f", "f90", "f95", "f03", "for", "ftn",
    "cbl", "cob", "cpy", "bas",
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
    /// `true` when the current working directory contains a usable tokensave
    /// index (`.tokensave/tokensave.db`). Without an index there is nothing
    /// to redirect to, so the hook always passes through.
    pub cwd_has_tokensave_db: bool,

    /// `true` when the user has opted out for this invocation via
    /// `TOKENSAVE_DISABLE_GREP_HOOK=1`.
    pub disable_grep_hook: bool,
}

impl HookEnv {
    /// Snapshot the real environment.
    pub fn from_runtime() -> Self {
        let cwd_has_tokensave_db = std::env::current_dir()
            .ok()
            .is_some_and(|c| c.join(".tokensave").join("tokensave.db").exists());
        let disable_grep_hook = std::env::var("TOKENSAVE_DISABLE_GREP_HOOK")
            .is_ok_and(|v| !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"));
        Self {
            cwd_has_tokensave_db,
            disable_grep_hook,
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
    if !decision.is_empty() {
        println!("{decision}");
    }
}

/// Parse Claude Code's `PreToolUse` stdin JSON and return the decision string.
///
/// Unwraps the nested `tool_input` object before delegating to
/// [`evaluate_hook_decision`]. If the payload isn't the expected wrapper shape,
/// falls back to treating `raw` as a flat tool-input object.
pub fn evaluate_claude_pre_tool_use(raw: &str) -> String {
    let tool_input = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("tool_input").cloned())
        .map(|ti| ti.to_string())
        .unwrap_or_else(|| raw.to_string());
    evaluate_hook_decision(&tool_input)
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
    let parsed: serde_json::Value =
        serde_json::from_str(tool_input).unwrap_or_else(|_| serde_json::json!({}));

    // Block Explore agents outright.
    if parsed.get("subagent_type").and_then(|v| v.as_str()) == Some("Explore") {
        return build_block_message(TOKENSAVE_RESEARCH_BLOCK_REASON);
    }

    // Block exploration-style Agent prompts.
    if let Some(prompt) = parsed.get("prompt").and_then(|v| v.as_str()) {
        if is_code_research_prompt(prompt) {
            return build_block_message(TOKENSAVE_RESEARCH_BLOCK_REASON);
        }
    }

    // Grep tool — `pattern` is the discriminating field.
    if parsed.get("pattern").is_some() {
        if let Some(reason) = evaluate_grep_tool_input(&parsed, env) {
            return build_block_message(&reason);
        }
    }

    // Bash tool — `command` is the discriminating field.
    if let Some(command) = parsed.get("command").and_then(|v| v.as_str()) {
        if let Some(reason) = evaluate_bash_command(command, env) {
            return build_block_message(&reason);
        }
    }

    // Empty string = no output -> Claude Code implicitly allows the tool call.
    String::new()
}

fn build_block_message(reason: &str) -> String {
    serde_json::json!({
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
    if !env.cwd_has_tokensave_db || env.disable_grep_hook {
        return None;
    }
    let pattern = parsed.get("pattern").and_then(|v| v.as_str())?;
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
        return None;
    }
    let output_mode = parsed
        .get("output_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("content");
    if matches!(output_mode, "files_with_matches" | "count") {
        return None;
    }
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let glob = parsed.get("glob").and_then(|v| v.as_str()).unwrap_or("");
    let ty = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if !target_looks_like_code(path, glob, ty) {
        return None;
    }
    let shape = classify_symbol_pattern(pattern)?;
    Some(redirect_message("Grep", pattern, shape))
}

/// Inspect a `Bash` tool command. Returns `Some(reason)` to redirect.
fn evaluate_bash_command(command: &str, env: &HookEnv) -> Option<String> {
    if !env.cwd_has_tokensave_db || env.disable_grep_hook {
        return None;
    }
    let inv = extract_grep_invocation(command)?;
    if inv.pattern.is_empty() || inv.pattern.len() > MAX_PATTERN_LEN {
        return None;
    }
    let target = inv.targets.first().map_or("", String::as_str);
    if !target_looks_like_code(target, "", "") {
        return None;
    }
    let shape = classify_symbol_pattern(&inv.pattern)?;
    Some(redirect_message("Bash grep", &inv.pattern, shape))
}

fn redirect_message(tool_label: &str, pattern: &str, shape: PatternShape) -> String {
    let suggestion = match shape {
        PatternShape::BareSymbol | PatternShape::WordBoundary => {
            "tokensave_search (definition) or tokensave_callers_for (usages)"
        }
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
        return None;
    }

    match (parts.len(), had_wb) {
        (1, true) => Some(PatternShape::WordBoundary),
        (1, false) => Some(PatternShape::BareSymbol),
        _ => Some(PatternShape::Alternation),
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
fn target_looks_like_code(path: &str, glob: &str, ty: &str) -> bool {
    if !ty.is_empty() {
        return CODE_TYPE_FILTERS.contains(&ty.to_ascii_lowercase().as_str());
    }

    let raw = if path.is_empty() { glob } else { path };
    let trimmed = raw.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if trimmed.is_empty() || trimmed == "." || trimmed == "./" {
        return true;
    }

    // Extension path: only block when the extension is in our supported list.
    if let Some(idx) = trimmed.rfind('.') {
        let after_dot = &trimmed[idx + 1..];
        let ext: String = after_dot
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '+')
            .collect::<String>()
            .to_ascii_lowercase();
        if !ext.is_empty() {
            return CODE_EXTENSIONS.contains(&ext.as_str());
        }
    }

    // No extension — treat as a directory. Block only when the last path
    // component is a recognized code root.
    let last = trimmed
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    CODE_DIRS.contains(&last)
}

#[derive(Debug)]
struct GrepInvocation {
    pattern: String,
    targets: Vec<String>,
}

/// Parse a bash command that *starts* with `grep`, `rg`, or `ag` (optionally
/// preceded by `rtk ` or `sudo `). Returns `None` for anything else, including
/// piped commands like `ls | grep foo` — we deliberately don't try to handle
/// those.
fn extract_grep_invocation(command: &str) -> Option<GrepInvocation> {
    let mut rest = command.trim();
    for prefix in ["rtk ", "sudo ", "time ", "nice "] {
        if let Some(after) = rest.strip_prefix(prefix) {
            rest = after.trim_start();
        }
    }

    // Identify the tool. `git grep` is intentionally excluded — it searches
    // git history, which tokensave does not index.
    let after_tool = ["grep ", "rg ", "ag "]
        .iter()
        .find_map(|prefix| rest.strip_prefix(prefix))?;

    let tokens = shell_split(after_tool);
    let mut pattern: Option<String> = None;
    let mut targets: Vec<String> = Vec::new();
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
    })
}

/// Minimal shell tokenizer covering single/double quotes and backslash
/// escapes. Stops at unquoted pipe / semicolon / redirect / background — the
/// pattern always appears before any of those in a normal grep invocation.
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
                    if let Some(next) = chars.next() {
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
    if let Some(reason) = evaluate_kiro_pre_tool_use(&event) {
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
    let parsed: Value = serde_json::from_str(event_json).ok()?;
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
    let parsed: Value = serde_json::from_str(event_json).ok()?;
    let cwd = parsed.get("cwd").and_then(Value::as_str)?;
    let path = Path::new(cwd);
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path.to_path_buf())
    }
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

    let stats = crate::accounting::parser::ingest(&gdb).await;
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
