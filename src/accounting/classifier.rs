//! Deterministic task classification for Claude Code turns.
//!
//! Classifies each API turn into one of 13 categories based on tool usage
//! patterns and keyword matching. Adapted from AgentSeal/codeburn (MIT).

use std::fmt;

/// Task category for a single API turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    Coding,
    Debugging,
    FeatureDev,
    Refactoring,
    Testing,
    Exploration,
    Planning,
    Delegation,
    GitOps,
    BuildDeploy,
    Brainstorming,
    Conversation,
    General,
}

impl fmt::Display for TaskCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TaskCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coding => "coding",
            Self::Debugging => "debugging",
            Self::FeatureDev => "feature_dev",
            Self::Refactoring => "refactoring",
            Self::Testing => "testing",
            Self::Exploration => "exploration",
            Self::Planning => "planning",
            Self::Delegation => "delegation",
            Self::GitOps => "git_ops",
            Self::BuildDeploy => "build_deploy",
            Self::Brainstorming => "brainstorming",
            Self::Conversation => "conversation",
            Self::General => "general",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Coding => "Coding",
            Self::Debugging => "Debugging",
            Self::FeatureDev => "Feature Dev",
            Self::Refactoring => "Refactoring",
            Self::Testing => "Testing",
            Self::Exploration => "Exploration",
            Self::Planning => "Planning",
            Self::Delegation => "Delegation",
            Self::GitOps => "Git Ops",
            Self::BuildDeploy => "Build/Deploy",
            Self::Brainstorming => "Brainstorming",
            Self::Conversation => "Conversation",
            Self::General => "General",
        }
    }
}

/// Classify a turn based on tool names and Bash command content.
///
/// `tool_names`: names of all tool_use blocks in the turn (e.g. "Edit", "Bash").
/// `bash_commands`: content of Bash tool inputs (the `command` field).
pub fn classify(tool_names: &[&str], bash_commands: &[&str]) -> TaskCategory {
    if tool_names.is_empty() {
        return TaskCategory::Conversation;
    }

    // Agent tool → delegation
    if tool_names.contains(&"Agent") {
        return TaskCategory::Delegation;
    }

    // Planning tools
    if tool_names.contains(&"EnterPlanMode") || tool_names.contains(&"TaskCreate") {
        return TaskCategory::Planning;
    }

    let has_edit = tool_names.contains(&"Edit") || tool_names.contains(&"Write");
    let has_read_only = tool_names.contains(&"Read")
        || tool_names.contains(&"Grep")
        || tool_names.contains(&"Glob")
        || tool_names.contains(&"WebSearch");

    // Check Bash commands for specific patterns
    let any_bash = |patterns: &[&str]| -> bool {
        bash_commands.iter().any(|cmd| {
            let lower = cmd.to_ascii_lowercase();
            patterns.iter().any(|p| lower.contains(p))
        })
    };

    // Git operations
    if any_bash(&["git "]) {
        return TaskCategory::GitOps;
    }

    // Testing: test runner commands
    if any_bash(&[
        "cargo test",
        "cargo nextest",
        "pytest",
        "vitest",
        "jest ",
        "mocha ",
        "npm test",
        "npm run test",
        "pnpm test",
        "pnpm run test",
        "go test",
        "dotnet test",
        "flutter test",
    ]) {
        return TaskCategory::Testing;
    }

    // Build/deploy
    if any_bash(&[
        "cargo build",
        "cargo check",
        "npm run build",
        "pnpm build",
        "docker ",
        "kubectl ",
        "pm2 ",
        "tsc",
        "next build",
    ]) {
        return TaskCategory::BuildDeploy;
    }

    // Debugging: error/fix keywords in bash output context
    if any_bash(&[
        "fix",
        "debug",
        "error",
        "bug",
        "issue",
        "stacktrace",
        "panic",
    ]) && has_edit
    {
        return TaskCategory::Debugging;
    }

    // Refactoring keywords in bash commands
    if any_bash(&["refactor", "rename", "simplify", "extract", "inline"]) {
        return TaskCategory::Refactoring;
    }

    // Coding: has edit/write tools
    if has_edit {
        return TaskCategory::Coding;
    }

    // Exploration: read-only tools without edits
    if has_read_only {
        return TaskCategory::Exploration;
    }

    TaskCategory::General
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tools_is_conversation() {
        assert_eq!(classify(&[], &[]), TaskCategory::Conversation);
    }

    #[test]
    fn test_edit_is_coding() {
        assert_eq!(classify(&["Edit"], &[]), TaskCategory::Coding);
    }

    #[test]
    fn test_write_is_coding() {
        assert_eq!(classify(&["Write"], &[]), TaskCategory::Coding);
    }

    #[test]
    fn test_agent_is_delegation() {
        assert_eq!(classify(&["Agent", "Edit"], &[]), TaskCategory::Delegation);
    }

    #[test]
    fn test_git_bash_is_gitops() {
        assert_eq!(classify(&["Bash"], &["git status"]), TaskCategory::GitOps);
    }

    #[test]
    fn test_cargo_test_is_testing() {
        assert_eq!(
            classify(&["Bash"], &["cargo test --lib"]),
            TaskCategory::Testing
        );
    }

    #[test]
    fn test_read_only_is_exploration() {
        assert_eq!(classify(&["Read", "Grep"], &[]), TaskCategory::Exploration);
    }

    #[test]
    fn test_plan_mode_is_planning() {
        assert_eq!(classify(&["EnterPlanMode"], &[]), TaskCategory::Planning);
    }

    #[test]
    fn test_docker_is_build_deploy() {
        assert_eq!(
            classify(&["Bash"], &["docker build -t myapp ."]),
            TaskCategory::BuildDeploy
        );
    }

    #[test]
    fn test_fix_with_edit_is_debugging() {
        assert_eq!(
            classify(&["Bash", "Edit"], &["fix the broken import"]),
            TaskCategory::Debugging
        );
    }

    #[test]
    fn test_category_display() {
        assert_eq!(TaskCategory::GitOps.as_str(), "git_ops");
        assert_eq!(TaskCategory::GitOps.label(), "Git Ops");
    }
}
