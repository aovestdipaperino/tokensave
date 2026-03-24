# Quick Setup

## 1. Install

**Homebrew (macOS):**

```bash
brew install aovestdipaperino/tap/tokensave
```

**Cargo (any platform):**

```bash
cargo install tokensave
```

Verify it works:

```bash
tokensave --help
```

## 2. Configure Claude Code

```bash
tokensave claude-install
```

This single command configures everything — MCP server, tool permissions, PreToolUse hook, and CLAUDE.md rules. No scripts, no `jq`, works on macOS/Linux/Windows. Safe to re-run after upgrading.

## 3. Index your project

```bash
cd /path/to/your/project
tokensave sync
```

This creates a `.tokensave/` directory and indexes all supported files (13 languages). Running `tokensave sync` again picks up only changed files. To force a full re-index, use `tokensave sync --force`.

Check what was indexed:

```bash
tokensave status
```

## 4. Use it with Claude

Once configured, Claude has access to these tools:

| Tool | What it does |
|------|-------------|
| `tokensave_search` | Find symbols by name or keyword |
| `tokensave_context` | Build AI-ready context for a task description |
| `tokensave_callers` | Find all callers of a function |
| `tokensave_callees` | Find all callees of a function |
| `tokensave_impact` | Compute the impact radius of a symbol |
| `tokensave_node` | Get detailed info about a specific symbol |
| `tokensave_files` | List indexed project files with filtering |
| `tokensave_affected` | Find test files affected by source changes |
| `tokensave_status` | Show graph statistics |

Claude will use these tools automatically when you ask questions about your codebase. Examples:

- *"How does the authentication module work?"* — uses `tokensave_context`
- *"What calls the `processPayment` function?"* — uses `tokensave_callers`
- *"If I change `UserService`, what else is affected?"* — uses `tokensave_impact`
- *"Which tests need to run after I changed db/connection.rs?"* — uses `tokensave_affected`

### Claude Desktop (manual)

For Claude Desktop, add the MCP server to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "tokensave": {
      "command": "tokensave",
      "args": ["serve", "--path", "/path/to/your/project"]
    }
  }
}
```

Replace `/path/to/your/project` with the absolute path to your indexed project.

## Keeping the index fresh

After making code changes, sync the graph:

```bash
tokensave sync
```

The MCP server reads from the database on each request, so it picks up synced changes without restarting.
