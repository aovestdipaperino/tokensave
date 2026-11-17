//! Exposes the resolved engine versions from `Cargo.lock` so result files
//! name the engine that was actually linked, even after a pin change.

use std::path::Path;

fn locked_version(lock: &str, name: &str) -> String {
    let header = format!("name = \"{name}\"\n");
    lock.split("[[package]]")
        .find(|pkg| pkg.trim_start().starts_with(header.trim_end()))
        .and_then(|pkg| pkg.lines().find(|l| l.starts_with("version = ")))
        .map(|l| {
            l.trim_start_matches("version = ")
                .trim_matches('"')
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn main() {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    let lock = std::fs::read_to_string(&lock_path).unwrap_or_default();
    for (name, var) in [
        ("turso", "TURSO_VERSION"),
        ("turso_core", "TURSO_CORE_VERSION"),
        ("tantivy", "TANTIVY_VERSION"),
        ("libsql", "LIBSQL_VERSION"),
    ] {
        println!("cargo:rustc-env={var}={}", locked_version(&lock, name));
    }
}
