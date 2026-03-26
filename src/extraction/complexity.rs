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
    /// Node types representing unsafe blocks (e.g. `unsafe_block` in Rust, `unsafe_statement` in C#).
    pub unsafe_types: &'static [&'static str],
    /// Node types that are inherently unchecked operations (e.g. `non_null_assertion_expression`).
    pub unchecked_types: &'static [&'static str],
    /// Method names that represent unchecked/force-unwrap calls (e.g. `unwrap`, `get`).
    /// Matched against the method name in call expressions.
    pub unchecked_methods: &'static [&'static str],
    /// Node types representing method/function call expressions, used for unchecked_methods matching.
    pub call_expression_types: &'static [&'static str],
    /// Field name used to extract the method name from a call expression node.
    /// e.g. "function" for TS, "method" for Rust. Empty to skip.
    pub call_method_field: &'static str,
    /// Macro/function names that count as assertions (e.g. `assert`, `assert_eq`, `assertEquals`).
    /// Matched against macro invocation names and function/method call names.
    pub assertion_names: &'static [&'static str],
    /// Node types representing macro invocations (e.g. `macro_invocation` in Rust).
    pub macro_invocation_types: &'static [&'static str],
}

/// Complexity metrics extracted from a function body.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComplexityMetrics {
    pub branches: u32,
    pub loops: u32,
    pub returns: u32,
    pub max_nesting: u32,
    /// Number of unsafe blocks/statements.
    pub unsafe_blocks: u32,
    /// Number of unchecked/force-unwrap calls or assertions.
    pub unchecked_calls: u32,
    /// Number of assertion calls (assert, debug_assert, assertEquals, etc.).
    pub assertions: u32,
}

/// Counts complexity metrics by iterating over all descendants of `node`.
///
/// Uses an explicit stack instead of recursion (NASA Power of 10, Rule 1).
/// The nesting depth tracks how many nesting-type ancestors enclose each node.
///
/// `source` is needed to extract method/macro names for unchecked-call and
/// assertion detection. Pass an empty slice to skip name-based matching.
pub fn count_complexity(node: TsNode<'_>, config: &ComplexityConfig, source: &[u8]) -> ComplexityMetrics {
    debug_assert!(!config.branch_types.is_empty() || !config.loop_types.is_empty(),
        "count_complexity called with config that has no branch or loop types");
    debug_assert!(node.child_count() > 0, "count_complexity called on a node with no children");
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

        // Unsafe blocks.
        if config.unsafe_types.contains(&kind) {
            metrics.unsafe_blocks += 1;
        }

        // Unchecked operator types (e.g. non_null_assertion_expression, `!!`).
        if config.unchecked_types.contains(&kind) {
            metrics.unchecked_calls += 1;
        }

        // Name-based detection for call expressions (unchecked methods + assertions).
        if !source.is_empty() && config.call_expression_types.contains(&kind) {
            if let Some(name) = extract_call_name(current, config.call_method_field, source) {
                if config.unchecked_methods.contains(&name.as_str()) {
                    metrics.unchecked_calls += 1;
                }
                if config.assertion_names.contains(&name.as_str()) {
                    metrics.assertions += 1;
                }
            }
        }

        // Name-based detection for macro invocations (Rust assert!, debug_assert!, etc.).
        if !source.is_empty() && config.macro_invocation_types.contains(&kind) {
            if let Some(name) = extract_macro_name(current, source) {
                if config.assertion_names.contains(&name.as_str()) {
                    metrics.assertions += 1;
                }
                if config.unchecked_methods.contains(&name.as_str()) {
                    metrics.unchecked_calls += 1;
                }
            }
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

    debug_assert!(metrics.max_nesting <= 500, "max_nesting unexpectedly large, possible analysis error");
    debug_assert!(iterations <= MAX_ITERATIONS, "iteration count invariant violated");
    metrics
}

/// Extracts the method/function name from a call expression node.
///
/// Tries the configured `method_field` first (e.g. "function", "method"),
/// then falls back to common child patterns: last identifier before `(`,
/// or a `field_expression`/`member_expression` selector.
fn extract_call_name(node: TsNode<'_>, method_field: &str, source: &[u8]) -> Option<String> {
    // Try the configured field name first.
    if !method_field.is_empty() {
        if let Some(field_node) = node.child_by_field_name(method_field) {
            // For chained calls like `x.unwrap()`, the field may be a
            // field_expression / member_expression — grab the rightmost identifier.
            let text = rightmost_identifier(field_node, source);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    // Fallback: first child that is an identifier or has a selector child.
    let child_count = node.child_count();
    let mut i: u32 = 0;
    while (i as usize) < child_count {
        if let Some(child) = node.child(i) {
            let ck = child.kind();
            if ck == "identifier" || ck == "field_identifier" || ck == "property_identifier" {
                if let Ok(text) = child.utf8_text(source) {
                    return Some(text.to_string());
                }
            }
            // member_expression / field_expression: grab the property/field child.
            if ck.contains("member_expression") || ck.contains("field_expression") {
                let text = rightmost_identifier(child, source);
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
        i += 1;
    }
    None
}

/// Extracts the macro name from a macro invocation node (e.g. `assert!`).
///
/// Looks for the first identifier child, stripping a trailing `!` if present.
fn extract_macro_name(node: TsNode<'_>, source: &[u8]) -> Option<String> {
    let child_count = node.child_count();
    let mut i: u32 = 0;
    while (i as usize) < child_count {
        if let Some(child) = node.child(i) {
            let ck = child.kind();
            if ck == "identifier" || ck == "scoped_identifier" {
                if let Ok(text) = child.utf8_text(source) {
                    return Some(text.trim_end_matches('!').to_string());
                }
            }
        }
        i += 1;
    }
    None
}

/// Returns the text of the rightmost identifier-like child of `node`.
fn rightmost_identifier(node: TsNode<'_>, source: &[u8]) -> String {
    // If node itself is a simple identifier, return it.
    let nk = node.kind();
    if nk == "identifier" || nk == "field_identifier" || nk == "property_identifier" {
        return node.utf8_text(source).unwrap_or("").to_string();
    }
    // Walk children right-to-left for the first identifier.
    let cc = node.child_count();
    let mut i = cc as u32;
    while i > 0 {
        i -= 1;
        if let Some(child) = node.child(i) {
            let ck = child.kind();
            if ck == "identifier" || ck == "field_identifier" || ck == "property_identifier" {
                return child.utf8_text(source).unwrap_or("").to_string();
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Per-language configurations
// ---------------------------------------------------------------------------

pub static RUST_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "match_arm", "else_clause"],
    loop_types: &["for_expression", "while_expression", "loop_expression"],
    return_types: &["return_expression", "break_expression", "continue_expression"],
    nesting_types: &["block"],
    unsafe_types: &["unsafe_block"],
    unchecked_types: &[],
    unchecked_methods: &["unwrap", "expect"],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "assert_eq", "assert_ne", "debug_assert", "debug_assert_eq", "debug_assert_ne"],
    macro_invocation_types: &["macro_invocation"],
};

pub static JAVA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_block_statement_group", "ternary_expression", "catch_clause", "else"],
    loop_types: &["for_statement", "enhanced_for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["get"],
    call_expression_types: &["method_invocation"],
    call_method_field: "name",
    assertion_names: &["assert", "assertEquals", "assertNotEquals", "assertTrue", "assertFalse", "assertNull", "assertNotNull", "assertThrows", "assertThat", "assertArrayEquals"],
    macro_invocation_types: &[],
};

pub static GO_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "expression_case", "type_case", "default_case"],
    loop_types: &["for_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "require", "Equal", "NotEqual", "True", "False", "Nil", "NotNil", "Error", "NoError"],
    macro_invocation_types: &[],
};

pub static PYTHON_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elif_clause", "else_clause", "conditional_expression", "except_clause"],
    loop_types: &["for_statement", "while_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "raise_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call"],
    call_method_field: "function",
    assertion_names: &["assert", "assertEqual", "assertNotEqual", "assertTrue", "assertFalse", "assertIs", "assertIsNone", "assertIsNotNone", "assertIn", "assertRaises", "assertAlmostEqual"],
    macro_invocation_types: &[],
};

pub static TYPESCRIPT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_case", "ternary_expression", "catch_clause", "else_clause"],
    loop_types: &["for_statement", "for_in_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["statement_block"],
    unsafe_types: &[],
    unchecked_types: &["non_null_assertion_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "expect", "assertEquals", "assertStrictEquals", "deepEqual", "strictEqual", "ok", "notOk"],
    macro_invocation_types: &[],
};

pub static C_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "conditional_expression", "else_clause"],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "assert_true", "assert_false", "assert_int_equal", "assert_string_equal", "assert_null", "assert_non_null", "CU_ASSERT", "CU_ASSERT_EQUAL"],
    macro_invocation_types: &[],
};

pub static CPP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "conditional_expression", "catch_clause", "else_clause"],
    loop_types: &["for_statement", "while_statement", "do_statement", "for_range_loop"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "ASSERT_TRUE", "ASSERT_FALSE", "ASSERT_EQ", "ASSERT_NE", "ASSERT_LT", "ASSERT_GT", "EXPECT_TRUE", "EXPECT_FALSE", "EXPECT_EQ", "EXPECT_NE", "static_assert"],
    macro_invocation_types: &[],
};

pub static KOTLIN_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "when_entry", "catch_block", "else"],
    loop_types: &["for_statement", "while_statement", "do_while_statement"],
    return_types: &["jump_expression"],
    nesting_types: &["statements"],
    unsafe_types: &[],
    unchecked_types: &["postfix_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &["assert", "assertEquals", "assertNotEquals", "assertTrue", "assertFalse", "assertNull", "assertNotNull", "assertIs", "assertIsNot"],
    macro_invocation_types: &[],
};

pub static SCALA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "case_clause", "catch_clause"],
    loop_types: &["for_expression", "while_expression"],
    return_types: &["return_expression"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["get"],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "assertEquals", "assertResult", "assertThrows"],
    macro_invocation_types: &[],
};

pub static DART_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_statement_case", "catch_clause", "conditional_expression"],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &["postfix_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "expect", "expectLater", "expectAsync"],
    macro_invocation_types: &[],
};

pub static CSHARP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_section", "conditional_expression", "catch_clause"],
    loop_types: &["for_statement", "for_each_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["block"],
    unsafe_types: &["unsafe_statement"],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation_expression"],
    call_method_field: "function",
    assertion_names: &["Assert", "AreEqual", "AreNotEqual", "IsTrue", "IsFalse", "IsNull", "IsNotNull", "ThrowsException"],
    macro_invocation_types: &[],
};

pub static PASCAL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_item", "else_clause"],
    loop_types: &["for_statement", "while_statement", "repeat_statement"],
    return_types: &["raise_statement"],
    nesting_types: &["begin_end_block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_statement"],
    call_method_field: "",
    assertion_names: &["Assert", "CheckEquals", "CheckTrue", "CheckFalse"],
    macro_invocation_types: &[],
};

pub static PHP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "catch_clause", "else_clause", "else_if_clause"],
    loop_types: &["for_statement", "foreach_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_expression"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["function_call_expression", "member_call_expression"],
    call_method_field: "name",
    assertion_names: &["assert", "assertEquals", "assertNotEquals", "assertTrue", "assertFalse", "assertNull", "assertNotNull", "assertSame", "assertInstanceOf"],
    macro_invocation_types: &[],
};

pub static RUBY_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if", "elsif", "when", "rescue", "conditional"],
    loop_types: &["for", "while", "until"],
    return_types: &["return", "break", "next"],
    nesting_types: &["body_statement", "do_block", "block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["fetch"],
    call_expression_types: &["call", "method_call"],
    call_method_field: "method",
    assertion_names: &["assert", "assert_equal", "assert_not_equal", "assert_nil", "assert_not_nil", "assert_raises", "assert_match", "refute"],
    macro_invocation_types: &[],
};

pub static SWIFT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "switch_entry", "guard_statement", "catch_keyword"],
    loop_types: &["for_in_statement", "while_statement", "repeat_while_statement"],
    return_types: &["control_transfer_statement"],
    nesting_types: &["code_block"],
    unsafe_types: &[],
    unchecked_types: &["force_unwrap_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &["assert", "precondition", "assertionFailure", "XCTAssert", "XCTAssertEqual", "XCTAssertTrue", "XCTAssertFalse", "XCTAssertNil", "XCTAssertNotNil"],
    macro_invocation_types: &[],
};

pub static BASH_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elif_clause", "else_clause", "case_item"],
    loop_types: &["for_statement", "while_statement", "c_style_for_statement"],
    return_types: &["return_statement"],
    nesting_types: &["compound_statement", "subshell"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["command"],
    call_method_field: "name",
    assertion_names: &[],
    macro_invocation_types: &[],
};

pub static LUA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elseif_statement", "else_statement"],
    loop_types: &["for_statement", "for_in_statement", "while_statement", "repeat_statement"],
    return_types: &["return_statement", "break_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["function_call"],
    call_method_field: "",
    assertion_names: &["assert", "assert_equal", "assert_true", "assert_false"],
    macro_invocation_types: &[],
};

pub static ZIG_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "switch_expression", "else_expression", "catch"],
    loop_types: &["for_expression", "while_expression"],
    return_types: &["return_expression", "break_expression", "continue_expression"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["orelse"],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &["expect", "expectEqual", "expectEqualStrings", "expectError"],
    macro_invocation_types: &[],
};

pub static NIX_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression"],
    loop_types: &[],
    return_types: &[],
    nesting_types: &["attrset_expression", "let_expression"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["apply_expression"],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
};

pub static VBNET_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elseif_clause", "else_clause", "select_case_statement", "catch_clause"],
    loop_types: &["for_statement", "for_each_statement", "while_statement", "do_loop_statement"],
    return_types: &["return_statement", "exit_statement", "throw_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation_expression"],
    call_method_field: "",
    assertion_names: &["Assert", "AreEqual", "AreNotEqual", "IsTrue", "IsFalse", "IsNull", "IsNotNull"],
    macro_invocation_types: &[],
};

pub static POWERSHELL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elseif_clause", "else_clause", "switch_statement", "catch_clause"],
    loop_types: &["for_statement", "foreach_statement", "while_statement", "do_while_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement", "throw_statement"],
    nesting_types: &["script_block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["command_expression"],
    call_method_field: "",
    assertion_names: &["Should", "Assert"],
    macro_invocation_types: &[],
};

pub static PERL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elsif_clause", "else_clause", "unless_statement", "conditional_expression"],
    loop_types: &["for_statement", "foreach_statement", "while_statement", "until_statement"],
    return_types: &["return_expression", "last_expression", "next_expression"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression", "method_call_expression"],
    call_method_field: "",
    assertion_names: &["ok", "is", "isnt", "like", "unlike", "cmp_ok", "is_deeply"],
    macro_invocation_types: &[],
};

pub static OBJC_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "case_statement", "conditional_expression", "catch_clause", "else_clause"],
    loop_types: &["for_statement", "while_statement", "do_statement", "for_in_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression", "message_expression"],
    call_method_field: "",
    assertion_names: &["NSAssert", "NSCAssert", "XCTAssert", "XCTAssertTrue", "XCTAssertFalse", "XCTAssertEqual", "XCTAssertNil", "XCTAssertNotNil"],
    macro_invocation_types: &[],
};
