//! Times a single-file extraction. Usage: `cargo run --release --example bench_extract <file>`.

use std::time::Instant;
use tokensave::extraction::{CExtractor, CppExtractor};

fn main() {
    let path = std::env::args().nth(1).expect("usage: bench_extract <file>");
    let source = std::fs::read_to_string(&path).expect("read file");
    eprintln!(
        "file: {} ({} bytes, {} lines)",
        path,
        source.len(),
        source.lines().count()
    );
    let t0 = Instant::now();
    let result = if path.ends_with(".cpp") || path.ends_with(".cc") || path.ends_with(".cxx")
        || path.ends_with(".hpp") || path.ends_with(".hh")
    {
        CppExtractor::extract_source(&path, &source)
    } else {
        CExtractor::extract_source(&path, &source)
    };
    let elapsed = t0.elapsed();
    eprintln!(
        "nodes={} edges={} time={:.3?}",
        result.nodes.len(),
        result.edges.len(),
        elapsed
    );
}
