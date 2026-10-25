# tree-sitter-gdscript (vendored)

Source: https://github.com/PrestonKnopp/tree-sitter-gdscript
Commit: 495cf07da02e5381f8147645c080bc56a13d8655
License: MIT (see LICENSE)

`src/parser.c` + `src/scanner.c` (external scanner) + `src/tree_sitter/` headers
are needed. Compiled by build.rs when the `lang-gdscript` feature is enabled
(mirrors the WGSL vendored-grammar-with-scanner block, not the ActionScript
parser-only block, since GDScript has an external scanner).

`LANGUAGE_VERSION 14`, ABI 14 — loads fine against `tree-sitter = 0.26`.
Node/field names verified against `src/node-types.json`.
