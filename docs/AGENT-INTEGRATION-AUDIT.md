# Agent integration audit

[Roadmap #1179](https://github.com/aovestdipaperino/tokensave/issues/1179) tracks the broader integration work. This audit selects project-scoped Codex CLI setup as the next Priority 0 item.

## Core follow-ups

| Agent | Issue | PR | Merged into `master` |
| --- | --- | --- | --- |
| Codex CLI | [#1185](https://github.com/aovestdipaperino/tokensave/issues/1185) | [#1186](https://github.com/aovestdipaperino/tokensave/pull/1186) | [`7ffbee15`](https://github.com/aovestdipaperino/tokensave/commit/7ffbee15) honors `CODEX_HOME`. |
| Claude Code | [#1183](https://github.com/aovestdipaperino/tokensave/issues/1183) | [#1187](https://github.com/aovestdipaperino/tokensave/pull/1187) | [`05ff4035`](https://github.com/aovestdipaperino/tokensave/commit/05ff4035) respects the user hook opt-out during doctor repair. |
| GitHub Copilot CLI | [#1184](https://github.com/aovestdipaperino/tokensave/issues/1184) | [#1188](https://github.com/aovestdipaperino/tokensave/pull/1188) | [`8760fcc4`](https://github.com/aovestdipaperino/tokensave/commit/8760fcc4) honors `COPILOT_HOME`. |

The merge commits are present in the local `origin/master` snapshot at `05ff4035`. Final CI and release publication for these three follow-ups remain unverified in this audit.

## Codex CLI project scope

[PR #1190](https://github.com/aovestdipaperino/tokensave/pull/1190) added project-local MCP registration, uninstall, dry-run planning, and doctor reporting. It merged into `master` as [`4be351ad`](https://github.com/aovestdipaperino/tokensave/commit/4be351ad7d6c7f21da9c20e9c3b6b4a418882340). The user-level adapter also installs the advisory shell hook and prompt rules and probes MCP startup in an indexed project.

Codex loads trusted project `.codex/config.toml` layers ahead of user config. It also discovers project `.codex/hooks.json` and can combine matching hooks from multiple layers. Linked Git worktrees can inherit hook sources from the main checkout. Project trust and review of the exact hook definition are distinct. [Config precedence](https://learn.chatgpt.com/docs/config-file/config-basic) · [Hook discovery and trust](https://learn.chatgpt.com/docs/hooks)

### Validation and remaining work

The PR passed 17 focused Windows Rust tests, 11 wiki tests, and a Windows CLI smoke test covering local install, both doctor entry points, and uninstall.

On Windows, the TokenSave binary from PR #1190 installed MCP into a disposable indexed Git repository. With an isolated `CODEX_HOME` and Codex CLI `0.171.0-alpha.13.2`, an untrusted app-server thread listed no TokenSave server. A trusted thread listed `tokensave_search` and called it through the app server. The result found the expected fixture symbol in `sample.py`.

Project hook installation still needs validation of combined user and project hooks, linked worktree behavior, and review through `/hooks`. Native plugin packaging, validation on the supported Codex CLI release, and TokenSave release publication remain open.

## GitHub Copilot CLI project scope

The Copilot adapter writes its CLI registration to `$COPILOT_HOME/mcp-config.json` or `~/.copilot/mcp-config.json`. The same adapter manages VS Code and JetBrains configuration at user scope. It has no project install path. The earlier parity issue [#814](https://github.com/aovestdipaperino/tokensave/issues/814) is closed as not planned. [Copilot adapter](https://github.com/aovestdipaperino/tokensave/blob/4be351ad7d6c7f21da9c20e9c3b6b4a418882340/src/agents/integrations/copilot.rs)

An isolated Windows probe with Copilot CLI `1.0.116` verified that `copilot mcp get tokensave` selects the user entry outside the repository and skips a workspace entry in an untrusted checkout. After the fixture checkout was trusted, Copilot selected a bare-map `.github/mcp.json` entry over user config, a root `.mcp.json` entry over `.github/mcp.json`, and a nearer `.mcp.json` from a child directory. The probe inspected configuration selection without starting a TokenSave server. [GitHub Copilot MCP configuration](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers)

### Proposed first slice

- Use root `.github/mcp.json` for `tokensave install --local --agent copilot`. Claude Code and other adapters already write `.mcp.json`, which Copilot can also read.
- Preserve both supported JSON shapes and unrelated entries. Use the same path for ownership-aware uninstall and dry run. Keep VS Code, JetBrains, and user-level Copilot configuration outside the project operation.
- Have doctor report project registration, any visible shadowing by `.mcp.json`, and the role of Copilot folder trust. Verify that a trusted Copilot session can start the configured server and call `tokensave_search`.
