//! Companion-doc discovery (#154 phase 1): sidecar and docs-directory
//! conventions, front-matter parsing, and coverage resolution.

use tokensave::docs::{
    discover_docs, is_in_docs_dir, parse_front_matter, resolve_globs, resolve_sidecar,
    sidecar_stem, DocOrigin, DEFAULT_DOCS_DIR,
};

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(ToString::to_string).collect()
}

// ---------------------------------------------------------------------------
// Sidecar recognition
// ---------------------------------------------------------------------------

#[test]
fn sidecar_stem_matches_the_reporters_convention() {
    assert_eq!(
        sidecar_stem("xxx/yyy/super_big_class.readme.md"),
        Some("xxx/yyy/super_big_class")
    );
    // Case-insensitive: README.md is conventionally shouted.
    assert_eq!(sidecar_stem("src/Foo.README.md"), Some("src/Foo"));
}

#[test]
fn sidecar_stem_rejects_non_sidecar_markdown() {
    // A plain doc, not a sidecar.
    assert_eq!(sidecar_stem("docs/guide.md"), None);
    // Top-level README is not a sidecar for anything.
    assert_eq!(sidecar_stem("README.md"), None);
    // Suffix present but no stem in front of it.
    assert_eq!(sidecar_stem(".readme.md"), None);
    assert_eq!(sidecar_stem("notes.txt"), None);
}

#[test]
fn sidecar_covers_every_extension_sharing_the_stem() {
    let files = owned(&[
        "src/Foo.cs",
        "src/Foo.designer.cs",
        "src/Foo.readme.md",
        "src/FooBar.cs",
        "src/other/Foo.cs",
    ]);
    let covered = resolve_sidecar("src/Foo", &files);
    // Sibling stem prefix (FooBar) and a same-named file in another directory
    // must not be swept in, and the doc never documents itself.
    assert_eq!(covered, vec!["src/Foo.cs", "src/Foo.designer.cs"]);
}

#[test]
fn sidecar_does_not_document_another_doc() {
    let files = owned(&["src/Foo.readme.md", "src/Foo.notes.readme.md"]);
    assert!(resolve_sidecar("src/Foo", &files).is_empty());
}

#[test]
fn sidecar_does_not_reach_into_a_nested_directory() {
    // `src/Foo.readme.md` must not claim `src/Foo.d/inner.cs`.
    let files = owned(&["src/Foo.d/inner.cs"]);
    assert!(resolve_sidecar("src/Foo", &files).is_empty());
}

// ---------------------------------------------------------------------------
// Docs directory
// ---------------------------------------------------------------------------

#[test]
fn docs_dir_membership_is_prefix_scoped() {
    assert!(is_in_docs_dir("tokensave-docs/es.md", DEFAULT_DOCS_DIR));
    assert!(is_in_docs_dir(
        "tokensave-docs/nested/es.md",
        DEFAULT_DOCS_DIR
    ));
    // A directory that merely starts with the same characters is not a match.
    assert!(!is_in_docs_dir(
        "tokensave-docs-old/es.md",
        DEFAULT_DOCS_DIR
    ));
    assert!(!is_in_docs_dir("docs/es.md", DEFAULT_DOCS_DIR));
    // Configurable location.
    assert!(is_in_docs_dir("meta/ai/es.md", "meta/ai"));
    assert!(is_in_docs_dir("meta/ai/es.md", "meta/ai/"));
}

#[test]
fn front_matter_reads_block_sequence_form() {
    let content = "---\napplies_to:\n  - \"**/*.es8.cs\"\n  - '**/*.es7.cs'\n---\n\n# ES\nprose\n";
    let front = parse_front_matter(content).expect("front matter");
    assert_eq!(front.applies_to, vec!["**/*.es8.cs", "**/*.es7.cs"]);
}

#[test]
fn front_matter_reads_inline_sequence_form() {
    let content =
        "---\ntitle: ES8 driver\napplies_to: [\"**/*.es8.cs\", src/Legacy.cs]\n---\nbody\n";
    let front = parse_front_matter(content).expect("front matter");
    assert_eq!(front.applies_to, vec!["**/*.es8.cs", "src/Legacy.cs"]);
}

#[test]
fn front_matter_tolerates_crlf_and_bom() {
    let content = "\u{feff}---\r\napplies_to:\r\n  - \"src/*.cs\"\r\n---\r\nbody\r\n";
    let front = parse_front_matter(content).expect("front matter");
    assert_eq!(front.applies_to, vec!["src/*.cs"]);
}

#[test]
fn front_matter_absent_or_unterminated_yields_none() {
    assert!(parse_front_matter("# Just a heading\n").is_none());
    // An unterminated fence must not silently swallow the whole document.
    assert!(parse_front_matter("---\napplies_to:\n  - a.cs\n").is_none());
}

#[test]
fn front_matter_without_applies_to_covers_nothing() {
    let front = parse_front_matter("---\ntitle: notes\n---\nbody\n").expect("front matter");
    assert!(front.applies_to.is_empty());
}

#[test]
fn front_matter_stops_the_sequence_at_the_next_key() {
    let content = "---\napplies_to:\n  - a.cs\nowner: platform\n  - b.cs\n---\nbody\n";
    let front = parse_front_matter(content).expect("front matter");
    assert_eq!(front.applies_to, vec!["a.cs"]);
}

#[test]
fn globs_match_full_project_relative_paths() {
    let files = owned(&[
        "src/Search.es8.cs",
        "src/Search.es7.cs",
        "src/deep/nested/Other.es8.cs",
        "src/Unrelated.cs",
    ]);
    let matched = resolve_globs(&owned(&["**/*.es8.cs"]), &files);
    assert_eq!(
        matched,
        vec!["src/Search.es8.cs", "src/deep/nested/Other.es8.cs"]
    );
}

#[test]
fn globs_matching_nothing_yield_nothing() {
    let files = owned(&["src/a.cs"]);
    assert!(resolve_globs(&owned(&["**/*.kt"]), &files).is_empty());
    // A malformed pattern must not panic or match everything.
    assert!(resolve_globs(&owned(&["["]), &files).is_empty());
}

// ---------------------------------------------------------------------------
// End-to-end discovery
// ---------------------------------------------------------------------------

#[test]
fn discovery_handles_both_conventions_together() {
    let markdown = owned(&[
        "src/BigClass.readme.md",
        "tokensave-docs/es8.md",
        "tokensave-docs/no-front-matter.md",
        "docs/unrelated.md",
        "README.md",
    ]);
    let indexed = owned(&[
        "src/BigClass.cs",
        "src/Search.es8.cs",
        "src/other/Feed.es8.cs",
        "src/Plain.cs",
    ]);
    let docs = discover_docs(&markdown, &indexed, DEFAULT_DOCS_DIR, |path| match path {
        "tokensave-docs/es8.md" => {
            Some("---\napplies_to:\n  - \"**/*.es8.cs\"\n---\nES8 notes\n".to_string())
        }
        "tokensave-docs/no-front-matter.md" => Some("# no coverage declared\n".to_string()),
        _ => None,
    });

    assert_eq!(docs.len(), 2, "{docs:?}");

    let sidecar = &docs[0];
    assert_eq!(sidecar.path, "src/BigClass.readme.md");
    assert_eq!(sidecar.origin, DocOrigin::Sidecar);
    assert_eq!(sidecar.covers, vec!["src/BigClass.cs"]);

    let dir_doc = &docs[1];
    assert_eq!(dir_doc.path, "tokensave-docs/es8.md");
    assert_eq!(dir_doc.origin, DocOrigin::DocsDir);
    assert_eq!(
        dir_doc.covers,
        vec!["src/Search.es8.cs", "src/other/Feed.es8.cs"]
    );
}

#[test]
fn discovery_drops_docs_that_cover_nothing() {
    // A doc whose glob has gone empty (files deleted) is not an error, but
    // there is no mapping worth recording either.
    let markdown = owned(&["tokensave-docs/gone.md", "src/Removed.readme.md"]);
    let indexed = owned(&["src/Present.cs"]);
    let docs = discover_docs(&markdown, &indexed, DEFAULT_DOCS_DIR, |_| {
        Some("---\napplies_to:\n  - \"**/*.deleted\"\n---\n".to_string())
    });
    assert!(docs.is_empty(), "{docs:?}");
}

#[test]
fn discovery_respects_a_custom_docs_dir() {
    let markdown = owned(&["meta/ai/guide.md", "tokensave-docs/ignored.md"]);
    let indexed = owned(&["src/a.cs"]);
    let docs = discover_docs(&markdown, &indexed, "meta/ai", |_| {
        Some("---\napplies_to: [\"src/*.cs\"]\n---\n".to_string())
    });
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].path, "meta/ai/guide.md");
}
