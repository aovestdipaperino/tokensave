//! Edit tools only write inside the project root or a worktree of its
//! repository (#866).
//!
//! Every edit tool resolves its target through `resolve_edit_target`. These
//! tests pin the boundary: an absolute path, a `..` escape, a `project_root`
//! override or a symlink must not point an edit outside those directories,
//! and `.git` and tokensave's state stay off limits. A linked git worktree of
//! the indexed repository, and a file the index tracks through an in-root
//! symlink (#34), stay editable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokensave::mcp::handle_tool_call;
use tokensave::tokensave::TokenSave;

const MAIN_RS: &str = "fn hello() {}\n";
const LIB_RS: &str = "pub fn greet_widget_example() -> String {\n    \"hi\".to_string()\n}\n";
const SECRET: &str = "secret = \"do-not-touch\"\n";

/// A temporary base holding `project/` (indexed, with `src/main.rs` and
/// `src/lib.rs`) and a sibling `outside/notes.txt` that no edit may reach.
struct Fixture {
    _base: TempDir,
    project: PathBuf,
    outside: PathBuf,
    cg: TokenSave,
}

impl Fixture {
    async fn new() -> Self {
        Self::with_setup(|_, _| {}).await
    }

    /// Runs `setup(project, outside)` before the project is indexed.
    async fn with_setup(setup: impl FnOnce(&Path, &Path)) -> Self {
        let base = TempDir::new().expect("temp dir");
        let project = base.path().join("project");
        let outside = base.path().join("outside");
        fs::create_dir_all(project.join("src")).expect("create src");
        fs::create_dir_all(&outside).expect("create outside");
        fs::write(project.join("src").join("main.rs"), MAIN_RS).expect("write main.rs");
        fs::write(project.join("src").join("lib.rs"), LIB_RS).expect("write lib.rs");
        fs::write(outside.join("notes.txt"), SECRET).expect("write notes.txt");
        setup(&project, &outside);
        let cg = TokenSave::init(&project).await.expect("init");
        cg.index_all().await.expect("index");
        Self {
            _base: base,
            project,
            outside,
            cg,
        }
    }

    fn outside_notes(&self) -> PathBuf {
        self.outside.join("notes.txt")
    }

    fn assert_outside_untouched(&self) {
        assert_eq!(
            fs::read_to_string(self.outside_notes()).expect("read notes.txt"),
            SECRET,
            "a refused edit must leave the outside file untouched"
        );
    }
}

/// Calls `tool` and returns the error text; panics if the call succeeds.
async fn refused(cg: &TokenSave, tool: &str, args: Value) -> String {
    match handle_tool_call(cg, tool, args, None, None).await {
        Err(error) => error.to_string(),
        Ok(result) => panic!("{tool} must refuse, but returned {}", result.value),
    }
}

/// Calls `tool` and returns its parsed JSON payload; panics on an error.
async fn accepted(cg: &TokenSave, tool: &str, args: Value) -> Value {
    let result = handle_tool_call(cg, tool, args, None, None)
        .await
        .unwrap_or_else(|error| panic!("{tool} must succeed, but failed: {error}"));
    let text = result.value["content"][0]["text"]
        .as_str()
        .expect("text content");
    serde_json::from_str(text).expect("json payload")
}

fn assert_refusal(message: &str, why: &str) {
    assert!(
        message.contains("refusing to edit") && message.contains(why),
        "expected a refusal mentioning {why:?}, got: {message}"
    );
}

/// Arguments for every path-taking edit tool that would replace `SECRET`'s
/// first line in `path`.
fn path_tool_calls(path: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "tokensave_str_replace",
            json!({ "path": path, "old_str": "do-not-touch", "new_str": "pwned" }),
        ),
        (
            "tokensave_multi_str_replace",
            json!({ "path": path, "replacements": [["do-not-touch", "pwned"]] }),
        ),
        (
            "tokensave_insert_at",
            json!({ "path": path, "anchor": "secret", "content": "pwned = 1" }),
        ),
        (
            "tokensave_replace_lines",
            json!({ "path": path, "start": 1, "end": 1, "new_content": "pwned = 1" }),
        ),
        (
            "tokensave_ast_grep_rewrite",
            json!({ "path": path, "pattern": "secret", "rewrite": "pwned" }),
        ),
    ]
}

#[tokio::test]
async fn an_absolute_path_outside_the_root_is_refused_by_every_path_tool() {
    let fx = Fixture::new().await;
    let target = fx.outside_notes().to_string_lossy().into_owned();
    for (tool, args) in path_tool_calls(&target) {
        let message = refused(&fx.cg, tool, args).await;
        assert_refusal(&message, "outside the project root");
        fx.assert_outside_untouched();
    }
}

#[tokio::test]
async fn a_dot_dot_escape_is_refused_by_every_path_tool() {
    let fx = Fixture::new().await;
    for path in ["../outside/notes.txt", "src/../../outside/notes.txt"] {
        for (tool, args) in path_tool_calls(path) {
            let message = refused(&fx.cg, tool, args).await;
            assert_refusal(&message, "outside the project root");
            fx.assert_outside_untouched();
        }
    }
}

#[tokio::test]
async fn a_project_root_override_outside_the_repository_is_refused() {
    let fx = Fixture::new().await;
    let override_root = fx.outside.to_string_lossy().into_owned();
    for key in ["project_root", "cwd"] {
        let message = refused(
            &fx.cg,
            "tokensave_str_replace",
            json!({
                "path": "notes.txt",
                "old_str": "do-not-touch",
                "new_str": "pwned",
                key: override_root,
            }),
        )
        .await;
        assert_refusal(&message, "outside the project root");
    }
    fx.assert_outside_untouched();
}

#[tokio::test]
async fn symbol_edits_with_an_outside_override_are_refused() {
    let fx = Fixture::new().await;
    // A copy with the same layout, so only the containment check can stop it.
    fs::create_dir_all(fx.outside.join("src")).expect("create outside src");
    let copy = fx.outside.join("src").join("lib.rs");
    fs::write(&copy, LIB_RS).expect("write copy");
    let override_root = fx.outside.to_string_lossy().into_owned();

    let calls = [
        (
            "tokensave_replace_symbol",
            json!({
                "symbol": "greet_widget_example",
                "new_source": "pub fn greet_widget_example() -> String { String::new() }",
                "project_root": override_root,
            }),
        ),
        (
            "tokensave_insert_at_symbol",
            json!({
                "symbol": "greet_widget_example",
                "content": "// pwned",
                "position": "before",
                "project_root": override_root,
            }),
        ),
        (
            "tokensave_delete_symbol",
            json!({ "symbol": "greet_widget_example", "project_root": override_root }),
        ),
    ];
    for (tool, args) in calls {
        let message = refused(&fx.cg, tool, args).await;
        assert_refusal(&message, "outside the project root");
        assert_eq!(fs::read_to_string(&copy).expect("read copy"), LIB_RS);
    }
}

#[tokio::test]
async fn a_rename_with_an_outside_override_is_blocked() {
    let fx = Fixture::new().await;
    fs::create_dir_all(fx.outside.join("src")).expect("create outside src");
    let copy = fx.outside.join("src").join("lib.rs");
    fs::write(&copy, LIB_RS).expect("write copy");

    let result = accepted(
        &fx.cg,
        "tokensave_rename",
        json!({
            "symbol": "greet_widget_example",
            "new_name": "greet_renamed",
            "dry_run": false,
            "project_root": fx.outside.to_string_lossy(),
        }),
    )
    .await;
    assert_eq!(result["applied"], false, "{result:#}");
    let blockers = result["blockers"].as_array().expect("blockers");
    assert!(
        blockers.iter().any(|blocker| blocker
            .as_str()
            .is_some_and(|text| text.contains("refusing to edit"))),
        "{blockers:?}"
    );
    assert_eq!(fs::read_to_string(&copy).expect("read copy"), LIB_RS);
    assert_eq!(
        fs::read_to_string(fx.project.join("src").join("lib.rs")).expect("read lib.rs"),
        LIB_RS
    );
}

#[tokio::test]
async fn git_and_state_directories_are_refused() {
    let fx = Fixture::new().await;
    let git_dir = fx.project.join(".git");
    fs::create_dir_all(&git_dir).expect("create .git");
    fs::write(git_dir.join("config"), SECRET).expect("write .git/config");
    let state_config = fx.project.join(".tokensave").join("config.json");
    let state_before = fs::read_to_string(&state_config).expect("read state config");

    for path in [".git/config", ".GIT/config"] {
        let message = refused(
            &fx.cg,
            "tokensave_str_replace",
            json!({ "path": path, "old_str": "do-not-touch", "new_str": "pwned" }),
        )
        .await;
        assert_refusal(&message, ".git");
    }
    assert_eq!(
        fs::read_to_string(git_dir.join("config")).expect("read .git/config"),
        SECRET
    );

    let message = refused(
        &fx.cg,
        "tokensave_replace_lines",
        json!({ "path": ".tokensave/config.json", "start": 1, "end": 1, "new_content": "{}" }),
    )
    .await;
    assert_refusal(&message, "tokensave's state");
    assert_eq!(
        fs::read_to_string(&state_config).expect("read state config"),
        state_before
    );
}

#[tokio::test]
async fn a_dot_dot_that_stays_inside_the_root_still_edits_and_reindexes() {
    let fx = Fixture::new().await;
    let parsed = accepted(
        &fx.cg,
        "tokensave_str_replace",
        json!({
            "path": "src/../src/main.rs",
            "old_str": "fn hello() {}",
            "new_str": "fn hello_again() {}",
        }),
    )
    .await;
    assert_eq!(parsed["ok"], true, "{parsed}");
    assert_eq!(
        fs::read_to_string(fx.project.join("src").join("main.rs")).expect("read main.rs"),
        "fn hello_again() {}\n"
    );
    // Reindexed under the normalized key, so the new name is searchable.
    let hits = fx.cg.search("hello_again", 5).await.expect("search");
    assert!(
        hits.iter().any(|hit| hit.node.file_path == "src/main.rs"),
        "the edit must be reindexed as src/main.rs: {hits:?}"
    );
}

#[tokio::test]
async fn an_untracked_symlinked_directory_leading_outside_is_refused() {
    let fx = Fixture::new().await;
    // Created after indexing, so the index does not track anything behind it.
    let link = fx.project.join("src").join("ext");
    if !symlink_dir_or_skip(&fx.outside, &link) {
        eprintln!("skipping: cannot create a directory link here");
        return;
    }
    for (tool, args) in path_tool_calls("src/ext/notes.txt") {
        let message = refused(&fx.cg, tool, args).await;
        assert_refusal(&message, "outside the project root");
        fx.assert_outside_untouched();
    }
}

#[tokio::test]
async fn an_untracked_symlinked_file_leading_outside_is_refused() {
    let fx = Fixture::new().await;
    let link = fx.project.join("src").join("notes.txt");
    if !symlink_file_or_skip(&fx.outside_notes(), &link) {
        eprintln!("skipping: cannot create a file symlink here");
        return;
    }
    let message = refused(
        &fx.cg,
        "tokensave_str_replace",
        json!({ "path": "src/notes.txt", "old_str": "do-not-touch", "new_str": "pwned" }),
    )
    .await;
    assert_refusal(&message, "outside the project root");
    fx.assert_outside_untouched();
}

/// The indexer follows an in-root link into a disjoint tree (#34), so a file
/// it tracks there stays editable through the link.
#[cfg(unix)]
#[tokio::test]
async fn a_tracked_file_behind_an_in_root_symlink_stays_editable() {
    let fx = Fixture::with_setup(|project, outside| {
        fs::write(outside.join("shared.rs"), "pub fn shared_fn() {}\n").expect("write shared.rs");
        std::os::unix::fs::symlink(outside, project.join("vendor")).expect("symlink");
    })
    .await;
    let parsed = accepted(
        &fx.cg,
        "tokensave_str_replace",
        json!({
            "path": "vendor/shared.rs",
            "old_str": "shared_fn",
            "new_str": "shared_fn_renamed",
        }),
    )
    .await;
    assert_eq!(parsed["ok"], true, "{parsed}");
    assert_eq!(
        fs::read_to_string(fx.outside.join("shared.rs")).expect("read shared.rs"),
        "pub fn shared_fn_renamed() {}\n"
    );
    // A file in the same linked tree that the index does not track stays
    // refused.
    let message = refused(
        &fx.cg,
        "tokensave_str_replace",
        json!({ "path": "vendor/notes.txt", "old_str": "do-not-touch", "new_str": "pwned" }),
    )
    .await;
    assert_refusal(&message, "outside the project root");
    fx.assert_outside_untouched();
}

#[tokio::test]
async fn a_linked_worktree_of_the_repository_is_editable() {
    let base = TempDir::new().expect("temp dir");
    let main = base.path().join("main_repo");
    fs::create_dir_all(main.join("src")).expect("create src");
    fs::write(main.join("src").join("main.rs"), MAIN_RS).expect("write main.rs");
    fs::write(main.join("src").join("lib.rs"), LIB_RS).expect("write lib.rs");
    git(&main, &["init", "-q", "-b", "master"]);
    // Keep LF on checkout, so the worktree's copy matches byte for byte on
    // Windows hosts that set `core.autocrlf`.
    git(&main, &["config", "core.autocrlf", "false"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "base"]);
    let worktree = base.path().join("main_repo_wt");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            worktree.to_str().expect("utf-8 path"),
        ],
    );
    let cg = TokenSave::init(&main).await.expect("init");
    cg.index_all().await.expect("index");

    // A relative path retargeted with `project_root`.
    let parsed = accepted(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": "src/main.rs",
            "old_str": "fn hello() {}",
            "new_str": "fn hello_worktree() {}",
            "project_root": worktree.to_string_lossy(),
        }),
    )
    .await;
    assert_eq!(parsed["ok"], true, "{parsed}");
    assert_eq!(
        fs::read_to_string(worktree.join("src").join("main.rs")).expect("read wt main.rs"),
        "fn hello_worktree() {}\n"
    );

    // An absolute path into the worktree.
    let absolute = worktree.join("src").join("main.rs");
    let parsed = accepted(
        &cg,
        "tokensave_insert_at",
        json!({ "path": absolute.to_string_lossy(), "anchor": "hello_worktree", "content": "// added" }),
    )
    .await;
    assert_eq!(parsed["ok"], true, "{parsed}");

    // A symbol edit, located through the primary index.
    let parsed = accepted(
        &cg,
        "tokensave_replace_symbol",
        json!({
            "symbol": "greet_widget_example",
            "new_source": "pub fn greet_widget_example() -> String {\n    \"hi from worktree\".to_string()\n}",
            "project_root": worktree.to_string_lossy(),
        }),
    )
    .await;
    assert_eq!(parsed["ok"], true, "{parsed}");
    assert!(fs::read_to_string(worktree.join("src").join("lib.rs"))
        .expect("read wt lib.rs")
        .contains("hi from worktree"));

    // The primary checkout is untouched.
    assert_eq!(
        fs::read_to_string(main.join("src").join("main.rs")).expect("read main.rs"),
        MAIN_RS
    );
    assert_eq!(
        fs::read_to_string(main.join("src").join("lib.rs")).expect("read lib.rs"),
        LIB_RS
    );

    // The worktree's own `.git` file is protected like the main `.git`.
    let message = refused(
        &cg,
        "tokensave_str_replace",
        json!({
            "path": worktree.join(".git").to_string_lossy(),
            "old_str": "gitdir",
            "new_str": "pwned",
        }),
    )
    .await;
    assert_refusal(&message, ".git");
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// `link` -> `target`, or `false` where the platform refuses, so the caller
/// can skip. On Windows without symlink privilege a directory junction, which
/// needs none and which path resolution follows the same way, stands in.
fn symlink_dir_or_skip(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link).is_ok()
        || Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|out| out.status.success());
    made
}

fn symlink_file_or_skip(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target, link);
    made.is_ok()
}
