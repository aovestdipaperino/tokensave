// Rust guideline compliant 2025-10-17
//! Generic complexity counting for tree-sitter AST nodes.
//!
//! Walks descendants of a function/method node and counts branches,
//! loops, early-exit statements, maximum nesting depth, cognitive
//! complexity, and Halstead operator/operand tokens — all in a single
//! AST walk. The counts are language-agnostic: each extractor supplies the
//! node type names that correspond to each category via [`ComplexityConfig`].
//!
//! Derived float metrics (Halstead volume/difficulty/effort and the
//! maintainability index) are provided as pure functions
//! ([`halstead_volume`], [`halstead_difficulty`], [`halstead_effort`],
//! [`maintainability_index`]) so callers can compute them on demand without
//! storing redundant floats on every node.
//!
//! The CRAP change-risk metric is intentionally out of scope: it needs
//! per-method test-coverage data tokensave does not collect.

use tree_sitter::Node as TsNode;

/// Configuration mapping tree-sitter node type names to complexity categories.
pub struct ComplexityConfig {
    /// Node types that count as branches (if, match/switch arm, ternary).
    pub branch_types: &'static [&'static str],
    /// Node types that count as loops (for, while, loop, do).
    pub loop_types: &'static [&'static str],
    /// Node types that count as early exits (return, break, continue, throw).
    pub return_types: &'static [&'static str],
    /// Node types that introduce a new nesting level (block, `compound_statement`).
    pub nesting_types: &'static [&'static str],
    /// Node types representing unsafe blocks (e.g. `unsafe_block` in Rust, `unsafe_statement` in C#).
    pub unsafe_types: &'static [&'static str],
    /// Node types that are inherently unchecked operations (e.g. `non_null_assertion_expression`).
    pub unchecked_types: &'static [&'static str],
    /// Method names that represent unchecked/force-unwrap calls (e.g. `unwrap`, `get`).
    /// Matched against the method name in call expressions.
    pub unchecked_methods: &'static [&'static str],
    /// Node types representing method/function call expressions, used for `unchecked_methods` matching.
    pub call_expression_types: &'static [&'static str],
    /// Field name used to extract the method name from a call expression node.
    /// e.g. "function" for TS, "method" for Rust. Empty to skip.
    pub call_method_field: &'static str,
    /// Macro/function names that count as assertions (e.g. `assert`, `assert_eq`, `assertEquals`).
    /// Matched against macro invocation names and function/method call names.
    pub assertion_names: &'static [&'static str],
    /// Node types representing macro invocations (e.g. `macro_invocation` in Rust).
    pub macro_invocation_types: &'static [&'static str],
    /// Node types classified as Halstead *operators* (operators, keywords, calls).
    ///
    /// Each matching node contributes one token to the total-operator count and
    /// its `kind` to the distinct-operator set. Leave empty to skip Halstead
    /// operator counting for a language.
    pub operator_types: &'static [&'static str],
    /// Node types classified as Halstead *operands* (identifiers, literals).
    ///
    /// Each matching node contributes one token to the total-operand count and
    /// its source text to the distinct-operand set. Leave empty to skip Halstead
    /// operand counting for a language.
    pub operand_types: &'static [&'static str],
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
    /// Number of assertion calls (assert, `debug_assert`, assertEquals, etc.).
    pub assertions: u32,
    /// `SonarSource`-style cognitive complexity.
    ///
    /// Increments for each break in linear control flow (if/else-if/switch
    /// arms, loops, catch, ternaries) plus a nesting penalty equal to the
    /// number of enclosing control-flow structures, and one increment per
    /// extra boolean operator in a logical sequence. Unlike cyclomatic
    /// complexity this is nesting-weighted, so it must be computed during the
    /// AST walk and cannot be derived from the scalar branch/loop counts.
    pub cognitive_complexity: u32,
    /// Number of distinct Halstead operators (n1).
    pub distinct_operators: u32,
    /// Number of distinct Halstead operands (n2).
    pub distinct_operands: u32,
    /// Total Halstead operator occurrences (N1).
    pub total_operators: u32,
    /// Total Halstead operand occurrences (N2).
    pub total_operands: u32,
}

/// Counts complexity metrics by iterating over all descendants of `node`.
///
/// Uses an explicit stack instead of recursion (NASA Power of 10, Rule 1).
/// The nesting depth tracks how many nesting-type ancestors enclose each node.
///
/// `source` is needed to extract method/macro names for unchecked-call and
/// assertion detection. Pass an empty slice to skip name-based matching.
pub fn count_complexity(
    node: TsNode<'_>,
    config: &ComplexityConfig,
    source: &[u8],
) -> ComplexityMetrics {
    const MAX_ITERATIONS: usize = 500_000;
    debug_assert!(
        !config.branch_types.is_empty() || !config.loop_types.is_empty(),
        "count_complexity called with config that has no branch or loop types"
    );
    debug_assert!(
        node.child_count() > 0,
        "count_complexity called on a node with no children"
    );
    let mut metrics = ComplexityMetrics::default();

    // Distinct-token sets for Halstead. Operators are keyed by node kind
    // (stable, allocation-free); operands by source text so two uses of the
    // same identifier/literal collapse to one distinct operand.
    let mut distinct_operators: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut distinct_operands: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Stack: (tree-sitter node, block nesting depth, cognitive nesting depth).
    // Block nesting feeds `max_nesting`; cognitive nesting feeds the
    // SonarSource nesting penalty and counts only control-flow ancestors.
    let mut stack: Vec<(TsNode<'_>, u32, u32)> = Vec::new();

    // Seed with direct children of the function node. Earlier revisions used
    // `node.child(i)` in a `for i in 0..N` loop — tree-sitter's `child(i)`
    // is O(i) because it walks sibling links from the first child, so the
    // seed loop alone was O(N²) for high-fanout nodes (e.g. the giant
    // `switch` in `kernel/bpf/verifier.c` with thousands of cases). Use a
    // cursor for O(1) per step.
    push_children(&mut stack, node, 0, 0);

    let mut iterations: usize = 0;

    while let Some((current, depth, cog_depth)) = stack.pop() {
        iterations += 1;
        if iterations >= MAX_ITERATIONS {
            break;
        }

        let kind = current.kind();

        // Classify the node.
        let is_branch = config.branch_types.contains(&kind);
        let is_loop = config.loop_types.contains(&kind);
        if is_branch {
            metrics.branches += 1;
        }
        if is_loop {
            metrics.loops += 1;
        }
        if config.return_types.contains(&kind) {
            metrics.returns += 1;
        }

        // Cognitive complexity: each control-flow structure adds one base
        // increment plus the current cognitive nesting penalty. Boolean
        // operator sequences add one increment each (handled below).
        let is_control_flow = is_branch || is_loop;
        if is_control_flow {
            metrics.cognitive_complexity += 1 + cog_depth;
        } else if is_logical_operator(current, source) {
            metrics.cognitive_complexity += 1;
        }

        // Halstead: classify the node as operator or operand.
        if config.operator_types.contains(&kind) {
            metrics.total_operators += 1;
            distinct_operators.insert(kind);
        }
        if config.operand_types.contains(&kind) {
            metrics.total_operands += 1;
            if let Ok(text) = current.utf8_text(source) {
                distinct_operands.insert(text.to_string());
            }
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

        // Track block nesting (for `max_nesting`).
        let new_depth = if config.nesting_types.contains(&kind) {
            let d = depth + 1;
            if d > metrics.max_nesting {
                metrics.max_nesting = d;
            }
            d
        } else {
            depth
        };

        // Track cognitive nesting: descendants of a control-flow structure
        // pay an extra nesting penalty for their own control-flow.
        let new_cog_depth = if is_control_flow {
            cog_depth + 1
        } else {
            cog_depth
        };

        // Push children via cursor — see `push_children`. Same O(N²) trap
        // as the seed loop above.
        push_children(&mut stack, current, new_depth, new_cog_depth);
    }

    metrics.distinct_operators = distinct_operators.len() as u32;
    metrics.distinct_operands = distinct_operands.len() as u32;

    debug_assert!(
        metrics.max_nesting <= 500,
        "max_nesting unexpectedly large, possible analysis error"
    );
    debug_assert!(
        iterations <= MAX_ITERATIONS,
        "iteration count invariant violated"
    );
    metrics
}

/// Returns true if `node` is a binary expression whose operator is a logical
/// `&&` or `||`. Used for the `SonarSource` boolean-sequence cognitive
/// increment. Walks immediate children looking for the operator token, which
/// keeps this grammar-agnostic across the supported languages.
fn is_logical_operator(node: TsNode<'_>, source: &[u8]) -> bool {
    let kind = node.kind();
    // Common binary-expression node kinds across grammars.
    if !(kind.contains("binary_expression")
        || kind == "boolean_operator"
        || kind == "logical_expression")
    {
        return false;
    }
    if source.is_empty() {
        return false;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            // Operator tokens are unnamed leaves; check their text directly.
            if !child.is_named() {
                if let Ok(text) = child.utf8_text(source) {
                    let t = text.trim();
                    if t == "&&" || t == "||" || t == "and" || t == "or" {
                        return true;
                    }
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Derived Halstead and maintainability metrics
//
// These are pure functions over the raw token counts produced by
// `count_complexity`, kept here so callers (e.g. the MCP complexity handler)
// can derive the float-valued metrics on demand without storing redundant
// floats on every `Node`.
//
// NOTE: the CRAP metric requested in issue #150 is intentionally *not*
// implemented. CRAP = comp^2 * (1 - coverage)^3 + comp, which requires
// per-method test-coverage data that tokensave does not collect. It is
// deferred until coverage tracking exists.
// ---------------------------------------------------------------------------

/// Computes Halstead volume `V = N * log2(n)`.
///
/// `N` is program length (`total_operators` + `total_operands`) and `n` is
/// vocabulary (`distinct_operators` + `distinct_operands`). Returns 0.0 for an
/// empty unit (vocabulary <= 1, where `log2` is undefined or zero).
///
/// # Examples
/// ```
/// use tokensave::extraction::complexity::halstead_volume;
/// assert!(halstead_volume(2, 3, 4, 6) > 0.0);
/// ```
#[must_use]
pub fn halstead_volume(
    distinct_operators: u32,
    distinct_operands: u32,
    total_operators: u32,
    total_operands: u32,
) -> f64 {
    let vocabulary = f64::from(distinct_operators + distinct_operands);
    let length = f64::from(total_operators + total_operands);
    if vocabulary <= 1.0 {
        return 0.0;
    }
    length * vocabulary.log2()
}

/// Computes Halstead difficulty `D = (n1 / 2) * (N2 / n2)`.
///
/// Returns 0.0 when there are no distinct operands (avoids divide-by-zero).
///
/// # Examples
/// ```
/// use tokensave::extraction::complexity::halstead_difficulty;
/// assert!(halstead_difficulty(4, 6, 12) > 0.0);
/// ```
#[must_use]
pub fn halstead_difficulty(
    distinct_operators: u32,
    distinct_operands: u32,
    total_operands: u32,
) -> f64 {
    if distinct_operands == 0 {
        return 0.0;
    }
    (f64::from(distinct_operators) / 2.0)
        * (f64::from(total_operands) / f64::from(distinct_operands))
}

/// Computes Halstead effort `E = D * V`.
///
/// # Examples
/// ```
/// use tokensave::extraction::complexity::halstead_effort;
/// assert!(halstead_effort(10.0, 2.0) > 0.0);
/// ```
#[must_use]
pub fn halstead_effort(volume: f64, difficulty: f64) -> f64 {
    volume * difficulty
}

/// Computes the maintainability index, clamped to `0..=100`.
///
/// Uses the widely adopted Microsoft/SEI variant scaled to a 0–100 range:
/// `MI = max(0, (171 - 5.2*ln(V) - 0.23*G - 16.2*ln(LOC)) * 100 / 171)`,
/// where `V` is Halstead volume, `G` is cyclomatic complexity, and `LOC` is
/// lines of code. Higher is more maintainable. Returns 100.0 for trivial units
/// (volume and LOC both 0).
///
/// # Examples
/// ```
/// use tokensave::extraction::complexity::maintainability_index;
/// let mi = maintainability_index(100.0, 3, 20);
/// assert!((0.0..=100.0).contains(&mi));
/// ```
#[must_use]
pub fn maintainability_index(volume: f64, cyclomatic: u32, lines_of_code: u32) -> f64 {
    // ln(0) is -inf; guard the volume and LOC terms so a tiny unit yields a
    // high (good) score rather than NaN/inf.
    let ln_v = if volume > 0.0 { volume.ln() } else { 0.0 };
    let loc = f64::from(lines_of_code);
    let ln_loc = if loc > 0.0 { loc.ln() } else { 0.0 };

    let raw = 171.0 - 5.2 * ln_v - 0.23 * f64::from(cyclomatic) - 16.2 * ln_loc;
    // Scale the classic 0–171 range to 0–100 and clamp.
    (raw * 100.0 / 171.0).clamp(0.0, 100.0)
}

/// Computes the CRAP (Change Risk Anti-Pattern) score for a unit.
///
/// `CRAP(m) = comp(m)² · (1 − cov(m))³ + comp(m)`, where `comp` is cyclomatic
/// complexity and `cov` is the fraction of `m` covered by tests (`0.0..=1.0`).
/// A fully covered unit scores exactly its cyclomatic complexity; an untested
/// unit scores `comp² + comp`, so risk grows quadratically with complexity when
/// tests are absent — the metric flags code that is both complex and untested.
///
/// tokensave derives `cov` from call-graph reachability (whether a test
/// function reaches the unit), which is binary today — callers pass `1.0` for a
/// test-reached unit and `0.0` otherwise. The formula accepts any fractional
/// coverage so it stays correct if execution-coverage ingestion is added later.
///
/// # Examples
/// ```
/// use tokensave::extraction::complexity::crap_score;
/// // Fully tested: CRAP == cyclomatic complexity.
/// assert!((crap_score(5, 1.0) - 5.0).abs() < 1e-9);
/// // Untested: comp² + comp.
/// assert!((crap_score(5, 0.0) - 30.0).abs() < 1e-9);
/// ```
#[must_use]
pub fn crap_score(cyclomatic: u32, coverage: f64) -> f64 {
    let comp = f64::from(cyclomatic);
    let uncovered = (1.0 - coverage.clamp(0.0, 1.0)).powi(3);
    comp * comp * uncovered + comp
}

/// Pushes the direct children of `parent` onto `stack` in reverse order, so
/// a LIFO pop reproduces left-to-right traversal. Iterates via a `TreeCursor`
/// — sibling walks are O(1) each, vs. O(i) for `parent.child(i)`. Skipping
/// this matters: high-fanout nodes (1 K+ children, common in switch-heavy
/// C files like `kernel/bpf/verifier.c`) turn `for i in 0..N { child(i) }`
/// into an O(N²) trap that dominated indexing time before this helper.
fn push_children<'a>(
    stack: &mut Vec<(TsNode<'a>, u32, u32)>,
    parent: TsNode<'a>,
    depth: u32,
    cog_depth: u32,
) {
    let start = stack.len();
    let mut cursor = parent.walk();
    if cursor.goto_first_child() {
        loop {
            stack.push((cursor.node(), depth, cog_depth));
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    // Reverse the slice we just appended so the next `pop()` sees the
    // first child first.
    stack[start..].reverse();
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

    // Fallback: scan direct children via cursor (O(N), not O(N²)).
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
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
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

/// Extracts the macro name from a macro invocation node (e.g. `assert!`).
///
/// Looks for the first identifier child, stripping a trailing `!` if present.
fn extract_macro_name(node: TsNode<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            let ck = child.kind();
            if ck == "identifier" || ck == "scoped_identifier" {
                if let Ok(text) = child.utf8_text(source) {
                    return Some(text.trim_end_matches('!').to_string());
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
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
    // Walk children via cursor and remember the rightmost match — `node.child(i)`
    // would be O(N²) for the right-to-left scan the previous revision did.
    let mut cursor = node.walk();
    let mut found = String::new();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            let ck = child.kind();
            if ck == "identifier" || ck == "field_identifier" || ck == "property_identifier" {
                if let Ok(text) = child.utf8_text(source) {
                    found = text.to_string();
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Per-language configurations
// ---------------------------------------------------------------------------

pub static RUST_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "match_arm", "else_clause"],
    loop_types: &["for_expression", "while_expression", "loop_expression"],
    return_types: &[
        "return_expression",
        "break_expression",
        "continue_expression",
    ],
    nesting_types: &["block"],
    unsafe_types: &["unsafe_block"],
    unchecked_types: &[],
    unchecked_methods: &["unwrap", "expect"],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &[
        "assert",
        "assert_eq",
        "assert_ne",
        "debug_assert",
        "debug_assert_eq",
        "debug_assert_ne",
    ],
    macro_invocation_types: &["macro_invocation"],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "assignment_expression",
        "compound_assignment_expr",
        "call_expression",
        "field_expression",
        "index_expression",
        "reference_expression",
        "try_expression",
        "await_expression",
        "macro_invocation",
        "range_expression",
    ],
    operand_types: &[
        "identifier",
        "field_identifier",
        "integer_literal",
        "float_literal",
        "string_literal",
        "char_literal",
        "boolean_literal",
        "self",
    ],
};

pub static JAVA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_block_statement_group",
        "ternary_expression",
        "catch_clause",
        "else",
    ],
    loop_types: &[
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["get"],
    call_expression_types: &["method_invocation"],
    call_method_field: "name",
    assertion_names: &[
        "assert",
        "assertEquals",
        "assertNotEquals",
        "assertTrue",
        "assertFalse",
        "assertNull",
        "assertNotNull",
        "assertThrows",
        "assertThat",
        "assertArrayEquals",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "assignment_expression",
        "update_expression",
        "method_invocation",
        "object_creation_expression",
        "field_access",
        "array_access",
        "cast_expression",
        "instanceof_expression",
    ],
    operand_types: &[
        "identifier",
        "decimal_integer_literal",
        "hex_integer_literal",
        "decimal_floating_point_literal",
        "string_literal",
        "character_literal",
        "true",
        "false",
        "null_literal",
        "this",
    ],
};

pub static GO_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "expression_case",
        "type_case",
        "default_case",
    ],
    loop_types: &["for_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &[
        "assert", "require", "Equal", "NotEqual", "True", "False", "Nil", "NotNil", "Error",
        "NoError",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "assignment_statement",
        "inc_statement",
        "dec_statement",
        "call_expression",
        "selector_expression",
        "index_expression",
        "composite_literal",
    ],
    operand_types: &[
        "identifier",
        "field_identifier",
        "int_literal",
        "float_literal",
        "interpreted_string_literal",
        "raw_string_literal",
        "rune_literal",
        "true",
        "false",
        "nil",
    ],
};

pub static PYTHON_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "elif_clause",
        "else_clause",
        "conditional_expression",
        "except_clause",
    ],
    loop_types: &["for_statement", "while_statement"],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "raise_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call"],
    call_method_field: "function",
    assertion_names: &[
        "assert",
        "assertEqual",
        "assertNotEqual",
        "assertTrue",
        "assertFalse",
        "assertIs",
        "assertIsNone",
        "assertIsNotNone",
        "assertIn",
        "assertRaises",
        "assertAlmostEqual",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_operator",
        "boolean_operator",
        "comparison_operator",
        "unary_operator",
        "not_operator",
        "assignment",
        "augmented_assignment",
        "call",
        "attribute",
        "subscript",
    ],
    operand_types: &[
        "identifier",
        "integer",
        "float",
        "string",
        "true",
        "false",
        "none",
    ],
};

pub static TYPESCRIPT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_case",
        "ternary_expression",
        "catch_clause",
        "else_clause",
    ],
    loop_types: &[
        "for_statement",
        "for_in_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["statement_block"],
    unsafe_types: &[],
    unchecked_types: &["non_null_assertion_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &[
        "assert",
        "expect",
        "assertEquals",
        "assertStrictEquals",
        "deepEqual",
        "strictEqual",
        "ok",
        "notOk",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "update_expression",
        "assignment_expression",
        "augmented_assignment_expression",
        "call_expression",
        "member_expression",
        "subscript_expression",
        "new_expression",
        "await_expression",
        "ternary_expression",
    ],
    operand_types: &[
        "identifier",
        "property_identifier",
        "number",
        "string",
        "template_string",
        "true",
        "false",
        "null",
        "undefined",
        "this",
    ],
};

pub static C_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "case_statement",
        "conditional_expression",
        "else_clause",
    ],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &[
        "assert",
        "assert_true",
        "assert_false",
        "assert_int_equal",
        "assert_string_equal",
        "assert_null",
        "assert_non_null",
        "CU_ASSERT",
        "CU_ASSERT_EQUAL",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "update_expression",
        "assignment_expression",
        "call_expression",
        "field_expression",
        "subscript_expression",
        "pointer_expression",
        "cast_expression",
        "sizeof_expression",
    ],
    operand_types: &[
        "identifier",
        "field_identifier",
        "number_literal",
        "string_literal",
        "char_literal",
        "true",
        "false",
        "null",
    ],
};

pub static CPP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "case_statement",
        "conditional_expression",
        "catch_clause",
        "else_clause",
    ],
    loop_types: &[
        "for_statement",
        "while_statement",
        "do_statement",
        "for_range_loop",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &[
        "assert",
        "ASSERT_TRUE",
        "ASSERT_FALSE",
        "ASSERT_EQ",
        "ASSERT_NE",
        "ASSERT_LT",
        "ASSERT_GT",
        "EXPECT_TRUE",
        "EXPECT_FALSE",
        "EXPECT_EQ",
        "EXPECT_NE",
        "static_assert",
    ],
    macro_invocation_types: &[],
    operator_types: &[
        "binary_expression",
        "unary_expression",
        "update_expression",
        "assignment_expression",
        "call_expression",
        "field_expression",
        "subscript_expression",
        "pointer_expression",
        "cast_expression",
        "new_expression",
        "delete_expression",
        "sizeof_expression",
    ],
    operand_types: &[
        "identifier",
        "field_identifier",
        "number_literal",
        "string_literal",
        "char_literal",
        "true",
        "false",
        "nullptr",
        "this",
    ],
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
    assertion_names: &[
        "assert",
        "assertEquals",
        "assertNotEquals",
        "assertTrue",
        "assertFalse",
        "assertNull",
        "assertNotNull",
        "assertIs",
        "assertIsNot",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
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
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-dart")]
pub static DART_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_statement_case",
        "catch_clause",
        "conditional_expression",
    ],
    loop_types: &["for_statement", "while_statement", "do_statement"],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &["postfix_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "expect", "expectLater", "expectAsync"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

pub static CSHARP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_section",
        "conditional_expression",
        "catch_clause",
    ],
    loop_types: &[
        "for_statement",
        "for_each_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &["unsafe_statement"],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation_expression"],
    call_method_field: "function",
    assertion_names: &[
        "Assert",
        "AreEqual",
        "AreNotEqual",
        "IsTrue",
        "IsFalse",
        "IsNull",
        "IsNotNull",
        "ThrowsException",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-pascal")]
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
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-php")]
pub static PHP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "case_statement",
        "catch_clause",
        "else_clause",
        "else_if_clause",
    ],
    loop_types: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_expression",
    ],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["function_call_expression", "member_call_expression"],
    call_method_field: "name",
    assertion_names: &[
        "assert",
        "assertEquals",
        "assertNotEquals",
        "assertTrue",
        "assertFalse",
        "assertNull",
        "assertNotNull",
        "assertSame",
        "assertInstanceOf",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-ruby")]
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
    assertion_names: &[
        "assert",
        "assert_equal",
        "assert_not_equal",
        "assert_nil",
        "assert_not_nil",
        "assert_raises",
        "assert_match",
        "refute",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

pub static SWIFT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_entry",
        "guard_statement",
        "catch_keyword",
    ],
    loop_types: &[
        "for_in_statement",
        "while_statement",
        "repeat_while_statement",
    ],
    return_types: &["control_transfer_statement"],
    nesting_types: &["code_block"],
    unsafe_types: &[],
    unchecked_types: &["force_unwrap_expression"],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &[
        "assert",
        "precondition",
        "assertionFailure",
        "XCTAssert",
        "XCTAssertEqual",
        "XCTAssertTrue",
        "XCTAssertFalse",
        "XCTAssertNil",
        "XCTAssertNotNil",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-bash")]
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
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-lua")]
pub static LUA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elseif_statement", "else_statement"],
    loop_types: &[
        "for_statement",
        "for_in_statement",
        "while_statement",
        "repeat_statement",
    ],
    return_types: &["return_statement", "break_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["function_call"],
    call_method_field: "",
    assertion_names: &["assert", "assert_equal", "assert_true", "assert_false"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-zig")]
pub static ZIG_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_expression",
        "switch_expression",
        "else_expression",
        "catch",
    ],
    loop_types: &["for_expression", "while_expression"],
    return_types: &[
        "return_expression",
        "break_expression",
        "continue_expression",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["orelse"],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &["expect", "expectEqual", "expectEqualStrings", "expectError"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-nix")]
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
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-vbnet")]
pub static VBNET_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "elseif_clause",
        "else_clause",
        "select_case_statement",
        "catch_clause",
    ],
    loop_types: &[
        "for_statement",
        "for_each_statement",
        "while_statement",
        "do_loop_statement",
    ],
    return_types: &["return_statement", "exit_statement", "throw_statement"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation_expression"],
    call_method_field: "",
    assertion_names: &[
        "Assert",
        "AreEqual",
        "AreNotEqual",
        "IsTrue",
        "IsFalse",
        "IsNull",
        "IsNotNull",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-powershell")]
pub static POWERSHELL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "elseif_clause",
        "else_clause",
        "switch_statement",
        "catch_clause",
    ],
    loop_types: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "do_while_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["script_block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["command_expression"],
    call_method_field: "",
    assertion_names: &["Should", "Assert"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-perl")]
pub static PERL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "elsif_clause",
        "else_clause",
        "unless_statement",
        "conditional_expression",
    ],
    loop_types: &[
        "for_statement",
        "foreach_statement",
        "while_statement",
        "until_statement",
    ],
    return_types: &["return_expression", "last_expression", "next_expression"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression", "method_call_expression"],
    call_method_field: "",
    assertion_names: &["ok", "is", "isnt", "like", "unlike", "cmp_ok", "is_deeply"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-objc")]
pub static OBJC_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "case_statement",
        "conditional_expression",
        "catch_clause",
        "else_clause",
    ],
    loop_types: &[
        "for_statement",
        "while_statement",
        "do_statement",
        "for_in_statement",
    ],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression", "message_expression"],
    call_method_field: "",
    assertion_names: &[
        "NSAssert",
        "NSCAssert",
        "XCTAssert",
        "XCTAssertTrue",
        "XCTAssertFalse",
        "XCTAssertEqual",
        "XCTAssertNil",
        "XCTAssertNotNil",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-fortran")]
pub static FORTRAN_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "elseif_clause",
        "else_clause",
        "case_statement",
        "where_statement",
    ],
    loop_types: &["do_loop_statement", "forall_statement"],
    return_types: &[
        "return_statement",
        "stop_statement",
        "exit_statement",
        "cycle_statement",
    ],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-cobol")]
pub static COBOL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_header", "evaluate_statement", "when_phrase"],
    loop_types: &["perform_statement_loop"],
    return_types: &["stop_statement", "goback_statement"],
    nesting_types: &[],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["perform_statement_call_proc"],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-msbasic2")]
pub static MSBASIC2_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement"],
    loop_types: &["for_statement"],
    return_types: &["return_statement"],
    nesting_types: &[],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &[],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-gwbasic")]
pub static GWBASIC_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement"],
    loop_types: &["for_statement", "while_statement"],
    return_types: &["return_statement"],
    nesting_types: &[],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &[],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-qbasic")]
pub static QBASIC_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["block_if_statement"],
    loop_types: &["for_statement", "while_statement", "do_loop_statement"],
    return_types: &["exit_statement"],
    nesting_types: &[],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_statement"],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-r")]
pub static R_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement"],
    loop_types: &["for_statement", "while_statement", "repeat_statement"],
    return_types: &["return"],
    nesting_types: &["braced_expression"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call"],
    call_method_field: "",
    assertion_names: &[
        "stopifnot",
        "assert_that",
        "expect_equal",
        "expect_true",
        "expect_false",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-sql")]
pub static SQL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if", "when_clause"],
    loop_types: &["loop"],
    return_types: &["return"],
    nesting_types: &["block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["invocation"],
    call_method_field: "",
    assertion_names: &[],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-julia")]
pub static JULIA_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_statement", "elseif_clause", "ternary_expression"],
    loop_types: &["for_statement", "while_statement"],
    return_types: &["return_statement", "break_statement", "continue_statement"],
    nesting_types: &["block", "compound_statement"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "",
    assertion_names: &["@assert", "assert", "@test", "@test_throws"],
    macro_invocation_types: &["macro_expression"],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-haskell")]
pub static HASKELL_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["alternative", "guard"],
    loop_types: &[],
    return_types: &[],
    nesting_types: &["where"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &["fromJust", "head"],
    call_expression_types: &["apply"],
    call_method_field: "",
    assertion_names: &[
        "assertBool",
        "assertEqual",
        "assertTrue",
        "assertFailure",
        "shouldBe",
        "shouldSatisfy",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-ocaml")]
pub static OCAML_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "match_case"],
    loop_types: &["for_expression", "while_expression"],
    return_types: &[],
    nesting_types: &["let_binding"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["application_expression"],
    call_method_field: "",
    assertion_names: &[
        "assert",
        "assert_equal",
        "assert_string_equal",
        "assert_bool",
        "check_bool",
    ],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-clojure")]
pub static CLOJURE_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["list_lit"],
    loop_types: &[],
    return_types: &[],
    nesting_types: &[],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["list_lit"],
    call_method_field: "",
    assertion_names: &["assert", "is", "are", "testing"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-erlang")]
pub static ERLANG_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["cr_clause", "if_clause"],
    loop_types: &[],
    return_types: &[],
    nesting_types: &["clause_body"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call"],
    call_method_field: "",
    assertion_names: &["assertEqual", "assert", "assertMatch", "assertError"],
    macro_invocation_types: &["macro_application"],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-elixir")]
pub static ELIXIR_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["stab_clause"],
    loop_types: &[],
    return_types: &[],
    nesting_types: &["do_block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call"],
    call_method_field: "",
    assertion_names: &["assert", "assert_raise", "assert_receive", "refute"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

#[cfg(feature = "lang-fsharp")]
pub static FSHARP_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &["if_expression", "elif_expression", "match_expression"],
    loop_types: &["for_expression", "while_expression"],
    return_types: &[],
    nesting_types: &["sequential_expression"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["application_expression"],
    call_method_field: "",
    assertion_names: &["Assert", "assertEqual", "assertTrue", "assertFalse"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};

/// ActionScript 2/3 (tree-sitter-actionscript grammar).
pub static ACTIONSCRIPT_COMPLEXITY: ComplexityConfig = ComplexityConfig {
    branch_types: &[
        "if_statement",
        "switch_case",
        "switch_default",
        "ternary_expression",
        "catch_clause",
    ],
    loop_types: &[
        "for_statement",
        "for_in_statement",
        "for_each_in_statement",
        "while_statement",
        "do_statement",
    ],
    return_types: &[
        "return_statement",
        "break_statement",
        "continue_statement",
        "throw_statement",
    ],
    nesting_types: &["statement_block"],
    unsafe_types: &[],
    unchecked_types: &[],
    unchecked_methods: &[],
    call_expression_types: &["call_expression"],
    call_method_field: "function",
    assertion_names: &["assert", "assertEquals", "assertTrue", "assertFalse"],
    macro_invocation_types: &[],
    operator_types: &[],
    operand_types: &[],
};
