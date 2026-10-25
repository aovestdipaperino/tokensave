#!/usr/bin/env python3
"""
Promote `## [Unreleased]` content into `## [<version>]` in CHANGELOG.md
so the release workflow's release-notes extraction picks up everything
that landed since the last release.

Why this exists
---------------
The release workflow looks for a `## [<version>]` block to extract notes
from. Maintainers sometimes hand-create a sparse `## [<version>]` block
in advance (e.g. one early fix documented before the rest of the work
lands). Without this script, the workflow extracts that sparse block and
silently ignores the much-larger `[Unreleased]` section above it. Ported
from codegraph's `scripts/prepare-release.mjs` (#436) after they hit the
same failure on v0.9.5.

What it does, idempotently
--------------------------
  Case A — `[<version>]` does not exist yet:
    Rename the `[Unreleased]` header to `[<version>] - <YYYY-MM-DD>` and
    add a fresh empty `## [Unreleased]` block above it. Common case.

  Case B — `[<version>]` exists AND `[Unreleased]` has content:
    Merge `[Unreleased]`'s sub-sections (### Added / ### Fixed /
    ### Changed / ### Removed / ### Deprecated / ### Security) into the
    corresponding sub-sections of `[<version>]`. Unmatched sub-sections
    are appended. The `[Unreleased]` block is then emptied.

  Case C — `[Unreleased]` has no content:
    No-op. Exit 0. Re-runs of the workflow are safe.

Also appends a `[X.Y.Z]: https://github.com/aovestdipaperino/tokensave/releases/tag/vX.Y.Z`
link reference at the bottom of the file when missing (idempotent).

Usage
-----
  scripts/prepare-release.py             # reads version from Cargo.toml
  scripts/prepare-release.py 1.2.3       # explicit version

Output
------
Writes CHANGELOG.md in place. Prints a summary line to stdout like
`prepare-release: 0.9.5 - promoted 6 Unreleased entries`. Exits non-zero
on parse failures.
"""
from __future__ import annotations

import datetime
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CHANGELOG_PATH = REPO_ROOT / "CHANGELOG.md"
CARGO_TOML_PATH = REPO_ROOT / "Cargo.toml"
GITHUB_URL = "https://github.com/aovestdipaperino/tokensave"

VERSION_HEADER_RE = re.compile(r"^## \[([^\]]+)\](?:\s+-\s+(.+))?\s*$")
SUBSECTION_RE = re.compile(r"^### (\w+)\s*$")
BULLET_RE = re.compile(r"^\s*([-*]|\d+\.)\s+")


def read_cargo_version() -> str:
    """Read the [package].version field from Cargo.toml."""
    text = CARGO_TOML_PATH.read_text(encoding="utf-8")
    # Find [package] section then walk to the first `version = "..."`.
    in_package = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_package = stripped == "[package]"
            continue
        if not in_package:
            continue
        m = re.match(r'version\s*=\s*"([^"]+)"', stripped)
        if m:
            return m.group(1)
    raise SystemExit("prepare-release: Cargo.toml [package].version not found")


def today_utc_iso() -> str:
    """YYYY-MM-DD in UTC, matching the existing CHANGELOG date format."""
    return datetime.datetime.now(datetime.timezone.utc).date().isoformat()


def parse_changelog(text: str):
    """Split CHANGELOG into preface + ordered list of version blocks."""
    lines = text.split("\n")
    preface: list[str] = []
    blocks: list[dict] = []
    cur: dict | None = None
    for line in lines:
        m = VERSION_HEADER_RE.match(line)
        if m:
            if cur:
                blocks.append(cur)
            cur = {
                "header": line,
                "name": m.group(1),
                "date": m.group(2),
                "body": [],
            }
        elif cur is not None:
            cur["body"].append(line)
        else:
            preface.append(line)
    if cur:
        blocks.append(cur)
    return preface, blocks


def join_changelog(preface: list[str], blocks: list[dict]) -> str:
    parts = ["\n".join(preface)]
    for b in blocks:
        parts.append("\n".join([b["header"], *b["body"]]))
    return "\n".join(parts)


def split_subsections(body: list[str]):
    """Split a block body into ordered sub-sections keyed by `### Heading`."""
    leading: list[str] = []
    subs: list[dict] = []
    cur: dict | None = None
    for line in body:
        m = SUBSECTION_RE.match(line)
        if m:
            if cur:
                subs.append(cur)
            cur = {"heading": m.group(1), "header_line": line, "body": []}
        elif cur is not None:
            cur["body"].append(line)
        else:
            leading.append(line)
    if cur:
        subs.append(cur)
    return leading, subs


def rebuild_body(leading: list[str], subs: list[dict]) -> list[str]:
    parts: list[str] = []
    if leading:
        parts.append("\n".join(leading))
    for s in subs:
        parts.append("\n".join([s["header_line"], *s["body"]]))
    return "\n".join(parts).split("\n")


def block_has_content(body: list[str]) -> bool:
    """True when the block has any bullet-shaped entries."""
    return any(BULLET_RE.match(line) for line in body)


def trim_trailing_blank(arr: list[str]) -> list[str]:
    i = len(arr)
    while i > 0 and not arr[i - 1].strip():
        i -= 1
    return arr[:i]


def append_link_ref(text: str, version: str) -> str:
    """Append `[X.Y.Z]: <url>` at file end if not already present."""
    ref = f"[{version}]: {GITHUB_URL}/releases/tag/v{version}"
    lines = text.split("\n")
    if any(line.strip() == ref for line in lines):
        return text
    trailing = "" if text.endswith("\n") else "\n"
    return text + trailing + ref + "\n"


def main() -> int:
    version = sys.argv[1] if len(sys.argv) > 1 else read_cargo_version()

    text = CHANGELOG_PATH.read_text(encoding="utf-8")
    preface, blocks = parse_changelog(text)

    unrel_idx = next(
        (i for i, b in enumerate(blocks) if b["name"] == "Unreleased"), -1
    )
    ver_idx = next(
        (i for i, b in enumerate(blocks) if b["name"] == version), -1
    )

    if unrel_idx == -1:
        print("prepare-release: no [Unreleased] block - nothing to do")
        return 0

    unrel = blocks[unrel_idx]
    if not block_has_content(unrel["body"]):
        print("prepare-release: [Unreleased] is empty - nothing to do")
        return 0

    if ver_idx == -1:
        # Case A — promote Unreleased -> [version].
        today = today_utc_iso()
        promoted = {
            "header": f"## [{version}] - {today}",
            "name": version,
            "date": today,
            "body": trim_trailing_blank(unrel["body"]) + [""],
        }
        emptied = {
            "header": "## [Unreleased]",
            "name": "Unreleased",
            "date": None,
            "body": ["", ""],
        }
        blocks[unrel_idx : unrel_idx + 1] = [emptied, promoted]
        out = append_link_ref(join_changelog(preface, blocks), version)
        CHANGELOG_PATH.write_text(out, encoding="utf-8")
        n = sum(1 for line in promoted["body"] if BULLET_RE.match(line))
        print(
            f"prepare-release: {version} - renamed [Unreleased] to "
            f"[{version}] - {today} ({n} entries promoted)"
        )
        return 0

    # Case B — merge Unreleased sub-sections into the existing [version].
    ver = blocks[ver_idx]
    _, unrel_subs = split_subsections(unrel["body"])
    ver_leading, ver_subs = split_subsections(ver["body"])

    merged = 0
    for us in unrel_subs:
        us_body = trim_trailing_blank(us["body"])
        if not us_body:
            continue
        target = next(
            (s for s in ver_subs if s["heading"] == us["heading"]), None
        )
        if target:
            existing = trim_trailing_blank(target["body"])
            sep = (
                [""]
                if existing and existing[-1].strip()
                else []
            )
            target["body"] = existing + sep + us_body + [""]
        else:
            ver_subs.append(
                {
                    "heading": us["heading"],
                    "header_line": us["header_line"],
                    "body": us_body + [""],
                }
            )
        merged += sum(1 for line in us_body if BULLET_RE.match(line))

    ver["body"] = rebuild_body(ver_leading, ver_subs)
    unrel["body"] = ["", ""]

    out = append_link_ref(join_changelog(preface, blocks), version)
    CHANGELOG_PATH.write_text(out, encoding="utf-8")
    print(
        f"prepare-release: {version} - merged {merged} Unreleased entries "
        f"into existing [{version}] block"
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except SystemExit:
        raise
    except Exception as err:
        print(f"prepare-release: {err}", file=sys.stderr)
        sys.exit(1)
