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
//! Separately, `Type::field` is parsed into a qualifier that is never
//! applied: the sites returned are the bare name's, i.e. the broad answer
//! under a narrow heading. Matching a text site to one struct needs the
//! receiver's resolved type, which this source-text scan does not compute, so
//! the fix here is to say so unmissably rather than to pretend.

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

/// The dangerous direction: the caller asked to narrow. It does not narrow,
/// so the response must say that in words, not only in a flag that is easy to
/// miss — and the sites must be exactly the bare-name answer, never a
/// silently different set.
#[tokio::test]
async fn an_unapplied_qualifier_is_stated_not_just_flagged() {
    let (_tmp, cg) = project_with_many_write_sites().await;

    let bare = field_sites(&cg, json!({ "field": "count", "writes_only": true })).await;
    let qualified = field_sites(
        &cg,
        json!({ "field": "Counter::count", "writes_only": true }),
    )
    .await;

    assert_eq!(
        qualified["qualifier"], "Counter",
        "the qualifier is still parsed and echoed back"
    );
    assert_eq!(
        qualified["qualifier_applied"], false,
        "and is still honestly reported as unapplied"
    );

    let note = qualified["qualifier_note"]
        .as_str()
        .expect("an unapplied qualifier must be explained in words");
    assert!(
        note.contains("NOT applied") && note.contains("Counter"),
        "the note must name the qualifier and say it did not apply: {note}"
    );

    // The results really are the broad answer — pinning this stops a future
    // half-narrowing from shipping behind an unchanged `false` flag.
    assert_eq!(
        qualified["write_count"], bare["write_count"],
        "an unapplied qualifier returns the bare-name sites, unchanged"
    );
    assert_eq!(qualified["write_sites"], bare["write_sites"]);

    // And a bare query carries no note to ignore.
    assert!(
        bare.get("qualifier_note").is_none(),
        "a bare field name has no qualifier to explain"
    );
    assert_eq!(bare["qualifier"], Value::Null);
}
