//! `tokensave purge` (#1055): show the plan, confirm, clean up, report.
//!
//! The planning and file removal live in [`tokensave::purge`]; this module
//! resolves the scope from the command line, runs the integration cleanup
//! shared with `tokensave uninstall`, and prints the results.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;

use tokensave::agents::uninstall::{UninstallOutput, UninstallPlan};
use tokensave::agents::{self, InstallContext, InstallScope};
use tokensave::purge::constants::{EXIT_INCOMPLETE, EXIT_REFUSED};
use tokensave::purge::provenance as install_provenance;
use tokensave::purge::{self, Category, PurgePlan, PurgeRequest};

/// The command-line options of `tokensave purge`.
pub(crate) struct PurgeArgs {
    pub dry_run: bool,
    pub yes: bool,
    pub local: bool,
    pub project: Vec<PathBuf>,
    pub all_projects: bool,
    pub include_history: bool,
}

/// Runs `tokensave purge` and returns the process exit code.
pub(crate) async fn run(args: PurgeArgs) -> tokensave::errors::Result<i32> {
    let home =
        tokensave::agents::home_dir().ok_or_else(|| tokensave::errors::TokenSaveError::Config {
            message: "could not determine home directory".to_string(),
        })?;
    // Decide the removal guidance before anything is deleted.
    let provenance = install_provenance::detect();

    let global = !args.local && args.project.is_empty();
    let mut projects = Vec::new();
    if args.local {
        projects.push(purge::ProjectTarget {
            root: tokensave::config::resolve_path_with_discovery(None),
            registered: false,
        });
    }
    for path in &args.project {
        let root = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        projects.push(purge::ProjectTarget {
            root,
            registered: false,
        });
    }
    let mut registry_note = None;
    if args.all_projects {
        let db_path = tokensave::config::get_global_tokensave_dir(&home)
            .join(purge::constants::GLOBAL_DB_FILENAME);
        match tokensave::global_db::GlobalDb::read_project_paths(&db_path).await {
            Some(paths) => {
                for path in paths {
                    let root = PathBuf::from(&path);
                    if root.is_absolute() {
                        projects.push(purge::ProjectTarget {
                            root,
                            registered: true,
                        });
                    } else {
                        registry_note = Some(format!(
                            "Ignored a registry entry that is not an absolute path: {path}"
                        ));
                    }
                }
            }
            None if db_path.exists() => {
                registry_note = Some(format!(
                    "Could not read the project registry in {}; no registered project is \
                     purged. Name projects with --project instead.",
                    db_path.display()
                ));
            }
            None => {}
        }
    }

    let request = PurgeRequest {
        agents: purge::agents_to_clean(&home),
        home: home.clone(),
        global,
        include_history: args.include_history,
        projects,
        all_projects: args.all_projects,
    };
    let plan = purge::plan(&request);
    print_plan(&plan, &request, registry_note.as_deref(), args.dry_run);
    let integration_plans = integration_plans(&plan, &request);
    let output = UninstallOutput {
        dry_run: args.dry_run,
        verbose: true,
        quiet: false,
    };
    for integrations in &integration_plans {
        integrations.print(output);
    }

    if args.dry_run {
        println!();
        println!("Dry run: nothing was changed.");
        print_reminder(&provenance);
        let failed: usize = integration_plans
            .iter()
            .map(|plan| plan.apply(output).failed)
            .sum();
        return Ok(if failed > 0 { EXIT_INCOMPLETE } else { 0 });
    }
    if plan.is_empty() {
        println!();
        println!("Nothing to purge.");
        print_reminder(&provenance);
        return Ok(0);
    }
    if !args.yes {
        if !io::stdin().is_terminal() {
            eprintln!(
                "\x1b[31merror:\x1b[0m refusing to purge without confirmation: stdin is not a \
                 terminal. Review the plan with --dry-run, then rerun with --yes."
            );
            return Ok(EXIT_REFUSED);
        }
        eprintln!();
        eprint!(
            "Type \x1b[1;31m{}\x1b[0m to delete the items above (anything else aborts): ",
            purge::constants::CONFIRMATION_WORD
        );
        io::stderr().flush().ok();
        let mut answer = String::new();
        if io::stdin().lock().read_line(&mut answer).is_err()
            || answer.trim() != purge::constants::CONFIRMATION_WORD
        {
            eprintln!("\x1b[33mAborted — nothing was changed.\x1b[0m");
            return Ok(EXIT_REFUSED);
        }
    }

    let integration_failures = integration_plans
        .iter()
        .map(|plan| plan.apply(output).failed)
        .sum::<usize>();
    let failures = integration_failures + execute(&plan, &request).await;
    println!();
    if failures == 0 {
        println!("\x1b[32mPurge complete.\x1b[0m");
    } else {
        println!(
            "\x1b[33mPurge incomplete: {failures} item(s) could not be removed (listed above).\x1b[0m \
             Fix the cause and run the same command again; it only acts on what is left."
        );
    }
    print_reminder(&provenance);
    Ok(if failures == 0 { 0 } else { EXIT_INCOMPLETE })
}

fn integration_plans(plan: &PurgePlan, request: &PurgeRequest) -> Vec<UninstallPlan> {
    let mut plans = Vec::new();
    let mut ctx = InstallContext {
        home: request.home.clone(),
        scope: InstallScope::Global,
        tokensave_bin: String::new(),
        tool_permissions: Vec::new(),
        force_permission_style: false,
    };
    if request.global {
        let mut global = UninstallPlan::default();
        global.plan_agents(&plan.global_agents, &ctx);
        if plan.global_hooks {
            agents::plan_global_hook_removal(&request.home, &mut global);
        }
        plans.push(global);
    }
    for root in &plan.project_roots {
        ctx.scope = InstallScope::Local {
            project_path: root.clone(),
        };
        let mut local = UninstallPlan::for_scope(&ctx.scope);
        local.plan_agents(&plan.local_agents, &ctx);
        agents::plan_local_hook_removal(root, &mut local);
        plans.push(local);
    }
    plans
}

/// Performs the data removal plan. Returns the number of failures.
async fn execute(plan: &PurgePlan, request: &PurgeRequest) -> usize {
    let mut failures = 0usize;
    println!();
    let report = purge::execute_removals(plan);
    for path in &report.removed {
        println!("\x1b[32m✔\x1b[0m deleted {}", path.display());
    }
    for path in &report.already_absent {
        println!("  already gone: {}", path.display());
    }
    for (path, error) in &report.failed {
        println!(
            "\x1b[31m✘\x1b[0m could not delete {}: {error}",
            path.display()
        );
    }
    failures += report.failed.len();

    if !plan.registry_rows.is_empty() {
        let db_path = tokensave::config::get_global_tokensave_dir(&request.home)
            .join(purge::constants::GLOBAL_DB_FILENAME);
        if db_path.is_file() {
            match tokensave::global_db::GlobalDb::open_at(&db_path).await {
                Some(db) => {
                    db.delete_projects(&plan.registry_rows).await;
                }
                None => {
                    println!(
                        "\x1b[31m✘\x1b[0m could not update the project registry in {}",
                        db_path.display()
                    );
                    failures += 1;
                }
            }
        }
    }
    failures
}

fn print_plan(
    plan: &PurgePlan,
    request: &PurgeRequest,
    registry_note: Option<&str>,
    dry_run: bool,
) {
    let heading = if dry_run {
        "tokensave purge — plan (dry run)"
    } else {
        "tokensave purge — plan"
    };
    println!("\x1b[1m{heading}\x1b[0m");
    println!();
    let project_count = request.projects.len();
    let scope = match (request.global, project_count) {
        (true, 0) => {
            "global state: agent integrations, global git hooks, settings and caches".to_string()
        }
        (true, n) => format!(
            "global state and {n} registered project(s): agent integrations, git hooks, \
             settings, caches and indexes"
        ),
        (false, n) => {
            format!("{n} project(s): index, project-scoped agent config and repository git hooks")
        }
    };
    println!("Scope: {scope}");
    if request.global {
        if plan.global_agents.is_empty() {
            println!("Agent integrations: none found");
        } else {
            println!(
                "Agent integrations to remove: {}",
                plan.global_agents.join(", ")
            );
        }
        println!(
            "Global git hooks: {}",
            if plan.global_hooks {
                "tokensave's sections will be removed"
            } else {
                "none installed"
            }
        );
    }
    if !plan.project_roots.is_empty() {
        println!("Project-scoped agent config and repository git hooks cleaned in:");
        for root in &plan.project_roots {
            println!("  {}", root.display());
        }
    }
    if let Some(note) = registry_note {
        println!("\x1b[33m{note}\x1b[0m");
    }

    println!();
    if plan.removals.is_empty() {
        println!("Files to delete: none");
    } else {
        println!(
            "Files to delete ({} item(s), about {}):",
            plan.removals.len(),
            purge::human_size(plan.total_bytes())
        );
        for removal in &plan.removals {
            let keep = removal
                .keep_child
                .map_or_else(String::new, |c| format!("; keeps {c}/"));
            let warn = if removal.category == Category::UsageHistory {
                "\x1b[31m"
            } else {
                ""
            };
            println!(
                "  {warn}[{}]\x1b[0m {} — {}, {}{keep}",
                removal.category.label(),
                removal.path.display(),
                removal.evidence,
                purge::human_size(removal.bytes)
            );
        }
    }
    if !plan.kept.is_empty() {
        println!("Kept:");
        for note in &plan.kept {
            println!("  {} — {}", note.path.display(), note.reason);
        }
    }
    if !plan.skipped.is_empty() {
        println!("Skipped:");
        for note in &plan.skipped {
            println!("  {} — {}", note.path.display(), note.reason);
        }
    }
    if !plan.absent.is_empty() {
        println!("Nothing to delete at:");
        for path in &plan.absent {
            println!("  {}", path.display());
        }
    }
    println!();
    if request.global && project_count == 0 {
        println!(
            "Project indexes are not touched in this scope; add --all-projects, or use \
             --local or --project <PATH>."
        );
    }
    if !request.global {
        println!("Global settings, caches and agent integrations are not touched in this scope.");
    }
    println!(
        "Never touched: project source files, other tools' settings in shared agent config \
         files, git hooks tokensave did not write, and the tokensave executable."
    );
    for note in &plan.notes {
        println!("\x1b[33mNote:\x1b[0m {note}");
    }
}

fn print_reminder(provenance: &install_provenance::Provenance) {
    println!("{}", install_provenance::removal_reminder(provenance));
}
