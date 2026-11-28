//! Bounded, advisory classification of PowerShell search commands.

use std::ops::Range;
use std::path::Path;

use tree_sitter::{Node, Parser};

use super::constants::{
    POWERSHELL_MAX_COMMAND_BYTES, POWERSHELL_SEARCH_NAMES, POWERSHELL_SHELL_NAMES,
};
use super::{bash_advisory_symbol, HookEnv};

struct Search<'a> {
    span: Range<usize>,
    pattern: &'a str,
    path: &'a str,
}

pub(super) fn is_shell(shell: &str) -> bool {
    let executable = shell.rsplit(['/', '\\']).next().unwrap_or(shell);
    POWERSHELL_SHELL_NAMES
        .iter()
        .any(|name| executable.eq_ignore_ascii_case(name))
}

pub(super) fn advice(command: &str, env: &HookEnv) -> Option<String> {
    if !env.in_tokensave_project || env.disable_grep_hook {
        return None;
    }

    let search = find_search(command)?;
    let root = env.project_root.as_deref()?.canonicalize().ok()?;
    let target = env.base()?.join(search.path).canonicalize().ok()?;
    let relative = target.strip_prefix(&root).ok()?.to_str()?;
    let symbol = bash_advisory_symbol(search.pattern, &target)?;
    let mut args = serde_json::json!({ "query": symbol });
    if !relative.is_empty() {
        args["path_include"] = serde_json::json!([relative]);
    }
    let Range { start, end } = search.span;
    Some(format!(
        "PowerShell file search at UTF-8 bytes {start}..{end}. Use the native command for text matches. \
         For indexed definitions in this scope, use tokensave_search {args} when available. \
         If the MCP server is serving a different checkout, add graph_root={root_json} to those arguments.",
        root_json = serde_json::json!(root)
    ))
}

fn find_search(source: &str) -> Option<Search<'_>> {
    if source.len() > POWERSHELL_MAX_COMMAND_BYTES {
        return None;
    }

    let mut parser = Parser::new();
    parser
        .set_language(&crate::extraction::ts_provider::language("powershell"))
        .ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let statements = root
        .named_children(&mut root.walk())
        .find(|node| node.kind() == "statement_list")?;
    let mut cursor = statements.walk();
    let search = statements
        .named_children(&mut cursor)
        .filter_map(standalone_command)
        .find_map(|command| search_command(command, source));
    search
}

fn standalone_command(statement: Node<'_>) -> Option<Node<'_>> {
    // A single file search can be a statement in a compound command.
    // Skip pipelines, conditional chains, substitutions and nested groups.
    if statement.kind() != "pipeline" || statement.named_child_count() != 1 {
        return None;
    }
    let pipeline = statement.named_child(0)?;
    if pipeline.kind() != "pipeline_chain" || pipeline.named_child_count() != 1 {
        return None;
    }
    pipeline.named_child(0)
}

fn search_command<'a>(node: Node<'_>, source: &'a str) -> Option<Search<'a>> {
    if node.kind() != "command" {
        return None;
    }
    let name = node.child_by_field_name("command_name")?;
    if name.kind() != "command_name" {
        return None;
    }
    let text = |node: Node<'_>| source.get(node.byte_range());
    let name = text(name)?;
    if !POWERSHELL_SEARCH_NAMES
        .iter()
        .any(|alias| name.eq_ignore_ascii_case(alias))
    {
        return None;
    }
    let elements = node.child_by_field_name("command_elements")?;
    let mut cursor = elements.walk();
    let mut args = elements
        .named_children(&mut cursor)
        .filter(|arg| arg.kind() != "command_argument_sep");
    let mut pattern = None;
    let mut path = None;
    while let Some(arg) = args.next() {
        let value = text(arg)?;
        if arg.kind() == "command_parameter" {
            if value.eq_ignore_ascii_case("-Pattern") && pattern.is_none() {
                pattern = Some(literal(text(args.next()?)?)?);
            } else if (value.eq_ignore_ascii_case("-Path")
                || value.eq_ignore_ascii_case("-LiteralPath"))
                && path.is_none()
            {
                path = Some(literal(text(args.next()?)?)?);
            } else if !value.eq_ignore_ascii_case("-SimpleMatch") {
                return None;
            }
        } else if pattern.is_none() {
            pattern = Some(literal(value)?);
        } else if path.is_none() {
            path = Some(literal(value)?);
        } else {
            return None;
        }
    }
    Some(Search {
        span: node.byte_range(),
        pattern: pattern?,
        path: path?,
    })
}

/// Accept the literal subset whose value is identical to its source text
/// after removing optional quotes. Any interpolation or escapes stay native.
fn literal(raw: &str) -> Option<&str> {
    let value = if let Some(inner) = raw.strip_prefix('\'') {
        let value = inner.strip_suffix('\'')?;
        (!value.contains('\'')).then_some(value)?
    } else if let Some(inner) = raw.strip_prefix('"') {
        let value = inner.strip_suffix('"')?;
        (!value.contains(['"', '$', '\x60'])).then_some(value)?
    } else {
        (!raw.starts_with('-') && !raw.contains(['$', '\x60', '@', '(', ')', ',', '\'', '"']))
            .then_some(raw)?
    };
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::find_search;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "the fixture must parse as a file search"
    )]
    fn finds_literal_search_and_its_span() {
        let command = "Write-Host header; sls -Pattern 'Widget' -LiteralPath './file.rs'";
        let search = find_search(command).expect("literal file search");
        assert_eq!(search.pattern, "Widget");
        assert_eq!(search.path, "./file.rs");
        assert_eq!(
            &command[search.span],
            "sls -Pattern 'Widget' -LiteralPath './file.rs'"
        );
    }

    #[test]
    fn keeps_pipeline_output_and_dynamic_searches_native() {
        assert!(
            find_search("build-command 2>&1 | Select-String -Pattern 'error|Localization'")
                .is_none()
        );
        for command in [
            "cargo check | Select-String 'Widget'",
            "Select-String 'Widget' './file.rs' | Select-Object -First 1",
            "Select-String $pattern './file.rs'",
            "Select-String $(Get-Content pattern.txt) './file.rs'",
            "Select-String 'unterminated",
        ] {
            assert!(find_search(command).is_none(), "{command}");
        }
    }
}
