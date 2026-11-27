# C# and .NET discovery evaluation

This work covers [temporary project registrations](https://github.com/aovestdipaperino/tokensave/issues/1119), [structural search](https://github.com/aovestdipaperino/tokensave/issues/1140), and [.NET resources and configuration](https://github.com/aovestdipaperino/tokensave/issues/1145).

- The ephemeral-project setting applies at the common registry writer, including CLI init, auto-init and the MCP server. Existing stale registrations remain eligible for interactive doctor cleanup. The check distinguishes absent paths from filesystem errors and rechecks candidates after confirmation.
- C# discovery uses predefined, typed predicates on the bundled Tree-sitter grammar. A raw Tree-sitter query interface could express additional patterns, but would also expose a grammar-specific DSL and require limits on query compilation and match expansion. The typed modes give both transports identical cost controls and leave language-specific syntax handling in the implementation. Literal extraction discussed in #1136 can use the same parser without changing what the structural modes return.
- .NET discovery selects paths within the existing index and parses XML and C# on demand. The file index records the presence of artifacts. Key definitions and reference candidates are derived from current source and require no new key store or migration. Source scope affects missing-translation and unused results, and generated classes are inferred from RESX stems.

The SQL query returns at most 257 paths. Scanning processes one source file at a time, with per-file and aggregate byte limits and a C# parse deadline. XML has a node limit, prohibits DTDs, and emits key names and locations only. The resource accumulator and each output page also have fixed limits. The structural search fixture measures the accuracy, runtime and returned context bytes of a same-line regex against a syntax-based assignment query.

## Fixture results

A Windows debug run of 100 passes took 2.62 ms for the precompiled
same-line regex and 157.50 ms for the C# parser and structured location
construction. The regex found one real assignment and one commented example.
It missed a multiline literal assignment. The AST found both real
assignments and skipped the comment. The two matched regex lines took
56 bytes as a compact JSON array, and the two AST records took 241 bytes as
a compact JSON array with file, ranges, kind, target, and enclosing symbol.
This is one fixture in memory, without CLI startup, index I/O, or MCP
transport. It does not establish latency or memory use on large projects.
