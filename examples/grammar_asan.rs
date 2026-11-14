//! Direct parser entry point for the development-only ASan corpus runner.

use std::collections::BTreeMap;
use std::{env, fs, process};
use tokensave::extraction::ts_provider;
use tree_sitter::{Language, Parser};

fn languages() -> BTreeMap<&'static str, Language> {
    let mut languages: BTreeMap<_, _> = tokensave_large_treesitters::all_languages()
        .into_iter()
        .map(|(key, language)| (key, language.into()))
        .collect();
    for key in [
        "wgsl",
        "actionscript",
        "gdscript",
        "systemverilog",
        "html",
        "css",
        "vhdl",
    ] {
        languages.insert(key, ts_provider::language(key));
    }
    languages.insert("hcl", tree_sitter_hcl::LANGUAGE.into());
    languages.insert("groovy", dekobon_tree_sitter_groovy::LANGUAGE.into());
    languages
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut args = env::args().skip(1);
    let key = args
        .next()
        .ok_or("usage: grammar_asan --list | GRAMMAR INPUT...")?;
    let languages = languages();
    if key == "--list" {
        println!(
            "{}",
            serde_json::to_string(&languages.keys().collect::<Vec<_>>())?
        );
        return Ok(());
    }
    let language = languages.get(key.as_str()).ok_or("unknown grammar")?;
    let mut parser = Parser::new();
    parser.set_language(language)?;
    let mut count = 0;
    for path in args {
        eprintln!("{}", serde_json::json!({"phase": "start", "input": path}));
        let mut source = Vec::new();
        fs::File::open(&path)?
            .take(1_048_577)
            .read_to_end(&mut source)?;
        if source.len() > 1_048_576 {
            return Err("input exceeds 1 MiB".into());
        }
        parser.reset();
        let tree = parser
            .parse(&source, None)
            .ok_or("parse returned no tree")?;
        eprintln!(
            "{}",
            serde_json::json!({
                "phase": "done", "input": path,
                "syntax_error": tree.root_node().has_error(),
            })
        );
        count += 1;
    }
    if count == 0 {
        return Err("at least one input is required".into());
    }
    println!("{}", serde_json::json!({"grammar": key, "inputs": count}));
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inventory_includes_the_production_gradle_grammar() {
        let expected: tree_sitter::Language = dekobon_tree_sitter_groovy::LANGUAGE.into();
        assert_eq!(
            super::languages().get("groovy"),
            Some(&expected),
            "the Gradle dependency parser uses the direct Groovy binding"
        );
    }
}
