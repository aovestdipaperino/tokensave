//! Display labels for the languages of indexed files, shared by the storage
//! backends' statistics.

/// Maps a file path to a human-readable language label used in
/// `GraphStats::files_by_language`. Anything we don't recognise lands in
/// `"Other"`. The label set must stay in sync with the language extractors
/// registered in `crate::extraction::LanguageRegistry`; the test
/// `files_by_language_covers_known_extensions` guards the mapping.
pub(crate) fn display_language_for_path(path: &str) -> &'static str {
    // Special-case extensionless files we still recognise by name.
    let basename = path.rsplit('/').next().unwrap_or(path);
    let lower = basename.to_ascii_lowercase();
    if lower == "dockerfile" || lower.starts_with("dockerfile.") {
        return "Dockerfile";
    }
    if lower == "makefile" {
        return "Makefile";
    }
    match path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "Rust",
        "go" => "Go",
        "py" | "pyi" | "pyx" => "Python",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "ts" | "tsx" | "mts" | "cts" => "TypeScript",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "scala" | "sc" => "Scala",
        "swift" => "Swift",
        "as" => "ActionScript",
        "c" | "h" => "C",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" | "inl" | "ipp" | "tcc" => "C++",
        "cu" | "cuh" => "CUDA",
        "cs" => "C#",
        "fs" | "fsi" | "fsx" => "F#",
        "fst" | "fsti" => "F*",
        "rb" | "rake" | "erb" | "slim" => "Ruby",
        "php" => "PHP",
        "dart" => "Dart",
        "lua" => "Lua",
        "pl" | "pm" => "Perl",
        "sh" | "bash" => "Bash",
        "ps1" | "psm1" => "PowerShell",
        "nix" => "Nix",
        "zig" => "Zig",
        "proto" => "Protobuf",
        "toml" => "TOML",
        "sql" => "SQL",
        "r" => "R",
        "jl" => "Julia",
        "ex" | "exs" => "Elixir",
        "erl" | "hrl" => "Erlang",
        "hs" => "Haskell",
        "clj" | "cljs" | "cljc" | "edn" => "Clojure",
        "ml" | "mli" => "OCaml",
        "lean" => "Lean",
        "m" | "mm" => "Objective-C",
        "f" | "f90" | "f95" | "f03" | "f08" | "for" => "Fortran",
        "cbl" | "cob" | "cpy" => "COBOL",
        "pas" | "pp" | "dpr" | "dpk" | "lpr" | "inc" => "Pascal",
        "vb" => "VB.NET",
        "bas" => "BASIC",
        "bat" | "cmd" => "Batch",
        "glsl" | "vert" | "frag" | "comp" | "geom" | "tesc" | "tese" => "GLSL",
        "v" | "vh" | "sv" | "svh" => "SystemVerilog",
        "qnt" => "Quint",
        "gd" => "GDScript",
        "vhd" | "vhdl" => "VHDL",
        "awk" => "AWK",
        "wl" | "wls" | "wlt" => "Wolfram",
        "lisp" | "lsp" | "cl" | "asd" => "Common Lisp",
        "scm" | "ss" => "Scheme",
        "fbs" => "FlatBuffers",
        _ => "Other",
    }
}

#[cfg(test)]
mod tests {
    use super::display_language_for_path;

    #[test]
    fn maps_common_extensions_to_named_languages() {
        assert_eq!(display_language_for_path("src/main.rs"), "Rust");
        assert_eq!(display_language_for_path("a/b/foo.py"), "Python");
        assert_eq!(display_language_for_path("foo.pyi"), "Python");
        assert_eq!(display_language_for_path("foo.tsx"), "TypeScript");
        assert_eq!(display_language_for_path("foo.cs"), "C#");
        assert_eq!(display_language_for_path("foo.fst"), "F*");
        assert_eq!(display_language_for_path("foo.fsti"), "F*");
        assert_eq!(display_language_for_path("foo.cpp"), "C++");
        assert_eq!(display_language_for_path("rtl/top.v"), "SystemVerilog");
        assert_eq!(display_language_for_path("rtl/defs.vh"), "SystemVerilog");
        assert_eq!(display_language_for_path("rtl/top.sv"), "SystemVerilog");
        assert_eq!(display_language_for_path("rtl/pkg.svh"), "SystemVerilog");
        assert_eq!(
            display_language_for_path("com/example/Game.as"),
            "ActionScript"
        );
        assert_eq!(display_language_for_path("Dockerfile"), "Dockerfile");
        assert_eq!(
            display_language_for_path("docker/Dockerfile.prod"),
            "Dockerfile"
        );
        assert_eq!(display_language_for_path("Makefile"), "Makefile");
        assert_eq!(display_language_for_path("readme.txt"), "Other");
        assert_eq!(display_language_for_path("noext"), "Other");
        assert_eq!(
            display_language_for_path("player.gd"),
            "GDScript",
            "GDScript files must not fall into the Other bucket in `status`'s files_by_language"
        );
    }

    #[test]
    fn extension_match_is_case_insensitive() {
        assert_eq!(display_language_for_path("Foo.RS"), "Rust");
        assert_eq!(display_language_for_path("Foo.PY"), "Python");
    }
}
