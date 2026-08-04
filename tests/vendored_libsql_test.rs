//! Guards the vendored libsql patch (#367).
//!
//! `vendor/libsql` is a copy of the published libsql crate carrying a one-line
//! fix for a use-after-free: upstream closes every local connection twice, which
//! is an intermittent `STATUS_ACCESS_VIOLATION` on Windows. The patch lives in a
//! dependency, so nothing in the normal test suite fails if it is lost — bumping
//! the `libsql` requirement without re-vendoring, or re-copying the crate over
//! the patched file, would silently reintroduce the crash. These tests fail
//! loudly in both cases.
//!
//! `cargo package` strips `[patch.crates-io]` and leaves `vendor/libsql` out of
//! the published archive, so the invariant these tests enforce is conditional:
//! the patch entry and a patched vendored copy must be present together, or
//! neither. Inside a published `.crate` neither exists and the tests pass
//! trivially — which is also the state to expect once an upstream release
//! carries the fix and this file, the `[patch]` entry, and `vendor/libsql` are
//! all deleted together.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads a file with CRLF normalised away.
///
/// A Windows checkout with `core.autocrlf` on — which is what GitHub Actions
/// gives you — delivers these files with `\r\n`, and the structural parsing
/// below splits on embedded newlines. Without this, every check here passes on
/// Linux and macOS and fails on Windows, which is precisely the platform the
/// patch it guards exists for.
fn read_lf(path: &std::path::Path) -> Option<String> {
    Some(std::fs::read_to_string(path).ok()?.replace("\r\n", "\n"))
}

fn our_manifest() -> String {
    read_lf(&repo_root().join("Cargo.toml")).expect("read Cargo.toml")
}

/// `true` when our manifest routes libsql through the vendored copy.
fn patch_is_active(manifest: &str) -> bool {
    manifest
        .split("[patch.crates-io]")
        .nth(1)
        .is_some_and(|patch| {
            patch
                .lines()
                .take_while(|line| !line.starts_with('['))
                .any(|line| line.starts_with("libsql") && line.contains("vendor/libsql"))
        })
}

/// The `version = "..."` of the first `[package]` table in a Cargo manifest.
///
/// Scanned line by line rather than by splitting on an embedded `"\n[package]\n"`.
/// That earlier form depended on both the line ending and on the table not being
/// the first thing in the file, and it failed on Windows CI for the first
/// reason. `str::lines` handles `\r\n` and `\n` alike.
fn package_version(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .find_map(|line| {
            let value = line
                .trim()
                .strip_prefix("version")?
                .trim_start()
                .strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_string())
        })
}

#[test]
fn patch_entry_and_vendored_copy_agree_on_existing() {
    let vendored = repo_root().join("vendor/libsql");
    assert_eq!(
        patch_is_active(&our_manifest()),
        vendored.is_dir(),
        "the `[patch.crates-io]` entry for libsql and {} must be added and removed \
         together — one without the other either silently drops the double-close \
         fix (#367) or leaves dead vendored source behind.",
        vendored.display()
    );
}

/// Deleting the patch entry *and* `vendor/libsql` would satisfy every other
/// assertion here while quietly restoring upstream 0.9.30 and its double free.
/// A source checkout must therefore carry the patch outright; only a build from
/// a published archive — which has no `.git`, because `cargo package` omits both
/// the patch entry and the vendored copy — is allowed to run without it.
#[test]
fn a_source_checkout_must_carry_the_patch() {
    if !repo_root().join(".git").exists() {
        return;
    }
    assert!(
        patch_is_active(&our_manifest()),
        "this is a source checkout, so Cargo.toml must route libsql through \
         vendor/libsql. Without it every database connection is closed twice, \
         which is the Windows STATUS_ACCESS_VIOLATION of #367. Remove this test, \
         the `[patch.crates-io]` entry, and vendor/libsql together — and only \
         once an upstream libsql release carries the fix."
    );
}

#[test]
fn vendored_libsql_matches_the_requested_version() {
    let ours = our_manifest();
    if !patch_is_active(&ours) {
        return;
    }

    let requested = ours
        .lines()
        .find_map(|line| line.strip_prefix("libsql = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("Cargo.toml declares `libsql = \"<version>\"`");

    let vendored = read_lf(&repo_root().join("vendor/libsql/Cargo.toml"))
        .expect("read vendor/libsql/Cargo.toml");
    let vendored = package_version(&vendored).expect("vendor/libsql declares a package version");

    assert_eq!(
        vendored, requested,
        "vendor/libsql is {vendored} but Cargo.toml asks for libsql {requested}. \
         Re-vendor the new version and re-apply the double-close fix (#367), or \
         drop the patch entirely if the release already carries it."
    );
}

#[test]
fn vendored_libsql_still_carries_the_double_close_fix() {
    if !patch_is_active(&our_manifest()) {
        return;
    }

    let connection = repo_root().join("vendor/libsql/src/local/connection.rs");
    let source = read_lf(&connection).expect("read vendored connection.rs");

    let disconnect = source
        .split("pub fn disconnect(&mut self)")
        .nth(1)
        .expect("vendored libsql defines `Connection::disconnect`");
    let body = disconnect
        .split("\n    }")
        .next()
        .expect("`disconnect` has a body");

    let close = body.find("sqlite3_close_v2(self.raw)");
    let null = body.find("self.raw = std::ptr::null_mut()");

    assert!(
        null.is_some(),
        "the double-close fix is missing from {}: `disconnect` must null `raw` \
         after `sqlite3_close_v2`, or the handle is closed twice (#367).",
        connection.display()
    );
    assert!(
        body.contains("!self.raw.is_null()"),
        "the double-close fix is incomplete in {}: `disconnect` must skip an \
         already-closed handle (#367).",
        connection.display()
    );
    // Order matters as much as presence: nulling `raw` before the close would
    // satisfy both assertions above while closing nothing, leaking every
    // connection and its file handles instead of double-freeing them.
    assert!(
        close.is_some() && close < null,
        "the double-close fix is inverted in {}: `disconnect` must close the \
         handle and only then null `raw`, otherwise no connection is ever closed \
         (#367).",
        connection.display()
    );
}

/// The parsing above must survive a CRLF checkout.
///
/// This is not hypothetical: the first version of these guards split on
/// `"\n[package]\n"`, passed everywhere except Windows CI, and failed there
/// because `core.autocrlf` had already turned every newline into `\r\n`.
#[test]
fn manifest_parsing_survives_crlf() {
    let lf = "# generated\n[package]\nname = \"libsql\"\nversion = \"0.9.30\"\n";
    let crlf = lf.replace('\n', "\r\n");
    assert!(crlf.contains("\r\n"), "the fixture must actually be CRLF");

    assert_eq!(package_version(lf).as_deref(), Some("0.9.30"));
    assert_eq!(package_version(&crlf).as_deref(), Some("0.9.30"));

    // And with the table as the very first line, no preceding newline at all.
    let bare = "[package]\nversion = \"1.2.3\"\n";
    assert_eq!(package_version(bare).as_deref(), Some("1.2.3"));

    let patch = "[patch.crates-io]\nlibsql = { path = \"vendor/libsql\" }\n";
    assert!(patch_is_active(patch));
    assert!(patch_is_active(&patch.replace('\n', "\r\n")));
}
