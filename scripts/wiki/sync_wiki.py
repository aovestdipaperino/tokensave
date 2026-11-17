#!/usr/bin/env python3
"""
Check the user documentation in docs/guide/ and generate the GitHub Wiki
from it.

Why this exists
---------------
The documentation is published twice: as Markdown in docs/guide/ (versioned
with the code and reviewed in pull requests) and as the GitHub Wiki (easy to
browse). Keeping two hand-edited copies aligned does not work, so docs/guide/
is the single source and the wiki is generated from it. The wiki workflow
runs `build` and pushes the result on every change to master; nobody edits
the wiki directly.

Commands
--------
  check
    Validate every relative link and `#anchor` in README.md, CONTRIBUTING.md
    and docs/guide/*.md, and verify that the sidebar block in
    docs/guide/README.md lists every guide page exactly once.

  build --out DIR [--repo OWNER/NAME] [--ref REF] [--commit SHA]
    Write the wiki pages to DIR (created if needed; existing *.md files in it
    are replaced). Each guide page becomes a wiki page named after its `#`
    title, docs/guide/README.md becomes Home, and _Sidebar.md and _Footer.md
    are generated. Links between guide pages become wiki links; links to other
    repository files become absolute GitHub URLs on REF; images point at
    raw.githubusercontent.com.

Markup understood in guide pages
--------------------------------
  <!-- wiki:omit-start --> ... <!-- wiki:omit-end -->
    Text that only makes sense in the repository; dropped from the wiki.
  <!-- wiki-sidebar:start --> ... <!-- wiki-sidebar:end -->
    (docs/guide/README.md only) the ordered page list used for _Sidebar.md.

Only the Python standard library is used.
"""

from __future__ import annotations

import argparse
import pathlib
import posixpath
import re
import sys
import urllib.parse

GUIDE_DIR = "docs/guide"
INDEX_PAGE = "README.md"
CHECKED_FILES = ("README.md", "CONTRIBUTING.md")
DEFAULT_REPO = "aovestdipaperino/tokensave"
DEFAULT_REF = "master"

FENCE_RE = re.compile(r"^\s*(```|~~~)")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
# Markdown links and images: [text](target) / ![alt](target). The target
# stops at whitespace so an optional "title" is left alone.
MD_LINK_RE = re.compile(r"(!?)\[((?:[^\[\]]|\[[^\]]*\])*)\]\(\s*<?([^)\s>]+)>?((?:\s+\"[^\"]*\")?)\s*\)")
HTML_ATTR_RE = re.compile(r"\b(src|srcset|href)=\"([^\"]+)\"")
HTML_ID_RE = re.compile(r"\b(?:id|name)=\"([^\"]+)\"")
SIDEBAR_RE = re.compile(r"<!-- wiki-sidebar:start -->(.*?)<!-- wiki-sidebar:end -->", re.S)
OMIT_RE = re.compile(r"\n?<!-- wiki:omit-start -->.*?<!-- wiki:omit-end -->\n?", re.S)
SCHEME_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")
CODE_SPAN_RE = re.compile(r"(`+)(?:(?!\1).)+?\1")


def outside_code_spans(line: str, transform) -> str:
    """Apply transform to the parts of a line that are not inline code."""
    out, last = [], 0
    for span in CODE_SPAN_RE.finditer(line):
        out.append(transform(line[last:span.start()]))
        out.append(span.group(0))
        last = span.end()
    out.append(transform(line[last:]))
    return "".join(out)


def github_slug(heading: str) -> str:
    """Anchor GitHub generates for a heading's text."""
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", heading)  # [x](y) -> x
    text = re.sub(r"<[^>]+>", "", text)  # inline HTML tags
    text = text.strip().lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def outside_fences(text: str):
    """Yield (line_number, line) for lines outside fenced code blocks."""
    fence = None
    for number, line in enumerate(text.split("\n"), 1):
        match = FENCE_RE.match(line)
        if match:
            if fence is None:
                fence = match.group(1)
            elif match.group(1) == fence:
                fence = None
            continue
        if fence is None:
            yield number, line


def anchors(text: str) -> set[str]:
    """Every anchor a Markdown file defines, with GitHub's duplicate suffixes."""
    seen: dict[str, int] = {}
    result = set()
    for _, line in outside_fences(text):
        heading = HEADING_RE.match(line)
        if heading:
            slug = github_slug(heading.group(2))
            count = seen.get(slug, 0)
            seen[slug] = count + 1
            result.add(slug if count == 0 else f"{slug}-{count}")
        result.update(HTML_ID_RE.findall(line))
    return result


def page_title(text: str) -> str | None:
    for _, line in outside_fences(text):
        heading = HEADING_RE.match(line)
        if heading and len(heading.group(1)) == 1:
            return heading.group(2)
    return None


def wiki_name(title: str) -> str:
    """Wiki page name for a title: GitHub shows hyphens as spaces."""
    name = re.sub(r"[^\w\- ]", "", title).strip()
    return re.sub(r"\s+", "-", name)


def is_external(target: str) -> bool:
    return bool(SCHEME_RE.match(target)) or target.startswith("//")


def split_target(target: str) -> tuple[str, str]:
    path, _, fragment = target.partition("#")
    return urllib.parse.unquote(path), fragment


def links(text: str):
    """Yield (line_number, target, is_image) for every link outside fences."""
    for number, line in outside_fences(text):
        line = CODE_SPAN_RE.sub("", line)
        for match in MD_LINK_RE.finditer(line):
            yield number, match.group(3), match.group(1) == "!"
        for match in HTML_ATTR_RE.finditer(line):
            kind, value = match.groups()
            for target in value.split(",") if kind == "srcset" else [value]:
                target = target.strip().split(" ")[0]
                if target:
                    yield number, target, kind != "href"


def guide_pages(root: pathlib.Path) -> list[pathlib.Path]:
    return sorted((root / GUIDE_DIR).glob("*.md"))


def sidebar_entries(index_text: str) -> list[str]:
    match = SIDEBAR_RE.search(index_text)
    if not match:
        return []
    return [target for _, target, _ in links(match.group(1))]


def check(root: pathlib.Path) -> list[str]:
    errors = []
    files = [root / name for name in CHECKED_FILES] + guide_pages(root)
    anchor_cache: dict[pathlib.Path, set[str]] = {}

    def anchors_of(path: pathlib.Path) -> set[str]:
        if path not in anchor_cache:
            anchor_cache[path] = anchors(path.read_text(encoding="utf-8"))
        return anchor_cache[path]

    for source in files:
        if not source.exists():
            continue
        rel = source.relative_to(root).as_posix()
        text = source.read_text(encoding="utf-8")
        for number, target, _ in links(text):
            if is_external(target):
                continue
            path, fragment = split_target(target)
            if path:
                resolved = (source.parent / path).resolve()
                if not resolved.is_relative_to(root.resolve()):
                    errors.append(f"{rel}:{number}: link leaves the repository: {target}")
                    continue
                if not resolved.exists():
                    errors.append(f"{rel}:{number}: missing link target: {target}")
                    continue
            else:
                resolved = source.resolve()
            if fragment and resolved.suffix == ".md" and fragment not in anchors_of(resolved):
                errors.append(f"{rel}:{number}: missing anchor #{fragment} in {target or rel}")

    guide = root / GUIDE_DIR
    index = guide / INDEX_PAGE
    if not index.exists():
        return errors + [f"{GUIDE_DIR}/{INDEX_PAGE}: missing guide index"]
    listed = [split_target(target)[0] for target in sidebar_entries(index.read_text(encoding="utf-8"))]
    if not listed:
        errors.append(f"{GUIDE_DIR}/{INDEX_PAGE}: no wiki-sidebar block")
    pages = [page.name for page in guide_pages(root) if page.name != INDEX_PAGE]
    for name in pages:
        if listed.count(name) != 1:
            errors.append(f"{GUIDE_DIR}/{INDEX_PAGE}: sidebar must list {name} exactly once")
    for name in listed:
        if name not in pages:
            errors.append(f"{GUIDE_DIR}/{INDEX_PAGE}: sidebar lists a non-guide page: {name}")
    names: dict[str, str] = {}
    for page in guide_pages(root):
        title = page_title(page.read_text(encoding="utf-8"))
        if not title:
            errors.append(f"{GUIDE_DIR}/{page.name}: no '# Title' heading")
            continue
        name = "Home" if page.name == INDEX_PAGE else wiki_name(title)
        if name in names:
            errors.append(f"{GUIDE_DIR}/{page.name}: wiki page name {name} also used by {names[name]}")
        names[name] = page.name
    return errors


class Converter:
    """Rewrites guide-page links for the wiki."""

    def __init__(self, root: pathlib.Path, repo: str, ref: str):
        self.root = root
        self.repo = repo
        self.ref = ref
        self.page_names = {}
        for page in guide_pages(root):
            if page.name == INDEX_PAGE:
                self.page_names[page.name] = "Home"
            else:
                self.page_names[page.name] = wiki_name(page_title(page.read_text(encoding="utf-8")) or page.stem)

    def target(self, target: str, image: bool) -> str:
        if is_external(target) or target.startswith("#"):
            return target
        path, fragment = split_target(target)
        suffix = f"#{fragment}" if fragment else ""
        repo_path = posixpath.normpath(posixpath.join(GUIDE_DIR, path))
        if posixpath.dirname(repo_path) == GUIDE_DIR and posixpath.basename(repo_path) in self.page_names:
            return self.page_names[posixpath.basename(repo_path)] + suffix
        quoted = urllib.parse.quote(repo_path)
        if image:
            return f"https://raw.githubusercontent.com/{self.repo}/{self.ref}/{quoted}"
        kind = "tree" if (self.root / repo_path).is_dir() else "blob"
        return f"https://github.com/{self.repo}/{kind}/{self.ref}/{quoted}{suffix}"

    def convert(self, text: str) -> str:
        text = OMIT_RE.sub("\n", text)
        out = []
        fence = None
        for line in text.split("\n"):
            match = FENCE_RE.match(line)
            if match:
                if fence is None:
                    fence = match.group(1)
                elif match.group(1) == fence:
                    fence = None
                out.append(line)
                continue
            if fence is None:
                line = outside_code_spans(line, self._rewrite)
            out.append(line)
        return "\n".join(out)

    def _rewrite(self, text: str) -> str:
        text = MD_LINK_RE.sub(
            lambda m: f"{m.group(1)}[{m.group(2)}]({self.target(m.group(3), m.group(1) == '!')}{m.group(4)})",
            text,
        )
        return HTML_ATTR_RE.sub(lambda m: self._attr(m.group(1), m.group(2)), text)

    def _attr(self, kind: str, value: str) -> str:
        if kind == "srcset":
            parts = []
            for item in value.split(","):
                url, _, descriptor = item.strip().partition(" ")
                parts.append(" ".join(filter(None, [self.target(url, True), descriptor])))
            return f'{kind}="{", ".join(parts)}"'
        return f'{kind}="{self.target(value, kind != "href")}"'


def build(root: pathlib.Path, out: pathlib.Path, repo: str, ref: str, commit: str | None) -> list[str]:
    converter = Converter(root, repo, ref)
    out.mkdir(parents=True, exist_ok=True)
    for stale in out.glob("*.md"):
        stale.unlink()
    written = []
    for page in guide_pages(root):
        name = converter.page_names[page.name]
        body = converter.convert(page.read_text(encoding="utf-8")).strip("\n")
        header = f"<!-- Generated from {GUIDE_DIR}/{page.name}. Edit that file, not the wiki. -->"
        (out / f"{name}.md").write_text(f"{header}\n\n{body}\n", encoding="utf-8")
        written.append(f"{name}.md")

    index_text = (root / GUIDE_DIR / INDEX_PAGE).read_text(encoding="utf-8")
    sidebar = ["**[Home](Home)**", ""]
    for target in sidebar_entries(index_text):
        name = split_target(target)[0]
        title = page_title((root / GUIDE_DIR / name).read_text(encoding="utf-8")) or name
        sidebar.append(f"- [{title}]({converter.page_names[name]})")
    (out / "_Sidebar.md").write_text("\n".join(sidebar) + "\n", encoding="utf-8")

    source = f"https://github.com/{repo}/tree/{ref}/{GUIDE_DIR}"
    at = f" at commit [`{commit[:12]}`](https://github.com/{repo}/commit/{commit})" if commit else ""
    footer = (
        f"This wiki is generated from [`{GUIDE_DIR}`]({source}){at}. "
        "Edit the files there in a pull request; direct wiki edits are overwritten."
    )
    (out / "_Footer.md").write_text(footer + "\n", encoding="utf-8")
    return written + ["_Sidebar.md", "_Footer.md"]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", default=pathlib.Path(__file__).resolve().parents[2], type=pathlib.Path,
                        help="repository root (default: the checkout containing this script)")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check", help="validate links, anchors and the sidebar")
    build_parser = commands.add_parser("build", help="generate wiki pages")
    build_parser.add_argument("--out", required=True, type=pathlib.Path)
    build_parser.add_argument("--repo", default=DEFAULT_REPO)
    build_parser.add_argument("--ref", default=DEFAULT_REF)
    build_parser.add_argument("--commit")
    args = parser.parse_args(argv)

    errors = check(args.root)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        print(f"{len(errors)} documentation problem(s) found", file=sys.stderr)
        return 1
    if args.command == "check":
        print("documentation links and wiki sidebar are consistent")
        return 0
    for name in build(args.root, args.out, args.repo, args.ref, args.commit):
        print(name)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
