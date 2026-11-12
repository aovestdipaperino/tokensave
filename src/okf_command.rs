//! `tokensave okf` subcommands (#858).

use std::fmt::Write as _;

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
    }
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
