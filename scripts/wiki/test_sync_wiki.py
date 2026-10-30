"""Tests for sync_wiki.py. Run: python3 -m unittest discover -s scripts/wiki"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import sync_wiki  # noqa: E402

INDEX = """# Docs

<!-- wiki:omit-start -->
Repository-only note.
<!-- wiki:omit-end -->

<!-- wiki-sidebar:start -->
- [First Page](FIRST.md): one
- [Second](SECOND.md): two
<!-- wiki-sidebar:end -->
"""

FIRST = """# First Page

See [the second page](SECOND.md#details), [the user guide](../USER-GUIDE.md#intro),
[the repository](../../README.md) and [elsewhere](https://example.com).

<img src="../../src/logo.png" alt="logo">

`[inline](NOT-A-LINK.md)`

```bash
# Not a heading [nor a link](MISSING.md)
```
"""

SECOND = """# Second

## Details

## Details
"""


class RepoFixture:
    def __init__(self, files: dict[str, str]):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        for name, text in files.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
        (self.root / "src").mkdir(exist_ok=True)
        (self.root / "src" / "logo.png").write_bytes(b"png")

    def close(self):
        self.tmp.cleanup()


def valid_files() -> dict[str, str]:
    return {
        "README.md": "# Project\n\n[Docs](docs/guide/README.md)\n",
        "CONTRIBUTING.md": "# Contributing\n",
        "docs/USER-GUIDE.md": "# Guide\n\n## Intro\n",
        "docs/guide/README.md": INDEX,
        "docs/guide/FIRST.md": FIRST,
        "docs/guide/SECOND.md": SECOND,
    }


class SlugTest(unittest.TestCase):
    def test_github_slugs(self):
        self.assertEqual(sync_wiki.github_slug("Versioning & upgrades"), "versioning--upgrades")
        self.assertEqual(sync_wiki.github_slug("`tokensave doctor`"), "tokensave-doctor")
        self.assertEqual(sync_wiki.github_slug("80+ MCP Tools"), "80-mcp-tools")
        self.assertEqual(sync_wiki.github_slug('"tokensave not initialized"'), "tokensave-not-initialized")

    def test_duplicate_headings_get_suffixes_and_fences_are_ignored(self):
        text = "# A\n\n## B\n\n## B\n\n```\n# C\n```\n"
        self.assertEqual(sync_wiki.anchors(text), {"a", "b", "b-1"})

    def test_wiki_name(self):
        self.assertEqual(sync_wiki.wiki_name("Installation and Upgrades"), "Installation-and-Upgrades")
        self.assertEqual(sync_wiki.wiki_name("MCP Tools"), "MCP-Tools")


class CheckTest(unittest.TestCase):
    def run_check(self, files):
        fixture = RepoFixture(files)
        try:
            return sync_wiki.check(fixture.root)
        finally:
            fixture.close()

    def test_valid_tree_passes(self):
        self.assertEqual(self.run_check(valid_files()), [])

    def test_missing_file_and_anchor_are_reported(self):
        files = valid_files()
        files["docs/guide/SECOND.md"] = SECOND + "\n[gone](GONE.md) [bad](FIRST.md#nowhere)\n"
        errors = self.run_check(files)
        self.assertTrue(any("missing link target: GONE.md" in e for e in errors), errors)
        self.assertTrue(any("missing anchor #nowhere" in e for e in errors), errors)

    def test_page_missing_from_sidebar_is_reported(self):
        files = valid_files()
        files["docs/guide/THIRD.md"] = "# Third\n"
        errors = self.run_check(files)
        self.assertTrue(any("THIRD.md exactly once" in e for e in errors), errors)

    def test_page_without_title_is_reported(self):
        files = valid_files()
        files["docs/guide/SECOND.md"] = "## Details\n"
        errors = self.run_check(files)
        self.assertTrue(any("no '# Title'" in e for e in errors), errors)


class BuildTest(unittest.TestCase):
    def setUp(self):
        self.fixture = RepoFixture(valid_files())
        self.out = self.fixture.root / "wiki"
        self.out.mkdir()
        (self.out / "Stale-Page.md").write_text("old\n")
        self.written = sync_wiki.build(self.fixture.root, self.out, "owner/repo", "main", "0123456789abcdef")

    def tearDown(self):
        self.fixture.close()

    def read(self, name):
        return (self.out / name).read_text(encoding="utf-8")

    def test_pages_are_named_after_titles_and_stale_pages_removed(self):
        self.assertEqual(
            sorted(self.written),
            ["First-Page.md", "Home.md", "Second.md", "_Footer.md", "_Sidebar.md"],
        )
        self.assertFalse((self.out / "Stale-Page.md").exists())

    def test_links_are_rewritten(self):
        page = self.read("First-Page.md")
        self.assertIn("[the second page](Second#details)", page)
        self.assertIn("(https://github.com/owner/repo/blob/main/docs/USER-GUIDE.md#intro)", page)
        self.assertIn("(https://github.com/owner/repo/blob/main/README.md)", page)
        self.assertIn("(https://example.com)", page)
        self.assertIn('src="https://raw.githubusercontent.com/owner/repo/main/src/logo.png"', page)

    def test_code_is_left_alone(self):
        page = self.read("First-Page.md")
        self.assertIn("`[inline](NOT-A-LINK.md)`", page)
        self.assertIn("[nor a link](MISSING.md)", page)

    def test_omitted_text_sidebar_and_footer(self):
        self.assertNotIn("Repository-only note", self.read("Home.md"))
        self.assertIn("- [First Page](First-Page)", self.read("_Sidebar.md"))
        self.assertIn("- [Second](Second)", self.read("_Sidebar.md"))
        self.assertIn("commit/0123456789abcdef", self.read("_Footer.md"))


if __name__ == "__main__":
    unittest.main()
