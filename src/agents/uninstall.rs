//! Read-only integration inventory and checked application of its changes.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use jsonc_parser::cst::{CstNode, CstRootNode};
use jsonc_parser::ParseOptions;
use serde_json::Value;
use sha2::{Digest, Sha256};
use toml_edit::{DocumentMut, Item};

const MAX_CONFIG_BYTES: u64 = 16 * 1024 * 1024;
const MCP_SERVER_NAME: &str = "tokensave";
const INSTALLED_AGENTS: &str = "installed_agents";

#[derive(Default)]
pub struct UninstallPlan {
    entries: Vec<Entry>,
    project_root: Option<PathBuf>,
    agent_name: Option<&'static str>,
    agent_id: Option<&'static str>,
    registry_lock: Option<PathBuf>,
}

struct Entry {
    label: String,
    path: Option<PathBuf>,
    change: Change,
    require_empty_dir: Option<PathBuf>,
}

enum Change {
    Absent,
    Skipped(&'static str),
    Failed(String),
    Edit {
        fingerprint: [u8; 32],
        canonical: PathBuf,
        removal: Removal,
    },
}

pub(crate) enum Removal {
    Write(String),
    Delete,
}

impl Removal {
    fn contents(&self) -> Option<&str> {
        match self {
            Self::Write(contents) => Some(contents),
            Self::Delete => None,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct UninstallOutput {
    pub dry_run: bool,
    pub verbose: bool,
    pub quiet: bool,
}

#[derive(Clone, Copy, Default)]
pub struct UninstallSummary {
    pub changed: usize,
    pub absent: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl UninstallPlan {
    pub fn for_scope(scope: &super::InstallScope) -> Self {
        let project_root = match scope {
            super::InstallScope::Local { project_path } => Some(project_path.clone()),
            super::InstallScope::Global => None,
        };
        Self {
            project_root,
            ..Self::default()
        }
    }

    pub fn plan_agents(&mut self, ids: &[String], ctx: &super::InstallContext) {
        for id in ids {
            match super::get_integration(id) {
                Ok(agent) if !ctx.is_local() || agent.supports_local() => {
                    self.plan_agent(agent.as_ref(), ctx);
                }
                Ok(_) => self.skipped(id, "no project-scoped configuration"),
                Err(_) => self.skipped(id, "unknown integration"),
            }
        }
    }

    pub fn plan_agent(&mut self, agent: &dyn super::AgentIntegration, ctx: &super::InstallContext) {
        self.agent_name = Some(agent.name());
        self.agent_id = Some(agent.id());
        if let Err(error) = agent.plan_uninstall(ctx, self) {
            self.failed("Integration discovery", error.to_string());
        }
        self.agent_name = None;
        self.agent_id = None;
    }

    fn push(&mut self, label: impl Into<String>, path: Option<&Path>, change: Change) {
        let label = label.into();
        let label = match self.agent_name {
            Some(agent) => format!("{agent} / {label}"),
            None => label,
        };
        self.entries.push(Entry {
            label,
            path: path.map(Path::to_path_buf),
            change,
            require_empty_dir: None,
        });
    }

    pub fn skipped(&mut self, label: impl Into<String>, reason: &'static str) {
        self.push(label, None, Change::Skipped(reason));
    }

    pub fn failed(&mut self, label: impl Into<String>, reason: String) {
        self.push(label, None, Change::Failed(reason));
    }

    pub(crate) fn inspect(
        &mut self,
        label: &str,
        path: &Path,
        transform: impl FnOnce(&str) -> Result<Option<Removal>, String>,
    ) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry.path.as_deref() == Some(path) && matches!(entry.change, Change::Edit { .. })
        }) {
            if let Change::Edit { removal, .. } = &mut entry.change {
                let Some(contents) = removal.contents() else {
                    return;
                };
                match transform(contents) {
                    Ok(Some(new_contents)) => {
                        *removal = new_contents;
                        if let Some(agent) = self.agent_name {
                            let _ = write!(entry.label, " + {agent} / {label}");
                        }
                    }
                    Ok(None) => {}
                    Err(error) => self.push(label, Some(path), Change::Failed(error)),
                }
            }
            return;
        }
        let change = match read_config(path) {
            Ok(Some(original)) => match fs::canonicalize(path) {
                Ok(canonical)
                    if self.project_root.as_ref().is_some_and(|root| {
                        fs::canonicalize(root).is_ok_and(|root| !canonical.starts_with(root))
                    }) =>
                {
                    Change::Skipped("file resolves outside the project")
                }
                Ok(canonical) => match transform(&original) {
                    Ok(Some(removal)) => Change::Edit {
                        fingerprint: Sha256::digest(original.as_bytes()).into(),
                        canonical,
                        removal,
                    },
                    Ok(None) => Change::Skipped("no TokenSave-owned content"),
                    Err(error) => Change::Failed(error),
                },
                Err(error) => Change::Failed(error.to_string()),
            },
            Ok(None) => Change::Absent,
            Err(error) => Change::Failed(error.to_string()),
        };
        self.push(label, Some(path), change);
    }

    pub(crate) fn will_delete(&self, path: &Path) -> bool {
        self.entries.iter().any(|entry| {
            entry.path.as_deref() == Some(path)
                && matches!(
                    entry.change,
                    Change::Edit {
                        removal: Removal::Delete,
                        ..
                    }
                )
        })
    }

    pub(crate) fn edit_if_directory_empty(
        &mut self,
        label: &str,
        path: &Path,
        dir: &Path,
        edit: impl FnOnce(&str) -> Option<String>,
    ) {
        self.inspect(label, path, |contents| {
            Ok(edit(contents).map(Removal::Write))
        });
        if let Some(entry) = self
            .entries
            .last_mut()
            .filter(|entry| entry.path.as_deref() == Some(path))
        {
            entry.require_empty_dir = Some(dir.to_path_buf());
        }
    }

    pub(crate) fn json(
        &mut self,
        label: &str,
        path: &Path,
        jsonc: bool,
        transform: impl FnOnce(&mut Value) -> bool,
    ) {
        self.inspect(label, path, |original| {
            let before = super::fs::parse_config_json(original, jsonc)?;
            if !before.is_object() {
                return Err("expected a JSON object".into());
            }
            let mut after = before.clone();
            if !transform(&mut after) {
                return Ok(None);
            }
            let root = CstRootNode::parse(original, &ParseOptions::default())
                .map_err(|_| "could not inventory JSON syntax".to_string())?;
            let node = root.value().ok_or("missing JSON value")?;
            remove_json_nodes(&node, &before, &after)?;
            let replacement = root.to_string();
            if super::fs::parse_config_json(&replacement, jsonc)? != after {
                return Err("could not preserve the rest of the JSON configuration".into());
            }
            Ok(Some(Removal::Write(replacement)))
        });
    }

    pub(crate) fn mcp(&mut self, path: &Path, keys: &[&str], jsonc: bool) {
        self.json("MCP server", path, jsonc, |config| remove_mcp(config, keys));
    }

    pub(crate) fn rules(&mut self, path: &Path, marker: &str, headings: &[&str]) {
        if !super::rules_management_enabled() {
            self.push(
                "Agent rules",
                Some(path),
                Change::Skipped("rules are user-managed"),
            );
            return;
        }
        let agent_id = self.agent_id;
        self.inspect("Agent rules", path, |original| {
            Ok(
                super::rules::strip_uninstall_rules(original, agent_id, marker, headings)
                    .map(Removal::Write),
            )
        });
    }

    pub(crate) fn managed_rules(&mut self, path: &Path) {
        if !super::rules_management_enabled() {
            self.push(
                "Managed rules",
                Some(path),
                Change::Skipped("rules are user-managed"),
            );
            return;
        }
        self.inspect("Managed rules", path, |original| {
            Ok(super::rules::strip_managed_rules(original))
        });
    }

    pub(crate) fn toml(
        &mut self,
        label: &str,
        path: &Path,
        transform: impl FnOnce(&mut DocumentMut) -> bool,
    ) {
        self.inspect(label, path, |original| {
            let mut config = original
                .parse::<DocumentMut>()
                .map_err(|_| "invalid TOML; correct its syntax and retry".to_string())?;
            Ok(transform(&mut config).then(|| Removal::Write(config.to_string())))
        });
    }

    pub(crate) fn toml_mcp(&mut self, path: &Path) {
        self.toml("MCP server", path, |config| {
            let Some(servers) = config
                .get_mut("mcp_servers")
                .and_then(Item::as_table_like_mut)
            else {
                return false;
            };
            let owned = servers.get(MCP_SERVER_NAME).is_some_and(|entry| {
                is_tokensave_command(entry.get("command").and_then(Item::as_str))
            });
            owned && servers.remove(MCP_SERVER_NAME).is_some()
        });
    }

    fn lock_registry(&self, output: UninstallOutput) -> io::Result<Option<fs::File>> {
        let Some(path) = self.registry_lock.as_deref().filter(|_| {
            !output.dry_run
                && self
                    .entries
                    .iter()
                    .any(|entry| matches!(entry.change, Change::Edit { .. }))
        }) else {
            return Ok(None);
        };
        if fs::symlink_metadata(path).is_ok_and(|meta| !meta.is_file()) {
            return Err(io::Error::other("expected a regular lock file"));
        }
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        fs2::FileExt::try_lock_exclusive(&lock)?;
        Ok(Some(lock))
    }

    pub fn report_without_writes(&self, output: UninstallOutput) -> UninstallSummary {
        self.apply(UninstallOutput {
            dry_run: true,
            ..output
        })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True when the inventory found TokenSave-owned content to remove, or a
    /// file it could not inspect. `doctor` uses this to tell a partial or
    /// incomplete removal from an agent that was never set up (#1116).
    pub fn has_owned_content(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| matches!(entry.change, Change::Edit { .. } | Change::Failed(_)))
    }

    pub fn print(&self, output: UninstallOutput) {
        if !output.quiet {
            for entry in &self.entries {
                if let Change::Edit { removal, .. } = &entry.change {
                    let action = if matches!(removal, Removal::Write(_)) {
                        "edit"
                    } else {
                        "delete"
                    };
                    eprintln!(
                        "  Plan: {action} {}",
                        entry.description(output.dry_run || output.verbose)
                    );
                }
            }
        }
    }

    pub fn apply(&self, output: UninstallOutput) -> UninstallSummary {
        let mut summary = UninstallSummary::default();
        let _registry_lock = match self.lock_registry(output) {
            Ok(lock) => lock,
            Err(error) => {
                eprintln!("  Failed to lock the installed-agent registry: {error}");
                summary.failed = 1;
                return summary;
            }
        };
        for entry in &self.entries {
            let result = match &entry.change {
                Change::Absent => {
                    summary.absent += 1;
                    "already absent"
                }
                Change::Skipped(reason) => {
                    summary.skipped += 1;
                    reason
                }
                Change::Failed(error) => {
                    summary.failed += 1;
                    eprintln!("  Failed {}: {error}", entry.description(true));
                    continue;
                }
                Change::Edit { .. } if output.dry_run => {
                    summary.changed += 1;
                    continue;
                }
                Change::Edit {
                    fingerprint,
                    canonical,
                    removal,
                } => {
                    if !output.quiet {
                        eprintln!("  Processing {}", entry.description(output.verbose));
                    }
                    if let Err(error) = entry.apply(fingerprint, canonical, removal.contents()) {
                        summary.failed += 1;
                        eprintln!(
                            "  Failed {}: {error}. Check the path and retry.",
                            entry.description(true)
                        );
                        continue;
                    }
                    summary.changed += 1;
                    "removed TokenSave content"
                }
            };
            if !output.quiet {
                eprintln!(
                    "  {}: {result}",
                    entry.description(output.dry_run || output.verbose)
                );
            }
        }
        summary
    }
}

impl Entry {
    fn description(&self, show_path: bool) -> String {
        match &self.path {
            Some(path) if show_path => format!("{} ({})", self.label, path.display()),
            _ => self.label.clone(),
        }
    }

    fn apply(
        &self,
        fingerprint: &[u8; 32],
        canonical: &Path,
        replacement: Option<&str>,
    ) -> io::Result<()> {
        if let Some(dir) = &self.require_empty_dir {
            if fs::read_dir(dir)?.next().is_some() {
                return Err(io::Error::other("Git hooks directory is no longer empty"));
            }
        }
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| io::Error::other("missing plan path"))?;
        if fs::canonicalize(path)? != canonical {
            return Err(io::Error::other("file moved since planning"));
        }
        let parent = canonical
            .parent()
            .ok_or_else(|| io::Error::other("path has no parent"))?;
        let staged = if let Some(text) = replacement {
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(text.as_bytes())?;
            file.as_file()
                .set_permissions(fs::symlink_metadata(canonical)?.permissions())?;
            file.as_file().sync_all()?;
            Some(file)
        } else {
            None
        };
        if fs::canonicalize(path)? != canonical
            || read_config(canonical)?
                .is_none_or(|current| Sha256::digest(current.as_bytes()).as_slice() != fingerprint)
        {
            return Err(io::Error::other("file changed since planning"));
        }
        if let Some(file) = staged {
            file.persist(canonical).map_err(|error| error.error)?;
        } else {
            fs::remove_file(canonical)?;
        }
        Ok(())
    }
}

fn read_config(path: &Path) -> io::Result<Option<String>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Err(io::Error::other(
            "expected a regular file; inspect links and special files manually",
        ));
    }
    let mut data = String::new();
    fs::File::open(path)?
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_string(&mut data)?;
    if data.len() as u64 > MAX_CONFIG_BYTES {
        return Err(io::Error::other("file is too large to inspect"));
    }
    Ok(Some(data))
}

pub(crate) fn is_tokensave_command(command: Option<&str>) -> bool {
    command.is_some_and(|command| {
        let command = command.trim_matches(['\'', '"']);
        matches!(
            command.rsplit(['/', '\\']).next(),
            Some("tokensave" | "tokensave.exe")
        )
    })
}

pub(crate) fn is_tokensave_hook(entry: &Value, subcommand: &str) -> bool {
    let Some(command) = entry.get("command").and_then(Value::as_str) else {
        return false;
    };
    if let Some(args) = entry.get("args").and_then(Value::as_array) {
        return args.len() == 1
            && args[0].as_str() == Some(subcommand)
            && is_tokensave_command(Some(command));
    }
    command
        .trim_end()
        .rsplit_once(char::is_whitespace)
        .is_some_and(|(bin, sub)| {
            let bin = bin.trim_end();
            let quoted = (bin.starts_with('"') && bin.ends_with('"'))
                || (bin.starts_with('\'') && bin.ends_with('\''));
            sub == subcommand
                && (quoted || !bin.contains(char::is_whitespace))
                && !bin.contains([';', '|', '&', '`'])
                && is_tokensave_command(Some(bin))
        })
}

pub(crate) fn remove_grouped_hooks(config: &mut Value, event: &str, subcommand: &str) -> bool {
    let Some(groups) = config
        .get_mut("hooks")
        .and_then(|hooks| hooks.get_mut(event))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let mut changed = false;
    groups.retain_mut(|group| {
        let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let before = hooks.len();
        hooks.retain(|hook| !is_tokensave_hook(hook, subcommand));
        changed |= before != hooks.len();
        before == hooks.len()
            || !hooks.is_empty()
            || group
                .as_object()
                .is_some_and(|group| group.keys().any(|key| key != "hooks" && key != "matcher"))
    });
    changed
}

pub(crate) fn remove_mcp(config: &mut Value, keys: &[&str]) -> bool {
    let mut nested = config;
    for key in keys {
        let Some(value) = nested.get_mut(key) else {
            return false;
        };
        nested = value;
    }
    let Some(servers) = nested.as_object_mut() else {
        return false;
    };
    let owned = servers.get(MCP_SERVER_NAME).is_some_and(|entry| {
        let command = &entry["command"];
        is_tokensave_command(
            command
                .as_str()
                .or_else(|| command.get(0).and_then(Value::as_str))
                .or_else(|| command.get("path").and_then(Value::as_str)),
        )
    });
    owned && servers.remove(MCP_SERVER_NAME).is_some()
}

impl UninstallSummary {
    pub fn add(&mut self, other: Self) {
        self.changed += other.changed;
        self.absent += other.absent;
        self.skipped += other.skipped;
        self.failed += other.failed;
    }
}

fn registry_paths(home: &Path) -> [PathBuf; 2] {
    let dir = home.join(crate::config::TOKENSAVE_DIR);
    [dir.join("state.toml"), dir.join("config.toml")]
}

pub fn registered_agents(home: &Path) -> Vec<String> {
    let mut agents = Vec::new();
    let available = super::available_integrations();
    for path in registry_paths(home) {
        let Ok(Some(text)) = read_config(&path) else {
            continue;
        };
        let Ok(table) = text.parse::<toml::Table>() else {
            continue;
        };
        if let Some(ids) = table.get(INSTALLED_AGENTS).and_then(toml::Value::as_array) {
            for id in ids.iter().filter_map(toml::Value::as_str) {
                if available.contains(&id) && !agents.iter().any(|known| known == id) {
                    agents.push(id.to_string());
                }
            }
        }
    }
    for agent in super::all_integrations() {
        let id = agent.id();
        if agents.iter().any(|known| known == id) {
            continue;
        }
        let installed = if id == "omp" {
            super::omp::has_recorded_integration(home)
        } else {
            agent.has_tokensave(home)
        };
        if installed {
            agents.push(id.to_string());
        }
    }
    agents
}

pub fn plan_registry_cleanup(home: &Path, agents: &[String], plan: &mut UninstallPlan) {
    plan.registry_lock = Some(
        home.join(crate::config::TOKENSAVE_DIR)
            .join(crate::user_config::CONFIG_LOCK_FILENAME),
    );
    for path in registry_paths(home) {
        plan.toml("Installed-agent registry", &path, |config| {
            let Some(ids) = config
                .get_mut(INSTALLED_AGENTS)
                .and_then(Item::as_array_mut)
            else {
                return false;
            };
            let before = ids.len();
            ids.retain(|id| {
                id.as_str()
                    .is_none_or(|id| !agents.iter().any(|agent| agent == id))
            });
            ids.len() != before
        });
    }
}

fn remove_json_nodes(node: &CstNode, before: &Value, after: &Value) -> Result<(), String> {
    if before == after {
        return Ok(());
    }
    if let (Some(object), Value::Object(before), Value::Object(after)) =
        (node.as_object(), before, after)
    {
        let mut seen = std::collections::HashSet::new();
        for property in object.properties() {
            let key = property
                .decoded_name()
                .ok_or("invalid JSON property name")?;
            if !seen.insert(key.clone()) {
                return Err("duplicate JSON keys; resolve them before uninstalling".into());
            }
            if let Some(kept) = after.get(&key) {
                let original = before.get(&key).ok_or("missing original JSON property")?;
                remove_json_nodes(
                    &property.value().ok_or("missing JSON property value")?,
                    original,
                    kept,
                )?;
            } else {
                property.remove();
            }
        }
        return Ok(());
    }
    if let (Some(array), Value::Array(before), Value::Array(after)) =
        (node.as_array(), before, after)
    {
        let elements = array.elements();
        let mut cursor = 0;
        for kept in after {
            let Some(offset) = before[cursor..]
                .iter()
                .position(|original| original == kept)
                .or_else(|| {
                    before[cursor..]
                        .iter()
                        .position(|original| is_json_subset(original, kept))
                })
            else {
                return Err("could not preserve a shared JSON array".into());
            };
            for node in &elements[cursor..cursor + offset] {
                node.clone().remove();
            }
            cursor += offset;
            remove_json_nodes(&elements[cursor], &before[cursor], kept)?;
            cursor += 1;
        }
        for node in &elements[cursor..] {
            node.clone().remove();
        }
        return Ok(());
    }
    Err("expected a removal from shared JSON".into())
}

fn is_json_subset(original: &Value, kept: &Value) -> bool {
    match (original, kept) {
        (Value::Object(original), Value::Object(kept)) => kept.iter().all(|(key, value)| {
            original
                .get(key)
                .is_some_and(|original| is_json_subset(original, value))
        }),
        (Value::Array(original), Value::Array(kept)) => {
            let mut original = original.iter();
            kept.iter()
                .all(|kept| original.any(|value| is_json_subset(value, kept)))
        }
        _ => original == kept,
    }
}
