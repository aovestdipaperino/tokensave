// Rust guideline compliant 2025-10-17
// Updated 2026-03-23: compact bordered table for status output
use clap::{Parser, Subcommand};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;

use codegraph::codegraph::CodeGraph;
use codegraph::context::{format_context_as_json, format_context_as_markdown};
use codegraph::types::*;

struct Spinner {
    frames: &'static [&'static str],
    idx: usize,
}

impl Spinner {
    fn new() -> Self {
        Self {
            frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            idx: 0,
        }
    }

    fn tick(&mut self, message: &str) {
        let frame = self.frames[self.idx % self.frames.len()];
        self.idx += 1;
        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r\x1b[2K{} {}", frame, message);
        let _ = stderr.flush();
    }

    fn done(message: &str) {
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "\r\x1b[2K\x1b[32m✔\x1b[0m {}", message);
        let _ = stderr.flush();
    }
}

/// Code intelligence for Rust codebases.
#[derive(Parser)]
#[command(name = "codegraph", about = "Code intelligence for Rust, Go, and Java codebases")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync the index (creates it if missing, incremental by default)
    Sync {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Force a full re-index
        #[arg(short, long)]
        force: bool,
    },
    /// Show project statistics
    Status {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Output as JSON
        #[arg(short, long)]
        json: bool,
    },
    /// Search for symbols
    Query {
        /// Search query
        search: String,
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Build context for a task
    Context {
        /// Task description
        task: String,
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
        /// Maximum symbols
        #[arg(short = 'n', long, default_value = "20")]
        max_nodes: usize,
        /// Output format (markdown or json)
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Start MCP server over stdio
    Serve {
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

async fn run(cli: Cli) -> codegraph::errors::Result<()> {
    let command = match cli.command {
        Some(cmd) => cmd,
        None => return handle_no_command().await,
    };
    match command {
        Commands::Sync { path, force } => {
            let project_path = resolve_path(path);
            if force || !CodeGraph::is_initialized(&project_path) {
                if !force {
                    eprintln!("No existing index found — performing full index");
                }
                init_and_index(&project_path).await?;
            } else {
                let cg = CodeGraph::open(&project_path).await?;
                let spinner = std::cell::RefCell::new(Spinner::new());
                let result = cg
                    .sync_with_progress(|phase, detail| {
                        let msg = if detail.is_empty() {
                            phase.to_string()
                        } else {
                            format!("{phase} {detail}")
                        };
                        spinner.borrow_mut().tick(&msg);
                    })
                    .await?;
                Spinner::done(&format!(
                    "sync done — {} added, {} modified, {} removed in {}ms",
                    result.files_added,
                    result.files_modified,
                    result.files_removed,
                    result.duration_ms
                ));
            }
        }
        Commands::Status { path, json } => {
            let project_path = resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let stats = cg.get_stats().await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats).unwrap_or_default()
                );
            } else {
                let tokens_saved = cg.get_tokens_saved().await.unwrap_or(0);
                print!("{}", include_str!("resources/logo.ansi"));
                print_status_table(&stats, tokens_saved);
            }
        }
        Commands::Query {
            search,
            path,
            limit,
        } => {
            let project_path = resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let results = cg.search(&search, limit).await?;
            if results.is_empty() {
                println!("No results found for '{}'", search);
            } else {
                for r in &results {
                    println!(
                        "{} ({}) - {}:{}",
                        r.node.name,
                        r.node.kind.as_str(),
                        r.node.file_path,
                        r.node.start_line
                    );
                    if let Some(sig) = &r.node.signature {
                        println!("  {}", sig);
                    }
                }
            }
        }
        Commands::Context {
            task,
            path,
            max_nodes,
            format,
        } => {
            let project_path = resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let output_format = if format == "json" {
                OutputFormat::Json
            } else {
                OutputFormat::Markdown
            };
            let options = BuildContextOptions {
                max_nodes,
                format: output_format.clone(),
                ..Default::default()
            };
            let context = cg.build_context(&task, &options).await?;
            match output_format {
                OutputFormat::Json => {
                    println!("{}", format_context_as_json(&context));
                }
                OutputFormat::Markdown => {
                    println!("{}", format_context_as_markdown(&context));
                }
            }
        }
        Commands::Serve { path } => {
            let project_path = resolve_path(path);
            let cg = ensure_initialized(&project_path).await?;
            let server = codegraph::mcp::McpServer::new(cg).await;
            server.run().await?;
        }
    }
    Ok(())
}

/// When invoked with no subcommand, offer to create the index if none exists.
async fn handle_no_command() -> codegraph::errors::Result<()> {
    let project_path = resolve_path(None);
    if CodeGraph::is_initialized(&project_path) {
        // Already initialized — show help via clap
        let _ = <Cli as clap::CommandFactory>::command().print_help();
        eprintln!();
        return Ok(());
    }
    eprint!(
        "No CodeGraph index found at '{}'. Create one now? [Y/n] ",
        project_path.display()
    );
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| codegraph::errors::CodeGraphError::Config {
            message: format!("failed to read stdin: {}", e),
        })?;
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("y") {
        init_and_index(&project_path).await?;
    }
    Ok(())
}

/// Initializes a new project (if needed) and runs a full index.
async fn init_and_index(project_path: &Path) -> codegraph::errors::Result<CodeGraph> {
    let cg = if CodeGraph::is_initialized(project_path) {
        CodeGraph::open(project_path).await?
    } else {
        let cg = CodeGraph::init(project_path).await?;
        eprintln!("Initialized CodeGraph at {}", project_path.display());
        cg
    };
    let spinner = std::cell::RefCell::new(Spinner::new());
    let result = cg.index_all_with_progress(|file| {
        spinner.borrow_mut().tick(&format!("indexing {}", file));
    }).await?;
    Spinner::done(&format!(
        "indexing done — {} files, {} nodes, {} edges in {}ms",
        result.file_count, result.node_count, result.edge_count, result.duration_ms
    ));
    Ok(cg)
}

/// Opens an existing project, or tells the user to run `codegraph sync` first.
async fn ensure_initialized(project_path: &Path) -> codegraph::errors::Result<CodeGraph> {
    if CodeGraph::is_initialized(project_path) {
        return CodeGraph::open(project_path).await;
    }
    Err(codegraph::errors::CodeGraphError::Config {
        message: format!(
            "no CodeGraph index found at '{}' — run 'codegraph sync' first",
            project_path.display()
        ),
    })
}

/// Formats a token count into a human-readable string (e.g. "12.3k", "1.5M").
fn format_token_count(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

/// Formats a byte count into a human-readable string (e.g. "798.0 MB").
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Formats a number with comma separators (e.g. 243302 -> "243,302").
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

/// Formats a single table cell with left-aligned label and right-aligned value.
fn format_cell(label: &str, value: &str, width: usize) -> String {
    let content_len = label.len() + value.len();
    let pad = width.saturating_sub(2 + content_len);
    format!(" {}{}{} ", label, " ".repeat(pad), value)
}

/// Builds a horizontal separator line (e.g. ├──┬──┬──┤).
fn table_separator(left: char, mid: char, right: char, cell_width: usize, num_cols: usize) -> String {
    let mut line = String::from(left);
    for i in 0..num_cols {
        line.push_str(&"─".repeat(cell_width));
        line.push(if i < num_cols - 1 { mid } else { right });
    }
    line
}

/// Prints the status output as a compact bordered table.
fn print_status_table(stats: &codegraph::types::GraphStats, tokens_saved: u64) {
    let version = env!("CARGO_PKG_VERSION");
    let num_cols = 3;

    // Prepare sorted node kinds
    let mut sorted_kinds: Vec<_> = stats.nodes_by_kind.iter().collect();
    sorted_kinds.sort_by_key(|(k, _)| (*k).clone());

    let num_kind_rows = sorted_kinds.len().div_ceil(num_cols);

    // Determine cell width from the widest node-kind entry
    let max_kind_len = sorted_kinds
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(10);
    let max_count_len = sorted_kinds
        .iter()
        .map(|(_, c)| format_number(**c).len())
        .max()
        .unwrap_or(5);
    // Ensure the cell also fits stat labels like "DB Size" + "798.0 MB"
    let cell_width = (max_kind_len + max_count_len + 3).max(22);
    let inner_width = cell_width * num_cols + (num_cols - 1);

    // Title row
    let title = format!("CodeGraph v{}", version);
    let tokens_text = format!("Tokens saved ~{}", format_token_count(tokens_saved));
    let title_pad = inner_width.saturating_sub(2 + title.len() + tokens_text.len());

    println!("{}", table_separator('╭', '─', '╮', cell_width, num_cols));
    println!(
        "│ {}{}\x1b[32m{}\x1b[0m │",
        title,
        " ".repeat(title_pad),
        tokens_text
    );

    // Stats rows
    println!("{}", table_separator('├', '┬', '┤', cell_width, num_cols));

    // Sort languages by file count descending
    let mut sorted_langs: Vec<_> = stats.files_by_language.iter().collect();
    sorted_langs.sort_by(|a, b| b.1.cmp(a.1));

    let db_size = format_bytes(stats.db_size_bytes);
    let source_size = format_bytes(stats.total_source_bytes);

    // Build stats rows: first row is counts, then DB/Source + one language per cell
    let mut stats_rows: Vec<Vec<(&str, String)>> = vec![vec![
        ("Files", format_number(stats.file_count)),
        ("Nodes", format_number(stats.node_count)),
        ("Edges", format_number(stats.edge_count)),
    ]];

    // Second row starts with DB Size + Source (or empty), then languages fill remaining cells
    let mut second_row: Vec<(&str, String)> = vec![("DB Size", db_size)];
    if stats.total_source_bytes > 0 {
        second_row.push(("Source", source_size));
    }
    // Fill languages into the remaining cells of this row and subsequent rows
    let mut lang_idx = 0;
    while second_row.len() < num_cols && lang_idx < sorted_langs.len() {
        let (lang, count) = sorted_langs[lang_idx];
        second_row.push((lang.as_str(), format_number(*count)));
        lang_idx += 1;
    }
    while second_row.len() < num_cols {
        second_row.push(("", String::new()));
    }
    stats_rows.push(second_row);

    // Any remaining languages go into additional rows
    while lang_idx < sorted_langs.len() {
        let mut row: Vec<(&str, String)> = Vec::new();
        for _ in 0..num_cols {
            if lang_idx < sorted_langs.len() {
                let (lang, count) = sorted_langs[lang_idx];
                row.push((lang.as_str(), format_number(*count)));
                lang_idx += 1;
            } else {
                row.push(("", String::new()));
            }
        }
        stats_rows.push(row);
    }

    for row in &stats_rows {
        print!("│");
        for (i, (label, value)) in row.iter().enumerate() {
            if label.is_empty() {
                print!("{}", " ".repeat(cell_width));
            } else {
                print!("{}", format_cell(label, value, cell_width));
            }
            print!("{}", if i < num_cols - 1 { "│" } else { "│\n" });
        }
    }

    // Node kinds section
    if !sorted_kinds.is_empty() {
        println!("{}", table_separator('├', '┼', '┤', cell_width, num_cols));

        for r in 0..num_kind_rows {
            print!("│");
            for c in 0..num_cols {
                let idx = r + c * num_kind_rows;
                if idx < sorted_kinds.len() {
                    let (kind, count) = &sorted_kinds[idx];
                    print!("{}", format_cell(kind, &format_number(**count), cell_width));
                } else {
                    print!("{}", " ".repeat(cell_width));
                }
                print!("{}", if c < num_cols - 1 { "│" } else { "│\n" });
            }
        }
    }

    println!("{}", table_separator('╰', '┴', '╯', cell_width, num_cols));
}

/// Resolves an optional path argument to an absolute `PathBuf`.
///
/// Defaults to the current working directory if no path is provided.
fn resolve_path(path: Option<String>) -> PathBuf {
    match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}
