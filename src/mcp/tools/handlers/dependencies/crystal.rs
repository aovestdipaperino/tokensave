//! Crystal ecosystem parser — `shard.yml` (+ `shard.lock`).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Workspace};
use super::yaml_util::{as_string, hash_entries, node_field, parse_root};

const ECOSYSTEM: &str = "crystal";

pub fn detect(root: &Path) -> bool {
    root.join("shard.yml").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let path = root.join("shard.yml");
    let raw = std::fs::read_to_string(&path).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", path.display()),
    })?;
    let root_doc = parse_root(&raw).ok_or_else(|| TokenSaveError::Config {
        message: format!("failed to parse YAML at {}", path.display()),
    })?;

    let name = as_string(&root_doc, "name").unwrap_or_else(|| "shard".to_string());
    let mut deps = Vec::new();
    for (section, kind) in [
        ("dependencies", DepKind::Normal),
        ("development_dependencies", DepKind::Dev),
    ] {
        let Some(block) = node_field(&root_doc, section) else {
            continue;
        };
        for (dep_name, value) in hash_entries(block) {
            // shard.yml deps are tables: { github: "user/repo", version: "~> 1.0" }
            let version = as_string(value, "version");
            let source = ["github", "gitlab", "git", "bitbucket"]
                .iter()
                .find_map(|key| as_string(value, key).map(|v| (key, v)));
            let final_version = version
                .clone()
                .or_else(|| source.map(|(k, v)| format!("{k}:{v}")));
            let local_path = as_string(value, "path");
            deps.push(Dep {
                name: dep_name,
                resolved: None,
                version: final_version,
                features: Vec::new(),
                optional: false,
                local_path,
                kind,
            });
        }
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members: vec![Member {
            path: "shard.yml".to_string(),
            name,
            license: None,
            deps,
        }],
        patches: Vec::new(),
    })
}

pub fn apply_lockfile(ws: &mut Workspace) {
    let path = ws.root.join("shard.lock");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let Some(root) = parse_root(&raw) else {
        return;
    };
    let Some(shards) = node_field(&root, "shards") else {
        return;
    };
    let mut versions: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (name, info) in hash_entries(shards) {
        if let Some(v) = as_string(info, "version") {
            versions.insert(name, v);
        }
    }
    for m in &mut ws.members {
        for d in &mut m.deps {
            if d.resolved.is_none() {
                if let Some(v) = versions.get(&d.name) {
                    d.resolved = Some(v.clone());
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_shard_yml() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("shard.yml"),
            "name: my_app\nversion: 0.1.0\n\ndependencies:\n  kemal:\n    github: kemalcr/kemal\n    version: ~> 1.4.0\n\ndevelopment_dependencies:\n  ameba:\n    github: crystal-ameba/ameba\n",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert_eq!(m.name, "my_app");
        let kemal = m.deps.iter().find(|d| d.name == "kemal").unwrap();
        assert_eq!(kemal.version.as_deref(), Some("~> 1.4.0"));
        let ameba = m.deps.iter().find(|d| d.name == "ameba").unwrap();
        assert_eq!(ameba.kind, DepKind::Dev);
        assert_eq!(ameba.version.as_deref(), Some("github:crystal-ameba/ameba"));
    }

    #[test]
    fn shard_lock_stamps_resolved() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("shard.yml"),
            "name: my_app\ndependencies:\n  kemal:\n    github: kemalcr/kemal\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("shard.lock"),
            "version: 2.0\nshards:\n  kemal:\n    github: kemalcr/kemal\n    version: 1.4.0\n",
        )
        .unwrap();
        let mut ws = parse(dir.path()).unwrap();
        apply_lockfile(&mut ws);
        let kemal = ws.members[0]
            .deps
            .iter()
            .find(|d| d.name == "kemal")
            .unwrap();
        assert_eq!(kemal.resolved.as_deref(), Some("1.4.0"));
    }
}
