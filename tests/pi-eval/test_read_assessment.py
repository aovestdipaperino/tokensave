"""Refs #834. Synthetic assessment fixtures never justify actual enforcement."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts" / "pi-eval"))
from assess_reads import ASSERTIONS, FALLBACKS, SCENARIOS, assess


class ReadEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.review = {"schema": 1, "reviewer": "offline-test-reviewer", "independent": True,
                       "real_model_evaluation": True,
                       "runs": [], "reads": []}
        for repeat in [0, 1]:
            for scenario in SCENARIOS:
                for arm in ["baseline", "enabled"]:
                    name = f"{repeat}-{scenario}-{arm}"
                    directory = self.root / name
                    directory.mkdir()
                    metadata = {"status": "completed", "repeat": repeat, "scenario": scenario,
                                "arm": arm, "provider": "synthetic-not-real-evidence", "model": "fixture",
                                "pi_version": "1.4.0", "pi_source_commit": "source-pin",
                                "tokensave_commit": "tokensave-pin", "tokensave_binary_sha256": "binary-hash",
                                "interface": "builtin-direct", "compacted": True,
                                "session_stats": {"tokens": {"input": 10}}}
                    (directory / "run.json").write_text(json.dumps(metadata))
                    context = {"wall_time_ms": 10, "systemMessages": [{"role": "system", "content": "base",
                               "sections": {"tokensave_retrieval": "guidance"} if arm == "enabled" else {},
                               "toolsAdded": [{"name": "read"}, {"name": "custom_retrieval_name"}]}]}
                    (directory / "contexts.jsonl").write_text(json.dumps(context) + "\n")
                    events = [
                        {"wall_time_ms": 11, "event": {"type": "tool_execution_start", "toolCallId": "graph",
                         "toolName": "custom_retrieval_name", "args": {"path": "keys.py"}}},
                        {"wall_time_ms": 12, "event": {"type": "tool_execution_end", "toolCallId": "graph", "isError": False,
                         "result": {"content": [{"type": "text", "text": "source body"}]}}},
                        {"wall_time_ms": 20, "event": {"type": "tool_execution_start", "toolCallId": "native",
                         "toolName": "read", "args": {"path": "keys.py"}}},
                        {"wall_time_ms": 21, "event": {"type": "tool_execution_end", "toolCallId": "native", "isError": False,
                         "result": {"content": [{"type": "text", "text": "source body"}]}}},
                    ]
                    (directory / "events.jsonl").write_text("".join(json.dumps(event) + "\n" for event in events))
                    self.review["runs"].append({"run": name, "correct": True, "fallback_ok": True})
                    if arm == "enabled" and scenario == "navigation":
                        self.review["reads"].append({"run": name, "call_id": "native", "path": "keys.py",
                            "alternative_call_id": "graph", "alternative_tool": "custom_retrieval_name",
                            "classification": "unnecessary-exploration", "reason": "Synthetic evidence fixture only",
                            **{field: True for field in ASSERTIONS}, "verification": False, "recovery": False})

    def test_positive_evidence_only_advances_to_design_review(self):
        result = assess(self.root, self.review)
        self.assertEqual(result["decision"], "eligible-for-design-review")
        self.assertEqual(len(result["candidates"]), 2)
        self.assertFalse(result["enforcement_enabled"])

    def test_every_fallback_category_is_preserved(self):
        for classification in FALLBACKS:
            with self.subTest(classification=classification):
                review = copy.deepcopy(self.review)
                for entry in review["reads"]:
                    entry["classification"] = classification
                result = assess(self.root, review)
                self.assertEqual(result["decision"], "defer")
                self.assertEqual(result["candidates"], [])

    def test_uncertain_index_coverage_range_or_interface_fails_open(self):
        for field in [*ASSERTIONS, "verification", "recovery"]:
            with self.subTest(field=field):
                review = copy.deepcopy(self.review)
                for entry in review["reads"]:
                    entry.pop(field)
                self.assertEqual(assess(self.root, review)["candidates"], [])

    def test_no_evidence_or_review_never_justifies_interception(self):
        with tempfile.TemporaryDirectory() as directory:
            result = assess(Path(directory), {})
        self.assertEqual(result["decision"], "defer")
        self.assertFalse(result["enforcement_enabled"])

    def test_missing_declared_tool_or_observation_rejects_invented_names(self):
        for entry in self.review["reads"]:
            entry["alternative_tool"] = "invented_tool"
        self.assertEqual(assess(self.root, self.review)["candidates"], [])

    def test_retrieval_error_or_late_success_keeps_native_recovery(self):
        for mode in ["error", "late", "empty"]:
            with self.subTest(mode=mode):
                for entry in self.review["reads"]:
                    path = self.root / entry["run"] / "events.jsonl"
                    rows = [json.loads(line) for line in path.read_text().splitlines()]
                    rows[1]["event"]["isError"] = mode == "error"
                    rows[1]["wall_time_ms"] = 25 if mode == "late" else 12
                    rows[1]["event"]["result"] = {"content": []} if mode == "empty" else {"content": [{"type": "text", "text": "body"}]}
                    path.write_text("".join(json.dumps(row) + "\n" for row in rows))
                self.assertEqual(assess(self.root, self.review)["candidates"], [])

    def test_proxy_uses_the_declared_interface_instead_of_invented_namespace(self):
        for entry in self.review["reads"]:
            directory = self.root / entry["run"]
            path = directory / "contexts.jsonl"
            context = json.loads(path.read_text())
            context["systemMessages"][0]["toolsAdded"] = [{"name": "read"}, {"name": "mcp"}]
            path.write_text(json.dumps(context) + "\n")
            path = directory / "events.jsonl"
            rows = [json.loads(line) for line in path.read_text().splitlines()]
            rows[0]["event"]["toolName"] = "mcp"
            rows[0]["event"]["args"] = {"tool": "tokensave_read", "args": {"path": "keys.py"}}
            path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            entry["alternative_tool"] = "mcp"
        result = assess(self.root, self.review)
        self.assertEqual(result["decision"], "eligible-for-design-review")
        self.assertEqual({row["alternative_tool"] for row in result["candidates"]}, {"mcp"})

    def test_missing_pair_or_compaction_or_usage_defers_decision(self):
        for field in ["compacted", "session_stats", "model"]:
            with self.subTest(field=field):
                path = self.root / "0-long-enabled" / "run.json"
                original = path.read_text()
                metadata = json.loads(original)
                metadata.pop(field)
                path.write_text(json.dumps(metadata))
                self.assertEqual(assess(self.root, self.review)["decision"], "defer")
                path.write_text(original)
        (self.root / "0-docs-baseline" / "run.json").unlink()
        self.assertEqual(assess(self.root, self.review)["decision"], "defer")

    def test_repeated_annotations_of_one_read_are_not_repeated_evidence(self):
        self.review["reads"] = [self.review["reads"][0]] * 3
        result = assess(self.root, self.review)
        self.assertEqual(result["decision"], "defer")
        self.assertEqual(len(result["candidates"]), 1)

    def test_insufficient_review_does_not_enable_enforcement(self):
        self.review["independent"] = False
        self.review["runs"][0]["correct"] = False
        result = assess(self.root, self.review)
        self.assertEqual(result["decision"], "defer")
        self.assertFalse(result["enforcement_enabled"])


if __name__ == "__main__":
    unittest.main()
