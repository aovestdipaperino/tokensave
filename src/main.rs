use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use codegraph::codegraph::CodeGraph;
use codegraph::context::{format_context_as_json, format_context_as_markdown};
use codegraph::types::*;

/// Code intelligence for Rust codebases.
#[derive(Parser)]
#[command(name = "codegraph", about = "Code intelligence for Rust codebases")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new CodeGraph project
    Init {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Run initial indexing after init
        #[arg(short, long)]
        index: bool,
    },
    /// Full re-index of the project
    Index {
        /// Project path (default: current directory)
        path: Option<String>,
        /// Clear existing data before indexing
        #[arg(short, long)]
        force: bool,
    },
    /// Incremental sync of changed files
    Sync {
        /// Project path (default: current directory)
        path: Option<String>,
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
    /// Start MCP server (stub for now)
    Serve {
        /// Project path
        #[arg(short, long)]
        path: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run(cli: Cli) -> codegraph::errors::Result<()> {
    match cli.command {
        Commands::Init { path, index } => {
            let project_path = resolve_path(path);
            let cg = CodeGraph::init(&project_path)?;
            println!("Initialized CodeGraph at {}", project_path.display());
            if index {
                let result = cg.index_all()?;
                println!(
                    "Indexed {} files: {} nodes, {} edges in {}ms",
                    result.file_count, result.node_count, result.edge_count, result.duration_ms
                );
            }
        }
        Commands::Index { path, force: _ } => {
            let project_path = resolve_path(path);
            let cg = CodeGraph::open(&project_path)?;
            let result = cg.index_all()?;
            println!(
                "Indexed {} files: {} nodes, {} edges in {}ms",
                result.file_count, result.node_count, result.edge_count, result.duration_ms
            );
        }
        Commands::Sync { path } => {
            let project_path = resolve_path(path);
            let cg = CodeGraph::open(&project_path)?;
            let result = cg.sync()?;
            println!(
                "Sync complete: {} added, {} modified, {} removed in {}ms",
                result.files_added, result.files_modified, result.files_removed, result.duration_ms
            );
        }
        Commands::Status { path, json } => {
            let project_path = resolve_path(path);
            let cg = CodeGraph::open(&project_path)?;
            let stats = cg.get_stats()?;
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
            let cg = CodeGraph::open(&project_path)?;
            let results = cg.search(&search, limit)?;
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
            let cg = CodeGraph::open(&project_path)?;
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
            let context = cg.build_context(&task, &options)?;
            match output_format {
                OutputFormat::Json => {
                    println!("{}", format_context_as_json(&context));
                }
                OutputFormat::Markdown => {
                    println!("{}", format_context_as_markdown(&context));
                }
            }
        }
        Commands::Serve { path: _ } => {
            println!("MCP server will be implemented in Task 10");
        }
    }
    Ok(())
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
