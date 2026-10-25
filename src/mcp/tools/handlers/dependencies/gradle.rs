//! Gradle ecosystem parser — `build.gradle`, `build.gradle.kts`,
//! `settings.gradle(.kts)`, and `gradle/libs.versions.toml` (#107).
//!
//! Architecture:
//! - `.kts` uses the existing Kotlin tree-sitter grammar (`ts_provider`).
//! - `.gradle` (Groovy DSL) uses `dekobon-tree-sitter-groovy`.
//! - `libs.versions.toml` (Version Catalog) is plain TOML.
//!
//! Both DSL parsers walk the AST for `call_expression`-like nodes whose name
//! is a configuration keyword (`implementation`, `api`, `testImplementation`,
//! etc.) and extract a string argument shaped like `group:name:version`.

use std::path::Path;

use tree_sitter::{Node, Parser};

use crate::errors::{Result, TokenSaveError};
use crate::extraction::ts_provider;

use super::common::{Dep, DepKind, Member, Workspace};

const ECOSYSTEM: &str = "gradle";

/// Gradle configuration keywords we recognise. The list intentionally
/// covers the common ones used by both Android and JVM projects; exotic
/// custom configurations (`myCustomConfiguration(...)`) are passed over.
const CONFIGS: &[(&str, DepKind)] = &[
    ("implementation", DepKind::Normal),
    ("api", DepKind::Normal),
    ("compile", DepKind::Normal), // legacy alias
    ("runtimeOnly", DepKind::Other("runtime")),
    ("compileOnly", DepKind::Other("provided")),
    ("compileOnlyApi", DepKind::Other("provided")),
    ("testImplementation", DepKind::Dev),
    ("androidTestImplementation", DepKind::Dev),
    ("testRuntimeOnly", DepKind::Dev),
    ("testCompileOnly", DepKind::Dev),
    ("annotationProcessor", DepKind::Build),
    ("kapt", DepKind::Build),
    ("ksp", DepKind::Build),
    // Android per-variant configurations — bucket them all as normal.
    ("debugImplementation", DepKind::Normal),
    ("releaseImplementation", DepKind::Normal),
];

pub fn detect(root: &Path) -> bool {
    root.join("build.gradle").exists()
        || root.join("build.gradle.kts").exists()
        || root.join("settings.gradle").exists()
        || root.join("settings.gradle.kts").exists()
        || root.join("gradle").join("libs.versions.toml").exists()
}

pub fn parse(root: &Path) -> Result<Workspace> {
    let mut members: Vec<Member> = Vec::new();

    // Root build file.
    if let Some(m) = parse_build_file(root, root, ".") {
        members.push(m);
    }

    // Multi-module projects — settings.gradle(.kts) `include 'mod'` /
    // `include(":mod")` lists subprojects. Each has its own build file.
    let included = parse_settings_includes(root);
    for module in included {
        // Module paths in settings are colon-prefixed (`:lib:core`); turn
        // them into filesystem paths (`lib/core`).
        let fs_rel = module.trim_start_matches(':').replace(':', "/");
        let module_dir = root.join(&fs_rel);
        if let Some(m) = parse_build_file(root, &module_dir, &fs_rel) {
            members.push(m);
        }
    }

    // Surface the version catalog as a virtual "member" so its deps are
    // discoverable via the standard summary/crate-lookup paths.
    if let Some(m) = parse_version_catalog(root) {
        members.push(m);
    }

    if members.is_empty() {
        return Err(TokenSaveError::Config {
            message: format!(
                "no Gradle build files found at {} (looked for build.gradle{{,.kts}}, settings.gradle{{,.kts}}, gradle/libs.versions.toml)",
                root.display()
            ),
        });
    }

    Ok(Workspace {
        ecosystem: ECOSYSTEM,
        root: root.to_path_buf(),
        members,
        patches: Vec::new(),
    })
}

fn parse_build_file(_root: &Path, module_dir: &Path, rel: &str) -> Option<Member> {
    let kts = module_dir.join("build.gradle.kts");
    let groovy = module_dir.join("build.gradle");
    let (path, language_key) = if kts.exists() {
        (kts, "kotlin")
    } else if groovy.exists() {
        (groovy, "groovy")
    } else {
        return None;
    };

    let raw = std::fs::read_to_string(&path).ok()?;
    let deps = extract_deps_from_source(&raw, language_key)?;

    Some(Member {
        path: rel.to_string(),
        name: if rel
            .trim_start_matches('.')
            .trim_start_matches('/')
            .is_empty()
        {
            module_dir
                .file_name()
                .map_or_else(|| "root".to_string(), |s| s.to_string_lossy().into_owned())
        } else {
            rel.to_string()
        },
        license: None,
        deps,
    })
}

fn extract_deps_from_source(source: &str, language_key: &str) -> Option<Vec<Dep>> {
    let language = if language_key == "groovy" {
        dekobon_tree_sitter_groovy::LANGUAGE.into()
    } else {
        ts_provider::language(language_key)
    };
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;
    let mut deps = Vec::new();
    walk_for_dep_calls(tree.root_node(), source.as_bytes(), &mut deps);
    Some(deps)
}

/// Walk the AST and accept any node whose first child is an identifier
/// matching a Gradle configuration keyword, where one of its arguments is
/// a string shaped like `"group:name[:version]"`.
fn walk_for_dep_calls(node: Node<'_>, src: &[u8], out: &mut Vec<Dep>) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            try_extract_call(child, src, out);
            walk_for_dep_calls(child, src, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn try_extract_call(node: Node<'_>, src: &[u8], out: &mut Vec<Dep>) {
    // We want `<identifier>(<args>)` or `<identifier> <args>` (Groovy can
    // omit parens). Identify the leading identifier and the first string
    // argument anywhere in the subtree.
    let Some(callee) = leading_identifier(node, src) else {
        return;
    };
    let Some((_, kind)) = CONFIGS.iter().find(|(name, _)| *name == callee) else {
        return;
    };

    let Some(spec) = first_string_literal(node, src) else {
        return;
    };
    if let Some(dep) = parse_coordinate(&spec, *kind) {
        out.push(dep);
    }
}

/// Extract the leading identifier text from a call-like node.
fn leading_identifier<'a>(node: Node<'_>, src: &'a [u8]) -> Option<&'a str> {
    // Tree-sitter shapes differ between Kotlin and Groovy; we just look at
    // the first identifier-shaped child whose kind contains "identifier".
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return None;
    }
    loop {
        let child = cursor.node();
        let kind = child.kind();
        if kind.contains("identifier") || kind == "simple_identifier" {
            return child.utf8_text(src).ok();
        }
        if !cursor.goto_next_sibling() {
            return None;
        }
    }
}

/// Pull the first string literal anywhere under this node. We strip the
/// surrounding quotes (single, double, or triple).
fn first_string_literal(node: Node<'_>, src: &[u8]) -> Option<String> {
    let mut found: Option<String> = None;
    visit(node, src, &mut found);
    found
}

fn visit(node: Node<'_>, src: &[u8], out: &mut Option<String>) {
    if out.is_some() {
        return;
    }
    let kind = node.kind();
    // Treat anything string-ish as a candidate. Kotlin uses "string_literal",
    // Groovy uses "string", and the grammars may surface "line_string_literal".
    if kind.contains("string") {
        if let Ok(text) = node.utf8_text(src) {
            let cleaned = strip_string_quotes(text);
            if !cleaned.is_empty() && looks_like_coordinate(&cleaned) {
                *out = Some(cleaned);
                return;
            }
        }
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            visit(cursor.node(), src, out);
            if out.is_some() {
                return;
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn strip_string_quotes(s: &str) -> String {
    let s = s.trim();
    let stripped = s
        .strip_prefix("\"\"\"")
        .and_then(|x| x.strip_suffix("\"\"\""))
        .or_else(|| s.strip_prefix('"').and_then(|x| x.strip_suffix('"')))
        .or_else(|| s.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
        .unwrap_or(s);
    stripped.to_string()
}

fn looks_like_coordinate(s: &str) -> bool {
    // `group:name` or `group:name:version`. Reject anything with whitespace
    // or template-style `${...}` so we don't capture random strings.
    if s.is_empty() || s.contains(char::is_whitespace) || s.contains("${") {
        return false;
    }
    let parts: Vec<&str> = s.split(':').collect();
    matches!(parts.len(), 2 | 3) && parts.iter().all(|p| !p.is_empty())
}

fn parse_coordinate(s: &str, kind: DepKind) -> Option<Dep> {
    let parts: Vec<&str> = s.split(':').collect();
    let (name, version) = match parts.len() {
        2 => (format!("{}:{}", parts[0], parts[1]), None),
        3 => (
            format!("{}:{}", parts[0], parts[1]),
            Some(parts[2].to_string()),
        ),
        _ => return None,
    };
    Some(Dep {
        name,
        resolved: None,
        version,
        features: Vec::new(),
        optional: false,
        local_path: None,
        kind,
    })
}

/// Extract `include 'a:b'` / `include(":a:b")` directives from settings files.
fn parse_settings_includes(root: &Path) -> Vec<String> {
    let mut modules = Vec::new();
    for filename in ["settings.gradle.kts", "settings.gradle"] {
        let path = root.join(filename);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Crude: collect every quoted string on a line that starts with `include`.
        for line in raw.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("include") {
                continue;
            }
            for piece in split_string_literals(trimmed) {
                if !piece.is_empty() {
                    modules.push(piece);
                }
            }
        }
        break;
    }
    modules
}

fn split_string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            out.push(line[start..j].to_string());
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// Gradle Version Catalogs: `gradle/libs.versions.toml`.
///
/// Schema:
/// ```toml
/// [versions]
/// kotlin = "2.0.0"
///
/// [libraries]
/// kotlin-stdlib = { group = "org.jetbrains.kotlin", name = "kotlin-stdlib", version.ref = "kotlin" }
/// guava = "com.google.guava:guava:33.0.0-jre"
/// ```
fn parse_version_catalog(root: &Path) -> Option<Member> {
    let path = root.join("gradle").join("libs.versions.toml");
    let raw = std::fs::read_to_string(&path).ok()?;
    let doc: toml::Value = toml::from_str(&raw).ok()?;

    let versions: std::collections::HashMap<String, String> = doc
        .get("versions")
        .and_then(|v| v.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let mut deps = Vec::new();
    if let Some(libs) = doc.get("libraries").and_then(|v| v.as_table()) {
        for (_alias, body) in libs {
            if let Some(dep) = catalog_dep(body, &versions) {
                deps.push(dep);
            }
        }
    }
    // Plugins are surfaced as DepKind::Build so users can audit
    // build-time-only artifacts separately.
    if let Some(plugins) = doc.get("plugins").and_then(|v| v.as_table()) {
        for (_alias, body) in plugins {
            if let Some(mut dep) = catalog_plugin(body, &versions) {
                dep.kind = DepKind::Build;
                deps.push(dep);
            }
        }
    }

    Some(Member {
        path: "gradle/libs.versions.toml".to_string(),
        name: "version-catalog".to_string(),
        license: None,
        deps,
    })
}

fn catalog_dep(
    value: &toml::Value,
    versions: &std::collections::HashMap<String, String>,
) -> Option<Dep> {
    match value {
        toml::Value::String(s) => parse_coordinate(s, DepKind::Normal),
        toml::Value::Table(t) => {
            // `module = "group:name"` form OR separate group/name.
            let coordinate = if let Some(module) = t.get("module").and_then(|v| v.as_str()) {
                module.to_string()
            } else {
                let group = t.get("group").and_then(|v| v.as_str())?;
                let name = t.get("name").and_then(|v| v.as_str())?;
                format!("{group}:{name}")
            };
            let version = resolve_catalog_version(t, versions);
            let name = coordinate.clone();
            Some(Dep {
                name,
                resolved: None,
                version,
                features: Vec::new(),
                optional: false,
                local_path: None,
                kind: DepKind::Normal,
            })
        }
        _ => None,
    }
}

fn catalog_plugin(
    value: &toml::Value,
    versions: &std::collections::HashMap<String, String>,
) -> Option<Dep> {
    let table = value.as_table()?;
    let id = table.get("id").and_then(|v| v.as_str())?.to_string();
    let version = resolve_catalog_version(table, versions);
    Some(Dep {
        name: id,
        resolved: None,
        version,
        features: Vec::new(),
        optional: false,
        local_path: None,
        kind: DepKind::Normal,
    })
}

fn resolve_catalog_version(
    table: &toml::map::Map<String, toml::Value>,
    versions: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // `version = "1.0"` direct form.
    if let Some(direct) = table.get("version").and_then(|v| v.as_str()) {
        return Some(direct.to_string());
    }
    // `version = { ref = "kotlin" }` table form.
    if let Some(version_table) = table.get("version").and_then(|v| v.as_table()) {
        if let Some(reference) = version_table.get("ref").and_then(|v| v.as_str()) {
            return versions.get(reference).cloned();
        }
    }
    // `version.ref = "kotlin"` dotted form — toml flattens this into nested
    // tables, so the above branch catches it too. No-op.
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    #[test]
    fn parses_build_gradle_kts_via_kotlin_grammar() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "build.gradle.kts",
            r#"plugins { kotlin("jvm") version "2.0.0" }

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-stdlib:2.0.0")
    api("com.google.guava:guava:33.0.0-jre")
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.0")
}
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = ws
            .members
            .iter()
            .find(|m| m.path == ".")
            .expect("root member should be present");
        let stdlib = m
            .deps
            .iter()
            .find(|d| d.name == "org.jetbrains.kotlin:kotlin-stdlib")
            .expect("stdlib not extracted");
        assert_eq!(stdlib.version.as_deref(), Some("2.0.0"));
        assert_eq!(stdlib.kind, DepKind::Normal);
        let junit = m
            .deps
            .iter()
            .find(|d| d.name == "org.junit.jupiter:junit-jupiter")
            .expect("junit not extracted");
        assert_eq!(junit.kind, DepKind::Dev);
    }

    #[test]
    fn parses_build_gradle_via_groovy_grammar() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "build.gradle",
            r"
plugins {
    id 'java'
}

dependencies {
    implementation 'com.google.guava:guava:33.0.0-jre'
    testImplementation 'junit:junit:4.13.2'
    annotationProcessor 'org.projectlombok:lombok:1.18.30'
}
",
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert!(m
            .deps
            .iter()
            .any(|d| d.name == "com.google.guava:guava"
                && d.version.as_deref() == Some("33.0.0-jre")));
        let junit = m
            .deps
            .iter()
            .find(|d| d.name == "junit:junit")
            .expect("junit");
        assert_eq!(junit.kind, DepKind::Dev);
        let lombok = m
            .deps
            .iter()
            .find(|d| d.name == "org.projectlombok:lombok")
            .expect("lombok");
        assert_eq!(lombok.kind, DepKind::Build);
    }

    #[test]
    fn parses_version_catalog_with_ref() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "gradle/libs.versions.toml",
            r#"
[versions]
kotlin = "2.0.0"

[libraries]
kotlin-stdlib = { group = "org.jetbrains.kotlin", name = "kotlin-stdlib", version.ref = "kotlin" }
guava = "com.google.guava:guava:33.0.0-jre"

[plugins]
kotlin-jvm = { id = "org.jetbrains.kotlin.jvm", version.ref = "kotlin" }
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let catalog = ws
            .members
            .iter()
            .find(|m| m.path == "gradle/libs.versions.toml")
            .expect("catalog member");
        let stdlib = catalog
            .deps
            .iter()
            .find(|d| d.name == "org.jetbrains.kotlin:kotlin-stdlib")
            .unwrap();
        assert_eq!(stdlib.version.as_deref(), Some("2.0.0"));
        let guava = catalog
            .deps
            .iter()
            .find(|d| d.name == "com.google.guava:guava")
            .unwrap();
        assert_eq!(guava.version.as_deref(), Some("33.0.0-jre"));
        let plugin = catalog
            .deps
            .iter()
            .find(|d| d.name == "org.jetbrains.kotlin.jvm")
            .unwrap();
        assert_eq!(plugin.kind, DepKind::Build);
        assert_eq!(plugin.version.as_deref(), Some("2.0.0"));
    }

    #[test]
    fn settings_gradle_kts_picks_up_subprojects() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "settings.gradle.kts",
            r#"rootProject.name = "demo"
include(":lib")
include(":app")
"#,
        );
        write(
            dir.path(),
            "lib/build.gradle.kts",
            "dependencies { implementation(\"a:b:1.0\") }\n",
        );
        write(
            dir.path(),
            "app/build.gradle.kts",
            "dependencies { implementation(\"c:d:2.0\") }\n",
        );
        let ws = parse(dir.path()).unwrap();
        let paths: Vec<&str> = ws.members.iter().map(|m| m.path.as_str()).collect();
        assert!(paths.contains(&"lib"));
        assert!(paths.contains(&"app"));
    }

    #[test]
    fn ignores_non_coordinate_strings() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "build.gradle.kts",
            r#"
android {
    namespace = "com.example.app"
    compileSdk = 34
}
dependencies {
    implementation("org.example:lib:1.0")
}
"#,
        );
        let ws = parse(dir.path()).unwrap();
        let m = &ws.members[0];
        assert!(m.deps.iter().any(|d| d.name == "org.example:lib"));
        // The `com.example.app` namespace string is NOT a coordinate.
        assert!(!m.deps.iter().any(|d| d.name.contains("com.example.app")));
    }
}
