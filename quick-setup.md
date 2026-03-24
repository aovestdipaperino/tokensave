# Quick Setup

## 1. Install

```bash
brew tap aovestdipaperino/tap
brew install tokensave
```

Verify it works:

```bash
tokensave --help
```

## 2. Index your project

```bash
cd /path/to/your/project
tokensave sync
```

This creates a `.tokensave/` directory (if needed) and indexes all Rust, Go, and Java files in the project. Running `tokensave sync` again picks up only changed files. To force a full re-index, use `tokensave sync --force`.

Check what was indexed:

```bash
tokensave status
```

## 3. Configure the MCP server in Claude

Add the following to your Claude settings file.

**Claude Code** (`~/.claude/settings.json`):

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

**Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`):

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

## 4. Use it with Claude

Once the MCP server is configured, Claude has access to these tools:

| Tool | What it does |
|------|-------------|
| `tokensave_search` | Find symbols by name or keyword |
| `tokensave_context` | Build AI-ready context for a task description |
| `tokensave_callers` | Find all callers of a function |
| `tokensave_callees` | Find all callees of a function |
| `tokensave_impact` | Compute the impact radius of a symbol |
| `tokensave_node` | Get detailed info about a specific symbol |
| `tokensave_status` | Show graph statistics |

Claude will use these tools automatically when you ask questions about your codebase. Examples:

- *"How does the authentication module work?"* -- uses `tokensave_context`
- *"What calls the `processPayment` function?"* -- uses `tokensave_callers`
- *"If I change `UserService`, what else is affected?"* -- uses `tokensave_impact`

## Keeping the index fresh

After making code changes, sync the graph:

```bash
tokensave sync
```

The MCP server reads from the database on each request, so it picks up synced changes without restarting.
