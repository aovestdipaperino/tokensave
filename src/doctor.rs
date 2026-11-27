//! Doctor command: comprehensive health check of the tokensave installation.
//!
//! Checks the binary, project index, global DB, user config, agent
//! integrations, and network connectivity.

use crate::agents::hooks as tokensave_hooks;
use std::path::Path;

use crate::agents::{self, DoctorCounters, HealthcheckContext};
use crate::display::{format_bytes, format_token_count};
use crate::tokensave::TokenSave;

/// Options for the Caveman compatibility check (`doctor --caveman`, #797).
#[derive(Debug, Clone, Copy, Default)]
pub struct CavemanCheck<'a> {
    /// Explicitly named Claude Code session whose Caveman mode to report.
    pub session_id: Option<&'a str>,
}

/// Runs a comprehensive health check of the tokensave installation.
///
/// With `caveman`, a Caveman compatibility section is added to the normal
/// checks and the per-agent report is printed as JSON on stdout after the
/// summary; the human-readable output stays on stderr.
pub async fn run_doctor(agent_filter: Option<&str>, caveman: Option<CavemanCheck<'_>>) {
    debug_assert!(
        !env!("CARGO_PKG_VERSION").is_empty(),
        "CARGO_PKG_VERSION must not be empty"
    );
    let mut dc = DoctorCounters::new();

    eprintln!(
        "\n\x1b[1mtokensave doctor v{}\x1b[0m\n",
        env!("CARGO_PKG_VERSION")
    );

    check_binary(&mut dc);

    eprintln!("\n\x1b[1mCurrent project\x1b[0m");
    let project_path = crate::config::resolve_path_with_discovery(None);
    if TokenSave::is_initialized(&project_path) {
        dc.pass(&format!(
            "Index found: {}",
            crate::config::get_tokensave_dir(&project_path).display()
        ));
        check_database(&mut dc, &project_path).await;
        check_hook_freshness(&mut dc, &project_path);
    } else {
        dc.warn(&format!(
            "No index at {} — run `tokensave init`",
            crate::config::get_tokensave_dir(&project_path).display()
        ));
    }

    check_global_db(&mut dc);
    check_index_scope(&mut dc, &project_path);
    check_stale_stores(&mut dc).await;
    check_user_config(&mut dc);
    check_state_location(&mut dc, &project_path);

    // Agent-specific health checks
    if let Some(ref home) = agents::home_dir() {
        let hctx = HealthcheckContext {
            home: home.clone(),
            project_path: project_path.clone(),
        };
        let installed = crate::user_config::UserConfig::load().installed_agents;
        match agent_filter {
            Some(id) => match agents::get_integration(id) {
                Ok(ag) => check_requested_agent(&mut dc, ag.as_ref(), &hctx, &installed),
                Err(e) => dc.fail(&format!("{e}")),
            },
            None => {
                check_agents(&mut dc, &agents::all_integrations(), &hctx, &installed);
            }
        }
    } else {
        dc.fail("Could not determine home directory");
    }

    let caveman_reports =
        caveman.map(|check| check_caveman(&mut dc, agent_filter, check, &project_path));

    check_network(&mut dc);
    print_summary(&dc);

    if let Some(reports) = caveman_reports {
        match serde_json::to_string_pretty(&reports) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Could not serialize the Caveman report: {e}"),
        }
    }
}

/// What [`check_agents`] found, for the summary and for tests.
#[derive(Debug, Default, PartialEq, Eq)]
struct AgentOverview {
    /// Agents checked in full because tokensave is installed in them.
    installed: Vec<&'static str>,
    /// Agents checked in full because tokensave-owned content remains
    /// although they are not registered.
    leftover: Vec<&'static str>,
    /// Agents present on this machine without tokensave: `(name, id)`.
    detected_absent: Vec<(&'static str, &'static str)>,
    /// Agents neither installed nor detected.
    absent: Vec<&'static str>,
}

/// Check every agent integration, counting failures only for agents where
/// tokensave is installed or has left content behind (#1116).
///
/// An agent the user never set up is not applicable: it is listed once with
/// the command that adds it, and adds nothing to the issue or warning totals.
fn check_agents(
    dc: &mut DoctorCounters,
    all: &[Box<dyn agents::AgentIntegration>],
    hctx: &HealthcheckContext,
    installed: &[String],
) -> AgentOverview {
    use agents::IntegrationPresence;
    let mut overview = AgentOverview::default();
    for ag in all {
        match agents::integration_presence(ag.as_ref(), &hctx.home, installed) {
            IntegrationPresence::Installed => {
                ag.healthcheck(dc, hctx);
                overview.installed.push(ag.name());
            }
            IntegrationPresence::Leftover => {
                ag.healthcheck(dc, hctx);
                dc.warn(&format!(
                    "{} has leftover tokensave content but is not registered as installed — \
                     run `tokensave install --agent {id}` to repair it, or \
                     `tokensave uninstall --agent {id}` to finish removing it",
                    ag.name(),
                    id = ag.id()
                ));
                overview.leftover.push(ag.name());
            }
            IntegrationPresence::Absent { detected: true } => {
                overview.detected_absent.push((ag.name(), ag.id()));
            }
            IntegrationPresence::Absent { detected: false } => {
                overview.absent.push(ag.name());
            }
        }
    }
    print_agent_overview(dc, &overview);
    overview
}

/// Print the agent integration summary (#1116).
fn print_agent_overview(dc: &mut DoctorCounters, overview: &AgentOverview) {
    eprintln!("\n\x1b[1mAgent integrations\x1b[0m");
    if overview.installed.is_empty() && overview.leftover.is_empty() {
        dc.warn("tokensave is not installed in any agent — run `tokensave install`");
    } else if !overview.installed.is_empty() {
        dc.pass(&format!("Installed: {}", overview.installed.join(", ")));
    }
    if !overview.leftover.is_empty() {
        dc.info(&format!(
            "Partially installed or incompletely removed: {}",
            overview.leftover.join(", ")
        ));
    }
    if !overview.detected_absent.is_empty() {
        let names: Vec<_> = overview.detected_absent.iter().map(|(n, _)| *n).collect();
        dc.info(&format!(
            "Detected without tokensave (not checked): {}",
            names.join(", ")
        ));
        let commands: Vec<_> = overview
            .detected_absent
            .iter()
            .map(|(_, id)| format!("tokensave install --agent {id}"))
            .collect();
        dc.info(&format!("  To add one: {}", commands.join(" / ")));
    }
    if !overview.absent.is_empty() {
        dc.info(&format!(
            "Not installed (not applicable): {}",
            overview.absent.join(", ")
        ));
    }
}

/// `doctor --agent ID`: the user asked about this agent, so it is checked in
/// full when tokensave is in it; otherwise one line says so instead of a
/// cascade of missing-file failures (#1116).
fn check_requested_agent(
    dc: &mut DoctorCounters,
    ag: &dyn agents::AgentIntegration,
    hctx: &HealthcheckContext,
    installed: &[String],
) {
    match agents::integration_presence(ag, &hctx.home, installed) {
        agents::IntegrationPresence::Absent { detected } => {
            eprintln!("\n\x1b[1m{} integration\x1b[0m", ag.name());
            let found = if detected {
                "is on this machine, but tokensave is not installed in it"
            } else {
                "was not found and tokensave is not installed in it"
            };
            dc.warn(&format!(
                "{} {found} — run `tokensave install --agent {}`",
                ag.name(),
                ag.id()
            ));
        }
        agents::IntegrationPresence::Installed | agents::IntegrationPresence::Leftover => {
            ag.healthcheck(dc, hctx);
        }
    }
}

/// Reports Caveman compatibility for the selected agents. Read-only: no
/// file is written and no Caveman code runs.
fn check_caveman(
    dc: &mut DoctorCounters,
    agent_filter: Option<&str>,
    check: CavemanCheck<'_>,
    project_path: &Path,
) -> Vec<crate::caveman::Report> {
    use crate::caveman::{Activation, Installation};

    eprintln!("\n\x1b[1mCaveman compatibility\x1b[0m");
    let reports = crate::caveman::diagnostics(agent_filter, check.session_id, project_path);
    let source = if std::env::var_os(crate::caveman::COMPATIBILITY_ENV).is_some() {
        crate::caveman::COMPATIBILITY_ENV
    } else {
        "config.toml"
    };
    let enabled = reports
        .first()
        .map_or_else(crate::caveman::enabled, |r| r.enabled);
    dc.info(&format!(
        "caveman_compatibility {} ({source})",
        if enabled { "on" } else { "off" }
    ));
    for report in &reports {
        let agent = &report.agent;
        match report.installation {
            Installation::Absent => dc.info(&format!("{agent}: no Caveman skill found")),
            Installation::Unsupported => {
                dc.info(&format!("{agent}: no Caveman discovery adapter"));
            }
            Installation::Partial => {
                dc.warn(&format!(
                    "{agent}: Caveman installation incomplete or malformed"
                ));
            }
            Installation::Installed => {
                let version = report.version.as_deref().unwrap_or("unknown release");
                let skill = report
                    .skill
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                dc.pass(&format!("{agent}: Caveman skill ({version}) at {skill}"));
            }
        }
        match report.activation {
            Activation::Active => dc.info(&format!("{agent}: Caveman active in this session")),
            Activation::Inactive => {
                dc.info(&format!("{agent}: Caveman off in this session"));
            }
            Activation::Unknown => {}
        }
        if report.installation != Installation::Absent
            && report.installation != Installation::Unsupported
        {
            for warning in &report.warnings {
                dc.info(&format!("{agent}: {warning}"));
            }
        }
    }
    reports
}

/// Check database health: report size and run VACUUM to reclaim space.
async fn check_database(dc: &mut DoctorCounters, project_path: &Path) {
    let db_path = crate::config::get_tokensave_dir(project_path).join("tokensave.db");
    let size_before = std::fs::metadata(&db_path).map_or(0, |m| m.len());

    let ts = match TokenSave::open(project_path).await {
        Ok(ts) => ts,
        Err(e) => {
            dc.fail(&format!("Could not open database: {e}"));
            return;
        }
    };

    dc.pass(&format!("DB size: {}", format_bytes(size_before)));

    eprintln!("    Compacting database (VACUUM)…");
    match ts.optimize().await {
        Ok(()) => {
            let size_after = std::fs::metadata(&db_path).map_or(size_before, |m| m.len());
            if size_before > size_after {
                let reclaimed = size_before - size_after;
                dc.pass(&format!(
                    "Compacted: {} → {} (reclaimed {})",
                    format_bytes(size_before),
                    format_bytes(size_after),
                    format_bytes(reclaimed),
                ));
            } else {
                dc.pass("Database already compact");
            }
        }
        Err(e) => {
            dc.warn(&format!("VACUUM failed: {e}"));
        }
    }
}

/// Check binary location and version.
fn check_binary(dc: &mut DoctorCounters) {
    eprintln!("\x1b[1mBinary\x1b[0m");
    if let Ok(exe) = std::env::current_exe() {
        dc.pass(&format!("Binary: {}", exe.display()));
    } else {
        dc.fail("Could not determine binary path");
    }
    dc.pass(&format!("Version: {}", env!("CARGO_PKG_VERSION")));
    if let Some(hint) = crate::integrity::current_low_integrity_hint() {
        dc.fail(&hint);
    }
}

/// Check global database exists.
fn check_global_db(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mGlobal database\x1b[0m");
    if let Some(db_path) = crate::global_db::global_db_path() {
        if db_path.exists() {
            dc.pass(&format!("Global DB: {}", db_path.display()));
        } else {
            dc.warn("Global DB not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for global DB");
    }
}

/// An index this large is more likely a scope accident than a big project
/// (#450). The reported runaway was 29.9 GB; a real project on the same
/// machine was 420 MB. Deliberately well above any plausible monorepo, since
/// this only warns and a false positive costs the user a line of output.
use crate::index_scope::{index_size_bytes, same_directory, OVERSIZED_INDEX_BYTES};

/// Warn about an index whose *scope* is wrong rather than whose contents are
/// (#450).
///
/// A home directory initialized as a project — the shape #372 could produce —
/// leaves an index that every later `serve` inherits and keeps syncing. The
/// reported case reached 29.9 GB on disk and a 38 GB footprint once mapped,
/// with the machine paging heavily, and nothing surfaced it: `serve`'s
/// existing guardrail refuses *uninitialized* directories, and this directory
/// is initialized, just absurd in scope.
///
/// Both checks are warnings, not failures. Neither state is corrupt, and the
/// remedy is a judgment call the user has to make — so this reports what is
/// there and leaves the decision alone.
fn check_index_scope(dc: &mut DoctorCounters, project_path: &Path) {
    eprintln!("\n\x1b[1mIndex scope\x1b[0m");
    check_index_scope_with(dc, project_path, agents::home_dir().as_deref());
}

/// [`check_index_scope`] with the home directory injected, so tests can build
/// both states on disk without mutating process-global environment.
fn check_index_scope_with(dc: &mut DoctorCounters, project_path: &Path, home: Option<&Path>) {
    // Checked whatever the working directory is: the whole problem with this
    // state is that nobody is standing in it when it does the damage.
    // Shared with the warning `serve` now prints (#450), so the two cannot
    // drift into disagreeing about what counts as a scope problem. `doctor`
    // additionally reports the healthy states, which a server start does not.
    let warnings = crate::index_scope::scope_warnings(project_path, home);
    for warning in &warnings {
        dc.warn(warning);
    }

    let home_indexed = home.filter(|home| TokenSave::is_initialized(home));
    if home_indexed.is_none() {
        dc.pass("Home directory is not indexed as a project");
    }
    let is_home = home_indexed.is_some_and(|home| same_directory(home, project_path));
    if !is_home && TokenSave::is_initialized(project_path) {
        let size = index_size_bytes(project_path);
        if size < OVERSIZED_INDEX_BYTES {
            dc.pass(&format!(
                "Index size is unremarkable: {}",
                format_bytes(size)
            ));
        }
    }
}

/// Lists projects registered in the global DB whose `.tokensave/` directory
/// is gone, and offers to purge them. Stale rows are harmless but show up in
/// `tokensave list --all` and inflate the global tokens-saved count.
async fn check_stale_stores(dc: &mut DoctorCounters) {
    use std::io::{IsTerminal, Write};

    let Some(gdb) = crate::global_db::GlobalDb::open().await else {
        return;
    };
    let stale: Vec<String> = gdb
        .list_project_paths()
        .await
        .into_iter()
        .filter(|p| {
            !crate::config::get_tokensave_dir(Path::new(p))
                .join("tokensave.db")
                .exists()
        })
        .collect();
    if stale.is_empty() {
        dc.pass("No stale projects in global DB");
        return;
    }

    eprintln!(
        "  \x1b[33m!\x1b[0m {} stale project(s) in global DB (registered but `.tokensave/` is gone):",
        stale.len()
    );
    let preview = stale.len().min(10);
    for p in &stale[..preview] {
        dc.info(&format!("  • {p}"));
    }
    if stale.len() > preview {
        dc.info(&format!("  … and {} more", stale.len() - preview));
    }

    if !std::io::stdin().is_terminal() {
        dc.warnings += 1;
        dc.info("    Re-run `tokensave doctor` interactively to purge them.");
        return;
    }

    eprint!(
        "  Purge {} stale row(s) from the global DB? [Y/n] ",
        stale.len()
    );
    std::io::stderr().flush().ok();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        dc.warnings += 1;
        return;
    }
    let answer = answer.trim();
    if !answer.is_empty() && !answer.eq_ignore_ascii_case("y") {
        dc.warnings += 1;
        dc.info("Skipped — run again later to purge.");
        return;
    }

    let purged = gdb.delete_projects(&stale).await;
    dc.pass(&format!("Purged {purged} stale project(s)"));
}

/// Check where project state lives (#919): the base state directory
/// setting, whether this project's state is where the setting puts it, and
/// relocated state whose project root is gone. Reports only; nothing is
/// moved or deleted here.
fn check_state_location(dc: &mut DoctorCounters, project_path: &Path) {
    use crate::state_dir::{self, BaseSource};
    eprintln!(
        "
[1mProject state location[0m"
    );
    let setting = state_dir::setting();
    let label = match setting.source {
        BaseSource::Env => state_dir::STATE_DIR_ENV,
        BaseSource::UserConfig => "state_dir in config.toml",
        BaseSource::Default => "",
    };
    match (&setting.base, setting.raw.as_deref().map(str::trim)) {
        (Some(base), _) => dc.pass(&format!("Base state directory ({label}): {}", base.display())),
        (None, Some(raw)) if !raw.is_empty() => dc.warn(&format!(
            "{label} = {raw:?} is not an absolute path, so it is ignored and state lives under each project root"
        )),
        (None, Some(_)) => dc.pass(&format!(
            "{label} is empty: state lives under each project root"
        )),
        (None, None) => dc.pass("State lives under each project root (`.tokensave/`)"),
    }

    if TokenSave::is_initialized(project_path) {
        let actual = crate::config::get_tokensave_dir(project_path);
        let configured = crate::config::configured_project_state_dir(project_path);
        if actual != configured {
            dc.warn(&format!(
                "This project's state is at {}, not at {} where the current setting puts it. tokensave keeps using the existing state; to follow the setting, stop its servers and move the directory there.",
                actual.display(),
                configured.display()
            ));
        }
    }

    let entries = state_dir::scan();
    let orphans: Vec<_> = entries.iter().filter(|e| e.is_orphaned()).collect();
    if !orphans.is_empty() {
        dc.warn(&format!(
            "{} relocated state director{} whose project no longer exists:",
            orphans.len(),
            if orphans.len() == 1 { "y" } else { "ies" }
        ));
        for entry in &orphans {
            if let Some(origin) = &entry.origin {
                dc.info(&format!(
                    "  • {} (project {}, {}, recorded by {})",
                    entry.dir.display(),
                    origin.project_root,
                    crate::display::format_bytes(state_dir::dir_size(&entry.dir)),
                    origin.version
                ));
            }
        }
        dc.info("    Delete them with `tokensave state prune` (preview with --dry-run).");
    } else if !entries.is_empty() {
        dc.pass(&format!(
            "{} relocated state director{}, none orphaned",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" }
        ));
    }
    let unknown = entries.iter().filter(|e| e.origin.is_none()).count();
    if unknown > 0 {
        dc.info(&format!(
            "  {unknown} relocated state director{} without {}; `tokensave state list` shows them",
            if unknown == 1 { "y" } else { "ies" },
            state_dir::ORIGIN_FILENAME
        ));
    }
}

/// Check user config file.
fn check_user_config(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mUser config\x1b[0m");
    if let Some(config_path) = crate::user_config::config_path() {
        if config_path.exists() {
            let config = crate::user_config::UserConfig::load();
            dc.pass(&format!("Config: {}", config_path.display()));
            if let Some(var) = crate::cloud::telemetry_opt_out() {
                dc.info(&format!("Telemetry disabled by {var} (no counter upload)"));
            } else if config.upload_enabled {
                dc.pass("Upload enabled");
            } else {
                dc.info("Upload disabled (opt-out)");
            }
            if config.pending_upload > 0 {
                dc.info(&format!("Pending upload: {} tokens", config.pending_upload));
            }
        } else {
            dc.warn("Config not yet created (created on first sync)");
        }
    } else {
        dc.fail("Could not determine home directory for config");
    }
    let source = if std::env::var_os(crate::auto_init::AUTO_INIT_ENV).is_some() {
        crate::auto_init::AUTO_INIT_ENV
    } else {
        "config.toml"
    };
    if crate::auto_init::enabled() {
        dc.info(&format!(
            "auto_init on ({source}): new git repositories are indexed on first commit or serve"
        ));
    } else {
        dc.info(&format!(
            "auto_init off ({source}): run `tokensave init` in new repositories"
        ));
    }
    let (off, unknown) =
        disabled_tool_areas_report(&crate::user_config::disabled_tool_areas_setting());
    for line in off {
        dc.info(&line);
    }
    if let Some(line) = unknown {
        dc.warn(&line);
    }
}

/// The doctor lines for the `disabled_tool_areas` setting: one line for each
/// area that is off, with the tools that the MCP server does not run, and a
/// warning for the values that name no area. No line when the setting is
/// empty.
fn disabled_tool_areas_report(configured: &[String]) -> (Vec<String>, Option<String>) {
    use crate::mcp::tools::{partition_tool_areas, tools_in_area, DISABLED_TOOL_AREAS_SETTING};
    let (areas, unknown) = partition_tool_areas(configured);
    let off = areas
        .iter()
        .map(|area| {
            format!(
                "Tool area \"{area}\" off ({DISABLED_TOOL_AREAS_SETTING}): the MCP server \
                 does not list or run {}",
                tools_in_area(area).join(", ")
            )
        })
        .collect();
    let unknown = (!unknown.is_empty()).then(|| {
        format!(
            "{DISABLED_TOOL_AREAS_SETTING} names no tool area: {}",
            unknown.join(", ")
        )
    });
    (off, unknown)
}

/// Check network connectivity.
fn check_network(dc: &mut DoctorCounters) {
    eprintln!("\n\x1b[1mNetwork\x1b[0m");
    if let Some(var) = crate::cloud::telemetry_opt_out() {
        dc.info(&format!(
            "Worldwide counter check skipped ({var} disables telemetry)"
        ));
    } else if let Some(total) = crate::cloud::fetch_worldwide_total() {
        dc.pass(&format!(
            "Worldwide counter reachable (total: {})",
            format_token_count(total)
        ));
    } else {
        dc.warn("Worldwide counter unreachable (offline or timeout)");
    }
    // Reachability and installability are separate findings: a release with no
    // asset for this platform means GitHub answered fine (#513).
    match crate::cloud::try_fetch_latest_version() {
        Ok(_) => dc.pass("GitHub releases API reachable"),
        Err(crate::cloud::VersionCheckError::Unreachable { detail }) => {
            dc.warn(&format!("GitHub releases API unreachable ({detail})"));
        }
        Err(e) => {
            dc.pass("GitHub releases API reachable");
            dc.warn(&format!("No installable release for this platform: {e}"));
        }
    }
}

/// Print final summary.
fn print_summary(dc: &DoctorCounters) {
    eprintln!();
    if dc.issues == 0 && dc.warnings == 0 {
        eprintln!("\x1b[32mAll checks passed.\x1b[0m");
    } else if dc.issues == 0 {
        eprintln!("\x1b[33m{} warning(s), no issues.\x1b[0m", dc.warnings);
    } else {
        eprintln!(
            "\x1b[31m{} issue(s), {} warning(s).\x1b[0m",
            dc.issues, dc.warnings
        );
        eprintln!("Run \x1b[1mtokensave install\x1b[0m to fix most issues.");
    }
    eprintln!();
}

/// Reports a git hook whose tokensave section is out of date (#342 Q1, #624).
///
/// Read-only: the rewrite happens on install/reinstall, not here. Surfacing it
/// is what keeps an automatic edit to a file in the user's repository from
/// being entirely silent — they can see that it is pending and what will fix
/// it.
fn check_hook_freshness(dc: &mut DoctorCounters, project_path: &std::path::Path) {
    let bin = std::env::current_exe()
        .map_or_else(|_| "tokensave".to_string(), |p| p.display().to_string());

    let stale = tokensave_hooks::stale_hook_blocks(project_path, &bin);
    if stale.is_empty() {
        return;
    }
    let local_dir = tokensave_hooks::repo_hooks_dir(project_path);
    for path in stale {
        let local_hook = local_dir
            .as_deref()
            .is_some_and(|dir| path.parent() == Some(dir));
        let repair = if local_hook {
            format!(
                "run `tokensave githooks on --local --path {}` to update it",
                project_path.display()
            )
        } else {
            "run `tokensave reinstall` to update it".to_string()
        };
        let hook = path
            .file_name()
            .map_or_else(|| "hook".into(), |n| n.to_string_lossy());
        dc.warn(&format!(
            "git {hook} hook at {} carries an outdated tokensave section — \
             {repair} (content outside tokensave's markers is preserved)",
            path.display()
        ));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn doctor_reports_each_disabled_tool_area_with_its_tools() {
        let configured =
            |names: &[&str]| -> Vec<String> { names.iter().map(ToString::to_string).collect() };
        // The default: nothing is off, and doctor prints no line.
        assert_eq!(disabled_tool_areas_report(&[]), (Vec::new(), None));

        let (off, unknown) = disabled_tool_areas_report(&configured(&[" Memory ", "git"]));
        assert_eq!(unknown, None);
        assert_eq!(off.len(), 2, "{off:?}");
        let memory = off.iter().find(|line| line.contains("\"memory\"")).unwrap();
        assert!(memory.contains("disabled_tool_areas"), "{memory}");
        assert!(memory.contains("tokensave_session_recall"), "{memory}");
        assert!(!memory.contains("tokensave_blame"), "{memory}");
        let git = off.iter().find(|line| line.contains("\"git\"")).unwrap();
        assert!(git.contains("tokensave_blame"), "{git}");
        // A core tool is in no area, so no line names one.
        assert!(off.iter().all(|line| !line.contains("tokensave_search")));

        let (off, unknown) = disabled_tool_areas_report(&configured(&["nope", "all"]));
        assert_eq!(off, Vec::<String>::new());
        assert_eq!(
            unknown.as_deref(),
            Some("disabled_tool_areas names no tool area: nope, all")
        );
    }

    /// A project directory with an index of `db_bytes`, plus a small WAL, so
    /// the size check has something real to stat. It also writes the
    /// `config.json` that `init` writes, without which a database in the home
    /// directory is a stray file rather than an indexed home (#1152).
    fn project_with_index(root: &Path, db_bytes: u64) {
        stray_database(root, db_bytes);
        let dir = crate::config::get_tokensave_dir(root);
        std::fs::write(dir.join(crate::config::CONFIG_FILENAME), "{}").unwrap();
    }

    /// A database and WAL of `db_bytes` with no `config.json` next to them.
    fn stray_database(root: &Path, db_bytes: u64) {
        let dir = crate::config::get_tokensave_dir(root);
        std::fs::create_dir_all(&dir).unwrap();
        let db = std::fs::File::create(dir.join("tokensave.db")).unwrap();
        db.set_len(db_bytes).unwrap();
        std::fs::write(dir.join("tokensave.db-wal"), vec![0u8; 512]).unwrap();
    }

    /// A stray database in the global state directory gets its own warning,
    /// once, even when it is large and the user stands in the home directory,
    /// and doctor does not report the home directory as unindexed (#1152).
    #[test]
    fn a_stray_home_database_is_reported_once_and_not_as_an_indexed_home() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        stray_database(&home, OVERSIZED_INDEX_BYTES + 1);

        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &home, Some(&home));
        assert_eq!(
            dc.warnings, 1,
            "a stray database must not also raise the size warning"
        );
        let warnings = crate::index_scope::scope_warnings(&home, Some(&home));
        assert!(
            warnings[0].contains("Ignoring a stray tokensave.db"),
            "got: {warnings:?}"
        );
    }

    /// The reported state: `$HOME` itself initialized as a project. It must
    /// warn no matter how big the index is or where the user is standing,
    /// because `serve` inherits it from anywhere and its own guardrail only
    /// refuses *uninitialized* directories (#450).
    #[test]
    fn an_indexed_home_directory_warns_regardless_of_size() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let elsewhere = tmp.path().join("work");
        std::fs::create_dir_all(&elsewhere).unwrap();
        project_with_index(&home, 1024);

        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &elsewhere, Some(&home));
        assert_eq!(
            dc.warnings, 1,
            "an indexed home directory must warn even when small and even from another cwd"
        );
    }

    /// Standing in the home directory must not report it twice.
    #[test]
    fn an_indexed_home_is_reported_once_when_it_is_also_the_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        project_with_index(&home, OVERSIZED_INDEX_BYTES + 1);

        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &home, Some(&home));
        assert_eq!(
            dc.warnings, 1,
            "the home warning and the size warning must not both fire for one directory"
        );
    }

    /// An ordinary project is silent, and a runaway one is not. The reported
    /// runaway was 29.9 GB against a 420 MB real project on the same machine.
    #[test]
    fn only_an_oversized_project_index_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();

        let ordinary = tmp.path().join("ordinary");
        project_with_index(&ordinary, 420 * 1024 * 1024);
        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &ordinary, Some(&home));
        assert_eq!(dc.warnings, 0, "a 420 MB index is a normal project");

        let runaway = tmp.path().join("runaway");
        project_with_index(&runaway, OVERSIZED_INDEX_BYTES + 1);
        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &runaway, Some(&home));
        assert_eq!(dc.warnings, 1, "an index past the cap must be surfaced");
    }

    /// The WAL counts: an actively-syncing runaway shows up there first, and
    /// a running server maps it too.
    #[test]
    fn index_size_includes_the_wal() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("p");
        project_with_index(&root, 2048);
        assert_eq!(
            index_size_bytes(&root),
            2048 + 512,
            "db and wal are both part of what a server maps"
        );
    }

    /// A home directory that is not a project is the ordinary case and must
    /// stay quiet.
    #[test]
    fn an_unindexed_home_directory_is_not_a_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let work = tmp.path().join("work");
        project_with_index(&work, 1024);

        let mut dc = DoctorCounters::new();
        check_index_scope_with(&mut dc, &work, Some(&home));
        assert_eq!(dc.warnings, 0);
    }

    // -- Agent presence (#1116) ---------------------------------------------

    use crate::agents::AgentIntegration as _;

    fn install_ctx(home: &Path) -> agents::InstallContext {
        let bin = home.join("bin").join("tokensave");
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        agents::InstallContext {
            home: home.to_path_buf(),
            tokensave_bin: bin.to_string_lossy().into_owned(),
            tool_permissions: agents::expected_tool_perms(),
            scope: agents::InstallScope::Global,
            force_permission_style: false,
        }
    }

    fn hctx(home: &Path) -> HealthcheckContext {
        HealthcheckContext {
            home: home.to_path_buf(),
            project_path: home.to_path_buf(),
        }
    }

    fn run_agent_checks(home: &Path, installed: &[&str]) -> (DoctorCounters, AgentOverview) {
        let installed: Vec<String> = installed.iter().map(|s| (*s).to_string()).collect();
        let mut dc = DoctorCounters::new();
        let overview = check_agents(
            &mut dc,
            &agents::all_integrations(),
            &hctx(home),
            &installed,
        );
        (dc, overview)
    }

    /// A machine with no agent set up: nothing is broken, so the only finding
    /// is the one actionable warning, not a failure per supported agent.
    #[test]
    fn a_clean_machine_reports_one_warning_and_no_issues() {
        let tmp = tempfile::tempdir().unwrap();
        let (dc, overview) = run_agent_checks(tmp.path(), &[]);
        assert_eq!(dc.issues, 0);
        assert_eq!(dc.warnings, 1, "only 'not installed in any agent'");
        assert!(overview.installed.is_empty() && overview.leftover.is_empty());
        assert_eq!(
            overview.absent.len() + overview.detected_absent.len(),
            agents::all_integrations().len()
        );
    }

    /// One installed agent is checked; every other agent adds nothing to the
    /// totals (the reported case: Codex healthy, eight issues from others).
    #[test]
    fn only_the_installed_agent_contributes_findings() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let ctx = install_ctx(home);
        agents::CodexIntegration.install(&ctx).unwrap();

        let mut alone = DoctorCounters::new();
        agents::CodexIntegration.healthcheck(&mut alone, &hctx(home));

        let (dc, overview) = run_agent_checks(home, &["codex"]);
        assert_eq!(overview.installed, vec!["Codex CLI"]);
        assert!(overview.leftover.is_empty());
        assert_eq!(dc.issues, alone.issues);
        assert_eq!(dc.warnings, alone.warnings);
        assert_eq!(dc.issues, 0, "a clean Codex install has no issues");
    }

    /// Rules left behind without an MCP registration is a partial install:
    /// checked in full and flagged, even though `installed_agents` is empty.
    #[test]
    fn a_partial_installation_is_checked_and_flagged() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        agents::GeminiIntegration
            .install(&install_ctx(home))
            .unwrap();
        std::fs::remove_file(home.join(".gemini").join("settings.json")).unwrap();

        let (dc, overview) = run_agent_checks(home, &[]);
        assert_eq!(overview.leftover, vec!["Gemini CLI"]);
        assert!(dc.issues + dc.warnings > 0);
    }

    /// An agent removed with `uninstall` is not applicable any more, even
    /// though the agent itself is still on the machine.
    #[test]
    fn an_intentionally_removed_agent_is_not_counted() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let ctx = install_ctx(home);
        agents::GeminiIntegration.install(&ctx).unwrap();
        agents::GeminiIntegration.uninstall(&ctx).unwrap();
        assert!(home.join(".gemini").is_dir(), "the agent itself stays");

        let (dc, overview) = run_agent_checks(home, &[]);
        assert!(overview.leftover.is_empty(), "{:?}", overview.leftover);
        assert!(overview
            .detected_absent
            .iter()
            .any(|(name, _)| *name == "Gemini CLI"));
        assert_eq!(dc.issues, 0);
        assert_eq!(dc.warnings, 1, "only 'not installed in any agent'");
    }

    /// A tracked agent whose files are gone is still checked, so the user
    /// sees what is broken rather than a silent skip.
    #[test]
    fn a_registered_agent_with_missing_files_is_still_checked() {
        let tmp = tempfile::tempdir().unwrap();
        let (dc, overview) = run_agent_checks(tmp.path(), &["gemini"]);
        assert_eq!(overview.installed, vec!["Gemini CLI"]);
        assert!(dc.issues + dc.warnings > 0);
    }

    #[test]
    fn format_bytes_boundaries() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024 - 1), "1024.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 512), "512.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.0 GB");
    }

    #[test]
    fn format_bytes_fractional_kb() {
        // 2048 bytes = 2.0 KB
        assert_eq!(format_bytes(2048), "2.0 KB");
        // 1536 = 1.5 KB
        assert_eq!(format_bytes(1536), "1.5 KB");
    }
}
