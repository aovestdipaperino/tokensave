//! Java / Maven ecosystem parser — `pom.xml`.
//!
//! Gradle (`build.gradle` / `build.gradle.kts`) is intentionally skipped:
//! its Groovy/Kotlin DSL requires actual evaluation to surface dependencies
//! reliably, which is out of scope for a static parser.

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Patch, Workspace};
use super::xml_util::{find_elements, first_text};

const ECOSYSTEM: &str = "java";

pub fn detect(root: &Path) -> bool {
    root.join("pom.xml").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let manifest = root.join("pom.xml");
    let raw = std::fs::read_to_string(&manifest).map_err(|e| TokenSaveError::Config {
        message: format!("failed to read {}: {e}", manifest.display()),
    })?;

    let project_name = derive_project_name(&raw);
    let mut members: Vec<Member> = Vec::new();
    members.push(parse_pom(&raw, ".", &project_name));

    // Resolve `<modules><module>core</module></modules>` for multi-module
    // builds. Each sub-module has its own pom.xml.
    if let Some(modules_block) = first_text(&raw, "modules") {
        for module in find_elements(modules_block, "module") {
            let rel = module.trim().to_string();
            if rel.is_empty() {
                continue;
            }
            let sub = root.join(&rel).join("pom.xml");
            if !sub.exists() {
                continue;
            }
            if let Ok(sub_raw) = std::fs::read_to_string(&sub) {
                let sub_name = derive_project_name(&sub_raw);
                members.push(parse_pom(&sub_raw, &rel, &sub_name));
            }
        }
    }

    let patches = collect_dep_mgmt_as_patches(&raw);

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches,
    })
}

fn derive_project_name(raw: &str) -> String {
    let artifact = first_text(raw, "artifactId").unwrap_or("pom").trim();
    let group = first_text(raw, "groupId").map(str::trim);
    match group {
        Some(g) if !g.is_empty() && !artifact.is_empty() => format!("{g}:{artifact}"),
        _ => artifact.to_string(),
    }
}

fn parse_pom(raw: &str, path: &str, name: &str) -> Member {
    let mut deps = Vec::new();
    if let Some(deps_block) = first_text(raw, "dependencies") {
        for dep_el in find_elements(deps_block, "dependency") {
            let group = first_text(dep_el, "groupId").unwrap_or("").trim();
            let artifact = first_text(dep_el, "artifactId").unwrap_or("").trim();
            if artifact.is_empty() {
                continue;
            }
            let version = first_text(dep_el, "version").map(|s| s.trim().to_string());
            let scope = first_text(dep_el, "scope").map_or("compile", str::trim);
            let optional = first_text(dep_el, "optional")
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("true"));
            let kind = match scope {
                "test" => DepKind::Dev,
                "provided" | "runtime" => DepKind::Other("runtime"),
                "system" => DepKind::Other("system"),
                "import" => DepKind::Other("import"),
                _ => DepKind::Normal,
            };
            let display = if group.is_empty() {
                artifact.to_string()
            } else {
                format!("{group}:{artifact}")
            };
            deps.push(Dep {
                name: display,
                resolved: None,
                version,
                features: Vec::new(),
                optional,
                local_path: None,
                kind,
            });
        }
    }
    Member {
        path: path.to_string(),
        name: name.to_string(),
        license: None,
        deps,
    }
}

/// Surface `<dependencyManagement>` BOMs as "patches" so callers know the
/// declared version of these deps comes from a managed parent.
fn collect_dep_mgmt_as_patches(raw: &str) -> Vec<Patch> {
    let mut out = Vec::new();
    let Some(mgmt) = first_text(raw, "dependencyManagement") else {
        return out;
    };
    let Some(deps_block) = first_text(mgmt, "dependencies") else {
        return out;
    };
    for dep_el in find_elements(deps_block, "dependency") {
        let artifact = first_text(dep_el, "artifactId").unwrap_or("").trim();
        if artifact.is_empty() {
            continue;
        }
        let group = first_text(dep_el, "groupId").unwrap_or("").trim();
        let version = first_text(dep_el, "version").map(|s| s.trim().to_string());
        let display = if group.is_empty() {
            artifact.to_string()
        } else {
            format!("{group}:{artifact}")
        };
        out.push(Patch {
            source: "dependency-management".to_string(),
            name: display,
            replacement: version.unwrap_or_default(),
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_pom_dependencies() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("pom.xml"),
            r#"<?xml version="1.0"?>
<project>
  <groupId>com.acme</groupId>
  <artifactId>app</artifactId>
  <version>1.0.0</version>
  <dependencies>
    <dependency>
      <groupId>org.springframework</groupId>
      <artifactId>spring-core</artifactId>
      <version>6.1.0</version>
    </dependency>
    <dependency>
      <groupId>junit</groupId>
      <artifactId>junit</artifactId>
      <version>4.13.2</version>
      <scope>test</scope>
    </dependency>
  </dependencies>
</project>"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "java");
        let m = &ws.members[0];
        assert_eq!(m.name, "com.acme:app");
        let spring = m
            .deps
            .iter()
            .find(|d| d.name == "org.springframework:spring-core")
            .unwrap();
        assert_eq!(spring.kind, DepKind::Normal);
        assert_eq!(spring.version.as_deref(), Some("6.1.0"));
        let junit = m.deps.iter().find(|d| d.name == "junit:junit").unwrap();
        assert_eq!(junit.kind, DepKind::Dev);
    }

    #[test]
    fn captures_dependency_management_as_patches() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("pom.xml"),
            r"<project>
  <artifactId>parent</artifactId>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>org.slf4j</groupId>
        <artifactId>slf4j-api</artifactId>
        <version>2.0.13</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
</project>",
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 1);
        assert_eq!(ws.patches[0].name, "org.slf4j:slf4j-api");
        assert_eq!(ws.patches[0].replacement, "2.0.13");
    }
}
