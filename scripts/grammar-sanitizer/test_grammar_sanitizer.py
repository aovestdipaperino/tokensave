import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("grammar_sanitizer.py")
SPEC = importlib.util.spec_from_file_location("grammar_sanitizer", SCRIPT)
grammar_sanitizer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(grammar_sanitizer)


class GrammarSanitizerTest(unittest.TestCase):
    def test_cargo_test_builds_every_language_and_filters_extraction_tests(self):
        command = grammar_sanitizer.cargo_command(
            "nightly-2026-11-10", "x86_64-unknown-linux-gnu", test=True
        )
        self.assertEqual(command[:3], ["cargo", "+nightly-2026-11-10", "test"])
        self.assertIn("full,test-transport", command)
        self.assertEqual(command[-4:], ["integration", "extraction_test::", "--", "--nocapture"])

    def test_environment_instruments_rust_c_and_cpp(self):
        environment = grammar_sanitizer.sanitizer_environment(Path("/private/target"))
        self.assertIn("-Zsanitizer=address", environment["RUSTFLAGS"])
        self.assertIn("-fsanitize=address", environment["CFLAGS"])
        self.assertEqual(environment["CFLAGS"], environment["CXXFLAGS"])
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(Path("/private/target")))

    def test_work_directory_cannot_be_inside_the_repository(self):
        with tempfile.TemporaryDirectory() as tmp:
            repository = Path(tmp).resolve()
            args = grammar_sanitizer.parse_args(
                [
                    "--work-dir",
                    str(repository / "work"),
                    "--output",
                    str(repository.parent / "result.json"),
                ]
            )
            with self.assertRaisesRegex(ValueError, "outside the repository"):
                grammar_sanitizer.validate_paths(repository, args)

    def test_skip_fixtures_requires_a_corpus(self):
        with tempfile.TemporaryDirectory() as tmp:
            repository = Path(tmp, "repo").resolve()
            repository.mkdir()
            args = grammar_sanitizer.parse_args(
                [
                    "--work-dir",
                    str(Path(tmp, "work")),
                    "--output",
                    str(Path(tmp, "result.json")),
                    "--skip-fixtures",
                ]
            )
            with self.assertRaisesRegex(ValueError, "requires at least one"):
                grammar_sanitizer.validate_paths(repository, args)

    def test_safe_name_is_stable_and_contains_no_path_punctuation(self):
        self.assertEqual(
            grammar_sanitizer.safe_name(Path("/src/my corpus"), 3),
            "corpus-03-my-corpus",
        )


if __name__ == "__main__":
    unittest.main()
