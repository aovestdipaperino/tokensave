//! Machine, toolchain and engine versions recorded with every result file.

use serde_json::{json, Value};

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn first_field(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_once(':').or_else(|| l.split_once('=')))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
}

fn command(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn collect() -> Value {
    let cpuinfo = read("/proc/cpuinfo");
    let meminfo = read("/proc/meminfo");
    let os = read("/etc/os-release");
    let kib = |key: &str| {
        first_field(&meminfo, key)
            .and_then(|v| v.trim_end_matches("kB").trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    json!({
        "cpu_model": first_field(&cpuinfo, "model name"),
        "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "mem_total_kib": kib("MemTotal"),
        "mem_available_kib": kib("MemAvailable"),
        "os": first_field(&os, "PRETTY_NAME"),
        "kernel": command("uname", &["-r"]),
        "rustc": command("rustc", &["-V"]),
        "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "allocator": if cfg!(feature = "turso-mimalloc") { "mimalloc (turso default feature)" } else { "system" },
        "tokio_runtime": "multi_thread (default worker count)",
        "libsql": env!("LIBSQL_VERSION"),
        "sqlite_bundled": "3.45.1 (vendored libsql, FTS5 enabled)",
        "turso": env!("TURSO_VERSION"),
        "turso_core": env!("TURSO_CORE_VERSION"),
        "tantivy": env!("TANTIVY_VERSION"),
    })
}
