//! The edge audit must see phantoms landing inside production (#536).
//!
//! The impossible-edge metric #533 was evaluated with counted edges crossing
//! into `tests/`. Measured against its own parent, #533 removed 1,216 edges on
//! a 515-file Python project while that metric saw 731 — the other 485 were
//! the same defect landing production → production, invisible to a
//! directory-crossing count. These tests pin that the audit sees both, and
//! that it does not mistake a legitimately reachable edge for a phantom.

use tempfile::tempdir;
use tokensave::edge_audit;
use tokensave::tokensave::TokenSave;

/// The measured shape: a closure `p` nested in a method, the sole symbol of
/// that name, plus an ordinary local `p` in a different production package
/// importing nothing from it. Neither end is in a test tree.
async fn fixture() -> (tempfile::TempDir, TokenSave) {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("ui")).unwrap();
    std::fs::create_dir_all(root.join("util")).unwrap();

    std::fs::write(
        root.join("ui/panel.py"),
        "import tkinter as tk\n\n\nclass Panel:\n    def writers(self):\n        t = self._txt\n\n        def p(s):\n            t.insert(tk.END, s)\n\n        return p\n",
    )
    .unwrap();
    std::fs::write(
        root.join("util/paths.py"),
        "import os\n\n\ndef cache_dir(root):\n    p = os.path.join(root, \".cache\")\n    return p\n",
    )
    .unwrap();
    // The recall control: an explicitly imported cross-package name.
    std::fs::write(
        root.join("ui/widgets.py"),
        "def render_banner():\n    return \"banner\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("util/report.py"),
        "from ui.widgets import render_banner\n\n\ndef build():\n    return render_banner()\n",
    )
    .unwrap();

    let cg = TokenSave::init(root).await.unwrap();
    cg.sync().await.unwrap();
    (tmp, cg)
}

#[tokio::test]
async fn a_correctly_resolved_index_reports_no_unreachable_edges() {
    let (_tmp, cg) = fixture().await;
    let report = edge_audit::audit(cg.db(), 10).await.unwrap();

    assert_eq!(
        report.unreachable, 0,
        "current resolution should leave no unreachable sole-candidate edges"
    );
    assert!(
        report.hot_targets.is_empty(),
        "nothing to report when there are no unreachable edges"
    );
    // The imported cross-package edge is still counted as sole-candidate and
    // cross-file — it is excluded by the reachability arm, not by failing to
    // be seen. Without this the zero above could be vacuous.
    assert!(
        report.sole_candidate_cross_file > 0,
        "the audit must still be looking at cross-file sole-candidate edges"
    );
}

#[tokio::test]
async fn a_phantom_edge_inside_production_is_counted() {
    let (_tmp, cg) = fixture().await;

    // Reintroduce the edge the gate now refuses, as a pre-#533 index would
    // carry it: `cache_dir` in util/ binding to the closure `p` in ui/. Both
    // ends are production, so a production-to-tests/ metric cannot see it.
    let source: String = {
        let mut rows = cg
            .db()
            .conn()
            .query(
                "SELECT id FROM nodes WHERE name = 'cache_dir' AND kind = 'function'",
                (),
            )
            .await
            .unwrap();
        rows.next().await.unwrap().unwrap().get(0).unwrap()
    };
    let target: String = {
        let mut rows = cg
            .db()
            .conn()
            .query(
                "SELECT id FROM nodes WHERE name = 'p' AND file_path = 'ui/panel.py'",
                (),
            )
            .await
            .unwrap();
        rows.next().await.unwrap().unwrap().get(0).unwrap()
    };
    cg.db()
        .conn()
        .execute(
            "INSERT INTO edges (source, target, kind, line) VALUES (?1, ?2, 'uses', 5)",
            libsql::params![source.as_str(), target.as_str()],
        )
        .await
        .unwrap();

    let report = edge_audit::audit(cg.db(), 10).await.unwrap();

    assert_eq!(
        report.unreachable, 1,
        "the production → production phantom must be counted"
    );
    let hot = &report.hot_targets[0];
    assert_eq!(hot.name, "p");
    assert_eq!(hot.file_path, "ui/panel.py");
    assert_eq!(hot.edges, 1);
    assert_eq!(hot.source_files, 1);
}
