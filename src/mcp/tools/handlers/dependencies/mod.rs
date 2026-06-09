//! `tokensave_dependencies` — multi-ecosystem package-manifest introspection
//! (issue #105).
//!
//! Each ecosystem module reports a `Workspace` describing its members and
//! their declared dependencies. The handler auto-detects which ecosystems
//! are present at the project root and either:
//!
//! - returns the single workspace when only one ecosystem is detected, or
//! - returns a polyglot view when multiple coexist (e.g. a Rust + Python
//!   monorepo), letting callers slice with the `ecosystem` argument.

mod common;
mod crystal;
mod dart;
mod dotnet;
mod elixir;
mod erlang;
mod go;
mod haskell;
mod java;
mod lockfiles;
mod node;
mod ocaml;
mod php;
mod python;
mod r_lang;
mod ruby;
mod rust;
mod swift;
mod xml_util;
mod yaml_util;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{json, Value};

use crate::errors::{Result, TokenSaveError};
use crate::tokensave::TokenSave;

use self::common::{dep_to_json, patch_to_json, DepKind, Workspace};

use super::super::ToolResult;
use super::truncate_response;

type DetectFn = fn(&Path) -> bool;
type ParseFn = fn(&Path) -> Result<Workspace>;
type EcosystemEntry = (&'static str, DetectFn, ParseFn);

/// All ecosystems we know how to parse. Order is also the dispatch order
/// when the user doesn't specify `ecosystem` — `rust` first because that's
/// our home turf.
const ECOSYSTEMS: &[EcosystemEntry] = &[
    ("rust", rust::detect, rust::parse),
    ("node", node::detect, node::parse),
    ("python", python::detect, python::parse),
    ("go", go::detect, go::parse),
    ("java", java::detect, java::parse),
    ("dotnet", dotnet::detect, dotnet::parse),
    ("php", php::detect, php::parse),
    ("ruby", ruby::detect, ruby::parse),
    ("swift", swift::detect, swift::parse),
    ("elixir", elixir::detect, elixir::parse),
    ("erlang", erlang::detect, erlang::parse),
    ("r", r_lang::detect, r_lang::parse),
    ("haskell", haskell::detect, haskell::parse),
    ("ocaml", ocaml::detect, ocaml::parse),
    ("dart", dart::detect, dart::parse),
    ("crystal", crystal::detect, crystal::parse),
];

/// Handles `tokensave_dependencies` tool calls.
///
/// All filesystem reads are blocking; the handler is `async` only so the
/// outer routing dispatch can `.await` it uniformly with the other tools.
#[allow(clippy::unused_async)]
pub(super) async fn handle_dependencies(cg: &TokenSave, args: Value) -> Result<ToolResult> {
    let crate_name = args
        .get("crate")
        .or_else(|| args.get("package"))
        .and_then(|v| v.as_str());
    let member_name = args.get("member").and_then(|v| v.as_str());
    let kind_filter = args
        .get("kind")
        .and_then(|v| v.as_str())
        .filter(|s| {
            matches!(
                *s,
                "normal" | "dev" | "build" | "peer" | "optional" | "all"
            )
        })
        .unwrap_or("all");
    let ecosystem_filter = args.get("ecosystem").and_then(|v| v.as_str());
    let include_lockfile = args
        .get("include_lockfile")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let mut workspaces = detect_workspaces(cg.project_root(), ecosystem_filter)?;
    if include_lockfile {
        for ws in &mut workspaces {
            lockfiles::apply_to_workspace(ws);
        }
    }
    if workspaces.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "no supported package manifest found at {} (looked for Cargo.toml, \
                 package.json, pyproject.toml, requirements*.txt, go.mod, pom.xml, \
                 *.csproj / *.fsproj / *.vbproj, composer.json, Gemfile)",
                cg.project_root().display()
            ),
        });
    }

    if let Some(name) = member_name {
        return Ok(render_member(&workspaces, name, kind_filter));
    }
    if let Some(name) = crate_name {
        return Ok(render_crate(&workspaces, name, kind_filter));
    }
    Ok(render_summary(&workspaces, kind_filter))
}

fn detect_workspaces(root: &Path, ecosystem_filter: Option<&str>) -> Result<Vec<Workspace>> {
    let mut out = Vec::new();
    for (name, detect, parse) in ECOSYSTEMS {
        if let Some(filter) = ecosystem_filter {
            if filter != *name {
                continue;
            }
        }
        if !detect(root) {
            continue;
        }
        // Surface per-ecosystem parse errors at the top so the user knows
        // *which* manifest failed, but keep going so partial polyglot
        // results are still useful.
        match parse(root) {
            Ok(ws) => out.push(ws),
            Err(_) if ecosystem_filter.is_none() => {}
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

fn render_summary(workspaces: &[Workspace], kind_filter: &str) -> ToolResult {
    let ecosystems: Vec<Value> = workspaces
        .iter()
        .map(|ws| ecosystem_summary(ws, kind_filter))
        .collect();

    let total_members: u64 = workspaces.iter().map(|w| w.members.len() as u64).sum();
    let detected_names: Vec<&str> = workspaces.iter().map(|w| w.ecosystem).collect();
    let single = if workspaces.len() == 1 {
        ecosystems[0].clone()
    } else {
        Value::Null
    };

    let flat = workspaces.len() == 1;
    let flat_get = |field: &str| -> Value {
        if !flat {
            return Value::Null;
        }
        single.get(field).cloned().unwrap_or(Value::Null)
    };
    let output = json!({
        "mode": "workspace",
        "kind_filter": kind_filter,
        "detected_ecosystems": detected_names,
        "total_members": total_members,
        // Convenience flat fields for the common single-ecosystem case.
        "ecosystem": flat.then(|| workspaces[0].ecosystem),
        "members": flat.then(|| {
            workspaces[0]
                .members
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>()
        }),
        "members_detail": flat_get("members_detail"),
        "licenses": flat_get("licenses"),
        "crates": flat_get("crates"),
        "version_drift": flat_get("version_drift"),
        "patches": flat_get("patches"),
        // Always include the polyglot breakdown.
        "ecosystems": ecosystems,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
    }
}

/// Map keyed by version-string → list of (member name, resolved version).
type VersionToMembers = BTreeMap<String, Vec<(String, Option<String>)>>;

fn ecosystem_summary(ws: &Workspace, kind_filter: &str) -> Value {
    let mut by_crate: BTreeMap<String, VersionToMembers> = BTreeMap::new();
    for m in &ws.members {
        for d in &m.deps {
            if !d.kind.passes(kind_filter) {
                continue;
            }
            // Outer map: crate → version → list of (member, resolved).
            let v_key = d.version.clone().unwrap_or_else(|| "*".to_string());
            by_crate
                .entry(d.name.clone())
                .or_default()
                .entry(v_key)
                .or_default()
                .push((m.name.clone(), d.resolved.clone()));
        }
    }

    let mut crate_rows: Vec<Value> = Vec::new();
    let mut drift_rows: Vec<Value> = Vec::new();
    for (name, versions) in &by_crate {
        let mut used_in: BTreeSet<String> = BTreeSet::new();
        for members in versions.values() {
            for (m_name, _) in members {
                used_in.insert(m_name.clone());
            }
        }
        crate_rows.push(json!({
            "crate": name,
            "used_in": used_in.into_iter().collect::<Vec<_>>(),
        }));
        // Drift = same crate declared at >1 distinct version range across members.
        if versions.len() > 1 {
            let by_version: Vec<Value> = versions
                .iter()
                .map(|(v, mems)| {
                    json!({
                        "version": v,
                        "members": mems.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            drift_rows.push(json!({
                "crate": name,
                "version_count": versions.len() as u64,
                "by_version": by_version,
            }));
        }
    }

    let member_rows: Vec<Value> = ws
        .members
        .iter()
        .map(|m| {
            json!({
                "name": m.name,
                "path": m.path,
                "license": m.license,
            })
        })
        .collect();
    let licenses_summary: Vec<Value> = collect_license_summary(&ws.members);
    let patch_rows: Vec<Value> = ws.patches.iter().map(patch_to_json).collect();

    let member_names: Vec<&str> = ws.members.iter().map(|m| m.name.as_str()).collect();
    json!({
        "ecosystem": ws.ecosystem,
        "root": ws.root.display().to_string(),
        "member_count": ws.members.len() as u64,
        "members": member_names,
        "members_detail": member_rows,
        "licenses": licenses_summary,
        "crates": crate_rows,
        "version_drift": drift_rows,
        "patches": patch_rows,
    })
}

/// Aggregate distinct license strings across members with a count of how
/// many members declare each. Members without a `license` field are bucketed
/// under `"<unknown>"`.
fn collect_license_summary(members: &[crate::mcp::tools::handlers::dependencies::common::Member]) -> Vec<Value> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for m in members {
        let key = m.license.clone().unwrap_or_else(|| "<unknown>".to_string());
        *counts.entry(key).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(license, count)| json!({ "license": license, "count": count }))
        .collect()
}

fn render_member(workspaces: &[Workspace], name: &str, kind_filter: &str) -> ToolResult {
    // Look up across all ecosystems.
    let mut found: Vec<(String, Value)> = Vec::new();
    for ws in workspaces {
        if let Some(m) = ws.members.iter().find(|m| m.name == name || m.path == name) {
            let deps: Vec<Value> = m
                .deps
                .iter()
                .filter(|d| d.kind.passes(kind_filter))
                .map(dep_to_json)
                .collect();
            found.push((
                ws.ecosystem.to_string(),
                json!({
                    "ecosystem": ws.ecosystem,
                    "member": m.name,
                    "path": m.path,
                    "kind_filter": kind_filter,
                    "dependency_count": deps.len(),
                    "dependencies": deps,
                }),
            ));
        }
    }

    if found.is_empty() {
        let known: Vec<String> = workspaces
            .iter()
            .flat_map(|ws| {
                ws.members
                    .iter()
                    .map(move |m| format!("{}:{}", ws.ecosystem, m.name))
            })
            .collect();
        let formatted = serde_json::to_string_pretty(&json!({
            "mode": "member",
            "error": format!("no member named '{name}'"),
            "available_members": known,
        }))
        .unwrap_or_default();
        return ToolResult {
            value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
            touched_files: vec![],
        };
    }

    // Flatten when only one match; keep ecosystems[] for polyglot collisions.
    let body = if found.len() == 1 {
        let Some((_, mut v)) = found.into_iter().next() else {
            unreachable!("non-empty checked above")
        };
        if let Value::Object(ref mut map) = v {
            map.insert("mode".to_string(), Value::String("member".to_string()));
        }
        v
    } else {
        json!({
            "mode": "member",
            "name": name,
            "matches": found.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        })
    };
    let formatted = serde_json::to_string_pretty(&body).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
    }
}

fn render_crate(workspaces: &[Workspace], name: &str, kind_filter: &str) -> ToolResult {
    let mut rows: Vec<Value> = Vec::new();
    for ws in workspaces {
        for m in &ws.members {
            for d in &m.deps {
                if d.name != name {
                    continue;
                }
                if !d.kind.passes(kind_filter) {
                    continue;
                }
                rows.push(json!({
                    "ecosystem": ws.ecosystem,
                    "member": m.name,
                    "path": m.path,
                    "kind": d.kind.as_str(),
                    "version": d.version,
                    "resolved": d.resolved,
                    "features": d.features,
                    "optional": d.optional,
                    "local_path": d.local_path,
                }));
            }
        }
    }
    let output = json!({
        "mode": "crate",
        "crate": name,
        "kind_filter": kind_filter,
        "usage_count": rows.len(),
        "usages": rows,
    });
    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
    }
}

// Silence unused-warning when no ecosystem uses a particular DepKind variant
// in tests-only builds.
#[allow(dead_code)]
fn _all_kinds_referenced() {
    let _ = DepKind::Normal;
    let _ = DepKind::Build;
}
