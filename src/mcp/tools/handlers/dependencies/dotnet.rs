//! .NET ecosystem parser — `*.csproj` (and `*.fsproj` / `*.vbproj`).
//!
//! Parses `<PackageReference Include="X" Version="Y" />` and
//! `<PackageVersion Include="X" Version="Y" />` (the `Directory.Packages.props`
//! central-management form).

use std::path::Path;

use crate::errors::{Result, TokenSaveError};

use super::common::{Dep, DepKind, Member, Patch, Workspace};
use super::xml_util::{attr_value, find_element_tags, find_elements};

const ECOSYSTEM: &str = "dotnet";

const PROJECT_EXTS: &[&str] = &["csproj", "fsproj", "vbproj"];

pub fn detect(root: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.filter_map(std::result::Result::ok) {
            let name = e.file_name();
            let s = name.to_string_lossy();
            if let Some((_, ext)) = s.rsplit_once('.') {
                if PROJECT_EXTS.contains(&ext) {
                    return true;
                }
            }
        }
    }
    root.join("Directory.Packages.props").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let mut members: Vec<Member> = Vec::new();

    // One member per *.csproj at the root.
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut files: Vec<String> = entries
            .filter_map(std::result::Result::ok)
            .filter_map(|e| {
                let s = e.file_name().to_string_lossy().into_owned();
                let ok = PROJECT_EXTS
                    .iter()
                    .any(|ext| s.ends_with(&format!(".{ext}")));
                ok.then_some(s)
            })
            .collect();
        files.sort();
        for f in &files {
            if let Some(m) = parse_project_file(root, f) {
                members.push(m);
            }
        }
    }

    let patches = parse_central_versions(root);

    if members.is_empty() && patches.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "no .NET project found at {} (looked for *.csproj / *.fsproj / *.vbproj / Directory.Packages.props)",
                root.display()
            ),
        });
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches,
    })
}

fn parse_project_file(root: &Path, filename: &str) -> Option<Member> {
    let path = root.join(filename);
    let raw = std::fs::read_to_string(&path).ok()?;
    let mut deps = Vec::new();
    for el in find_element_tags(&raw, "PackageReference") {
        let Some(name) = attr_value(el, "Include") else {
            continue;
        };
        let version = attr_value(el, "Version").map(str::to_string);
        // ProjectReference is similar but for sibling projects — `Include`
        // is a relative path, version is absent. We capture those too with
        // `kind = Other("project-ref")` for visibility.
        deps.push(Dep {
            name: name.to_string(),
            resolved: None,
            version,
            features: Vec::new(),
            optional: false,
            local_path: None,
            kind: DepKind::Normal,
        });
    }
    for el in find_element_tags(&raw, "ProjectReference") {
        if let Some(include) = attr_value(el, "Include") {
            deps.push(Dep {
                name: include.to_string(),
                resolved: None,
                version: None,
                features: Vec::new(),
                optional: false,
                local_path: Some(include.to_string()),
                kind: DepKind::Other("project-ref"),
            });
        }
    }

    Some(Member {
        path: filename.to_string(),
        name: filename
            .rsplit_once('.')
            .map_or_else(|| filename.to_string(), |(stem, _)| stem.to_string()),
        license: None,
        deps,
    })
}

fn parse_central_versions(root: &Path) -> Vec<Patch> {
    let path = root.join("Directory.Packages.props");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // Self-closing form first.
    for el in find_element_tags(&raw, "PackageVersion") {
        let Some(name) = attr_value(el, "Include") else {
            continue;
        };
        let version = attr_value(el, "Version").unwrap_or("").to_string();
        out.push(Patch {
            source: "central-package-management".to_string(),
            name: name.to_string(),
            replacement: version,
        });
    }
    // Open-form `<PackageVersion ...>...</PackageVersion>` (rare but valid).
    for el in find_elements(&raw, "PackageVersion") {
        // skip empty body that already matched above
        let _ = el;
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
    fn parses_csproj_package_references() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("App.csproj"),
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><TargetFramework>net8.0</TargetFramework></PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Serilog" Version="3.1.1" />
    <ProjectReference Include="..\Lib\Lib.csproj" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.ecosystem, "dotnet");
        let m = &ws.members[0];
        assert_eq!(m.name, "App");
        let newtonsoft = m.deps.iter().find(|d| d.name == "Newtonsoft.Json").unwrap();
        assert_eq!(newtonsoft.version.as_deref(), Some("13.0.3"));
        assert!(m
            .deps
            .iter()
            .any(|d| matches!(d.kind, DepKind::Other("project-ref"))
                && d.local_path
                    .as_deref()
                    .is_some_and(|p| p.contains("Lib.csproj"))));
    }

    #[test]
    fn surfaces_central_package_management_as_patches() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Directory.Packages.props"),
            r#"<Project>
  <ItemGroup>
    <PackageVersion Include="Microsoft.Extensions.Logging" Version="8.0.0" />
    <PackageVersion Include="xunit" Version="2.7.0" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();
        // Need at least one project file to satisfy the detect() check.
        fs::write(dir.path().join("dummy.csproj"), r"<Project />").unwrap();
        let ws = parse(dir.path()).unwrap();
        assert_eq!(ws.patches.len(), 2);
        assert!(ws.patches.iter().any(|p| p.name == "xunit"));
    }
}
