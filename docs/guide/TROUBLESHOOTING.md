# Troubleshooting

Fixes for common problems, and the `tokensave doctor` health check.

## `tokensave doctor`

Run a comprehensive health check of your tokensave installation:

```bash
tokensave doctor
```

Checks: binary location, project index, global DB, user config, agent integration (MCP server, hooks, permissions, prompt rules), and network connectivity. If any tool permissions are missing after an upgrade, it tells you to run `tokensave install`. Use `--agent` to check a specific agent only.

Doctor also validates that each installed hook uses the correct tokensave subcommand and auto-repairs broken hooks.

## "tokensave not initialized"

The `.tokensave/` directory doesn't exist in your project.

```bash
tokensave init
```

## MCP server not connecting

The AI agent doesn't see tokensave tools.

1. Ensure the agent config includes the tokensave MCP server (run `tokensave doctor`)
2. Restart the agent completely
3. Check that `tokensave` is in your PATH: `which tokensave`

## Missing symbols in search

- Run `tokensave sync` to update the index
- Check that the language is supported (see [Supported Languages](LANGUAGES.md))
- Verify the file isn't excluded by `.gitignore`

## Indexing is slow

Large projects take longer on the first full index.

- Subsequent runs use incremental sync and are much faster
- Use `tokensave sync` (not `--force`) for day-to-day updates
- Staleness is checked automatically on every MCP tool call while an agent is connected

## Disabling tokensave for specific projects

If a project is too large and tokensave uses too much RAM, you can disable the MCP server per-project by setting `TOKENSAVE_DISABLE_SERVER=true` in its environment. The server exits cleanly without initializing.

**Claude Code** — add to your project's `.claude/settings.json`:

```json
{
  "mcpServers": {
    "tokensave": {
      "command": "tokensave",
      "args": ["serve"],
      "env": {
        "TOKENSAVE_DISABLE_SERVER": "true"
      }
    }
  }
}
```

**Other agents** — set the environment variable in whatever config your agent uses to launch MCP servers.

You can also set it globally via the shell (`TOKENSAVE_DISABLE_SERVER=true claude`), but this disables the tokensave MCP server for every project in the session.

`DISABLE_TOKENSAVE=true` remains supported as a deprecated compatibility alias for configurations created before this variable was namespaced.
