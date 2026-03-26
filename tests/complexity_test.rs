use tokensave::extraction::complexity::{count_complexity, RUST_COMPLEXITY};

/// Helper: parse Rust source, find the first `function_item` node, and return its complexity.
fn rust_fn_complexity(source: &str) -> tokensave::extraction::complexity::ComplexityMetrics {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("failed to load Rust grammar");
    let tree = parser.parse(source, None).expect("parse failed");
    let root = tree.root_node();
    let fn_node = find_first_kind(root, "function_item")
        .expect("no function_item found in source");
    count_complexity(fn_node, &RUST_COMPLEXITY, source.as_bytes())
}

/// Recursively find the first node of the given kind.
fn find_first_kind<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if let Some(found) = find_first_kind(cursor.node(), kind) {
                return Some(found);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

// ── Branch counting ─────────────────────────────────────────────────────────

#[test]
fn test_complexity_no_branches() {
    let m = rust_fn_complexity("fn simple() { let x = 1; }");
    assert_eq!(m.branches, 0);
    assert_eq!(m.loops, 0);
    assert_eq!(m.returns, 0);
}

#[test]
fn test_complexity_single_if() {
    let m = rust_fn_complexity(r#"
fn check(x: i32) {
    if x > 0 {
        println!("positive");
    }
}
"#);
    assert_eq!(m.branches, 1, "single if = 1 branch");
}

#[test]
fn test_complexity_if_else() {
    let m = rust_fn_complexity(r#"
fn check(x: i32) {
    if x > 0 {
        println!("positive");
    } else {
        println!("non-positive");
    }
}
"#);
    // if + else_clause
    assert!(m.branches >= 2, "if/else = at least 2 branches, got {}", m.branches);
}

#[test]
fn test_complexity_match_arms() {
    let m = rust_fn_complexity(r#"
fn classify(x: i32) -> &'static str {
    match x {
        0 => "zero",
        1..=9 => "small",
        _ => "big",
    }
}
"#);
    assert!(m.branches >= 3, "match with 3 arms = at least 3 branches, got {}", m.branches);
}

// ── Loop counting ───────────────────────────────────────────────────────────

#[test]
fn test_complexity_for_loop() {
    let m = rust_fn_complexity(r#"
fn sum(items: &[i32]) -> i32 {
    let mut s = 0;
    for &x in items {
        s += x;
    }
    s
}
"#);
    assert_eq!(m.loops, 1);
}

#[test]
fn test_complexity_while_loop() {
    let m = rust_fn_complexity(r#"
fn countdown(mut n: i32) {
    while n > 0 {
        n -= 1;
    }
}
"#);
    assert_eq!(m.loops, 1);
}

#[test]
fn test_complexity_loop_keyword() {
    let m = rust_fn_complexity(r#"
fn infinite() {
    loop {
        break;
    }
}
"#);
    assert_eq!(m.loops, 1);
}

// ── Return / early exit counting ────────────────────────────────────────────

#[test]
fn test_complexity_return_and_break() {
    let m = rust_fn_complexity(r#"
fn find(items: &[i32], target: i32) -> Option<usize> {
    for (i, &val) in items.iter().enumerate() {
        if val == target {
            return Some(i);
        }
    }
    None
}
"#);
    assert!(m.returns >= 1, "expected at least 1 return");
}

// ── Nesting depth ───────────────────────────────────────────────────────────

#[test]
fn test_complexity_nesting_depth() {
    let m = rust_fn_complexity(r#"
fn deep(x: i32) {
    if x > 0 {
        for i in 0..x {
            if i > 5 {
                println!("deep");
            }
        }
    }
}
"#);
    assert!(m.max_nesting >= 3, "expected nesting >= 3, got {}", m.max_nesting);
}

#[test]
fn test_complexity_flat_function() {
    let m = rust_fn_complexity(r#"
fn flat() {
    let a = 1;
    let b = 2;
    let c = a + b;
}
"#);
    // The function body block itself counts as nesting level 1
    assert!(m.max_nesting <= 1, "flat function should have low nesting, got {}", m.max_nesting);
}

// ── Unsafe blocks ───────────────────────────────────────────────────────────

#[test]
fn test_complexity_unsafe_block() {
    let m = rust_fn_complexity(r#"
fn dangerous() {
    unsafe {
        std::ptr::null::<i32>().read();
    }
    unsafe {
        std::ptr::null::<i32>().read();
    }
}
"#);
    assert_eq!(m.unsafe_blocks, 2, "expected 2 unsafe blocks");
}

#[test]
fn test_complexity_no_unsafe() {
    let m = rust_fn_complexity("fn safe() { let x = 42; }");
    assert_eq!(m.unsafe_blocks, 0);
}

// ── Unchecked calls (unwrap/expect) ─────────────────────────────────────────

#[test]
fn test_complexity_unwrap_detection() {
    let m = rust_fn_complexity(r#"
fn risky(v: Option<i32>) -> i32 {
    v.unwrap()
}
"#);
    assert!(m.unchecked_calls >= 1, "expected unwrap to be detected, got {}", m.unchecked_calls);
}

#[test]
fn test_complexity_expect_detection() {
    let m = rust_fn_complexity(r#"
fn risky(v: Option<i32>) -> i32 {
    v.expect("missing")
}
"#);
    assert!(m.unchecked_calls >= 1, "expected expect() to be detected, got {}", m.unchecked_calls);
}

#[test]
fn test_complexity_no_unchecked() {
    let m = rust_fn_complexity(r#"
fn safe(v: Option<i32>) -> i32 {
    v.unwrap_or(0)
}
"#);
    // unwrap_or is NOT in the unchecked list
    assert_eq!(m.unchecked_calls, 0, "unwrap_or should not be flagged");
}

// ── Assertion detection ─────────────────────────────────────────────────────

#[test]
fn test_complexity_assert_macro() {
    let m = rust_fn_complexity(r#"
fn checked(x: i32) {
    assert!(x > 0);
    assert_eq!(x, 42);
    debug_assert!(x < 100);
}
"#);
    assert!(m.assertions >= 3, "expected >= 3 assertions, got {}", m.assertions);
}

#[test]
fn test_complexity_no_assertions() {
    let m = rust_fn_complexity("fn plain() { let x = 1; }");
    assert_eq!(m.assertions, 0);
}

// ── Combined complexity ─────────────────────────────────────────────────────

#[test]
fn test_complexity_combined() {
    let m = rust_fn_complexity(r#"
fn complex(data: &[Option<i32>]) -> i32 {
    let mut sum = 0;
    for item in data {
        if let Some(val) = item {
            match val {
                0 => continue,
                n if *n < 0 => {
                    unsafe { std::ptr::read(n) };
                }
                n => {
                    sum += n.checked_add(1).unwrap();
                }
            }
        }
    }
    assert!(sum >= 0);
    sum
}
"#);
    assert!(m.branches >= 2, "branches: {}", m.branches);
    assert!(m.loops >= 1, "loops: {}", m.loops);
    assert!(m.unsafe_blocks >= 1, "unsafe: {}", m.unsafe_blocks);
    assert!(m.unchecked_calls >= 1, "unchecked: {}", m.unchecked_calls);
    assert!(m.assertions >= 1, "assertions: {}", m.assertions);
    assert!(m.max_nesting >= 3, "nesting: {}", m.max_nesting);
}
