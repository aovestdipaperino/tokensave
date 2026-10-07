//! Relative JavaScript / TypeScript import specifiers mapped to files (#647).
//!
//! The extractors record an `import ... from "<specifier>"` statement as a
//! `Use` node named after the specifier, verbatim. For a relative specifier
//! that string names a *file*, not a symbol, and the name-keyed resolver has
//! nothing to match it against: `../../src/lib/hash.js` has no node called
//! that, and its trailing dotted segment, `js`, is not the module either.
//!
//! The specifier also does not always name the file on disk. Under
//! `"module": "NodeNext"` (and the `bundler` resolution mode most ESM projects
//! use) TypeScript requires relative imports to spell the *emitted* file, so
//! `src/lib/hash.ts` is imported as `./hash.js`. TypeScript maps the specifier
//! back to the source: `.js` to `.ts` / `.tsx` / `.d.ts`, `.jsx` to `.tsx`,
//! `.mjs` to `.mts` / `.d.mts`, `.cjs` to `.cts` / `.d.cts`. An extensionless
//! specifier (classic `node` / `bundler` resolution) tries each source
//! extension and then the directory's `index` file.
//!
//! This module turns an importer path and a specifier into the ordered list of
//! project-relative paths the import may name. The resolver binds the `Use`
//! node to the first one that is indexed, the reachability gate treats that
//! file as imported, and the incremental touched set re-attempts the reference
//! when any of the candidates gains or loses its `File` node.

/// Source extensions tried, in order, for an extensionless specifier.
const EXTENSIONLESS_SUFFIXES: &[&str] = &[
    ".ts", ".tsx", ".d.ts", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs",
];

/// Whether `path` is a file whose `import` statements follow JS module
/// resolution: JavaScript, TypeScript, and the component formats whose
/// script blocks the TypeScript extractor handles.
#[must_use]
pub fn is_js_family_importer(path: &str) -> bool {
    matches!(
        extension(path),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "vue" | "svelte" | "astro"
    )
}

/// Whether `specifier` is relative (`./x`, `../x`, `.` or `..`) rather than a
/// bare package name or an absolute path.
#[must_use]
pub fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
}

/// The project-relative paths a relative import may name, most preferred
/// first. Empty when the importer is not a JS-family file, the specifier is
/// not relative, or the specifier climbs above the project root.
///
/// A TypeScript importer prefers the TypeScript source over a same-named
/// `.js` file, as the compiler does: when both `hash.ts` and `hash.js` exist,
/// the `.js` one is normally build output. A JavaScript importer prefers the
/// file it literally names.
#[must_use]
pub fn relative_module_candidates(importer: &str, specifier: &str) -> Vec<String> {
    if !is_js_family_importer(importer) || !is_relative_specifier(specifier) {
        return Vec::new();
    }
    // Query strings and fragments (`./worker.js?url`) are bundler syntax, not
    // part of the path.
    let specifier = specifier.split(['?', '#']).next().unwrap_or(specifier);
    let base_dir = importer.rfind('/').map_or("", |i| &importer[..i]);
    let Some(target) = normalize_join(base_dir, specifier) else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    let mut push = |path: String| {
        if !path.is_empty() && !out.contains(&path) {
            out.push(path);
        }
    };

    // A trailing slash names a directory: only its index file can match.
    if specifier.ends_with('/') || specifier == "." || specifier == ".." {
        for suffix in EXTENSIONLESS_SUFFIXES {
            push(join_index(&target, suffix));
        }
        return out;
    }

    let ts_importer = matches!(extension(importer), "ts" | "tsx" | "mts" | "cts");
    let file_name = target.rsplit('/').next().unwrap_or(&target);
    let mapped: Option<(&str, &[&str])> = match extension(file_name) {
        "js" => Some(("js", &[".ts", ".tsx", ".d.ts"])),
        "jsx" => Some(("jsx", &[".tsx"])),
        "mjs" => Some(("mjs", &[".mts", ".d.mts"])),
        "cjs" => Some(("cjs", &[".cts", ".d.cts"])),
        _ => None,
    };

    if let Some((ext, sources)) = mapped {
        let stem = &target[..target.len() - ext.len() - 1];
        if !ts_importer {
            push(target.clone());
        }
        for source in sources {
            push(format!("{stem}{source}"));
        }
        push(target.clone());
        return out;
    }

    // Any other extension the specifier spells out (`./data.json`,
    // `./styles.css`, `./hash.ts` under `allowImportingTsExtensions`) names
    // the file as written. Extensionless specifiers, and names whose dot is
    // part of the name rather than an extension (`./foo.service`), go
    // through the source extensions and the directory index as well.
    push(target.clone());
    for suffix in EXTENSIONLESS_SUFFIXES {
        push(format!("{target}{suffix}"));
    }
    for suffix in EXTENSIONLESS_SUFFIXES {
        push(join_index(&target, suffix));
    }
    out
}

/// The text after the last `.` of the last path segment, or `""`.
fn extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rfind('.').map_or("", |i| &name[i + 1..])
}

fn join_index(dir: &str, suffix: &str) -> String {
    if dir.is_empty() {
        format!("index{suffix}")
    } else {
        format!("{dir}/index{suffix}")
    }
}

/// Joins `relative` onto `base_dir` and collapses `.` / `..` segments.
/// `None` when the result would climb above the project root.
fn normalize_join(base_dir: &str, relative: &str) -> Option<String> {
    let mut segments: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for segment in relative.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_specifier_from_ts_prefers_ts_source() {
        assert_eq!(
            relative_module_candidates("tests/lib/hash.test.ts", "../../src/lib/hash.js"),
            vec![
                "src/lib/hash.ts",
                "src/lib/hash.tsx",
                "src/lib/hash.d.ts",
                "src/lib/hash.js",
            ]
        );
    }

    #[test]
    fn js_specifier_from_js_prefers_named_file() {
        assert_eq!(
            relative_module_candidates("src/a.mjs", "./b.js")[0],
            "src/b.js".to_string()
        );
    }

    #[test]
    fn module_extension_mapping() {
        assert_eq!(
            relative_module_candidates("a.ts", "./v.jsx"),
            vec!["v.tsx", "v.jsx"]
        );
        assert_eq!(
            relative_module_candidates("a.ts", "./m.mjs"),
            vec!["m.mts", "m.d.mts", "m.mjs"]
        );
        assert_eq!(
            relative_module_candidates("a.ts", "./c.cjs"),
            vec!["c.cts", "c.d.cts", "c.cjs"]
        );
    }

    #[test]
    fn extensionless_tries_sources_then_index() {
        let c = relative_module_candidates("src/a.ts", "./util");
        assert_eq!(c[0], "src/util");
        assert_eq!(c[1], "src/util.ts");
        assert!(c.contains(&"src/util/index.ts".to_string()));
    }

    #[test]
    fn non_relative_or_escaping_specifiers_yield_nothing() {
        assert!(relative_module_candidates("src/a.ts", "vitest").is_empty());
        assert!(relative_module_candidates("src/a.ts", "@scope/pkg/x.js").is_empty());
        assert!(relative_module_candidates("a.ts", "../outside.js").is_empty());
        assert!(relative_module_candidates("src/a.py", "./b.js").is_empty());
    }

    #[test]
    fn query_suffix_and_directory_specifiers() {
        assert_eq!(
            relative_module_candidates("src/a.ts", "./w.js?worker")[0],
            "src/w.ts"
        );
        assert_eq!(
            relative_module_candidates("src/a/b.ts", "..")[0],
            "src/index.ts"
        );
    }
}
