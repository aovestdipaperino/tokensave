//! Environment manifest recorded with every result file, so a measurement
//! names the hardware, toolchain, engine and commit it came from.

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};
use tokensave::tokensave::TokenSave;

fn command_line(program: &str, args: &[&str], dir: &Path) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

async fn sqlite_version(cg: &TokenSave) -> Option<String> {
    let mut rows = cg
        .db()
        .conn()
        .query("SELECT sqlite_version()", ())
        .await
        .ok()?;
    let row = rows.next().await.ok()??;
    row.get::<String>(0).ok()
}

pub async fn collect(repo_root: &Path, cg: &TokenSave) -> Value {
    let sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::new()
            .with_memory(sysinfo::MemoryRefreshKind::new().with_ram())
            .with_cpu(sysinfo::CpuRefreshKind::new()),
    );
    let cpu = sys.cpus().first().map(|c| c.brand().trim().to_string());
    let dirty = command_line("git", &["status", "--porcelain"], repo_root).map(|s| !s.is_empty());
    json!({
        "tokensave_version": env!("CARGO_PKG_VERSION"),
        "git_commit": command_line("git", &["rev-parse", "HEAD"], repo_root),
        "git_dirty": dirty,
        "rustc": command_line("rustc", &["--version"], repo_root),
        "engine": "libsql 0.9.30 (vendored)",
        "sqlite_version": sqlite_version(cg).await,
        "os": sysinfo::System::long_os_version(),
        "kernel": sysinfo::System::kernel_version(),
        "cpu": cpu,
        "logical_cpus": sys.cpus().len(),
        "total_memory_bytes": sys.total_memory(),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
    })
}
