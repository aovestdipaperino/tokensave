//! Regression test for #647: `tokensave_affected` found no tests for a
//! TypeScript module imported with a `.js` specifier.
//!
//! Under `"module": "NodeNext"` (and `"moduleResolution": "bundler"` with
//! `allowImportingTsExtensions` off) TypeScript requires relative imports to
//! name the *emitted* file, so `src/lib/hash.ts` is imported as
//! `../../src/lib/hash.js`. TypeScript maps the specifier back to the source:
//! `.js` to `.ts`/`.tsx`, `.jsx` to `.tsx`, `.mjs` to `.mts`, `.cjs` to `.cts`.
//! File-level dependency queries must apply the same rule.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::EdgeKind;

fn write(project: &std::path::Path, rel: &str, contents: &str) {
    let path = project.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

async fn fixture() -> (TempDir, TokenSave) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    write(
        project,
        "package.json",
        r#"{"name":"nodenext-fixture","type":"module"}"#,
    );
    write(
        project,
        "tsconfig.json",
        r#"{"compilerOptions":{"module":"NodeNext","moduleResolution":"NodeNext"}}"#,
    );
    write(
        project,
        "src/lib/hash.ts",
        "export const HASH_SEED = 7;\n\
         \n\
         export function computeHash(input: string): string {\n\
         \x20 return `${HASH_SEED}:${input.length}`;\n\
         }\n",
    );
    write(
        project,
        "src/lib/view.tsx",
        "export function renderHash(value: string) {\n\
         \x20 return <span>{value}</span>;\n\
         }\n",
    );
    write(
        project,
        "src/lib/esm.mts",
        "export function esmHelper(): number {\n\
         \x20 return 1;\n\
         }\n",
    );
    write(
        project,
        "src/lib/cjs.cts",
        "export function cjsHelper(): number {\n\
         \x20 return 2;\n\
         }\n",
    );

    // Call inside test callbacks.
    write(
        project,
        "tests/lib/hash.test.ts",
        "import { describe, it, expect } from \"vitest\";\n\
         import { computeHash } from \"../../src/lib/hash.js\";\n\
         \n\
         describe(\"computeHash\", () => {\n\
         \x20 it(\"hashes\", () => {\n\
         \x20   expect(computeHash(\"abc\")).toBe(\"7:3\");\n\
         \x20 });\n\
         });\n",
    );
    // Uses only a constant, never calls a function from the module.
    write(
        project,
        "tests/lib/seed.test.ts",
        "import { HASH_SEED } from \"../../src/lib/hash.js\";\n\
         import { test, expect } from \"vitest\";\n\
         \n\
         test(\"seed\", () => {\n\
         \x20 expect(HASH_SEED).toBe(7);\n\
         });\n",
    );
    // Namespace import.
    write(
        project,
        "tests/lib/namespace.test.ts",
        "import * as hash from \"../../src/lib/hash.js\";\n\
         import { test, expect } from \"vitest\";\n\
         \n\
         test(\"namespace\", () => {\n\
         \x20 expect(typeof hash).toBe(\"object\");\n\
         });\n",
    );
    write(
        project,
        "tests/lib/view.test.tsx",
        "import { renderHash } from \"../../src/lib/view.jsx\";\n\
         import { test } from \"vitest\";\n\
         \n\
         test(\"view\", () => {\n\
         \x20 renderHash(\"x\");\n\
         });\n",
    );
    write(
        project,
        "tests/lib/esm.test.ts",
        "import { esmHelper } from \"../../src/lib/esm.mjs\";\n\
         import { test } from \"vitest\";\n\
         \n\
         test(\"esm\", () => {\n\
         \x20 esmHelper();\n\
         });\n",
    );
    write(
        project,
        "tests/lib/cjs.test.ts",
        "import { cjsHelper } from \"../../src/lib/cjs.cjs\";\n\
         import { test } from \"vitest\";\n\
         \n\
         test(\"cjs\", () => {\n\
         \x20 cjsHelper();\n\
         });\n",
    );

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (dir, cg)
}

async fn affected(cg: &TokenSave, file: &str) -> Value {
    let result = handle_tool_call(
        cg,
        "tokensave_affected",
        json!({"files": [file]}),
        None,
        None,
    )
    .await
    .unwrap();
    let text = result.value["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn affected_finds_tests_importing_ts_module_via_js_specifier() {
    let (_dir, cg) = fixture().await;
    let output = affected(&cg, "src/lib/hash.ts").await;
    assert_eq!(
        output["affected_tests"],
        json!([
            "tests/lib/hash.test.ts",
            "tests/lib/namespace.test.ts",
            "tests/lib/seed.test.ts"
        ]),
        "{output:#}"
    );
    assert_eq!(output["count"], 3);
}

#[tokio::test]
async fn file_dependents_map_js_specifier_to_ts_source() {
    let (_dir, cg) = fixture().await;
    let dependents = cg.get_file_dependents("src/lib/hash.ts").await.unwrap();
    for test in [
        "tests/lib/hash.test.ts",
        "tests/lib/namespace.test.ts",
        "tests/lib/seed.test.ts",
    ] {
        assert!(
            dependents.iter().any(|d| d == test),
            "{test} missing from {dependents:?}"
        );
    }
}

#[tokio::test]
async fn jsx_mjs_cjs_specifiers_map_to_tsx_mts_cts() {
    let (_dir, cg) = fixture().await;
    for (source, test) in [
        ("src/lib/view.tsx", "tests/lib/view.test.tsx"),
        ("src/lib/esm.mts", "tests/lib/esm.test.ts"),
        ("src/lib/cjs.cts", "tests/lib/cjs.test.ts"),
    ] {
        let output = affected(&cg, source).await;
        assert_eq!(
            output["affected_tests"],
            json!([test]),
            "{source}: {output:#}"
        );
    }
}

/// The reachability gate on JS/TS bare-name calls must accept a candidate
/// whose file the caller imports through a `.js` specifier. Before #647 the
/// gate compared the candidate's module stem (`hash`) against the specifier's
/// last dotted segment (`js`) and dropped the call edge.
#[tokio::test]
async fn call_through_js_specifier_import_resolves_across_directories() {
    let (_dir, cg) = fixture().await;
    let edges = cg.get_all_edges().await.unwrap();
    let nodes = cg.get_all_nodes().await.unwrap();
    let id_of = |name: &str| {
        nodes
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.id.clone())
            .unwrap()
    };
    let target = id_of("computeHash");
    let source = id_of("it hashes");
    assert!(
        edges
            .iter()
            .any(|e| e.kind == EdgeKind::Calls && e.source == source && e.target == target),
        "no calls edge from the test into computeHash"
    );
}

#[tokio::test]
async fn module_import_graph_sees_js_specifier_imports() {
    let (_dir, cg) = fixture().await;
    let graph = cg.build_module_import_graph(2).await.unwrap();
    let dep = graph
        .dependencies()
        .into_iter()
        .find(|d| d.from == "tests/lib" && d.to == "src/lib")
        .expect("tests/lib -> src/lib import dependency");
    assert!(
        dep.sites
            .iter()
            .any(|s| s.imported == "../../src/lib/hash.js" && s.resolved_file == "src/lib/hash.ts"),
        "{:?}",
        dep.sites
    );
}

/// Re-extracting the imported file deletes its `File` node and every edge
/// into it. The importer's specifier is not a symbol name, so the touched set
/// has to recognise it by the paths it may resolve to, or the edge is lost on
/// the next incremental sync.
#[tokio::test]
async fn incremental_sync_keeps_js_specifier_import_edge() {
    let (dir, cg) = fixture().await;
    write(
        dir.path(),
        "src/lib/hash.ts",
        "export const HASH_SEED = 9;\n\
         \n\
         export function computeHash(input: string): string {\n\
         \x20 return `${HASH_SEED}:${input}`;\n\
         }\n",
    );
    cg.sync().await.unwrap();
    let dependents = cg.get_file_dependents("src/lib/hash.ts").await.unwrap();
    assert!(
        dependents.iter().any(|d| d == "tests/lib/seed.test.ts"),
        "{dependents:?}"
    );
}

/// A specifier written before its target exists resolves once the target is
/// indexed, without the importer being touched.
#[tokio::test]
async fn incremental_sync_binds_specifier_when_target_appears() {
    let (dir, cg) = fixture().await;
    write(
        dir.path(),
        "tests/lib/late.test.ts",
        "import { lateHelper } from \"../../src/lib/late.js\";\n\
         import { test } from \"vitest\";\n\
         \n\
         test(\"late\", () => {\n\
         \x20 void lateHelper;\n\
         });\n",
    );
    cg.sync().await.unwrap();
    assert!(cg
        .get_file_dependents("src/lib/late.ts")
        .await
        .unwrap()
        .is_empty());

    write(
        dir.path(),
        "src/lib/late.ts",
        "export function lateHelper(): number {\n\
         \x20 return 3;\n\
         }\n",
    );
    cg.sync().await.unwrap();
    let dependents = cg.get_file_dependents("src/lib/late.ts").await.unwrap();
    assert_eq!(dependents, vec!["tests/lib/late.test.ts".to_string()]);
}
