# tokensave Documentation

tokensave gives AI coding agents a pre-indexed semantic code graph through an
MCP server, so they spend fewer tokens and tool calls exploring a codebase.
The [project README](../../README.md) is the short introduction; these pages hold
the details.

<!-- wiki:omit-start -->
This directory is the source of the
[GitHub Wiki](https://github.com/aovestdipaperino/tokensave/wiki). The wiki is
generated from it on every change to `master`; edit the files here, not the
wiki.
<!-- wiki:omit-end -->

## Guide

<!-- wiki-sidebar:start -->
- [Installation and Upgrades](INSTALLATION.md): install methods, language tiers, self-upgrade, versioning, building from source
- [Agent Integrations](AGENT-INTEGRATIONS.md): `tokensave install`, supported agents, project-local setup, what is written for Claude Code
- [Indexing and Freshness](INDEXING.md): `init` and `sync`, project discovery, index freshness, crash-resilient sync, what gets indexed
- [MCP Tools](MCP-TOOLS.md): the tools and resources exposed to agents, cross-project queries
- [Multi-Branch Indexing](MULTI-BRANCH.md): per-branch graphs, automatic tracking, branch quality regressions
- [Token Tracking and Cost](TOKEN-TRACKING.md): per-call metrics, savings ledger, cost, monitor, memory diagnostics
- [CLI Reference](CLI-REFERENCE.md): every command at a glance
- [Supported Languages](LANGUAGES.md): languages and feature tiers
- [Network Calls and Privacy](PRIVACY.md): what leaves your machine and how to opt out
- [Troubleshooting](TROUBLESHOOTING.md): `tokensave doctor` and common problems
- [Benchmarks](BENCHMARKS.md): `tokensave bench`, the criterion suite, the MCP test-matrix probe
- [Comparisons](COMPARISONS.md): tokensave compared with CodeGraph and other tools
<!-- wiki-sidebar:end -->

## In-depth references

These documents stay in the repository and are linked from the wiki:

- [User Guide](../USER-GUIDE.md): a long-form walkthrough of everyday use
- [Multi-Branch Indexing Guide](../BRANCHING-USER-GUIDE.md)
- [Database compatibility](../DATABASE-COMPATIBILITY.md) and
  [database recovery](../DATABASE-RECOVERY.md)
- [Versioning policy](../../TOKENSAVE-VERSIONING.md)
- [Changelog](../../CHANGELOG.md)

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for how to build, test, and send
changes. Documentation changes follow the
[documentation rules](../../CONTRIBUTING.md#documentation): update these pages in
the same change as the behavior they describe.
