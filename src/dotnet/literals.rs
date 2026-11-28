use tree_sitter::Node;

use super::{terminal, text};

pub(super) fn category(source: &str, literal: Node<'_>) -> Option<&'static str> {
    if !matches!(
        literal.kind(),
        "string_literal"
            | "verbatim_string_literal"
            | "raw_string_literal"
            | "interpolated_string_expression"
    ) {
        return None;
    }

    let Some(parent) = literal.parent() else {
        return Some("unknown");
    };
    let category = match parent.kind() {
        "assignment_expression" => parent
            .child_by_field_name("left")
            .map(terminal)
            .map(|name| text(source, name))
            .filter(|name| matches!(*name, "Text" | "HeaderText" | "Content" | "Title"))
            .map(|_| "ui"),
        "argument" => argument_category(source, parent),
        _ => None,
    };
    Some(category.unwrap_or("unknown"))
}

fn argument_category(source: &str, argument: Node<'_>) -> Option<&'static str> {
    let owner = argument.parent()?.parent()?;
    match owner.kind() {
        "element_access_expression" => Some("configuration-key").filter(|_| {
            owner
                .child_by_field_name("expression")
                .is_some_and(|receiver| {
                    matches!(
                        text(source, terminal(receiver)),
                        "Configuration" | "AppSettings"
                    )
                })
        }),
        "object_creation_expression" => {
            let ty = owner.child_by_field_name("type")?;
            text(source, terminal(ty))
                .ends_with("Exception")
                .then_some("exception")
        }
        "invocation_expression" => {
            let function = owner.child_by_field_name("function")?;
            let name = text(source, terminal(function));
            let receiver = function
                .child_by_field_name("expression")
                .map(terminal)
                .map(|node| text(source, node));
            match (receiver, name) {
                (Some("MessageBox"), "Show") => Some("dialog"),
                (Some("Path"), "Combine" | "Join" | "GetFullPath") => Some("path"),
                (
                    _,
                    "LogTrace" | "LogDebug" | "LogInformation" | "LogWarning" | "LogError"
                    | "LogCritical",
                ) => Some("logging"),
                (_, "GetSection" | "GetConnectionString") => Some("configuration-key"),
                _ => None,
            }
        }
        _ => None,
    }
}
