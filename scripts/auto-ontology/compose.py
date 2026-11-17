#!/usr/bin/env python3
"""Launch an interactive agent session with TokenSave and Auto Ontology."""
import argparse
import ipaddress
import json
import os
import shutil
from urllib.parse import urlsplit


ROUTING = """Use the local TokenSave MCP server to locate source, symbols, and call paths.
Use Auto Ontology search_terms and get_term for catalog definitions; use
check_readiness before asking it to generate SQL. For a mixed question, look
up code and terminology independently and cite each source in the answer.
Send only the user-supplied business terms to Auto Ontology by default.
Ask for explicit approval before including local source, paths, symbols,
or indexed metadata in any Auto Ontology call. If one server is unavailable,
report which evidence is missing and use the other server where appropriate."""


def endpoint(value):
    message = (
        "use an HTTPS /mcp URL without credentials, query, or fragment "
        "(HTTP is allowed on loopback)"
    )
    if any(character.isspace() or ord(character) < 32 for character in value):
        raise argparse.ArgumentTypeError(message)
    try:
        url = urlsplit(value)
        port = url.port
    except ValueError:
        raise argparse.ArgumentTypeError(message) from None
    if (
        not url.hostname or url.username is not None or url.password is not None
        or any(char in value for char in "?#\\$")
        or not url.path.endswith("/mcp")
        or port == 0
    ):
        raise argparse.ArgumentTypeError(message)
    if url.scheme == "https":
        return value
    if url.scheme == "http":
        if url.hostname == "localhost":
            return value
        try:
            if ipaddress.ip_address(url.hostname).is_loopback:
                return value
        except ValueError:
            pass
    raise argparse.ArgumentTypeError(message)


def claude_arguments(tokensave, url):
    config = {
        "mcpServers": {
            "tokensave": {"type": "stdio", "command": tokensave, "args": ["serve"]},
            "auto-ontology": {"type": "http", "url": url},
        }
    }
    permissions = {"permissions": {"ask": ["mcp__auto-ontology__*"]}}
    return [
        "--strict-mcp-config", "--mcp-config", json.dumps(config),
        "--settings", json.dumps(permissions),
        "--permission-mode", "default",
        "--append-system-prompt", ROUTING,
    ]


def codex_arguments(tokensave, url, login):
    endpoint = json.dumps(url, ensure_ascii=False)
    remote = [
        "-c", f'mcp_servers.tokensave_ontology={{url={endpoint}, '
        'default_tools_approval_mode="prompt", scopes=["openid"]}',
        "-c", 'approvals_reviewer="user"',
    ]
    if login:
        return [*remote, "mcp", "login", "tokensave_ontology"]
    command = json.dumps(tokensave, ensure_ascii=False)
    return [
        "-c", f'mcp_servers.tokensave={{command={command}, args=["serve"]}}',
        *remote, ROUTING + "\nWait for my first task before calling either server.",
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True, type=endpoint, help="Auto Ontology MCP endpoint")
    parser.add_argument("--agent", choices=("claude", "codex"),
                        help="use Claude Code or Codex CLI (detects either by default)")
    parser.add_argument("--login", action="store_true", help="sign in to Auto Ontology via Codex")
    args = parser.parse_args()

    agent = args.agent
    if agent is None:
        agent = "codex" if args.login or not shutil.which("claude") else "claude"
    if args.login and agent != "codex":
        parser.error("sign in to Auto Ontology using /mcp inside Claude Code")

    executable = shutil.which(agent)
    if executable is None:
        parser.error(f"{agent} was not found on PATH")
    tokensave = shutil.which("tokensave")
    if tokensave is None and not args.login:
        parser.error("tokensave was not found on PATH")

    if agent == "codex":
        arguments = codex_arguments(tokensave, args.url, args.login)
    else:
        arguments = claude_arguments(tokensave, args.url)
    try:
        os.execv(executable, [executable, *arguments])
    except OSError as error:
        parser.exit(1, f"cannot launch {agent}: {error.strerror}\n")


if __name__ == "__main__":
    main()
