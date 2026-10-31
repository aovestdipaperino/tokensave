# Supported Languages

The languages tokensave indexes and the Cargo feature tiers that select them.

## Language tiers

tokensave supports more than 50 programming languages organized into three tiers controlled by Cargo feature flags. Each tier includes all languages from the tier below it. Markdown headers are extracted as `Module` nodes with hierarchical `Contains` edges so document structure participates in graph queries alongside source code.

### Lite -- `--no-default-features`

Always compiled. The smallest binary for the most popular languages, plus Svelte and Astro (script-block extraction via the TypeScript extractor, no extra grammar dependency).

| Language | Extensions |
|----------|-----------|
| Rust | `.rs` |
| Go | `.go` |
| Java | `.java` |
| Scala | `.scala`, `.sc` |
| TypeScript | `.ts`, `.tsx` |
| JavaScript | `.js`, `.jsx` |
| Python | `.py` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.hpp`, `.cc`, `.cxx`, `.hh` |
| Kotlin | `.kt`, `.kts` |
| C# | `.cs` |
| Swift | `.swift` |
| Svelte | `.svelte` |
| Astro | `.astro` |

### Medium (Lite + 9 more) -- `--features medium`

| Language | Extensions | Feature flag |
|----------|-----------|-------------|
| Dart | `.dart` | `lang-dart` |
| Pascal | `.pas`, `.pp`, `.dpr` | `lang-pascal` |
| PHP | `.php` | `lang-php` |
| Ruby (including ERB and Slim templates) | `.rb`, `.rake`, `.erb`, `.slim` | `lang-ruby` |
| Bash | `.sh`, `.bash` | `lang-bash` |
| Protobuf | `.proto` | `lang-protobuf` |
| PowerShell | `.ps1`, `.psm1` | `lang-powershell` |
| Nix | `.nix` | `lang-nix` |
| VB.NET | `.vb` | `lang-vbnet` |

ERB and Slim templates use the Ruby grammar without requiring Ruby or template gems at indexing time. Template calls belong to the file node and retain their original line and byte column. ERB supports code/output tags, trim markers, comments, and escaped tags. Slim supports control/output lines, indentation-based blocks, attributes (including multiline attribute lists), text interpolation, and `ruby:` blocks.

Custom Slim shortcuts and embedded-language compilation are not supported. Locals supplied by Rails at render time cannot be distinguished from bare helper calls; names assigned or bound in the template are excluded conservatively. Controller-to-view and partial-render relationships require separate Rails semantic wiring.

### Full (Medium + everything else) -- default

| Language | Extensions | Feature flag |
|----------|-----------|-------------|
| ActionScript | `.as` | `lang-actionscript` |
| Lua | `.lua` | `lang-lua` |
| Zig | `.zig` | `lang-zig` |
| Objective-C | `.m`, `.mm` | `lang-objc` |
| Perl | `.pl`, `.pm` | `lang-perl` |
| Batch/CMD | `.bat`, `.cmd` | `lang-batch` |
| Fortran | `.f90`, `.f95`, `.f03`, `.f08`, `.f18`, `.f`, `.for` | `lang-fortran` |
| COBOL | `.cob`, `.cbl`, `.cpy` | `lang-cobol` |
| MS BASIC 2.0 | `.bas` | `lang-msbasic2` |
| GW-BASIC | `.gw` | `lang-gwbasic` |
| QBasic | `.qb` | `lang-qbasic` |
| QuickBASIC 4.5 | `.bi`, `.bm` | `lang-qbasic` |
| Dockerfile | `Dockerfile`, `.dockerfile` | `lang-dockerfile` |
| GLSL | `.glsl`, `.vert`, `.frag`, `.comp` | `lang-glsl` |
| Godot Shader | `.gdshader`, `.gdshaderinc` | `lang-glsl` |
| Minecraft Function | `.mcfunction` | `lang-mcfunction` |
| WGSL | `.wgsl` | `lang-wgsl` |
| HLSL | `.hlsl`, `.fx` | `lang-hlsl` |
| Verilog / SystemVerilog | `.v`, `.vh`, `.sv`, `.svh` | `lang-systemverilog` |
| VHDL | `.vhd`, `.vhdl` | `lang-vhdl` |
| Metal | `.metal` | `lang-metal` |
| CUDA / HIP | `.cu`, `.cuh` | `lang-cuda` |
| Markdown | `.md`, `.markdown` | `lang-markdown` |
| R | `.r`, `.R` | `lang-r` |
| SQL | `.sql` | `lang-sql` |
| Julia | `.jl` | `lang-julia` |
| Haskell | `.hs`, `.lhs` | `lang-haskell` |
| OCaml | `.ml`, `.mli` | `lang-ocaml` |
| Clojure | `.clj`, `.cljs`, `.cljc` | `lang-clojure` |
| Erlang | `.erl`, `.hrl` | `lang-erlang` |
| Elixir | `.ex`, `.exs` | `lang-elixir` |
| F# | `.fs`, `.fsi`, `.fsx` | `lang-fsharp` |
| F* | `.fst`, `.fsti` | `lang-fstar` |
| Quint | `.qnt` | `lang-quint` |
| Terraform | `.tf`, `.tfvars` | `lang-terraform` |
| TOML | `.toml` | `lang-toml` |
| Lean | `.lean` | `lang-lean` |
| HTML | `.html`, `.htm` | `lang-html` |
| CSS | `.css` | `lang-css` |

Individual languages can also be cherry-picked without a full tier:

```bash
cargo install tokensave --no-default-features --features lang-nix,lang-bash
```

All extractors share the same depth: functions, classes, methods, fields, imports, call graphs, inheritance chains, docstrings, complexity metrics, decorator/annotation extraction, and cross-file dependency tracking.

HTML and CSS are the exception, because neither language has callable symbols. HTML records elements carrying an `id`, custom elements, and the stylesheets and scripts a page pulls in; CSS records class and id selectors, custom properties, `@keyframes` names, and `@import`s. Neither resolves a `class="..."` attribute to a stylesheet rule: class names are ordinary words, and matching them by bare name across a whole project invents edges rather than finding them.
