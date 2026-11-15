//! `tokensave grep` v1 (#688): the search command runs unchanged and each
//! `path:line:` hit gains index facts after a tab.
//!
//! `git grep` is the search command in most tests, because git is on each
//! CI runner. The `grep` tests are Unix only. The `rg` tests run when `rg`
//! is on `PATH`.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::time::Duration;
use std::time::SystemTime;

use tempfile::tempdir;
use tempfile::TempDir;
use tokensave::grep_annotate::FACTS_MARKER;
use tokensave::tokensave::TokenSave;

const LIB_RS: &str = "\
mod util;

/// Adds one.
pub fn add_one(value: i32) -> i32 {
    util::clamp(value + 1)
}

pub struct Counter {
    pub total: i32,
}

impl Counter {
    pub fn bump(&mut self) {
        self.total = add_one(self.total);
    }
}

pub fn reset(counter: &mut Counter) {
    counter.total = add_one(-1);
}

// add_one has no other user
";

const UTIL_RS: &str = "\
pub fn clamp(value: i32) -> i32 {
    value.min(100)
}
";

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git must be on PATH");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A git repository with an index. `notes.txt` is not an indexed file type.
async fn fixture() -> TempDir {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    let src = root.join("src");
    fs::create_dir_all(&src).expect("src dir");
    fs::write(src.join("lib.rs"), LIB_RS).expect("lib.rs");
    fs::write(src.join("util.rs"), UTIL_RS).expect("util.rs");
    // Two classes define `dispose`, and the receiver has no type, so the
    // resolver cannot bind the call to one of them.
    fs::write(
        src.join("a.ts"),
        "export class StatusBar {\n    dispose(): void {\n        console.log('bar');\n    }\n}\n",
    )
    .expect("a.ts");
    fs::write(
        src.join("b.ts"),
        "export class ProviderManager {\n    dispose(): void {\n        console.log('mgr');\n    }\n}\n",
    )
    .expect("b.ts");
    fs::write(
        src.join("extension.ts"),
        "export class Extension {\n    deactivate(thing): void {\n        thing.dispose();\n    }\n}\n",
    )
    .expect("extension.ts");
    // The index records the length of this file without its byte order mark.
    fs::write(
        src.join("bom.rs"),
        "\u{feff}pub fn with_bom() -> i32 {\n    1\n}\n",
    )
    .expect("bom.rs");
    fs::write(src.join("color.rs"), "pub enum Color {\n    Red,\n}\n").expect("color.rs");
    fs::write(root.join("notes.txt"), "add_one is the entry point\n").expect("notes.txt");
    fs::write(root.join(".gitignore"), ".tokensave/\n").expect(".gitignore");
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "-A"]);

    let cg = TokenSave::init(root).await.expect("init");
    cg.index_all().await.expect("index");
    cg.checkpoint().await.expect("checkpoint");
    drop(cg);
    dir
}

/// Runs `tokensave grep ARGS...` in `dir`.
fn tokensave_grep(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tokensave"))
        .current_dir(dir)
        .arg("grep")
        .args(args)
        .output()
        .expect("tokensave must start")
}

/// Runs the search command itself in `dir`.
fn bare(dir: &Path, command: &[&str]) -> Output {
    Command::new(command[0])
        .current_dir(dir)
        .args(&command[1..])
        .output()
        .expect("the search command must start")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Removes the index facts from each line of an output.
fn strip_facts(output: &[u8]) -> String {
    text(output)
        .split_inclusive('\n')
        .map(|line| match line.find(FACTS_MARKER) {
            Some(at) if line.ends_with('\n') => format!("{}\n", &line[..at]),
            Some(at) => line[..at].to_string(),
            None => line.to_string(),
        })
        .collect()
}

/// Returns the lines of an output that carry index facts.
fn lines_with_facts(output: &[u8]) -> Vec<String> {
    text(output)
        .lines()
        .filter(|line| line.contains(FACTS_MARKER))
        .map(str::to_owned)
        .collect()
}

fn rg_is_available() -> bool {
    let found = Command::new("rg").arg("--version").output().is_ok();
    if !found {
        eprintln!("rg is not on PATH: the rg part of this test did not run");
    }
    found
}

/// Sets the modification time of a file to a time after the index was made.
fn touch_later(path: &Path) {
    let later = SystemTime::now() + Duration::from_secs(60);
    fs::File::options()
        .write(true)
        .open(path)
        .expect("open for touch")
        .set_modified(later)
        .expect("set modification time");
}

/// With the facts removed, the output is the output of the bare command.
fn assert_same_as_bare(dir: &Path, command: &[&str]) {
    let wrapped = tokensave_grep(dir, command);
    let plain = bare(dir, command);
    assert_eq!(
        strip_facts(&wrapped.stdout),
        text(&plain.stdout),
        "{command:?}"
    );
    assert_eq!(wrapped.status.code(), plain.status.code(), "{command:?}");
}

#[tokio::test]
async fn each_kind_of_fact_is_reported() {
    let dir = fixture().await;
    let root = dir.path();

    let out = tokensave_grep(root, &["git", "grep", "-n", "add_one"]);
    assert_eq!(
        text(&out.stdout),
        "notes.txt:1:add_one is the entry point\n\
         src/lib.rs:4:pub fn add_one(value: i32) -> i32 {\t#ts defines function add_one (4–6)\n\
         src/lib.rs:14:        self.total = add_one(self.total);\t#ts in Counter::bump (13–15); \
         call -> add_one (src/lib.rs:4–6)\n\
         src/lib.rs:19:    counter.total = add_one(-1);\t#ts in reset (18–20); \
         call -> add_one (src/lib.rs:4)\n\
         src/lib.rs:22:// add_one has no other user\n",
        "a definition, the enclosing symbol, a call with the line span of its target on the \
         first hit and the first line on a later hit, and no facts for an unindexed file or a \
         line outside each symbol"
    );

    let out = tokensave_grep(root, &["git", "grep", "-n", "clamp"]);
    assert_eq!(
        text(&out.stdout),
        "src/lib.rs:5:    util::clamp(value + 1)\t#ts in add_one (4–6); \
         call -> clamp (src/util.rs:1–3)\n\
         src/util.rs:1:pub fn clamp(value: i32) -> i32 {\t#ts defines function clamp (1–3)\n",
        "a call names a target in a different file"
    );

    let out = tokensave_grep(
        root,
        &["git", "grep", "-n", "dispose", "--", "src/extension.ts"],
    );
    assert_eq!(
        text(&out.stdout),
        "src/extension.ts:3:        thing.dispose();\t#ts in Extension::deactivate (2–4); \
         call -> ? (2 candidates: ProviderManager::dispose, StatusBar::dispose)\n",
        "a call that the resolver could not bind lists its candidates"
    );

    let out = tokensave_grep(root, &["git", "grep", "-n", "counter: &mut Counter"]);
    assert_eq!(
        text(&out.stdout),
        "src/lib.rs:18:pub fn reset(counter: &mut Counter) {\t#ts defines function reset (18–20); \
         reference -> Counter (src/lib.rs:8–10)\n",
        "an edge that is not a call is a reference"
    );
}

/// The kind is the name that the index and the MCP tools use, for example
/// `enum_variant`.
#[tokio::test]
async fn a_definition_shows_the_kind_name_of_the_index() {
    let dir = fixture().await;
    let out = tokensave_grep(dir.path(), &["git", "grep", "-n", "Red"]);
    assert_eq!(
        text(&out.stdout),
        "src/color.rs:2:    Red,\t#ts defines enum_variant Color::Red (2)\n"
    );
}

#[tokio::test]
async fn a_name_that_the_search_does_not_ask_for_is_not_reported() {
    let dir = fixture().await;
    // Line 14 calls `add_one`, but the search is for `total`.
    let out = tokensave_grep(dir.path(), &["git", "grep", "-n", "self.total"]);
    assert_eq!(
        text(&out.stdout),
        "src/lib.rs:14:        self.total = add_one(self.total);\t#ts in Counter::bump (13–15)\n"
    );
}

#[tokio::test]
async fn hits_keep_their_bytes() {
    let dir = fixture().await;
    let root = dir.path();
    for command in [
        &["git", "grep", "-n", "add_one"][..],
        &["git", "grep", "-n", "-e", "clamp", "-e", "total"],
        &["git", "grep", "-n", "-C1", "add_one"],
        &["git", "grep", "-n", "-i", "COUNTER"],
        &["git", "grep", "-n", "--column", "dispose", "--", "src"],
        &["git", "grep", "-n", "-o", "add_one"],
    ] {
        assert_same_as_bare(root, command);
    }
    let wrapped = tokensave_grep(root, &["git", "grep", "-n", "-C1", "add_one"]);
    assert!(
        !lines_with_facts(&wrapped.stdout).is_empty(),
        "the comparison must not be empty of facts"
    );
    #[cfg(unix)]
    for command in [
        &["grep", "-rn", "add_one", "src"][..],
        &["grep", "-n", "add_one", "src/lib.rs"],
        &["grep", "-rn", "-A1", "clamp", "src"],
        &["grep", "-rnH", "total", "src/lib.rs"],
    ] {
        assert_same_as_bare(root, command);
    }
    if rg_is_available() {
        for command in [
            &["rg", "-n", "-j1", "add_one", "src"][..],
            &["rg", "-n", "-j1", "-C1", "clamp", "src"],
            &["rg", "-n", "add_one", "src/lib.rs"],
        ] {
            assert_same_as_bare(root, command);
        }
        let wrapped = tokensave_grep(root, &["rg", "-n", "-j1", "add_one", "src"]);
        assert_eq!(lines_with_facts(&wrapped.stdout).len(), 3);
    }
}

#[tokio::test]
async fn context_lines_carry_no_facts() {
    let dir = fixture().await;
    let out = tokensave_grep(
        dir.path(),
        &["git", "grep", "-n", "-C1", "clamp", "--", "src/util.rs"],
    );
    assert_eq!(
        text(&out.stdout),
        "src/util.rs:1:pub fn clamp(value: i32) -> i32 {\t#ts defines function clamp (1–3)\n\
         src/util.rs-2-    value.min(100)\n"
    );
}

#[tokio::test]
async fn output_without_hits_is_untouched() {
    let dir = fixture().await;
    let root = dir.path();
    for command in [
        &["git", "grep", "-l", "add_one"][..],
        &["git", "grep", "-c", "add_one"],
        &["git", "grep", "-q", "add_one"],
        &["git", "grep", "--name-only", "clamp"],
    ] {
        let wrapped = tokensave_grep(root, command);
        let plain = bare(root, command);
        assert_eq!(wrapped.stdout, plain.stdout, "{command:?}");
        assert_eq!(wrapped.status.code(), plain.status.code(), "{command:?}");
    }
    #[cfg(unix)]
    for command in [
        &["grep", "-rl", "add_one", "src"][..],
        &["grep", "-rc", "add_one", "src"],
        &["grep", "-q", "add_one", "src/lib.rs"],
    ] {
        let wrapped = tokensave_grep(root, command);
        let plain = bare(root, command);
        assert_eq!(wrapped.stdout, plain.stdout, "{command:?}");
        assert_eq!(wrapped.status.code(), plain.status.code(), "{command:?}");
    }
    if rg_is_available() {
        for command in [
            &["rg", "--files", "--sort", "path", "src"][..],
            &["rg", "-l", "--sort", "path", "add_one"],
            &["rg", "-c", "--sort", "path", "add_one", "src"],
        ] {
            let wrapped = tokensave_grep(root, command);
            let plain = bare(root, command);
            assert_eq!(wrapped.stdout, plain.stdout, "{command:?}");
            assert_eq!(wrapped.status.code(), plain.status.code(), "{command:?}");
        }
    }
}

#[tokio::test]
async fn line_numbers_are_added_when_missing() {
    let dir = fixture().await;
    let root = dir.path();
    let wrapped = tokensave_grep(root, &["git", "grep", "clamp"]);
    let numbered = bare(root, &["git", "grep", "-n", "clamp"]);
    assert_eq!(strip_facts(&wrapped.stdout), text(&numbered.stdout));
    assert_eq!(lines_with_facts(&wrapped.stdout).len(), 2);
    if rg_is_available() {
        let wrapped = tokensave_grep(root, &["rg", "-j1", "clamp", "src"]);
        let numbered = bare(root, &["rg", "-n", "-j1", "clamp", "src"]);
        assert_eq!(strip_facts(&wrapped.stdout), text(&numbered.stdout));
        assert_eq!(lines_with_facts(&wrapped.stdout).len(), 2);
    }
}

#[tokio::test]
async fn graph_none_runs_the_command_unchanged() {
    let dir = fixture().await;
    let root = dir.path();
    let wrapped = tokensave_grep(root, &["--graph=none", "git", "grep", "clamp"]);
    let plain = bare(root, &["git", "grep", "clamp"]);
    assert_eq!(wrapped.stdout, plain.stdout);
    assert_eq!(wrapped.status.code(), plain.status.code());
}

#[tokio::test]
async fn a_graph_value_of_a_later_version_is_refused() {
    let dir = fixture().await;
    let out = tokensave_grep(dir.path(), &["--graph=callers", "git", "grep", "clamp"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    assert!(text(&out.stderr).contains("--graph=callers is not available yet"));
}

#[tokio::test]
async fn a_command_that_is_not_a_search_is_refused() {
    let dir = fixture().await;
    for command in [
        &["ls", "-la"][..],
        &["git", "status"],
        &["sh", "-c", "grep x"],
    ] {
        let out = tokensave_grep(dir.path(), command);
        assert_eq!(out.status.code(), Some(2), "{command:?}");
        assert_eq!(text(&out.stdout), "", "{command:?}");
    }
}

#[tokio::test]
async fn the_exit_code_is_the_search_commands() {
    let dir = fixture().await;
    let root = dir.path();
    let found = tokensave_grep(root, &["git", "grep", "-n", "add_one"]);
    assert_eq!(found.status.code(), Some(0));
    let absent = tokensave_grep(root, &["git", "grep", "-n", "no_such_name_here"]);
    assert_eq!(absent.status.code(), Some(1));
    assert_eq!(text(&absent.stdout), "");
    // A flag that the search command refuses: the code and the message are
    // the command's own.
    let command = ["git", "grep", "-n", "--no-such-flag", "add_one"];
    let refused = tokensave_grep(root, &command);
    let plain = bare(root, &command);
    assert_ne!(plain.status.code(), Some(0));
    assert_eq!(refused.status.code(), plain.status.code());
    assert_eq!(refused.stderr, plain.stderr);
}

#[tokio::test]
async fn line_tools_see_the_same_lines() {
    let dir = fixture().await;
    let root = dir.path();
    let command = ["git", "grep", "-n", "add_one"];
    let wrapped = text(&tokensave_grep(root, &command).stdout);
    let plain = text(&bare(root, &command).stdout);
    // `wc -l`
    assert_eq!(wrapped.lines().count(), plain.lines().count());
    // `cut -d: -f1,2`
    let fields = |output: &str| -> Vec<String> {
        output
            .lines()
            .map(|line| line.split(':').take(2).collect::<Vec<_>>().join(":"))
            .collect()
    };
    assert_eq!(fields(&wrapped), fields(&plain));
    // `head -1`
    assert!(wrapped
        .lines()
        .next()
        .zip(plain.lines().next())
        .is_some_and(|(with_facts, without)| with_facts.starts_with(without)));
}

/// `head -1` closes the output while the search still prints. The command
/// must stop, not wait and not fail with an error message.
#[cfg(unix)]
#[tokio::test]
async fn a_reader_that_stops_early_ends_the_search() {
    let dir = fixture().await;
    let root = dir.path();
    // More output than a pipe holds.
    fs::write(
        root.join("big.txt"),
        "needle in a long file\n".repeat(40_000),
    )
    .expect("big.txt");
    let out = Command::new("sh")
        .current_dir(root)
        .arg("-c")
        .arg(r#""$0" grep grep -n needle big.txt | head -1"#)
        .arg(env!("CARGO_BIN_EXE_tokensave"))
        .output()
        .expect("sh must start");
    assert_eq!(text(&out.stdout), "1:needle in a long file\n");
    assert_eq!(text(&out.stderr), "");
    assert_eq!(out.status.code(), Some(0));
}

#[tokio::test]
async fn a_changed_file_is_stale_and_has_no_other_fact() {
    let dir = fixture().await;
    let root = dir.path();
    touch_later(&root.join("src").join("util.rs"));
    let out = tokensave_grep(root, &["git", "grep", "-n", "clamp"]);
    assert_eq!(
        text(&out.stdout),
        "src/lib.rs:5:    util::clamp(value + 1)\t#ts in add_one (4–6); call -> clamp\n\
         src/util.rs:1:pub fn clamp(value: i32) -> i32 {\t#ts stale\n",
        "the changed file is stale, and a call into it gives no line span"
    );
}

/// The automatic sync tests the modification time only. A file that was
/// replaced by an older copy has an old time and a different size.
#[tokio::test]
async fn a_file_with_an_old_time_and_a_new_size_is_stale() {
    let dir = fixture().await;
    let root = dir.path();
    let util = root.join("src").join("util.rs");
    let indexed_time = fs::metadata(&util)
        .and_then(|meta| meta.modified())
        .expect("modification time");
    fs::write(&util, format!("// moved here from a backup\n{UTIL_RS}")).expect("util.rs");
    fs::File::options()
        .write(true)
        .open(&util)
        .expect("open util.rs")
        .set_modified(indexed_time - Duration::from_secs(3600))
        .expect("set modification time");
    let out = tokensave_grep(root, &["git", "grep", "-n", "pub fn clamp"]);
    assert_eq!(
        text(&out.stdout),
        "src/util.rs:2:pub fn clamp(value: i32) -> i32 {\t#ts stale\n"
    );
}

/// The recorded size of a file with a byte order mark is not its size on
/// disk. Such a file must not be stale.
#[tokio::test]
async fn a_file_with_a_byte_order_mark_is_not_stale() {
    let dir = fixture().await;
    let out = tokensave_grep(dir.path(), &["git", "grep", "-n", "1", "--", "src/bom.rs"]);
    assert_eq!(
        text(&out.stdout),
        "src/bom.rs:2:    1\t#ts in with_bom (1–3)\n"
    );
}

#[tokio::test]
async fn paths_are_read_from_the_directory_of_the_search() {
    let dir = fixture().await;
    let root = dir.path();
    let src = root.join("src");
    let out = tokensave_grep(&src, &["git", "grep", "-n", "clamp"]);
    assert_eq!(
        text(&out.stdout),
        "lib.rs:5:    util::clamp(value + 1)\t#ts in add_one (4–6); \
         call -> clamp (src/util.rs:1–3)\n\
         util.rs:1:pub fn clamp(value: i32) -> i32 {\t#ts defines function clamp (1–3)\n"
    );
    let out = tokensave_grep(root, &["git", "-C", "src", "grep", "-n", "value.min"]);
    assert_eq!(
        text(&out.stdout),
        "util.rs:2:    value.min(100)\t#ts in clamp (1–3)\n"
    );
    // The search is in the project, and the command starts outside it.
    let outside = tempdir().expect("temp dir");
    let project = root.to_str().expect("utf-8 path");
    let out = tokensave_grep(
        outside.path(),
        &["git", "-C", project, "grep", "-n", "value.min"],
    );
    assert_eq!(
        text(&out.stdout),
        "src/util.rs:2:    value.min(100)\t#ts in clamp (1–3)\n"
    );
    let out = tokensave_grep(&src, &["git", "grep", "-n", "--full-name", "value.min"]);
    assert_eq!(
        text(&out.stdout),
        "src/util.rs:2:    value.min(100)\t#ts in clamp (1–3)\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_absolute_path_names_the_same_file() {
    let dir = fixture().await;
    let root = dir.path();
    let lib = root.join("src").join("lib.rs");
    let util = root.join("src").join("util.rs");
    let command = [
        "grep",
        "-n",
        "value.min",
        lib.to_str().expect("utf-8 path"),
        util.to_str().expect("utf-8 path"),
    ];
    let out = tokensave_grep(root, &command);
    assert_eq!(
        text(&out.stdout),
        format!(
            "{}:2:    value.min(100)\t#ts in clamp (1–3)\n",
            util.display()
        )
    );
}

#[tokio::test]
async fn without_an_index_the_command_runs_unchanged() {
    let dir = tempdir().expect("temp dir");
    let root = dir.path();
    fs::write(root.join("a.rs"), "fn add_one() {}\n").expect("a.rs");
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "-A"]);
    // No `-n` is added: there are no facts that need a line number.
    for command in [
        &["git", "grep", "add_one"][..],
        &["git", "grep", "-n", "add_one"],
    ] {
        let wrapped = tokensave_grep(root, command);
        let plain = bare(root, command);
        assert_eq!(wrapped.stdout, plain.stdout, "{command:?}");
        assert_eq!(wrapped.status.code(), plain.status.code(), "{command:?}");
    }
}

/// An index of an older schema needs a migration, and the search never
/// writes. The search runs as written and says why its hits have no facts.
#[tokio::test]
async fn an_index_of_an_older_schema_is_reported_and_not_used() {
    let dir = fixture().await;
    let root = dir.path();
    let database = tokensave::config::get_tokensave_dir(root).join("tokensave.db");
    let older = tokensave::db::migrations::latest_version() - 1;
    let db = libsql::Builder::new_local(&database)
        .build()
        .await
        .expect("open database");
    db.connect()
        .expect("connect")
        .execute(&format!("PRAGMA user_version = {older}"), ())
        .await
        .expect("set schema version");
    drop(db);

    let command = ["git", "grep", "clamp"];
    let wrapped = tokensave_grep(root, &command);
    let plain = bare(root, &command);
    assert_eq!(wrapped.stdout, plain.stdout);
    assert_eq!(wrapped.status.code(), plain.status.code());
    let note = text(&wrapped.stderr);
    assert!(
        note.contains("tokensave grep: no index facts: the index has schema version")
            && note.contains("run `tokensave sync`"),
        "{note}"
    );
}

#[tokio::test]
async fn the_search_does_not_change_the_index() {
    let dir = fixture().await;
    let root = dir.path();
    let database = tokensave::config::get_tokensave_dir(root).join("tokensave.db");
    let before = fs::read(&database).expect("database");
    let out = tokensave_grep(root, &["git", "grep", "-n", "add_one"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(fs::read(&database).expect("database"), before);
}
