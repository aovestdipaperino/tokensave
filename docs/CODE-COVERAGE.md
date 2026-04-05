# Code Coverage Report

**Version:** 3.2.2
**Date:** 2026-04-05
**Tool:** [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov)
**Total tests:** 1,046
**Overall line coverage:** 83.3% (32,358 / 38,841 lines)

## Coverage by Module

| Module | Covered | Total | Coverage |
|--------|--------:|------:|---------:|
| agents | 2,103 | 3,265 | 64.4% |
| cloud | 75 | 143 | 52.4% |
| config | 61 | 90 | 67.8% |
| context | 440 | 461 | 95.4% |
| daemon | 45 | 304 | 14.8% |
| db | 1,794 | 2,384 | 75.3% |
| display | 244 | 247 | 98.8% |
| doctor | 0 | 156 | 0.0% |
| extraction | 23,372 | 26,208 | 89.2% |
| global_db | 57 | 87 | 65.5% |
| graph | 508 | 546 | 93.0% |
| main | 0 | 721 | 0.0% |
| mcp | 2,604 | 3,055 | 85.2% |
| resolution | 147 | 155 | 94.8% |
| sync | 39 | 39 | 100.0% |
| tokensave | 507 | 599 | 84.6% |
| types | 202 | 205 | 98.5% |
| user_config | 37 | 50 | 74.0% |
| vectors | 123 | 126 | 97.6% |

## Coverage by File

### Extraction (89.2%)

| File | Covered | Total | Coverage |
|------|--------:|------:|---------:|
| extraction/bash_extractor.rs | 326 | 348 | 93.7% |
| extraction/batch_extractor.rs | 306 | 331 | 92.4% |
| extraction/c_extractor.rs | 1,122 | 1,200 | 93.5% |
| extraction/cobol_extractor.rs | 461 | 502 | 91.8% |
| extraction/complexity.rs | 120 | 133 | 90.2% |
| extraction/cpp_extractor.rs | 1,336 | 1,854 | 72.1% |
| extraction/csharp_extractor.rs | 1,145 | 1,396 | 82.0% |
| extraction/dart_extractor.rs | 1,068 | 1,279 | 83.5% |
| extraction/fortran_extractor.rs | 731 | 756 | 96.7% |
| extraction/go_extractor.rs | 948 | 1,048 | 90.5% |
| extraction/gwbasic_extractor.rs | 505 | 535 | 94.4% |
| extraction/java_extractor.rs | 1,061 | 1,129 | 94.0% |
| extraction/kotlin_extractor.rs | 1,175 | 1,252 | 93.8% |
| extraction/lua_extractor.rs | 396 | 427 | 92.7% |
| extraction/mod.rs | 51 | 54 | 94.4% |
| extraction/msbasic2_extractor.rs | 414 | 443 | 93.5% |
| extraction/nix_extractor.rs | 699 | 752 | 93.0% |
| extraction/objc_extractor.rs | 993 | 1,178 | 84.3% |
| extraction/pascal_extractor.rs | 1,100 | 1,141 | 96.4% |
| extraction/perl_extractor.rs | 516 | 549 | 94.0% |
| extraction/php_extractor.rs | 866 | 1,001 | 86.5% |
| extraction/powershell_extractor.rs | 357 | 376 | 94.9% |
| extraction/proto_extractor.rs | 656 | 686 | 95.6% |
| extraction/python_extractor.rs | 618 | 699 | 88.4% |
| extraction/qbasic_extractor.rs | 525 | 547 | 96.0% |
| extraction/quickbasic_extractor.rs | 9 | 9 | 100.0% |
| extraction/ruby_extractor.rs | 500 | 541 | 92.4% |
| extraction/rust_extractor.rs | 893 | 973 | 91.8% |
| extraction/scala_extractor.rs | 835 | 1,126 | 74.2% |
| extraction/swift_extractor.rs | 908 | 1,000 | 90.8% |
| extraction/ts_provider.rs | 13 | 14 | 92.9% |
| extraction/typescript_extractor.rs | 1,135 | 1,179 | 96.3% |
| extraction/vbnet_extractor.rs | 919 | 1,039 | 88.5% |
| extraction/zig_extractor.rs | 665 | 711 | 93.5% |

### MCP (85.2%)

| File | Covered | Total | Coverage |
|------|--------:|------:|---------:|
| mcp/server.rs | 281 | 377 | 74.5% |
| mcp/tools/definitions.rs | 581 | 581 | 100.0% |
| mcp/tools/handlers.rs | 1,637 | 1,971 | 83.1% |
| mcp/transport.rs | 105 | 126 | 83.3% |

### Agents (64.4%)

| File | Covered | Total | Coverage |
|------|--------:|------:|---------:|
| agents/claude.rs | 597 | 787 | 75.9% |
| agents/cline.rs | 44 | 132 | 33.3% |
| agents/codex.rs | 161 | 255 | 63.1% |
| agents/copilot.rs | 150 | 261 | 57.5% |
| agents/cursor.rs | 98 | 126 | 77.8% |
| agents/gemini.rs | 178 | 232 | 76.7% |
| agents/mod.rs | 657 | 986 | 66.6% |
| agents/opencode.rs | 127 | 232 | 54.7% |
| agents/roo_code.rs | 44 | 132 | 33.3% |
| agents/zed.rs | 47 | 122 | 38.5% |

### Core

| File | Covered | Total | Coverage |
|------|--------:|------:|---------:|
| cloud.rs | 75 | 143 | 52.4% |
| config.rs | 61 | 90 | 67.8% |
| context/builder.rs | 264 | 283 | 93.3% |
| context/formatter.rs | 176 | 178 | 98.9% |
| daemon.rs | 45 | 304 | 14.8% |
| db/connection.rs | 142 | 199 | 71.4% |
| db/migrations.rs | 346 | 417 | 83.0% |
| db/queries.rs | 1,306 | 1,768 | 73.9% |
| display.rs | 244 | 247 | 98.8% |
| doctor.rs | 0 | 156 | 0.0% |
| global_db.rs | 57 | 87 | 65.5% |
| graph/queries.rs | 191 | 195 | 97.9% |
| graph/traversal.rs | 317 | 351 | 90.3% |
| main.rs | 0 | 721 | 0.0% |
| resolution/resolver.rs | 147 | 155 | 94.8% |
| sync.rs | 39 | 39 | 100.0% |
| tokensave.rs | 507 | 599 | 84.6% |
| types.rs | 202 | 205 | 98.5% |
| user_config.rs | 37 | 50 | 74.0% |
| vectors/search.rs | 123 | 126 | 97.6% |

## Zero-Coverage Files

These files have 0% line coverage and are not exercised by any test:

| File | Lines | Reason |
|------|------:|--------|
| main.rs | 721 | Binary entrypoint — CLI dispatch, not reachable from `cargo test` |
| doctor.rs | 156 | System health checks — calls external binaries, inspects filesystem state |

## Regenerating This Report

```sh
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
cargo llvm-cov --tests --features test-transport --json > coverage.json
```

Note: `--features test-transport` is required to include the MCP server end-to-end tests
(`tests/mcp_server_test.rs`) which use the `ChannelTransport` in-memory transport.
