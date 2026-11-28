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

The Codex adapter resolves the user home for install, uninstall, dry-run planning, and health checks. It inherits the default `supports_local() == false`, so the shared CLI rejects `install --local --agent codex`. The user-level adapter already registers MCP, installs the advisory shell hook and prompt rules, and probes MCP startup in an indexed project. [Codex adapter](https://github.com/aovestdipaperino/tokensave/blob/05ff4035/src/agents/integrations/codex.rs) · [Agent trait](https://github.com/aovestdipaperino/tokensave/blob/05ff4035/src/agents/traits.rs)

Codex loads trusted project `.codex/config.toml` layers ahead of user config. It also discovers project `.codex/hooks.json` and can combine matching hooks from multiple layers. Linked Git worktrees can inherit hook sources from the main checkout. Project trust and review of the exact hook definition are distinct. [Config precedence](https://learn.chatgpt.com/docs/config-file/config-basic) · [Hook discovery and trust](https://learn.chatgpt.com/docs/hooks)

### Proposed first slice

- Add project-local MCP registration using the existing install scope and TOML handling. Write the registration at the selected repository root and preserve existing settings, including user-supplied timeouts and tool filters.
- Support local uninstall and its dry run. Remove the integration entry according to the existing ownership and approval rules. Preserve unrelated tables and the configuration of other scopes.
- Have doctor identify the project file and the user file independently. Keep startup probing bounded and retain Codex's project trust requirements.
- Cover repeated install and uninstall, malformed TOML, paths with spaces, a custom `CODEX_HOME`, and the same server name at both scopes. Verify that a trusted Codex CLI session can discover and call a TokenSave tool and that an untrusted session skips the project registration.

Project hook installation needs separate validation of combined user and project hooks, linked worktree behavior, and review through `/hooks`. Native plugin packaging and live activation on the supported Codex CLI version remain separate audit items. The available local executable reported `codex-cli 0.171.0-alpha.13.2`. The project-scope assessment uses source and official documentation, with live client behavior still to verify.
