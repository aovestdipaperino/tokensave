"""Exercise the launcher without starting an agent or connecting to a catalog."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("compose.py")


class ComposeTest(unittest.TestCase):
    def test_launches_both_servers_in_the_current_project(self):
        with tempfile.TemporaryDirectory(prefix="ontology compose ") as temp:
            project = Path(temp)
            claude = project / "claude"
            claude.write_text(
                f"#!{sys.executable}\nimport json, os, sys\n"
                "print(json.dumps([os.getcwd(), sys.argv[1:]]))\nsys.exit(17)\n",
                encoding="utf-8",
            )
            claude.chmod(0o700)
            tokensave = project / "tokensave"
            tokensave.touch(mode=0o700)
            result = self.run_launcher("http://127.0.0.1:3003/mcp", project)

        self.assertEqual(result.returncode, 17, result.stderr)
        cwd, args = json.loads(result.stdout)
        self.assertEqual(cwd, temp)
        self.assertEqual(args[:2], ["--strict-mcp-config", "--mcp-config"])
        self.assertEqual(json.loads(args[2]), {"mcpServers": {
            "tokensave": {"type": "stdio", "command": str(tokensave), "args": ["serve"]},
            "auto-ontology": {"type": "http", "url": "http://127.0.0.1:3003/mcp"},
        }})
        self.assertEqual(args[3], "--settings")
        self.assertEqual(json.loads(args[4]), {
            "permissions": {"ask": ["mcp__auto-ontology__*"]},
        })
        self.assertEqual(args[5:8], ["--permission-mode", "default", "--append-system-prompt"])
        self.assertIn("Ask for explicit approval", args[8])

    def test_rejects_unusable_or_credential_bearing_endpoints(self):
        with tempfile.TemporaryDirectory() as temp:
            for url in (
                "http://catalog.example/mcp",
                "https://user:password@catalog.example/mcp",
                "https://catalog.example/mcp?token=secret",
                "https://catalog.example/mcp#secret",
                "https://catalog.example",
                "https://catalog.example:wrong/mcp",
            ):
                with self.subTest(url=url):
                    result = self.run_launcher(url, Path(temp))
                    self.assertEqual(result.returncode, 2)
                    self.assertIn("use an HTTPS /mcp URL", result.stderr)
                    self.assertNotIn(url, result.stderr)

    def test_reports_missing_clients(self):
        with tempfile.TemporaryDirectory() as temp:
            result = self.run_launcher("https://catalog.example/mcp", Path(temp))
        self.assertEqual(result.returncode, 2)
        self.assertIn("both claude and tokensave must be on PATH", result.stderr)

    @staticmethod
    def run_launcher(url, directory):
        return subprocess.run(
            [sys.executable, SCRIPT, "--url", url],
            cwd=directory,
            env={**os.environ, "PATH": str(directory)},
            capture_output=True,
            text=True,
            timeout=5,
        )


if __name__ == "__main__":
    unittest.main()
