//! Commands run from a nested directory resolve the project that owns it.
//!
//! Driven through the real binary because the resolution lives in the CLI:
//! `init` must reuse a parent project (or target the git root) instead of
//! creating a nested `.tokensave/`, and `sync`, `tool`, and `serve` must serve
//! the parent's index from any subdirectory. Git repository roots isolate an
//! independent nested repository from an outer project's index.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// An isolated fixture: a home directory (so the global DB and agent configs
/// never touch the developer's) and a workspace to build projects in.
struct Fixture {
    home: TempDir,
    workspace: TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            home: TempDir::new().unwrap(),
            workspace: TempDir::new().unwrap(),
        }
    }

    /// Creates `parts` below the workspace and returns it.
    fn dir(&self, parts: &[&str]) -> PathBuf {
        let mut dir = self.workspace.path().to_path_buf();
        for part in parts {
            dir.push(part);
        }
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn tokensave(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(cwd, args)
            .stdin(Stdio::null())
            .output()
            .expect("failed to spawn tokensave")
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tokensave"));
        command
            .args(args)
            .current_dir(cwd)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("APPDATA", self.home.path())
            .env("LOCALAPPDATA", self.home.path())
            .env("TOKENSAVE_DISABLE_GREP_HOOK", "1")
            .env_remove("TOKENSAVE_DISABLE_SERVER")
            .env_remove("DISABLE_TOKENSAVE")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A git repository (`git init` for real, so `.git` is what git makes) with a
/// source file defining `fn_name`.
fn git_repo(dir: &Path, fn_name: &str) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src").join("lib.rs"),
        format!("pub fn {fn_name}() {{}}\n"),
    )
    .unwrap();
    let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status()
        .expect("git must be on PATH for this test");
    assert!(status.success());
}

/// Runs `init <dir>` and asserts it succeeded.
fn init_at(fx: &Fixture, dir: &Path) {
    let out = fx.tokensave(dir, &["init", &dir.to_string_lossy()]);
    assert!(out.status.success(), "init failed: {}", stderr(&out));
    assert!(db_path(dir).is_file());
}

fn db_path(root: &Path) -> PathBuf {
    root.join(".tokensave").join("tokensave.db")
}

fn config_path(root: &Path) -> PathBuf {
    root.join(".tokensave").join("config.json")
}

/// Index contents a no-op must leave alone: the DB bytes and the config text.
fn snapshot(root: &Path) -> (Vec<u8>, String) {
    (
        std::fs::read(db_path(root)).unwrap(),
        std::fs::read_to_string(config_path(root)).unwrap(),
    )
}

#[test]
fn init_below_an_existing_project_reuses_it_without_indexing() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "hello");
    init_at(&fx, &root);
    let before = snapshot(&root);

    for parts in [
        &["repo", "src"][..],
        &["repo", "src", "backend", "services"],
    ] {
        let cwd = fx.dir(parts);
        let out = fx.tokensave(&cwd, &["init"]);
        let err = stderr(&out);
        assert_eq!(out.status.code(), Some(0), "stderr: {err}");
        assert!(err.contains("already initialized in"), "stderr: {err}");
        assert!(err.contains(&root.display().to_string()), "stderr: {err}");
        assert!(err.contains("No initialization or indexing required"));
        assert!(!err.contains("indexing done"), "must not index: {err}");
        assert!(!cwd.join(".tokensave").exists(), "no nested .tokensave");
        assert_eq!(
            snapshot(&root),
            before,
            "index and config must be untouched"
        );
    }
}

#[test]
fn init_at_the_project_root_still_refuses() {
    // #372: re-initializing the project root itself stays a non-zero no-op.
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "hello");
    init_at(&fx, &root);
    let before = snapshot(&root);

    let out = fx.tokensave(&root, &["init"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("already initialized"));
    assert_eq!(snapshot(&root), before);
}

#[test]
fn init_in_an_uninitialized_git_subdirectory_targets_the_git_root() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "hello");
    let cwd = fx.dir(&["repo", "src", "component"]);

    let out = fx.tokensave(&cwd, &["init"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(db_path(&root).is_file(), "the git root must be initialized");
    assert!(!cwd.join(".tokensave").exists());
    assert!(!root.join("src").join(".tokensave").exists());
}

#[test]
fn independent_nested_repository_gets_its_own_index() {
    let fx = Fixture::new();
    let outer = fx.dir(&["outer"]);
    git_repo(&outer, "outer_fn");
    init_at(&fx, &outer);
    let before = snapshot(&outer);
    let nested = fx.dir(&["outer", "vendor", "nested"]);
    git_repo(&nested, "nested_fn");
    let cwd = fx.dir(&["outer", "vendor", "nested", "src"]);

    let out = fx.tokensave(&cwd, &["init"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        db_path(&nested).is_file(),
        "the nested repository must not borrow the outer index"
    );
    assert_eq!(
        snapshot(&outer),
        before,
        "the outer index must be untouched"
    );

    // Each repository now answers from its own index.
    let out = fx.tokensave(&cwd, &["tool", "search", "outer_fn"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(!stdout(&out).contains("outer_fn"), "{}", stdout(&out));
    let out = fx.tokensave(&cwd, &["tool", "search", "nested_fn"]);
    assert!(stdout(&out).contains("nested_fn"), "{}", stdout(&out));
}

#[test]
fn non_git_project_is_discovered_from_a_subdirectory() {
    let fx = Fixture::new();
    let root = fx.dir(&["plain project"]);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src").join("lib.rs"), "pub fn plain() {}\n").unwrap();
    init_at(&fx, &root);
    let cwd = fx.dir(&["plain project", "src", "deep dir"]);

    let out = fx.tokensave(&cwd, &["init"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("already initialized in"));
    assert!(!cwd.join(".tokensave").exists());
}

#[test]
fn search_from_a_subdirectory_uses_the_root_index_and_root_relative_paths() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "connection_pool");
    init_at(&fx, &root);
    let cwd = fx.dir(&["repo", "src", "backend"]);

    let out = fx.tokensave(&cwd, &["tool", "search", "connection_pool"]);
    let text = stdout(&out);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(text.contains("src/lib.rs"), "{text}");
    assert!(
        !text.contains("src/backend"),
        "no cwd-relative prefix: {text}"
    );
    assert!(!cwd.join(".tokensave").exists());
}

#[test]
fn sync_and_status_from_a_subdirectory_use_the_root_index() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "first");
    init_at(&fx, &root);
    std::fs::write(root.join("src").join("more.rs"), "pub fn second() {}\n").unwrap();
    let cwd = fx.dir(&["repo", "src", "backend"]);

    let out = fx.tokensave(&cwd, &["sync"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(!cwd.join(".tokensave").exists());

    let out = fx.tokensave(&cwd, &["tool", "search", "second"]);
    assert!(stdout(&out).contains("src/more.rs"), "{}", stdout(&out));

    let out = fx.tokensave(&cwd, &["status"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

#[test]
fn multiple_repositories_never_share_an_index() {
    let fx = Fixture::new();
    let a = fx.dir(&["a"]);
    let b = fx.dir(&["b"]);
    git_repo(&a, "only_in_a");
    git_repo(&b, "only_in_b");
    init_at(&fx, &a);
    init_at(&fx, &b);

    let out = fx.tokensave(&fx.dir(&["a", "src"]), &["tool", "search", "only_in_b"]);
    assert!(!stdout(&out).contains("only_in_b"), "{}", stdout(&out));
    let out = fx.tokensave(&fx.dir(&["b", "src"]), &["tool", "search", "only_in_b"]);
    assert!(stdout(&out).contains("only_in_b"), "{}", stdout(&out));
}

#[test]
fn malformed_parent_configuration_is_an_actionable_error() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "hello");
    init_at(&fx, &root);
    std::fs::write(config_path(&root), "{ not json").unwrap();
    let cwd = fx.dir(&["repo", "src"]);

    let out = fx.tokensave(&cwd, &["init"]);
    let err = stderr(&out);
    assert!(!out.status.success(), "stderr: {err}");
    assert!(
        err.contains("configuration cannot be loaded"),
        "stderr: {err}"
    );
    assert!(err.contains("config.json"), "stderr: {err}");
    assert!(!cwd.join(".tokensave").exists());
}

#[test]
fn explicit_path_overrides_discovery() {
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "hello");
    init_at(&fx, &root);
    let pkg = fx.dir(&["repo", "packages", "pkg"]);
    std::fs::write(pkg.join("pkg.rs"), "pub fn pkg_only() {}\n").unwrap();

    // `init <path>` initializes exactly where it is told.
    init_at(&fx, &pkg);

    // `--project` targets the root index even from inside the nested project.
    let root_arg = root.to_string_lossy().into_owned();
    let out = fx.tokensave(&pkg, &["tool", "search", "hello", "--project", &root_arg]);
    assert!(stdout(&out).contains("hello"), "{}", stdout(&out));
    // Without it, the nearest project — the nested one — wins.
    let out = fx.tokensave(&pkg, &["tool", "search", "hello"]);
    assert!(!stdout(&out).contains("src/lib.rs"), "{}", stdout(&out));
}

#[test]
fn mcp_server_launched_from_a_subdirectory_serves_the_root_project() {
    // The server scopes listing tools to its launch directory by default
    // (`scope_prefix`), so the symbol lives inside it; the result must still
    // carry the root-relative path from the root's index.
    let fx = Fixture::new();
    let root = fx.dir(&["repo"]);
    git_repo(&root, "unrelated");
    let cwd = fx.dir(&["repo", "src", "backend"]);
    std::fs::write(cwd.join("db.rs"), "pub fn served_symbol() {}\n").unwrap();
    init_at(&fx, &root);

    let mut child = fx
        .command(&cwd, &["serve"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn tokensave serve");
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        let requests = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tokensave_search","arguments":{"query":"served_symbol"}}}"#,
        ];
        for request in requests {
            writeln!(stdin, "{request}").expect("write MCP request");
        }
        // Dropping stdin closes it: the server answers, then exits on EOF.
    }
    let out = child.wait_with_output().expect("wait for tokensave serve");
    let text = stdout(&out);
    assert!(
        text.contains("served_symbol") && text.contains("src/backend/db.rs"),
        "stdout: {text}\nstderr: {}",
        stderr(&out)
    );
    assert!(!cwd.join(".tokensave").exists());
}
