# tree-sitter-actionscript (vendored)

Source: https://github.com/jcs090218/tree-sitter-actionscript
Commit: 12fc0c4c822c6edd924c13b328a93fe69454b299
License: MIT (see LICENSE)

Only `src/parser.c` + `src/tree_sitter/` headers are needed (no external scanner).
Compiled by build.rs when the `lang-actionscript` feature is enabled.

Chosen over Rileran/tree-sitter-actionscript after a coverage test on a 177-file
FFDec-decompiled AS2 corpus: both scored 98.87% (identical failures); jcs090218 is
the more maintained upstream, so grammar fixes (AVM1 `§…§` identifiers and the
worded `add`/`lt`/`eq`/`and` operators — the two known gaps) are more likely to land there.
