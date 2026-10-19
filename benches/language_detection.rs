//! Run with `cargo bench --bench language_detection -- recognized-off` (or
//! recognized-on, unknown-off, unknown-on). Wrap the executable in `/usr/bin/time`
//! for CPU time and peak RSS; each case runs in its own process.
use std::time::Instant;
use tokensave::config::{load_config, save_config};
use tokensave::language_detection::statistics;
use tokensave::tokensave::TokenSave;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let case = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "unknown-on".into());
    let enabled = case.ends_with("-on");
    let recognized = case.starts_with("recognized-");
    let count = if recognized { 10_000 } else { 500 };
    let extension = if recognized { "rs" } else { "custom" };
    let dir = tempfile::tempdir()?;
    for i in 0..count {
        let source = if recognized || i % 5 != 0 {
            format!("use std::fmt;\npub fn symbol_{i}() {{ println!(\"hello\"); }}\n")
        } else {
            "ordinary text with no programming syntax\n".into()
        };
        std::fs::write(dir.path().join(format!("file_{i}.{extension}")), source)?;
    }
    drop(TokenSave::init(dir.path()).await?);
    let mut config = load_config(dir.path())?;
    config.language_detection.enabled = enabled;
    save_config(dir.path(), &config)?;
    let graph = TokenSave::open(dir.path()).await?;
    let before = statistics();
    let start = Instant::now();
    graph.index_all().await?;
    report(&case, "cold", start, before);
    // Establish settings metadata before measuring the no-op incremental path.
    graph.sync().await?;
    for _ in 0..3 {
        let before = statistics();
        let start = Instant::now();
        graph.sync().await?;
        report(&case, "warm", start, before);
    }
    for i in 1..=10 {
        std::fs::write(
            dir.path().join(format!("file_{i}.{extension}")),
            format!("use std::fmt;\npub fn changed_symbol_{i}() {{ println!(\"changed\"); }}\n"),
        )?;
    }
    let before = statistics();
    let start = Instant::now();
    graph.sync().await?;
    report(&case, "modified", start, before);
    let files = graph.get_all_files().await?;
    let false_positives = if recognized || !enabled {
        0
    } else {
        files
            .iter()
            .filter(|f| {
                f.path
                    .strip_prefix("file_")
                    .and_then(|s| s.strip_suffix(".custom"))
                    .and_then(|s| s.parse::<usize>().ok())
                    .is_some_and(|i| i % 5 == 0 && i > 10)
            })
            .count()
    };
    println!(
        "indexed_files={} false_positives={false_positives}/98_negative_fixtures",
        files.len()
    );
    drop(graph);
    dir.close()?;
    Ok(())
}

fn report(
    case: &str,
    phase: &str,
    start: Instant,
    before: tokensave::language_detection::Statistics,
) {
    let d = statistics() - before;
    let rss = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("VmRSS:"))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "RSS unavailable".into());
    println!("{rss}");
    println!("case={case} phase={phase} sync_ms={:.3} detection_ms={:.3} examined={} cache_hits={} shebang={} content={} unsupported={}",
        start.elapsed().as_secs_f64() * 1000.0, d.micros as f64 / 1000.0,
        d.examined, d.cache_hits, d.shebang, d.content, d.unsupported);
}
