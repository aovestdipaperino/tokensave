//! Classification of the search command that `tokensave grep` wraps (#688).
//!
//! The command runs unchanged. This module only reads its arguments to decide
//! whether the output has `path:line:` hits that can carry index facts.

use std::ffi::OsString;
use std::path::Path;

/// Maximum number of identifier tokens kept from the search patterns.
const MAX_TOKENS: usize = 32;

/// The search tool that a wrapped command starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// `grep`, `egrep` or `fgrep`.
    Grep,
    /// `rg`.
    Ripgrep,
    /// `git grep`.
    GitGrep,
}

/// What `tokensave grep` does with the output of the wrapped command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchMode {
    /// Run the command unchanged and copy its output.
    Passthrough,
    /// Add index facts to the `path:line:` hits.
    Annotate(AnnotatePlan),
}

/// The parts of a search command that hit annotation needs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnotatePlan {
    /// Position in the command where `-n` goes. `None` when the command
    /// already prints line numbers.
    pub line_number_flag_at: Option<usize>,
    /// Identifier tokens of the search patterns.
    pub tokens: Vec<String>,
    /// Whether the search ignores letter case.
    pub ignore_case: bool,
    /// The one path operand of a `grep` or `rg` command. Such a command
    /// prints `line:` hits with no path when the operand is a file.
    pub single_operand: Option<String>,
    /// Directories from `git -C`, in command order.
    pub git_dirs: Vec<String>,
}

/// The result of [`classify_search`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPlan {
    /// The tool that the command starts.
    pub tool: Tool,
    /// How to treat the output.
    pub mode: SearchMode,
}

/// A command that `tokensave grep` does not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The flags of one tool that change how its arguments or output are read.
struct FlagTable {
    /// Short flags that take a value.
    short_value: &'static str,
    /// Long flags that take a value in the next argument.
    long_value: &'static [&'static str],
    /// Long flags that take a value in the next argument only when that
    /// argument is a number. Some grep builds make this value optional.
    long_number: &'static [&'static str],
    /// Short flags whose output has no `path:line:` hits.
    short_passthrough: &'static str,
    /// Long flags whose output has no `path:line:` hits.
    long_passthrough: &'static [&'static str],
    /// Short flags that make the search ignore letter case.
    short_ignore_case: &'static str,
}

const GREP_FLAGS: FlagTable = FlagTable {
    short_value: "efmABCdD",
    long_value: &[
        "regexp",
        "file",
        "include",
        "exclude",
        "exclude-dir",
        "exclude-from",
        "label",
        "binary-files",
        "devices",
        "directories",
        "group-separator",
    ],
    long_number: &["max-count", "after-context", "before-context", "context"],
    short_passthrough: "lLcqZzVT",
    long_passthrough: &[
        "files-with-matches",
        "files-without-match",
        "count",
        "quiet",
        "silent",
        "null",
        "null-data",
        "initial-tab",
        "help",
        "version",
    ],
    short_ignore_case: "iy",
};

const RIPGREP_FLAGS: FlagTable = FlagTable {
    short_value: "efgtTmABCjMEdr",
    long_value: &[
        "regexp",
        "file",
        "glob",
        "iglob",
        "type",
        "type-not",
        "type-add",
        "type-clear",
        "max-count",
        "after-context",
        "before-context",
        "context",
        "threads",
        "max-columns",
        "encoding",
        "max-depth",
        "maxdepth",
        "replace",
        "max-filesize",
        "color",
        "colors",
        "sort",
        "sortr",
        "path-separator",
        "context-separator",
        "field-context-separator",
        "field-match-separator",
        "pre",
        "pre-glob",
        "engine",
        "ignore-file",
        "dfa-size-limit",
        "regex-size-limit",
        "hostname-bin",
        "hyperlink-format",
        "generate",
    ],
    long_number: &[],
    short_passthrough: "lcq0NhVp",
    long_passthrough: &[
        "files-with-matches",
        "files-without-match",
        "count",
        "count-matches",
        "quiet",
        "files",
        "json",
        "null",
        "null-data",
        "heading",
        "no-line-number",
        "field-match-separator",
        "hyperlink-format",
        "pretty",
        "type-list",
        "generate",
        "pcre2-version",
        "help",
        "version",
    ],
    short_ignore_case: "i",
};

const GIT_GREP_FLAGS: FlagTable = FlagTable {
    short_value: "efABCm",
    long_value: &["max-depth", "threads"],
    long_number: &["max-count", "after-context", "before-context", "context"],
    short_passthrough: "lLcqzO",
    long_passthrough: &[
        "files-with-matches",
        "name-only",
        "files-without-match",
        "count",
        "quiet",
        "null",
        "heading",
        "open-files-in-pager",
        "no-line-number",
        "help",
    ],
    short_ignore_case: "i",
};

/// Global `git` options that take a value in the next argument.
const GIT_GLOBAL_VALUE: &[&str] = &[
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--super-prefix",
    "--config-env",
];

/// What the arguments of one search command say.
#[derive(Default)]
struct Seen {
    patterns: Vec<String>,
    pattern_flag: bool,
    operands: Vec<String>,
    line_numbers: bool,
    passthrough: bool,
    ignore_case: bool,
    smart_case: bool,
}

impl Seen {
    fn short(&mut self, flag: char, table: &FlagTable, tool: Tool) {
        if flag == 'n' {
            self.line_numbers = true;
        }
        if table.short_passthrough.contains(flag) {
            self.passthrough = true;
        }
        if table.short_ignore_case.contains(flag) {
            self.ignore_case = true;
        }
        if tool == Tool::Ripgrep && flag == 'S' {
            self.smart_case = true;
        }
    }

    fn short_value(&mut self, flag: char, value: Option<&str>) {
        match flag {
            'e' => {
                self.pattern_flag = true;
                self.patterns.extend(value.map(str::to_owned));
            }
            'f' => self.pattern_flag = true,
            _ => {}
        }
    }

    fn long(&mut self, name: &str, value: Option<&str>, table: &FlagTable) {
        if table.long_passthrough.contains(&name) {
            self.passthrough = true;
        }
        match name {
            "line-number" => self.line_numbers = true,
            "ignore-case" => self.ignore_case = true,
            "smart-case" => self.smart_case = true,
            "regexp" => {
                self.pattern_flag = true;
                self.patterns.extend(value.map(str::to_owned));
            }
            "file" => self.pattern_flag = true,
            // Colored output puts escape sequences in the path, so no line
            // is a `path:line:` hit.
            "color" | "colour" if matches!(value, Some("always" | "ansi")) => {
                self.passthrough = true;
            }
            _ => {}
        }
    }

    fn positional(&mut self, value: &str) {
        if !self.pattern_flag && self.patterns.is_empty() {
            self.patterns.push(value.to_owned());
        } else {
            self.operands.push(value.to_owned());
        }
    }
}

/// Reads the arguments that follow the tool name.
fn read_arguments(args: &[String], table: &FlagTable, tool: Tool) -> Seen {
    let mut seen = Seen::default();
    let mut only_operands = false;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        index += 1;
        if only_operands || arg == "-" || !arg.starts_with('-') {
            seen.positional(arg);
            continue;
        }
        if arg == "--" {
            only_operands = true;
            continue;
        }
        if let Some(long) = arg.strip_prefix("--") {
            let (name, mut value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            if value.is_none() {
                let next = args.get(index).map(String::as_str);
                let takes_next = table.long_value.contains(&name)
                    || (table.long_number.contains(&name)
                        && next.is_some_and(|next| {
                            !next.is_empty() && next.bytes().all(|b| b.is_ascii_digit())
                        }));
                if takes_next {
                    value = next;
                    index += 1;
                }
            }
            seen.long(name, value, table);
            continue;
        }
        let cluster = &arg[1..];
        for (position, flag) in cluster.char_indices() {
            if table.short_value.contains(flag) {
                let attached = &cluster[position + flag.len_utf8()..];
                let value = if attached.is_empty() {
                    let next = args.get(index).map(String::as_str);
                    index += 1;
                    next
                } else {
                    Some(attached)
                };
                seen.short_value(flag, value);
                break;
            }
            seen.short(flag, table, tool);
        }
    }
    seen
}

/// Returns the identifier tokens of a search pattern.
///
/// A backslash and the character after it are an escape (`\b`, `\w`, `\(`),
/// not part of a name.
pub fn pattern_tokens(pattern: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c.is_alphanumeric() || c == '_' {
            current.push(c);
            continue;
        }
        push_token(&mut tokens, &mut current);
    }
    push_token(&mut tokens, &mut current);
    tokens
}

fn push_token(tokens: &mut Vec<String>, current: &mut String) {
    let token = std::mem::take(current);
    let is_name = token
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_');
    if is_name && !tokens.contains(&token) {
        tokens.push(token);
    }
}

/// Returns the tool name of a command, without its directory and extension.
fn tool_name(program: &str) -> String {
    let name = Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase();
    name.strip_suffix(".exe")
        .map_or(name.clone(), str::to_owned)
}

/// Finds `grep` after the global options of `git`.
///
/// Returns the position of the first `git grep` argument and the `-C`
/// directories.
fn git_grep_arguments(args: &[String]) -> Result<(usize, Vec<String>), UsageError> {
    let mut dirs = Vec::new();
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        index += 1;
        if arg == "grep" {
            return Ok((index, dirs));
        }
        if !arg.starts_with('-') {
            break;
        }
        if GIT_GLOBAL_VALUE.contains(&arg.as_str()) {
            if arg == "-C" {
                dirs.extend(args.get(index).cloned());
            }
            index += 1;
        }
    }
    Err(UsageError(
        "tokensave grep runs `git grep` only, not other git commands".to_string(),
    ))
}

/// Decides how `tokensave grep` treats a search command.
///
/// `command` is the command as the caller wrote it: `grep …`, `rg …` or
/// `git grep …`. Any other command is a [`UsageError`].
pub fn classify_search(command: &[OsString]) -> Result<SearchPlan, UsageError> {
    let args: Vec<String> = command
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let Some(program) = args.first() else {
        return Err(UsageError(
            "tokensave grep needs a search command: grep, rg or git grep".to_string(),
        ));
    };
    let (tool, first_argument, git_dirs, table) = match tool_name(program).as_str() {
        "grep" | "egrep" | "fgrep" => (Tool::Grep, 1, Vec::new(), &GREP_FLAGS),
        "rg" => (Tool::Ripgrep, 1, Vec::new(), &RIPGREP_FLAGS),
        "git" => {
            let (first_argument, dirs) = git_grep_arguments(&args)?;
            (Tool::GitGrep, first_argument, dirs, &GIT_GREP_FLAGS)
        }
        other => {
            return Err(UsageError(format!(
                "tokensave grep runs grep, rg or git grep, not `{other}`"
            )));
        }
    };
    let seen = read_arguments(args.get(first_argument..).unwrap_or_default(), table, tool);
    if seen.passthrough {
        return Ok(SearchPlan {
            tool,
            mode: SearchMode::Passthrough,
        });
    }
    let mut tokens = Vec::new();
    for pattern in &seen.patterns {
        for token in pattern_tokens(pattern) {
            if tokens.len() < MAX_TOKENS && !tokens.contains(&token) {
                tokens.push(token);
            }
        }
    }
    // ripgrep's smart case ignores letter case when the pattern has no
    // uppercase letter.
    let smart = seen.smart_case
        && !seen
            .patterns
            .iter()
            .any(|pattern| pattern.chars().any(char::is_uppercase));
    let single_operand = match (tool, seen.operands.as_slice()) {
        (Tool::Grep | Tool::Ripgrep, [operand]) => Some(operand.clone()),
        _ => None,
    };
    Ok(SearchPlan {
        tool,
        mode: SearchMode::Annotate(AnnotatePlan {
            line_number_flag_at: (!seen.line_numbers).then_some(first_argument),
            tokens,
            ignore_case: seen.ignore_case || smart,
            single_operand,
            git_dirs,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(command: &[&str]) -> Result<SearchPlan, UsageError> {
        let command: Vec<OsString> = command.iter().map(OsString::from).collect();
        classify_search(&command)
    }

    fn annotate(command: &[&str]) -> AnnotatePlan {
        match classify(command) {
            Ok(SearchPlan {
                mode: SearchMode::Annotate(plan),
                ..
            }) => plan,
            other => panic!("{command:?} is not annotated: {other:?}"),
        }
    }

    fn is_passthrough(command: &[&str]) -> bool {
        matches!(
            classify(command),
            Ok(SearchPlan {
                mode: SearchMode::Passthrough,
                ..
            })
        )
    }

    #[test]
    fn names_the_tool() {
        assert_eq!(classify(&["grep", "x"]).map(|p| p.tool), Ok(Tool::Grep));
        assert_eq!(classify(&["egrep", "x"]).map(|p| p.tool), Ok(Tool::Grep));
        assert_eq!(
            classify(&["/usr/bin/rg", "x"]).map(|p| p.tool),
            Ok(Tool::Ripgrep)
        );
        assert_eq!(
            classify(&["RG.EXE", "x"]).map(|p| p.tool),
            Ok(Tool::Ripgrep)
        );
        assert_eq!(
            classify(&["git", "--no-pager", "grep", "x"]).map(|p| p.tool),
            Ok(Tool::GitGrep)
        );
    }

    #[test]
    fn refuses_other_commands() {
        assert!(classify(&[]).is_err());
        assert!(classify(&["rm", "-rf", "x"]).is_err());
        assert!(classify(&["git", "status"]).is_err());
        assert!(classify(&["git", "-C", "grep", "status"]).is_err());
    }

    #[test]
    fn adds_the_line_number_flag_only_when_missing() {
        assert_eq!(
            annotate(&["grep", "-r", "x", "."]).line_number_flag_at,
            Some(1)
        );
        assert_eq!(
            annotate(&["grep", "-rn", "x", "."]).line_number_flag_at,
            None
        );
        assert_eq!(
            annotate(&["rg", "--line-number", "x"]).line_number_flag_at,
            None
        );
        assert_eq!(
            annotate(&["git", "-C", "sub", "grep", "x"]).line_number_flag_at,
            Some(4)
        );
    }

    #[test]
    fn a_value_is_not_a_flag() {
        // `-e n` and `-A 1n` hold an `n` that is a value, not `-n`.
        assert_eq!(
            annotate(&["grep", "-e", "-n", "f"]).line_number_flag_at,
            Some(1)
        );
        assert_eq!(annotate(&["grep", "-en", "f"]).line_number_flag_at, Some(1));
        // `-l` after `--` is an operand.
        assert!(!is_passthrough(&["grep", "x", "--", "-l"]));
        // The value of `--include` is not the pattern.
        assert_eq!(
            annotate(&["grep", "-r", "--include", "*.rs", "Name", "."]).tokens,
            ["Name"]
        );
    }

    #[test]
    fn output_without_hits_is_passed_through() {
        for command in [
            &["grep", "-l", "x", "."][..],
            &["grep", "-rc", "x", "."],
            &["grep", "-q", "x", "f"],
            &["grep", "--files-with-matches", "x", "f"],
            &["grep", "-Z", "x", "f"],
            &["rg", "--files"],
            &["rg", "--json", "x"],
            &["rg", "-N", "x"],
            &["rg", "--heading", "x"],
            &["rg", "--color", "always", "x"],
            &["rg", "--color=always", "x"],
            &["grep", "--color=always", "x", "f"],
            &["git", "grep", "-l", "x"],
            &["git", "grep", "--name-only", "x"],
            &["git", "grep", "-c", "x"],
        ] {
            assert!(is_passthrough(command), "{command:?}");
        }
        // `-L` follows symbolic links in rg. It does not list files.
        assert!(!is_passthrough(&["rg", "-L", "x"]));
        assert!(!is_passthrough(&["grep", "--color=auto", "x", "f"]));
    }

    #[test]
    fn reads_the_patterns() {
        assert_eq!(
            annotate(&["grep", "-rn", r"\bGetString\(", "."]).tokens,
            ["GetString"]
        );
        assert_eq!(
            annotate(&["rg", "-e", "foo_bar", "--regexp=Baz|qux", "src"]).tokens,
            ["foo_bar", "Baz", "qux"]
        );
        assert_eq!(
            annotate(&["git", "grep", "-n", "fn parse"]).tokens,
            ["fn", "parse"]
        );
        // With `-f`, the first positional argument is an operand.
        let plan = annotate(&["grep", "-f", "patterns.txt", "file.rs"]);
        assert_eq!(plan.tokens, Vec::<String>::new());
        assert_eq!(plan.single_operand.as_deref(), Some("file.rs"));
    }

    #[test]
    fn pattern_tokens_skip_escapes_and_numbers() {
        assert_eq!(pattern_tokens(r"\bName\b"), ["Name"]);
        assert_eq!(pattern_tokens(r"\w+_id\s*=\s*42"), ["_id"]);
        assert_eq!(pattern_tokens("a.b::c"), ["a", "b", "c"]);
        assert_eq!(pattern_tokens("x|x|y"), ["x", "y"]);
        assert_eq!(pattern_tokens("1.2.3"), Vec::<String>::new());
    }

    #[test]
    fn knows_the_single_operand() {
        assert_eq!(
            annotate(&["grep", "-n", "x", "src/a.rs"])
                .single_operand
                .as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(
            annotate(&["grep", "-n", "x", "a.rs", "b.rs"]).single_operand,
            None
        );
        assert_eq!(annotate(&["rg", "x"]).single_operand, None);
        // `git grep` prints the path for each hit.
        assert_eq!(annotate(&["git", "grep", "x", "a.rs"]).single_operand, None);
    }

    #[test]
    fn knows_when_case_is_ignored() {
        assert!(annotate(&["grep", "-rin", "x", "."]).ignore_case);
        assert!(annotate(&["rg", "--ignore-case", "x"]).ignore_case);
        assert!(annotate(&["rg", "-S", "name"]).ignore_case);
        assert!(!annotate(&["rg", "-S", "Name"]).ignore_case);
        assert!(!annotate(&["grep", "-rn", "x", "."]).ignore_case);
    }

    #[test]
    fn keeps_the_git_directories() {
        assert_eq!(
            annotate(&["git", "-C", "a", "-c", "x=y", "-C", "b", "grep", "x"]).git_dirs,
            ["a", "b"]
        );
    }
}
