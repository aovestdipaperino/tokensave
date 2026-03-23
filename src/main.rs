// Rust guideline compliant 2025-10-17
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
                let result = cg.sync().await?;
                println!(
                    "Sync complete: {} added, {} modified, {} removed in {}ms",
                    result.files_added, result.files_modified, result.files_removed, result.duration_ms
                );
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
                println!("CodeGraph Status");
                println!("  Files:  {}", stats.file_count);
                println!("  Nodes:  {}", stats.node_count);
                println!("  Edges:  {}", stats.edge_count);
                println!("  DB Size: {} bytes", stats.db_size_bytes);
                if !stats.nodes_by_kind.is_empty() {
                    println!("\n  Nodes by kind:");
                    let mut sorted: Vec<_> = stats.nodes_by_kind.iter().collect();
                    sorted.sort_by_key(|(k, _)| (*k).clone());
                    for (kind, count) in &sorted {
                        println!("    {}: {}", kind, count);
                    }
                }
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

/// Resolves an optional path argument to an absolute `PathBuf`.
///
/// Defaults to the current working directory if no path is provided.
fn resolve_path(path: Option<String>) -> PathBuf {
    match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}
