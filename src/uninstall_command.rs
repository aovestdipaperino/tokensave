//! CLI orchestration for integration-only cleanup.

use tokensave::agents::uninstall::{UninstallOutput, UninstallPlan};
use tokensave::agents::{self, InstallContext, InstallScope};
use tokensave::errors::{Result, TokenSaveError};
use tokensave::purge::provenance;

pub(crate) fn run(
    agent: Option<&str>,
    scope: InstallScope,
    keep_git_hooks: bool,
    output: UninstallOutput,
) -> Result<()> {
    let home = agents::home_dir().ok_or_else(|| TokenSaveError::Config {
        message: "could not determine home directory".into(),
    })?;
    let local = matches!(scope, InstallScope::Local { .. });
    let ctx = InstallContext {
        home,
        tokensave_bin: String::new(),
        tool_permissions: Vec::new(),
        scope,
        force_permission_style: false,
    };
    let mut plan = UninstallPlan::for_scope(&ctx.scope);
    let ids: Vec<_> = match agent {
        Some(id) => {
            if local && !agents::get_integration(id)?.supports_local() {
                return Err(TokenSaveError::Config {
                    message: format!("{id} has no project-scoped configuration. Omit --local."),
                });
            }
            vec![id.to_string()]
        }
        None if local => agents::all_integrations()
            .into_iter()
            .filter(|integration| integration.supports_local())
            .map(|integration| integration.id().to_string())
            .collect(),
        None => agents::uninstall::registered_agents(&ctx.home),
    };
    if !output.quiet {
        match &ctx.scope {
            InstallScope::Global => eprintln!("TokenSave uninstall — global integrations"),
            InstallScope::Local { project_path } => {
                eprintln!("TokenSave uninstall — project {}", project_path.display());
            }
        }
        if !ids.is_empty() {
            eprintln!("Agents: {}", ids.join(", "));
        }
        eprintln!("Checking integration ownership...");
    }
    plan.plan_agents(&ids, &ctx);
    if !local && agent.is_none() && !keep_git_hooks {
        agents::plan_global_hook_removal(&ctx.home, &mut plan);
    } else {
        plan.skipped("Global Git hooks", "kept outside the requested cleanup");
    }
    let mut registry = UninstallPlan::default();
    if !local {
        agents::uninstall::plan_registry_cleanup(&ctx.home, &ids, &mut registry);
    }
    let installation = provenance::detect();
    plan.print(output);
    registry.print(output);
    let mut summary = plan.apply(output);
    if summary.failed == 0 {
        summary.add(registry.apply(output));
    } else if !registry.is_empty() {
        let mut retained = registry.report_without_writes(output);
        retained.skipped += retained.changed;
        retained.changed = 0;
        summary.add(retained);
        if !output.quiet {
            eprintln!("  Installation registry retained for retry.");
        }
    }
    let verb = if output.dry_run { "planned" } else { "removed" };
    if !output.quiet || summary.failed > 0 {
        eprintln!(
            "Summary: {} {verb}, {} already absent, {} skipped, {} failed.",
            summary.changed, summary.absent, summary.skipped, summary.failed
        );
    }
    if !output.quiet {
        eprintln!("Preserved: executable, user settings, caches, indexes and project data.");
    }
    eprintln!("{}", provenance::removal_reminder(&installation));
    if summary.failed > 0 {
        return Err(TokenSaveError::Config {
            message: "Integration cleanup is incomplete. Resolve the errors above and retry."
                .into(),
        });
    }
    Ok(())
}
