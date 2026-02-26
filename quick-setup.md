# Quick Setup

## 1. Install

```bash
brew tap aovestdipaperino/tap
brew install codegraph
```

Verify it works:

```bash
codegraph --help
```

## 2. Initialize and index your project

```bash
cd /path/to/your/project
codegraph init --index
```

This creates a `.codegraph/` directory and indexes all Rust, Go, and Java files in the project. You can re-index at any time with `codegraph index`, or incrementally sync changes with `codegraph sync`.

Check what was indexed:

```bash
codegraph status
```

## 3. Configure the MCP server in Claude

Add the following to your Claude settings file.

**Claude Code** (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
      "args": ["serve", "--path", "/path/to/your/project"]
    }
  }
}
```

**Claude Desktop** (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "codegraph": {
      "command": "codegraph",
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
| `codegraph_search` | Find symbols by name or keyword |
| `codegraph_context` | Build AI-ready context for a task description |
| `codegraph_callers` | Find all callers of a function |
| `codegraph_callees` | Find all callees of a function |
| `codegraph_impact` | Compute the impact radius of a symbol |
| `codegraph_node` | Get detailed info about a specific symbol |
| `codegraph_status` | Show graph statistics |

Claude will use these tools automatically when you ask questions about your codebase. Examples:

- *"How does the authentication module work?"* -- uses `codegraph_context`
- *"What calls the `processPayment` function?"* -- uses `codegraph_callers`
- *"If I change `UserService`, what else is affected?"* -- uses `codegraph_impact`

## Keeping the index fresh

After making code changes, sync the graph:

```bash
codegraph sync
```

The MCP server reads from the database on each request, so it picks up synced changes without restarting.
