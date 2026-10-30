//! `auto_init` initializes repositories created with `git init` (#876).
//!
//! A cloned repository is indexed by the `post-checkout` hook, but `git init`
//! runs no hook, so a repository started that way stayed unindexed. The
//! `post-commit` hook's pathless `sync` and `serve` now initialize the top
//! level of a git repository that has no index, unless `auto_init` is off or
//! a guard applies.
//!
//! Every repository here lives under the system temp dir, which the temp-dir
//! guard (#569) skips. Each child therefore gets a `TMPDIR` of its own, so the
//! repository is outside it, except in the test of that guard. `HOME` is a
//! throwaway too, so `~/.tokensave/config.toml` is the test's own.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

/// A throwaway home and system temp dir for the spawned binary.
struct Sandbox {
    home: TempDir,
    temp: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            home: TempDir::new().unwrap(),
            temp: TempDir::new().unwrap(),
        }
    }

    /// Writes `~/.tokensave/config.toml` in the sandbox home.
    fn write_user_config(&self, contents: &str) {
        let dir = self.home.path().join(".tokensave");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), contents).unwrap();
    }

    /// Applies the sandbox to a child command.
    fn apply<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        self.apply_with_temp(command, self.temp.path())
    }

    fn apply_with_temp<'a>(&self, command: &'a mut Command, temp: &Path) -> &'a mut Command {
        let home = self.home.path();
        command
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env("APPDATA", home)
            .env("LOCALAPPDATA", home)
            .env("TMPDIR", temp)
            .env("TMP", temp)
            .env("TEMP", temp)
            // A fixed profile skips the hardware benchmark a new index runs.
            .env("TOKENSAVE_CPU_USAGE", "low")
            .env_remove("TOKENSAVE_AUTO_INIT")
            .env_remove("TOKENSAVE_DISABLE_SERVER")
            .env_remove("DISABLE_TOKENSAVE")
            .stdin(Stdio::null())
    }

    fn tokensave(&self, cwd: &Path, args: &[&str]) -> Output {
        self.apply(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .args(args)
                .current_dir(cwd),
        )
        .output()
        .unwrap()
    }
}

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A repository made with `git init`, holding one Rust source file and no
/// commit yet.
fn new_repository() -> TempDir {
    let repo = TempDir::new().unwrap();
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    std::fs::write(
        repo.path().join("src").join("lib.rs"),
        "pub fn freshly_initialized() -> u32 { 1 }\n",
    )
    .unwrap();
    git(repo.path(), &["init", "-q"]);
    repo
}

fn is_initialized(root: &Path) -> bool {
    tokensave::config::get_tokensave_dir(root)
        .join("tokensave.db")
        .exists()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn pathless_sync_initializes_a_new_repository() {
    let sandbox = Sandbox::new();
    let repo = new_repository();

    let output = sandbox.tokensave(repo.path(), &["sync"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("auto_init"), "{}", stderr(&output));
    assert!(is_initialized(repo.path()));
    let search = sandbox.tokensave(repo.path(), &["tool", "search", "freshly_initialized"]);
    assert!(
        String::from_utf8_lossy(&search.stdout).contains("src/lib.rs"),
        "the new index should hold the repository's symbols: {}",
        stderr(&search)
    );
}

#[test]
fn env_override_disables_auto_init() {
    let sandbox = Sandbox::new();
    let repo = new_repository();

    let output = sandbox
        .apply(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .arg("sync")
                .current_dir(repo.path()),
        )
        .env("TOKENSAVE_AUTO_INIT", "0")
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
}

#[test]
fn config_setting_disables_auto_init() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("auto_init = false\n");
    let repo = new_repository();

    let output = sandbox.tokensave(repo.path(), &["sync"]);

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
}

#[test]
fn env_override_enables_auto_init_over_the_config() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("auto_init = false\n");
    let repo = new_repository();

    let output = sandbox
        .apply(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .arg("sync")
                .current_dir(repo.path()),
        )
        .env("TOKENSAVE_AUTO_INIT", "1")
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(is_initialized(repo.path()));
}

#[test]
fn repository_under_the_system_temp_dir_is_skipped() {
    let sandbox = Sandbox::new();
    let repo = new_repository();
    let parent = repo.path().parent().unwrap().to_path_buf();

    let output = sandbox
        .apply_with_temp(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .arg("sync")
                .current_dir(repo.path()),
            &parent,
        )
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
}

#[test]
fn subdirectory_of_a_repository_is_skipped() {
    let sandbox = Sandbox::new();
    let repo = new_repository();

    let output = sandbox.tokensave(&repo.path().join("src"), &["sync"]);

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
    assert!(!is_initialized(&repo.path().join("src")));
}

#[test]
fn explicit_sync_path_is_not_auto_initialized() {
    let sandbox = Sandbox::new();
    let repo = new_repository();
    let path = repo.path().to_string_lossy().into_owned();

    let output = sandbox.tokensave(repo.path(), &["sync", &path]);

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
}

#[test]
fn folder_outside_any_repository_is_skipped() {
    let sandbox = Sandbox::new();
    let folder = TempDir::new().unwrap();
    std::fs::write(folder.path().join("lib.rs"), "pub fn f() {}\n").unwrap();

    let output = sandbox.tokensave(folder.path(), &["sync"]);

    assert!(!output.status.success());
    assert!(!is_initialized(folder.path()));
}

#[test]
fn bare_repository_is_skipped() {
    let sandbox = Sandbox::new();
    let bare = TempDir::new().unwrap();
    git(bare.path(), &["init", "-q", "--bare"]);

    let output = sandbox.tokensave(bare.path(), &["sync"]);

    assert!(!output.status.success());
    assert!(!is_initialized(bare.path()));
}

#[test]
fn home_directory_repository_is_skipped() {
    let repo = new_repository();
    let sandbox = Sandbox::new();

    let output = Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .arg("sync")
        .current_dir(repo.path())
        .env("HOME", repo.path())
        .env("USERPROFILE", repo.path())
        .env("TMPDIR", sandbox.temp.path())
        .env("TMP", sandbox.temp.path())
        .env("TEMP", sandbox.temp.path())
        .env_remove("TOKENSAVE_AUTO_INIT")
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
}

#[test]
fn repository_above_max_auto_sync_files_is_deferred_with_a_hint() {
    let sandbox = Sandbox::new();
    let repo = new_repository();
    let many = repo.path().join("many");
    std::fs::create_dir_all(&many).unwrap();
    for i in 0..=tokensave::tokensave::DEFAULT_MAX_AUTO_SYNC_FILES {
        std::fs::write(many.join(format!("f{i}.rs")), "").unwrap();
    }

    let output = sandbox.tokensave(repo.path(), &["sync"]);

    assert!(!output.status.success());
    assert!(!is_initialized(repo.path()));
    let text = stderr(&output);
    assert!(text.contains("auto_init deferred"), "{text}");
    assert!(text.contains("max_auto_sync_files"), "{text}");
}

#[test]
fn existing_index_is_synced_not_reinitialized() {
    let sandbox = Sandbox::new();
    let repo = new_repository();
    assert!(sandbox.tokensave(repo.path(), &["sync"]).status.success());

    let output = sandbox.tokensave(repo.path(), &["sync"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        !stderr(&output).contains("auto_init"),
        "{}",
        stderr(&output)
    );
}

/// The acceptance scenario: global hooks installed, then `git init` and a
/// first commit. The installed `post-commit` body is unchanged since #624.
#[cfg(unix)]
#[test]
fn global_post_commit_hook_initializes_on_first_commit() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new();
    let hooks = sandbox.home.path().join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("post-commit");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\n# tokensave: auto-sync\n'{}' sync >/dev/null 2>&1 &\n# tokensave: end auto-sync\n",
            env!("CARGO_BIN_EXE_tokensave")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let global_config = sandbox.home.path().join("gitconfig");
    std::fs::write(
        &global_config,
        format!("[core]\n\thooksPath = {}\n", hooks.display()),
    )
    .unwrap();
    let repo = new_repository();

    let output = sandbox
        .apply(
            Command::new("git")
                .args(["commit", "-q", "--allow-empty", "-m", "init"])
                .current_dir(repo.path()),
        )
        .env("GIT_CONFIG_GLOBAL", &global_config)
        // Cancel the command-scope core.hooksPath Cargo sets for fixtures.
        .env("GIT_CONFIG_COUNT", "0")
        .env("GIT_AUTHOR_NAME", "TokenSave Test")
        .env("GIT_AUTHOR_EMAIL", "tokensave@example.com")
        .env("GIT_COMMITTER_NAME", "TokenSave Test")
        .env("GIT_COMMITTER_EMAIL", "tokensave@example.com")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    // The hook backgrounds the binary; wait for the index it writes.
    let deadline = Instant::now() + Duration::from_secs(60);
    while !is_initialized(repo.path()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        is_initialized(repo.path()),
        "the first commit should have initialized the repository"
    );
    wait_for_index_completion(repo.path(), deadline);
}

/// Waits until the background index records its version, so the job is done
/// with the repository before the temp dir is removed.
fn wait_for_index_completion(root: &Path, deadline: Instant) {
    let config: PathBuf = tokensave::config::get_tokensave_dir(root).join("config.json");
    while Instant::now() < deadline {
        let indexed = std::fs::read_to_string(&config)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|config| config["last_indexed_version"].as_str().map(str::to_owned))
            .is_some_and(|version| !version.is_empty());
        if indexed {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn serve_initializes_a_new_repository_before_answering() {
    let sandbox = Sandbox::new();
    let repo = new_repository();

    let mut child = sandbox
        .apply(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .arg("serve")
                .current_dir(repo.path()),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut stdin = child.stdin.take().unwrap();
    let mut request = |id: u64, method: &str, params: Value| -> Value {
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
        loop {
            let line = lines
                .recv_timeout(Duration::from_secs(60))
                .unwrap_or_else(|_| panic!("no response to {method}"));
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if value.get("id") == Some(&json!(id)) {
                    return value;
                }
            }
        }
    };

    let init = request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "t", "version": "1"}
        }),
    );
    assert!(init.get("result").is_some(), "{init}");
    assert!(is_initialized(repo.path()));
    let search = request(
        2,
        "tools/call",
        json!({"name": "tokensave_search", "arguments": {"query": "freshly_initialized"}}),
    );
    assert!(
        search.to_string().contains("freshly_initialized"),
        "the served index should hold the repository's symbols: {search}"
    );

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn serve_with_auto_init_off_does_not_initialize() {
    let sandbox = Sandbox::new();
    sandbox.write_user_config("auto_init = false\n");
    let repo = new_repository();

    // Stdin closes at once; without a project the server exits after EOF.
    let output = sandbox
        .apply(
            Command::new(env!("CARGO_BIN_EXE_tokensave"))
                .arg("serve")
                .current_dir(repo.path()),
        )
        .output()
        .unwrap();

    assert!(!is_initialized(repo.path()), "{}", stderr(&output));
}
