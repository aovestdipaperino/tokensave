//! `tokensave okf` subcommands (#858).

use std::fmt::Write as _;

use tokensave::okf::store;
use tokensave::okf::store::ImportOptions;
use tokensave::okf::store::ImportOutcome;
use tokensave::okf::validate::Report;
use tokensave::okf::validate::Severity;

use crate::cli::OkfAction;

/// Runs an `okf` subcommand and returns the process exit code.
pub(crate) async fn run(action: &OkfAction) -> tokensave::errors::Result<i32> {
    match action {
        OkfAction::Validate {
            path,
            json,
            strict,
            verbose,
        } => {
            let report = tokensave::okf::validate_dir(path)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", render(&report, *verbose));
            }
            let failed = !report.conformant || (*strict && report.warnings > 0);
            Ok(i32::from(failed))
        }
        OkfAction::Export {
            project,
            output,
            max_files,
            max_symbols,
            max_links,
        } => {
            let root = tokensave::config::resolve_path_with_discovery(project.clone());
            tokensave::cancel::install_signal_handlers();
            let options = tokensave::okf::export::Options {
                max_files: *max_files,
                max_symbols_per_file: *max_symbols,
                max_links: *max_links,
                generated_at: None,
            };
            let summary = tokensave::okf::export::export(&root, output, &options).await?;
            println!(
                "Exported {} source files, {} symbols and {} file dependencies to {} ({} \
                 files truncated).",
                summary.files,
                summary.symbols,
                summary.links,
                output.display(),
                summary.truncated_files
            );
            Ok(0)
        }
        OkfAction::Import {
            path,
            project,
            name,
            replace,
            strict,
            json,
        } => {
            let root = tokensave::config::resolve_path_with_discovery(project.clone());
            tokensave::cancel::install_signal_handlers();
            let options = ImportOptions {
                name: name.clone(),
                replace: *replace,
                strict: *strict,
                ..ImportOptions::default()
            };
            let outcome = store::import(&root, path, &options)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                print!("{}", render_import(&outcome));
            }
            Ok(0)
        }
        OkfAction::List { project, json } => {
            let root = tokensave::config::resolve_path_with_discovery(project.clone());
            let bundles = store::list(&root)?;
            if *json {
                let records: Vec<_> = bundles.iter().map(|b| &b.record).collect();
                println!("{}", serde_json::to_string_pretty(&records)?);
            } else if bundles.is_empty() {
                println!("No OKF bundles imported. Run `tokensave okf import <dir>`.");
            } else {
                for b in &bundles {
                    let r = &b.record;
                    println!(
                        "{}: {} concepts, {} files, imported {} from {} ({})",
                        r.name,
                        r.concepts,
                        r.files,
                        r.imported_at,
                        r.source,
                        if r.conformant {
                            "conformant"
                        } else {
                            "not conformant"
                        }
                    );
                }
                println!("Stored in {}", store::store_dir(&root).display());
            }
            Ok(0)
        }
        OkfAction::Remove { name, project } => {
            let root = tokensave::config::resolve_path_with_discovery(project.clone());
            if store::remove(&root, name)? {
                println!("Removed imported bundle `{name}`.");
                Ok(0)
            } else {
                eprintln!("No imported bundle named `{name}`.");
                Ok(1)
            }
        }
    }
}

fn render_import(outcome: &ImportOutcome) -> String {
    let mut out = String::new();
    let r = &outcome.record;
    let _ = writeln!(
        out,
        "{} bundle `{}`: {} concepts, {} files, {} bytes{}.",
        if outcome.replaced {
            "Replaced"
        } else {
            "Imported"
        },
        r.name,
        r.concepts,
        r.files,
        r.bytes,
        if r.unreadable_concepts > 0 {
            format!(
                "; {} concept files without a readable `type` are not searchable",
                r.unreadable_concepts
            )
        } else {
            String::new()
        }
    );
    if !r.conformant {
        let _ = writeln!(
            out,
            "The bundle is not conformant ({} errors, {} warnings); it was imported \
             best-effort. Run `tokensave okf validate` for details.",
            r.errors, r.warnings
        );
    } else if r.warnings > 0 {
        let _ = writeln!(
            out,
            "The bundle is conformant with {} warnings.",
            r.warnings
        );
    }
    for skipped in &outcome.skipped {
        let _ = writeln!(out, "Skipped {skipped}");
    }
    out
}

fn render(report: &Report, verbose: bool) -> String {
    let mut out = String::new();
    let mut infos = 0usize;
    for d in &report.diagnostics {
        if d.severity == Severity::Info && !verbose {
            infos += 1;
            continue;
        }
        let location = match (d.path.as_str(), d.line) {
            ("", _) => "<bundle>".to_string(),
            (path, Some(line)) => format!("{path}:{line}"),
            (path, None) => path.to_string(),
        };
        let _ = writeln!(out, "{} [{}] {location}: {}", d.code, d.severity, d.message);
    }
    let verdict = if report.conformant {
        "conformant"
    } else {
        "NOT conformant"
    };
    let declared = report
        .declared_version
        .as_deref()
        .map(|v| format!(", declares okf_version {v}"))
        .unwrap_or_default();
    let _ = writeln!(
        out,
        "OKF v{}: {verdict} — {} concepts, {} index and {} log files{declared}; {} errors, {} \
         warnings, {} info",
        report.spec_version,
        report.concepts,
        report.index_files,
        report.log_files,
        report.errors,
        report.warnings,
        report
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Info)
            .count(),
    );
    if infos > 0 {
        let _ = writeln!(
            out,
            "({infos} info findings hidden; use --verbose to list them)"
        );
    }
    out
}
