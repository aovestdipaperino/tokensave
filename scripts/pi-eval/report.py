#!/usr/bin/env python3
"""Summarize observed traces; correctness and read eligibility require human review."""
import argparse
from collections import Counter
import json
from pathlib import Path


def records(path):
    if not path.exists():
        return
    if path.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("Trace exceeds 256 MiB")
    with path.open() as source:
        while line := source.readline(4 * 1024 * 1024 + 1):
            if len(line) > 4 * 1024 * 1024:
                raise ValueError("Trace record exceeds 4 MiB")
            yield json.loads(line)


def summarize(directory):
    run = json.loads((directory / "run.json").read_text())
    calls = Counter()
    seen = set()
    errors = []
    for row in records(directory / "events.jsonl"):
        event = row["event"]
        if event.get("type") == "tool_execution_start" and event["toolCallId"] not in seen:
            seen.add(event["toolCallId"])
            calls[event["toolName"]] += 1
        if event.get("type") == "tool_execution_end" and event.get("isError"):
            errors.append(event["toolCallId"])
    contexts = []
    for row in records(directory / "contexts.jsonl"):
        tools = {}
        sections = {}
        text = []
        for message in row["systemMessages"]:
            content = message.get("content", "")
            text.append(content if isinstance(content, str) else "\n".join(
                block.get("text", "") for block in content))
            for name, value in message.get("sections", {}).items():
                if value is None:
                    sections.pop(name, None)
                else:
                    sections[name] = value
            for tool in message.get("toolsRemoved", []):
                tools.pop(tool["name"], None)
            for tool in message.get("toolsAdded", []):
                tools[tool["name"]] = tool
        contexts.append({"sections": sections, "content": "\n\n".join(text),
                         "tools": list(tools.values())})
    expected_guidance = run.get("arm") == "enabled"
    guidance_verified = bool(contexts) and all(
        bool(context["sections"].get("tokensave_retrieval")) == expected_guidance
        for context in contexts)
    return {"run": directory.name, "metadata": run, "calls": dict(calls),
            "tool_errors": errors, "contexts": contexts,
            "guidance_verified": guidance_verified,
            "cost_provenance": "Pi session stats (includes reported compaction usage); estimated pricing, not invoice",
            "correctness": "unreviewed", "native_fallback": "unreviewed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    for directory in sorted(args.directory.iterdir()):
        if directory.is_dir() and (directory / "run.json").exists():
            print(json.dumps(summarize(directory)))


if __name__ == "__main__":
    main()
