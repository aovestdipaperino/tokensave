"""Unit tests of the pure helpers in blaze_bench.py (#984). No builds run."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import blaze_bench  # noqa: E402

SOURCE = """impl X {
    pub async fn find_stale_files(&self) -> Vec<String> {
        let files = self.files();
        files
    }
}
"""
ANCHOR = "pub async fn find_stale_files(&self) -> Vec<String> {"


class ApplyEditTest(unittest.TestCase):
    def test_inserts_a_statement_after_the_anchor(self):
        edited = blaze_bench.apply_edit(SOURCE, ANCHOR, 3)
        self.assertIn(
            ANCHOR + "\n        let _blaze_bench_edit: u64 = std::hint::black_box(3);\n"
            "        let files = self.files();",
            edited,
        )

    def test_replaces_a_previous_edit(self):
        once = blaze_bench.apply_edit(SOURCE, ANCHOR, 1)
        twice = blaze_bench.apply_edit(once, ANCHOR, 2)
        self.assertEqual(twice.count("_blaze_bench_edit"), 1)
        self.assertIn("black_box(2)", twice)
        self.assertEqual(twice, blaze_bench.apply_edit(SOURCE, ANCHOR, 2))

    def test_each_counter_changes_the_text(self):
        self.assertNotEqual(blaze_bench.apply_edit(SOURCE, ANCHOR, 1),
                            blaze_bench.apply_edit(SOURCE, ANCHOR, 2))

    def test_missing_anchor(self):
        with self.assertRaises(ValueError):
            blaze_bench.apply_edit(SOURCE, "fn missing() {", 1)

    def test_ambiguous_anchor(self):
        with self.assertRaises(ValueError):
            blaze_bench.apply_edit(SOURCE + SOURCE, ANCHOR, 1)

    def test_default_anchor_exists_in_the_repository(self):
        root = Path(__file__).resolve().parents[2]
        text = (root / blaze_bench.DEFAULT_EDIT_FILE).read_text(encoding="utf-8")
        edited = blaze_bench.apply_edit(text, blaze_bench.DEFAULT_EDIT_ANCHOR, 1)
        self.assertEqual(edited.count("_blaze_bench_edit"), 1)


class SummaryTest(unittest.TestCase):
    def test_summarize(self):
        self.assertIsNone(blaze_bench.summarize([]))
        self.assertEqual(blaze_bench.summarize([3.0, 1.0, 2.0]),
                         {"median": 2.0, "min": 1.0, "max": 3.0, "n": 3})

    def test_formatting(self):
        self.assertEqual(blaze_bench.format_seconds(0.0564), "56 ms")
        self.assertEqual(blaze_bench.format_seconds(12.345), "12.35 s")
        self.assertEqual(blaze_bench.format_seconds(None), "n/a")
        self.assertEqual(blaze_bench.format_mib(512), "512 MiB")
        self.assertEqual(blaze_bench.format_mib(2048), "2.00 GiB")


def step(name, ok=True, wall=1.0, exit_code=0):
    return {"name": name, "ok": ok, "exit_code": exit_code, "wall_s": wall,
            "cpu_s": wall * 2, "peak_rss_mib": 100.0, "error_tail": "boom"}


class RenderTest(unittest.TestCase):
    def result(self):
        return {
            "revision": "abc123",
            "features": "lite",
            "environment": {"cpus": 8, "mem_total_mib": 11000, "kernel": "6.x",
                            "cargo_version": "cargo 1.100.0",
                            "blaze_version": "blaze 0.1"},
            "tools": {
                "cargo": {"steps": [step("cold-build", wall=100.0),
                                    step("edit-build", wall=2.0),
                                    step("edit-build", wall=4.0)],
                          "disk": {"target_after_cold_mib": 2048.0},
                          "status": None},
                "blaze": {"steps": [step("cold-build", ok=False, exit_code=101)],
                          "disk": {}, "status": "served 0, cargo 1"},
            },
        }

    def test_rows_and_failures(self):
        report = blaze_bench.render_markdown(self.result())
        self.assertIn("| cold-build | cargo | 1/1 | 100.00 s |", report)
        self.assertIn("| edit-build | cargo | 2/2 | 3.00 s (2.00 s-4.00 s) |", report)
        self.assertIn("| cold-build | blaze | 0/1 | failed |", report)
        self.assertIn("blaze `cold-build` exit 101", report)
        self.assertIn("served 0, cargo 1", report)
        self.assertIn("| cargo | 2.00 GiB |", report)


if __name__ == "__main__":
    unittest.main()
