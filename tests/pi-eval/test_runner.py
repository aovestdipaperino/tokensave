"""Refs #832. Offline protocol tests are not model-routing evidence."""
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / "pi-eval" / f"{name}.py")
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


runner = module("run")
report = module("report")


class ProtocolTests(unittest.TestCase):
    def test_prompt_waits_for_settled_and_preserves_unicode(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            script = path / "fake.py"
            script.write_text('''import json,sys
for line in sys.stdin:
    request=json.loads(line)
    print(json.dumps({"type":"agent_settled","aborted":False}), flush=True)
    print(json.dumps({"type":"message_end","message":{"role":"assistant","content":"a\\u2028b"}},ensure_ascii=False), flush=True)
    print(json.dumps({"id":request["id"],"type":"response","success":True,"data":{"disposition":"started"}}),flush=True)
''')
            trace = io.StringIO()
            rpc = runner.Rpc([sys.executable, str(script)], path, {}, trace)
            try:
                self.assertTrue(rpc.command("prompt", 2, message="test")["success"])
            finally:
                rpc.close()
            # U+2028 is content, never a JSONL record boundary.
            rows = [json.loads(line) for line in trace.getvalue().split("\n") if line]
            message = next(row for row in rows if row["event"]["type"] == "message_end")
            self.assertEqual(message["event"]["message"]["content"], "a\u2028b")
            self.assertIsNotNone(rpc.process.returncode)

    def test_timeout_kills_and_reaps_owned_process(self):
        with tempfile.TemporaryDirectory() as temporary:
            rpc = runner.Rpc([sys.executable, "-c", "import time; time.sleep(30)"],
                             temporary, {}, io.StringIO())
            try:
                with self.assertRaises(TimeoutError):
                    rpc.command("get_state", 0.02)
            finally:
                rpc.close()
            self.assertIsNotNone(rpc.process.returncode)

    def test_provider_error_is_not_scored_as_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            rpc = runner.Rpc([sys.executable, "-c", 'import json; print(json.dumps({"type":"message_end","message":{"role":"assistant","stopReason":"error"}}),flush=True); input()'],
                             temporary, {}, io.StringIO())
            try:
                with self.assertRaisesRegex(RuntimeError, "Provider failed"):
                    rpc.command("prompt", 2, message="test")
            finally:
                rpc.close()

    def test_report_replays_tool_deltas_and_never_infers_correctness(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            runner.write_json(path / "run.json", {"arm": "enabled", "status": "completed"})
            context = {"systemMessages": [
                {"role": "system", "content": "base", "toolsAdded": [{"name": "old"}],
                 "sections": {"tokensave_retrieval": "prefer retrieval"}},
                {"role": "system", "content": "", "toolsRemoved": [{"name": "old"}],
                 "toolsAdded": [{"name": "actual_adapter_name"}]},
            ]}
            (path / "contexts.jsonl").write_text(json.dumps(context) + "\n")
            call = {"event": {"type": "tool_execution_start", "toolCallId": "a", "toolName": "actual_adapter_name"}}
            (path / "events.jsonl").write_text((json.dumps(call) + "\n") * 2)
            summary = report.summarize(path)
            self.assertEqual(summary["calls"], {"actual_adapter_name": 1})
            self.assertEqual(summary["contexts"][0]["tools"], [{"name": "actual_adapter_name"}])
            self.assertTrue(summary["guidance_verified"])
            self.assertEqual(summary["correctness"], "unreviewed")

    def test_missing_observation_is_not_verified(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            runner.write_json(path / "run.json", {"arm": "enabled", "status": "failed"})
            self.assertFalse(report.summarize(path)["guidance_verified"])


if __name__ == "__main__":
    unittest.main()
