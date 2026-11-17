#!/usr/bin/env python3
"""Launch an interactive Claude Code session with TokenSave and Auto Ontology."""
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True, type=endpoint, help="Auto Ontology MCP endpoint")
    args = parser.parse_args()

    claude = shutil.which("claude")
    tokensave = shutil.which("tokensave")
    if claude is None or tokensave is None:
        parser.error("both claude and tokensave must be on PATH")

    config = {
        "mcpServers": {
            "tokensave": {"type": "stdio", "command": tokensave, "args": ["serve"]},
            "auto-ontology": {"type": "http", "url": args.url},
        }
    }
    permissions = {"permissions": {"ask": ["mcp__auto-ontology__*"]}}
    try:
        os.execv(claude, [
            claude, "--strict-mcp-config", "--mcp-config", json.dumps(config),
            "--settings", json.dumps(permissions),
            "--permission-mode", "default",
            "--append-system-prompt", ROUTING,
        ])
    except OSError as error:
        parser.exit(1, f"cannot launch Claude Code: {error.strerror}\n")


if __name__ == "__main__":
    main()
