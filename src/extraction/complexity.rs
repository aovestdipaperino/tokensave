// Rust guideline compliant 2025-10-17
//! Generic complexity counting for tree-sitter AST nodes.
//!
//! Walks descendants of a function/method node and counts branches,
//! loops, early-exit statements, and maximum nesting depth. The counts
//! are language-agnostic — each extractor supplies the node type names
//! that correspond to each category.

use tree_sitter::Node as TsNode;

/// Configuration mapping tree-sitter node type names to complexity categories.
pub struct ComplexityConfig {
    /// Node types that count as branches (if, match/switch arm, ternary).
    pub branch_types: &'static [&'static str],
    /// Node types that count as loops (for, while, loop, do).
    pub loop_types: &'static [&'static str],
    /// Node types that count as early exits (return, break, continue, throw).
    pub return_types: &'static [&'static str],
    /// Node types that introduce a new nesting level (block, compound_statement).
    pub nesting_types: &'static [&'static str],
}

/// Complexity metrics extracted from a function body.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComplexityMetrics {
    pub branches: u32,
    pub loops: u32,
    pub returns: u32,
    pub max_nesting: u32,
}

/// Counts complexity metrics by iterating over all descendants of `node`.
///
/// Uses an explicit stack instead of recursion (NASA Power of 10, Rule 1).
/// The nesting depth tracks how many nesting-type ancestors enclose each node.
pub fn count_complexity(node: TsNode<'_>, config: &ComplexityConfig) -> ComplexityMetrics {
    let mut metrics = ComplexityMetrics::default();

    // Stack: (tree-sitter node, current nesting depth)
    let mut stack: Vec<(TsNode<'_>, u32)> = Vec::new();

    // Seed with direct children of the function node (skip the function
    // declaration itself so we only measure the body).
    let child_count = node.child_count();
    let mut idx: u32 = 0;
    while (idx as usize) < child_count {
        if let Some(child) = node.child(idx) {
            stack.push((child, 0));
        }
        idx += 1;
    }

    const MAX_ITERATIONS: usize = 500_000;
    let mut iterations: usize = 0;

    while let Some((current, depth)) = stack.pop() {
        iterations += 1;
        if iterations >= MAX_ITERATIONS {
            break;
        }

        let kind = current.kind();

        // Classify the node.
        if config.branch_types.contains(&kind) {
            metrics.branches += 1;
        }
        if config.loop_types.contains(&kind) {
            metrics.loops += 1;
        }
        if config.return_types.contains(&kind) {
            metrics.returns += 1;
        }

        // Track nesting.
        let new_depth = if config.nesting_types.contains(&kind) {
            let d = depth + 1;
            if d > metrics.max_nesting {
                metrics.max_nesting = d;
            }
            d
        } else {
            depth
        };

        // Push children (reverse order so left-to-right processing).
        let cc = current.child_count() as u32;
        let mut ci = cc;
        while ci > 0 {
            ci -= 1;
            if let Some(child) = current.child(ci) {
                stack.push((child, new_depth));
            }
        }
    }

    metrics
}

// ---------------------------------------------------------------------------
// Per-language configurations
// ---------------------------------------------------------------------------

pub static RUST_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "match_arm", "else_clause"],
    loop_types: &["for_expression", "while_expression", "loop_expression"],
    return_types: &["return_expression", "break_expression", "continue_expression"],
    nesting_types: &["block"],
};

pub static JAVA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_block_statement_group", "ternary_expression", "catch_clause", "else"],
    loop_types: &["for_statement", "enhanced_for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
};

pub static GO_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "expression_case", "type_case", "default_case"],
    loop_types: &["for_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["block"],
};

pub static PYTHON_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elif_clause", "else_clause", "conditional_expression", "except_clause"],
    loop_types: &["for_statement", "while_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "raise_statement"],
    nesting_types: &["block"],
};

pub static TYPESCRIPT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_case", "ternary_expression", "catch_clause", "else_clause"],
    loop_types: &["for_statement", "for_in_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["statement_block"],
};

pub static C_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "conditional_expression", "else_clause"],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["compound_statement"],
};

pub static CPP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "conditional_expression", "catch_clause", "else_clause"],
    loop_types: &["for_statement", "while_statement", "do_statement", "for_range_loop"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["compound_statement"],
};

pub static KOTLIN_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "when_entry", "catch_block", "else"],
    loop_types: &["for_statement", "while_statement", "do_while_statement"],
    return_types: &["jump_expression"],
    nesting_types: &["statements"],
};

pub static SCALA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "case_clause", "catch_clause"],
    loop_types: &["for_expression", "while_expression"],
    return_types: &["return_expression"],
    nesting_types: &["block"],
};

pub static DART_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_statement_case", "catch_clause", "conditional_expression"],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
};

pub static CSHARP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_section", "conditional_expression", "catch_clause"],
    loop_types: &["for_statement", "for_each_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
};

pub static PASCAL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_item", "else_clause"],
    loop_types: &["for_statement", "while_statement", "repeat_statement"],
    return_types: &["raise_statement"],
    nesting_types: &["begin_end_block"],
};

pub static PHP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "catch_clause", "else_clause", "else_if_clause"],
    loop_types: &["for_statement", "foreach_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_expression"],
    nesting_types: &["compound_statement"],
};

pub static RUBY_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if", "elsif", "when", "rescue", "conditional"],
    loop_types: &["for", "while", "until"],
    return_types: &["return", "break", "next"],
    nesting_types: &["body_statement", "do_block", "block"],
};
