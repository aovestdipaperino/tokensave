//! `field_sites` counts must be totals, and a qualifier must not silently
//! widen the answer — #457 and #458.
//!
//! The tool exists to answer one question: if I change this field, how many
//! places are affected? `write_count` was `writes.len()` *after* the `limit`
//! cap, so it tracked the limit exactly — asking for 20 reported 20, asking
//! for 21 reported 21 — and a capped page read as an authoritative total.
//! That understates a blast radius in precisely the case where the number
//! matters, with nothing in the response to say so.
//!
//! Separately, `Type::field` used to be parsed into a qualifier that was
//! never applied: the sites returned were the bare name's, i.e. the broad
//! answer under a narrow heading. It now narrows for real — a site is kept
//! only when its receiver resolves to the named type — and the sites it
//! cannot type are counted rather than quietly included or dropped, so a
//! narrowed answer never poses as a complete one.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Two structs share the field name `count`, so a qualifier has something to
/// narrow to if it ever narrows. `writes()` holds twelve write sites, enough
/// to cap several ways, and one line carries two writes so a site count and a
/// line count are distinguishable.
async fn project_with_many_write_sites() -> (TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path().to_path_buf();

    git(&root, &["init", "-b", "master"]);

    let mut src = String::from(
        "pub struct Counter { pub count: u32 }\n\
         pub struct Tally { pub count: u32 }\n\n\
         pub fn writes(a: &mut Counter, b: &mut Tally) {\n",
    );
    // Ten single-write lines.
    for i in 0..10 {
        src.push_str(&format!("    a.count = {i};\n"));
    }
    // One line carrying two writes: two sites, one line.
    src.push_str("    a.count = 1; b.count = 2;\n");
    src.push_str("}\n\n");
    src.push_str(
        "pub fn reads(a: &Counter, b: &Tally) -> u32 {\n\
         \x20   a.count + b.count\n\
         }\n",
    );
    std::fs::write(root.join("lib.rs"), src).unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-m", "init"]);

    let cg = TokenSave::init(&root).await.unwrap();
    cg.index_all().await.unwrap();
    (tmp, cg)
}

async fn field_sites(cg: &TokenSave, args: Value) -> Value {
    let result = handle_tool_call(cg, "tokensave_field_sites", args, None, None)
        .await
        .expect("field_sites must succeed");
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("tool result carries text");
    serde_json::from_str(text).expect("field_sites returns JSON")
}

/// The reported repro: the same field, back to back, at two limits. The count
/// followed the limit exactly. It must now stand still.
#[tokio::test]
async fn write_count_is_a_total_and_does_not_track_the_limit() {
    let (_tmp, cg) = project_with_many_write_sites().await;

    let uncapped = field_sites(&cg, json!({ "field": "count", "writes_only": true })).await;
    let total = uncapped["write_count"].as_u64().expect("write_count");

    // Precondition: there must be enough sites for a cap to bite, otherwise
    // this test proves nothing.
    assert!(
        total >= 4,
        "fixture must produce enough write sites to cap, got {total}"
    );
    assert_eq!(
        uncapped["truncated"], false,
        "an uncapped answer is not truncated"
    );
    assert_eq!(
        uncapped["write_returned"].as_u64(),
        Some(total),
        "uncapped, every site is listed"
    );

    for limit in [2u64, 3] {
        let capped = field_sites(
            &cg,
            json!({ "field": "count", "writes_only": true, "limit": limit }),
        )
        .await;
        assert_eq!(
            capped["write_count"].as_u64(),
            Some(total),
            "write_count must stay the true total at limit={limit}, not follow the limit"
        );
        assert_eq!(
            capped["write_returned"].as_u64(),
            Some(limit),
            "write_returned is the page size at limit={limit}"
        );
        assert_eq!(
            capped["write_sites"].as_array().map(Vec::len),
            Some(limit as usize),
            "the array is capped at limit={limit}"
        );
        assert_eq!(
            capped["truncated"], true,
            "a capped answer must say it was capped at limit={limit}"
        );
        assert!(
            capped["truncation_note"].as_str().is_some(),
            "a capped answer explains the two numbers at limit={limit}"
        );
    }
}

/// The secondary report: one entry per occurrence, not per site, so a count
/// was neither a site count nor a line count. Both are now named.
#[tokio::test]
async fn sites_and_distinct_lines_are_separately_reported() {
    let (_tmp, cg) = project_with_many_write_sites().await;
    let payload = field_sites(&cg, json!({ "field": "count", "writes_only": true })).await;

    let sites = payload["write_count"].as_u64().expect("write_count");
    let lines = payload["write_lines"].as_u64().expect("write_lines");
    assert!(
        lines < sites,
        "the fixture puts two writes on one line, so lines ({lines}) must be \
         fewer than sites ({sites})"
    );
    assert_eq!(
        sites - lines,
        1,
        "exactly one line carries a second write site"
    );
}

/// The dangerous direction: the caller asked to narrow. Now that it really
/// narrows, the thing to pin is that it narrows to the *right* sites — the
/// fixture writes `a.count` on a `Counter` and `b.count` on a `Tally`, so a
/// qualifier that works must keep one and drop the other.
#[tokio::test]
async fn a_qualifier_narrows_to_the_named_type() {
    let (_tmp, cg) = project_with_many_write_sites().await;

    let bare = field_sites(&cg, json!({ "field": "count", "writes_only": true })).await;
    let qualified = field_sites(
        &cg,
        json!({ "field": "Counter::count", "writes_only": true }),
    )
    .await;

    assert_eq!(qualified["qualifier"], "Counter");
    assert_eq!(
        qualified["qualifier_applied"], true,
        "the qualified form must now be applied, not merely echoed"
    );

    let bare_total = bare["write_count"].as_u64().expect("bare write_count");
    let narrow_total = qualified["write_count"].as_u64().expect("write_count");
    assert!(
        narrow_total < bare_total,
        "narrowing must drop the other struct's sites: {narrow_total} vs {bare_total}"
    );
    assert!(
        qualified["excluded_count"]
            .as_u64()
            .expect("excluded_count")
            > 0,
        "the Tally sites must be reported as excluded, not silently missing"
    );

    // Every site kept must actually be a `Counter` write.
    for site in qualified["write_sites"]
        .as_array()
        .expect("write_sites is an array")
    {
        let snippet = site["snippet"].as_str().unwrap_or_default();
        assert!(
            snippet.contains("a.count"),
            "a kept site must be the Counter receiver, got: {snippet}"
        );
    }

    // The other type's qualifier is the mirror image, and the two partitions
    // must together account for the bare answer — no site invented, none lost.
    let tally = field_sites(&cg, json!({ "field": "Tally::count", "writes_only": true })).await;
    let tally_total = tally["write_count"].as_u64().expect("write_count");
    assert_eq!(
        narrow_total + tally_total,
        bare_total,
        "the two narrowed answers must partition the bare one exactly"
    );

    // And a bare query carries no note to ignore.
    assert!(
        bare.get("qualifier_note").is_none(),
        "a bare field name has no qualifier to explain"
    );
    assert_eq!(bare["qualifier"], Value::Null);
}

/// A qualified query naming a field the type does not declare is answerable
/// outright. Returning every *other* type's sites for it is the original
/// complaint in its purest form.
#[tokio::test]
async fn a_field_the_type_does_not_declare_returns_nothing_and_says_why() {
    let (_tmp, cg) = project_with_many_write_sites().await;

    let result = field_sites(&cg, json!({ "field": "Counter::no_such_field" })).await;
    assert_eq!(result["qualifier_applied"], true);
    assert_eq!(result["write_count"], 0);
    assert_eq!(result["read_count"], 0);
    let note = result["qualifier_note"].as_str().expect("a note");
    assert!(
        note.contains("no_such_field") && note.contains("Counter"),
        "the note must name both the field and the type: {note}"
    );
}

/// A receiver the scan cannot type is neither kept nor dropped silently. The
/// count is what stops a narrowed answer from reading as a complete one.
#[tokio::test]
async fn sites_that_cannot_be_typed_are_counted_rather_than_guessed() {
    let (_tmp, cg) = project_with_many_write_sites().await;
    let qualified = field_sites(
        &cg,
        json!({ "field": "Counter::count", "writes_only": true }),
    )
    .await;

    assert!(
        qualified.get("unattributed_count").is_some(),
        "a narrowed answer must always report how much it could not attribute"
    );
    let note = qualified["qualifier_note"].as_str().expect("a note");
    assert!(
        note.contains("lower bound"),
        "the note must warn that a narrowed count is a lower bound: {note}"
    );
}
