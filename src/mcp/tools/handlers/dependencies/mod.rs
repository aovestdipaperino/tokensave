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
mod dotnet;
mod go;
mod java;
mod node;
mod php;
mod python;
mod ruby;
mod rust;
mod xml_util;

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

    let workspaces = detect_workspaces(cg.project_root(), ecosystem_filter)?;
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

    let output = json!({
        "mode": "workspace",
        "kind_filter": kind_filter,
        "detected_ecosystems": detected_names,
        "total_members": total_members,
        // Convenience flat fields for the common single-ecosystem case.
        "ecosystem": (workspaces.len() == 1).then(|| workspaces[0].ecosystem),
        "members": (workspaces.len() == 1).then(|| {
            workspaces[0]
                .members
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<_>>()
        }),
        "crates": (workspaces.len() == 1).then(|| single.get("crates").cloned().unwrap_or(Value::Null)),
        "patches": (workspaces.len() == 1).then(|| single.get("patches").cloned().unwrap_or(Value::Null)),
        // Always include the polyglot breakdown.
        "ecosystems": ecosystems,
    });

    let formatted = serde_json::to_string_pretty(&output).unwrap_or_default();
    ToolResult {
        value: json!({"content": [{"type": "text", "text": truncate_response(&formatted)}]}),
        touched_files: vec![],
    }
}

fn ecosystem_summary(ws: &Workspace, kind_filter: &str) -> Value {
    let mut all_crates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for m in &ws.members {
        for d in &m.deps {
            if d.kind.passes(kind_filter) {
                all_crates
                    .entry(d.name.clone())
                    .or_default()
                    .insert(m.name.clone());
            }
        }
    }
    let crate_rows: Vec<Value> = all_crates
        .into_iter()
        .map(|(name, members)| {
            json!({
                "crate": name,
                "used_in": members.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect();
    let member_names: Vec<&str> = ws.members.iter().map(|m| m.name.as_str()).collect();
    let patch_rows: Vec<Value> = ws.patches.iter().map(patch_to_json).collect();
    json!({
        "ecosystem": ws.ecosystem,
        "root": ws.root.display().to_string(),
        "member_count": ws.members.len() as u64,
        "members": member_names,
        "crates": crate_rows,
        "patches": patch_rows,
    })
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
