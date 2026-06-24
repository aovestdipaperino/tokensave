//! Regression tests for #153 — two residual Go false results that survived the
//! #149 fix:
//!
//! * **Bug 1** (`callers`/`callees`/`impact` accuracy): the #149 selector fix
//!   correctly added an import-path edge for `pkg.Fn()`, but the old bare-name
//!   resolution still *also* fired and collapsed every same-name call onto a
//!   single ranking-tie winner. The result was the correct edge *plus* a stale
//!   phantom edge — N + (N−1) edges for N call sites. Each same-named function
//!   called exactly once must end up with exactly one incoming `calls` edge.
//! * **Bug 2** (`unused_imports`): a bare (un-aliased) `/vN` import whose
//!   package name differs from the pre-`/vN` segment
//!   (`github.com/resend/resend-go/v3`, package `resend`) derived the invalid
//!   identifier `resend-go`, which can never appear in source, so the (used)
//!   import was reported unused. A derived identifier that is not a legal Go
//!   identifier must never be the basis for an unused flag.
//!
//! The fixtures are faithful transcriptions of the minimal repros in the issue.

use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;
use tokensave::types::{EdgeKind, NodeKind};

fn extract_text(value: &Value) -> &str {
    value["content"][0]["text"]
        .as_str()
        .unwrap_or("<missing text>")
}

// ---------------------------------------------------------------------------
// Bug 1 — phantom duplicate call edges onto the ranking-tie winner.
// ---------------------------------------------------------------------------

/// Builds the issue's `bug1-phantom-edges/` module verbatim: three packages
/// share the name `jobs`, each defines `NewCleanupWorker`, and `wire()` calls
/// each exactly once.
async fn setup_bug1() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("internal/a/jobs")).unwrap();
    fs::create_dir_all(project.join("internal/b/jobs")).unwrap();
    fs::create_dir_all(project.join("internal/c/jobs")).unwrap();

    fs::write(
        project.join("go.mod"),
        "module example.com/phantom\n\ngo 1.22\n",
    )
    .unwrap();

    fs::write(
        project.join("main.go"),
        "package main\n\nfunc main() {\n\twire()\n}\n",
    )
    .unwrap();

    fs::write(
        project.join("wiring.go"),
        r#"package main

import (
	ajobs "example.com/phantom/internal/a/jobs"
	bjobs "example.com/phantom/internal/b/jobs"
	cjobs "example.com/phantom/internal/c/jobs"
)

func wire() {
	_ = ajobs.NewCleanupWorker() // should add exactly 1 edge -> a/jobs
	_ = bjobs.NewCleanupWorker() // should add exactly 1 edge -> b/jobs
	_ = cjobs.NewCleanupWorker() // should add exactly 1 edge -> c/jobs
}
"#,
    )
    .unwrap();

    for (pkg, ret) in [("a", 1), ("b", 2), ("c", 3)] {
        fs::write(
            project.join(format!("internal/{pkg}/jobs/jobs.go")),
            format!("package jobs\n\nfunc NewCleanupWorker() int {{ return {ret} }}\n"),
        )
        .unwrap();
    }

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn no_phantom_call_edges_for_same_name_funcs_in_same_name_packages() {
    let (cg, _dir) = setup_bug1().await;
    let nodes = cg.get_all_nodes().await.unwrap();

    let workers: Vec<_> = nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Function && n.name == "NewCleanupWorker")
        .collect();
    assert_eq!(
        workers.len(),
        3,
        "expected three NewCleanupWorker definitions, got {}",
        workers.len()
    );

    let mut total_calls = 0u32;
    for w in &workers {
        let incoming = cg.get_incoming_edges(&w.id).await.unwrap();
        let calls = incoming
            .iter()
            .filter(|e| e.kind == EdgeKind::Calls)
            .count() as u32;
        total_calls += calls;
        assert_eq!(
            calls, 1,
            "each NewCleanupWorker is called exactly once; {} has {calls} incoming calls edges",
            w.file_path
        );
    }

    // Three call sites, three correct edges — never N + (N-1) = 5.
    assert_eq!(
        total_calls, 3,
        "three call sites must yield exactly three calls edges, got {total_calls}"
    );
}

// ---------------------------------------------------------------------------
// Bug 2 — a `/vN` import whose package name != pre-`/vN` segment.
// ---------------------------------------------------------------------------

/// Builds the issue's `bug2-unusedimport-pkgname/` module: the import path
/// `github.com/resend/resend-go/v3` has package clause `resend`, but the
/// pre-`/vN` segment is `resend-go` (not a legal identifier).
async fn setup_bug2() -> (TokenSave, TempDir) {
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::create_dir_all(project.join("internal/mail")).unwrap();

    fs::write(
        project.join("go.mod"),
        "module example.com/bug2pkgname\n\ngo 1.23\n\nrequire github.com/resend/resend-go/v3 v3.6.0\n",
    )
    .unwrap();

    fs::write(
        project.join("main.go"),
        r#"package main

import (
	"context"

	"example.com/bug2pkgname/internal/mail"
)

func main() {
	s := mail.NewResendSender("k", "from@example.com", "", "[x] ")
	_ = s.Send(context.Background(), mail.Message{To: "a@example.com"})
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("internal/mail/resend.go"),
        r#"package mail

import (
	"context"
	"fmt"

	"github.com/resend/resend-go/v3"
)

type ResendSender struct {
	client        *resend.Client
	from          string
	replyTo       string
	subjectPrefix string
}

func NewResendSender(apiKey, from, replyTo, subjectPrefix string) *ResendSender {
	return &ResendSender{
		client:        resend.NewClient(apiKey),
		from:          from,
		replyTo:       replyTo,
		subjectPrefix: subjectPrefix,
	}
}

func (s *ResendSender) Send(ctx context.Context, m Message) error {
	req := &resend.SendEmailRequest{
		From:    s.from,
		To:      []string{m.To},
		Subject: s.subjectPrefix + m.Subject,
	}
	if s.replyTo != "" {
		req.ReplyTo = s.replyTo
	}
	if _, err := s.client.Emails.SendWithContext(ctx, req); err != nil {
		return fmt.Errorf("mail: resend send: %w", err)
	}
	return nil
}
"#,
    )
    .unwrap();

    fs::write(
        project.join("internal/mail/message.go"),
        "package mail\n\ntype Message struct {\n\tTo      string\n\tSubject string\n}\n",
    )
    .unwrap();

    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();
    (cg, dir)
}

#[tokio::test]
async fn unused_imports_does_not_flag_versioned_import_with_hyphenated_segment() {
    let (cg, _dir) = setup_bug2().await;
    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let imports: Vec<(String, String)> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| {
            (
                u["name"].as_str().unwrap_or_default().to_string(),
                u["unused"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    // The resend-go/v3 import is used (resend.Client / resend.NewClient /
    // resend.SendEmailRequest) and must not be reported — and certainly not
    // with the smoking-gun `resend-go` identifier.
    assert!(
        !imports.iter().any(|(name, _)| name.contains("resend-go")),
        "used resend-go/v3 import must not be flagged; imports={imports:?}"
    );
    assert!(
        !imports.iter().any(|(_, unused)| unused == "resend-go"),
        "no import may derive the invalid `resend-go` identifier; imports={imports:?}"
    );
}

#[tokio::test]
async fn unused_imports_still_flags_truly_unused_plain_import() {
    // Regression guard: the hyphen-rejection must not break the normal path —
    // a genuinely unused import with a legal identifier must STILL be flagged.
    let dir = TempDir::new().unwrap();
    let project = dir.path();
    fs::write(project.join("go.mod"), "module example.com/u\n\ngo 1.22\n").unwrap();
    fs::write(
        project.join("a.go"),
        "package main\n\nimport \"strings\"\n\nfunc main() {}\n",
    )
    .unwrap();
    let cg = TokenSave::init(project).await.unwrap();
    cg.index_all().await.unwrap();

    let result = handle_tool_call(&cg, "tokensave_unused_imports", json!({}), None, None)
        .await
        .unwrap();
    let output: Value = serde_json::from_str(extract_text(&result.value)).unwrap();
    let flagged: Vec<String> = output["imports"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["unused"].as_str().map(String::from))
        .collect();

    assert!(
        flagged.contains(&"strings".to_string()),
        "unused `strings` import should still be flagged; flagged={flagged:?}"
    );
}
