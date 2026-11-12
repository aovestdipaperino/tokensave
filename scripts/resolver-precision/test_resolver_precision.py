import importlib.util
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("resolver_precision.py")
SPEC = importlib.util.spec_from_file_location("resolver_precision", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"cannot load resolver precision module from {MODULE_PATH}")
resolver_precision = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(resolver_precision)


class MatchingTests(unittest.TestCase):
    def test_score_sites_counts_precision_and_recall(self):
        predicted = [
            {"file": "src/a.py", "line": 2, "column": 3, "kind": "calls"},
            {"file": "src/a.py", "line": 4, "column": 3, "kind": "calls"},
            {"file": "src/a.py", "line": 1, "column": 1, "kind": "definition"},
        ]
        oracle = {("src/a.py", 2, 3), ("src/a.py", 6, 3)}
        score = resolver_precision.score_sites(predicted, oracle)
        self.assertEqual(score["true_positive"], 1)
        self.assertEqual(score["false_positive"], 1)
        self.assertEqual(score["false_negative"], 1)
        self.assertEqual(score["precision"], 0.5)
        self.assertEqual(score["recall"], 0.5)

    def test_unlocated_sites_are_not_predictions(self):
        score = resolver_precision.score_sites(
            [{"file": "a.rs", "line": 1, "column": 0, "kind": "calls"}], set()
        )
        self.assertEqual(score["predicted"], 0)
        self.assertIsNone(score["precision"])

    def test_aggregate_is_micro_averaged(self):
        rows = [
            {"language": "rust", "edge_kind": "calls", "predicted": 2, "oracle": 2,
             "true_positive": 1, "false_positive": 1, "false_negative": 1},
            {"language": "rust", "edge_kind": "calls", "predicted": 1, "oracle": 1,
             "true_positive": 1, "false_positive": 0, "false_negative": 0},
        ]
        result = resolver_precision.aggregate(rows, ("language", "edge_kind"))[0]
        self.assertEqual(result["precision"], 0.666667)
        self.assertEqual(result["recall"], 0.666667)

    def test_fixture_digest_is_path_and_content_stable(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / "b").mkdir()
            (root / "b" / "x.txt").write_text("x", encoding="utf-8")
            first = resolver_precision.fixture_digest(root)
            self.assertEqual(first, resolver_precision.fixture_digest(root))
            (root / "b" / "x.txt").write_text("y", encoding="utf-8")
            self.assertNotEqual(first, resolver_precision.fixture_digest(root))

    def test_lsp_utf16_column_converts_to_one_based_byte_column(self):
        with tempfile.TemporaryDirectory() as name:
            source = Path(name) / "unicode.py"
            source.write_text("é target\n", encoding="utf-8")
            self.assertEqual(resolver_precision.lsp_column_to_byte(source, 0, 2), 4)
            self.assertEqual(resolver_precision.byte_column_to_lsp(source, 1, 4), 2)

    def test_normalized_locations_remove_temporary_uri(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = root / "sample.py"
            source.write_text("target()\n", encoding="utf-8")
            result = [{"uri": source.resolve().as_uri(),
                       "range": {"start": {"line": 0, "character": 0}}}]
            self.assertEqual(
                resolver_precision.normalized_locations(result, root),
                [{"file": "sample.py", "line": 1, "column": 1}],
            )

    def test_rename_sites_flattens_files_and_text_only(self):
        plan = {
            "files": [{"file": "a.rs", "sites": [{"kind": "calls"}]}],
            "text_only": [{"kind": "comment"}],
        }
        self.assertEqual(
            [site["kind"] for site in resolver_precision.rename_sites(plan)],
            ["calls", "comment"],
        )

    def test_executable_command_rejects_missing_program(self):
        with self.assertRaises(resolver_precision.HarnessError):
            resolver_precision.executable_command(["definitely-not-a-real-tokensave-program"])


if __name__ == "__main__":
    unittest.main()
