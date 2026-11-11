"""Synthetic tests for the agent benchmark harness (#948). No model calls."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import agent_bench  # noqa: E402
import agent_trace  # noqa: E402
import profiling  # noqa: E402
import report  # noqa: E402

SALT = b"s" * 32
SECRET_PATH = "src/secret_module/credentials.rs"
SECRET_QUERY = "where is the api key loaded"
SECRET_OUTPUT = "fn load_api_key() { PRIVATE_SOURCE_TEXT }"
SECRET_ANSWER = "The answer mentions PRIVATE_ANSWER_TEXT"


def ev(t, obj):
    return agent_trace.Event(t, obj)


def assistant(mid, *blocks, usage=None, parent=None):
    msg = {"id": mid, "content": list(blocks)}
    if usage:
        msg["usage"] = usage
    return {"type": "assistant", "message": msg, "parent_tool_use_id": parent}


def tool_use(tid, name, **args):
    return {"type": "tool_use", "id": tid, "name": name, "input": args}


def tool_result(tid, content, is_error=False):
    return {
        "type": "user",
        "message": {"content": [{"type": "tool_result", "tool_use_id": tid, "content": content, "is_error": is_error}]},
    }


def sample_trace(with_result=True):
    events = [
        ev(100, {"type": "system", "subtype": "init", "model": "m-1",
                 "tools": ["Read", "mcp__tokensave__tokensave_context", "mcp__tokensave__tokensave_search"],
                 "mcp_servers": [{"name": "tokensave", "status": "connected"}]}),
        # model 100 -> 1100, then two parallel tools.
        ev(1100, assistant("a1",
                           tool_use("t1", "mcp__tokensave__tokensave_context", task=SECRET_QUERY),
                           tool_use("t2", "Read", file_path=SECRET_PATH),
                           usage={"input_tokens": 10, "output_tokens": 5})),
        ev(1300, tool_result("t2", SECRET_OUTPUT)),
        ev(1600, tool_result("t1", [{"type": "text", "text": "x" * 400}])),
        # model 1600 -> 2600; exact repeat of t1, near repeat (different arg).
        ev(2600, assistant("a2",
                           tool_use("t3", "mcp__tokensave__tokensave_context", task=SECRET_QUERY),
                           tool_use("t4", "mcp__tokensave__tokensave_context", task="  Where is the API key LOADED ", max_nodes=5),
                           usage={"input_tokens": 20, "output_tokens": 7})),
        ev(2700, tool_result("t3", "y" * 40)),
        ev(2800, tool_result("t4", "z" * 40, is_error=True)),
        # an edit, then the same retrieval again: a revalidation.
        ev(3800, assistant("a3", tool_use("t5", "Edit", file_path=SECRET_PATH, old_string="a", new_string="b"))),
        ev(3900, tool_result("t5", "ok")),
        ev(4900, assistant("a4", tool_use("t6", "mcp__tokensave__tokensave_context", task=SECRET_QUERY))),
        ev(5000, tool_result("t6", "w" * 8)),
        # subagent tool call, not a top-level model call.
        ev(5100, assistant("sub1", tool_use("t7", "Grep", pattern="x"), parent="t6")),
        ev(5200, tool_result("t7", "")),
        ev(6200, assistant("a5", {"type": "text", "text": SECRET_ANSWER})),
    ]
    if with_result:
        events.append(ev(6300, {"type": "result", "subtype": "success", "is_error": False,
                                "num_turns": 5, "duration_ms": 6150, "duration_api_ms": 4000,
                                "total_cost_usd": 0.0123, "result": SECRET_ANSWER,
                                "usage": {"input_tokens": 99, "output_tokens": 33,
                                          "cache_read_input_tokens": 1000}}))
    return events


class TraceTest(unittest.TestCase):
    def test_partition_adds_up_to_wall_time(self):
        m, answer = agent_trace.parse(sample_trace(), SALT, process_end_ms=6500)
        self.assertEqual(answer, SECRET_ANSWER)
        self.assertEqual(m.wall_ms, 6500)
        parts = (m.startup_ms or 0) + m.model_wait_ms + m.tool_wait_ms + m.tail_ms
        self.assertAlmostEqual(parts, m.wall_ms)
        self.assertEqual(m.startup_ms, 100)
        self.assertEqual(m.tail_ms, 200)
        # Parallel t1/t2 overlap: wall tool wait 500, summed waits 200 + 500.
        self.assertGreater(m.tool_wait_summed_ms, m.tool_wait_ms - 400)

    def test_counts_calls_duplicates_and_revalidations(self):
        m, _ = agent_trace.parse(sample_trace(), SALT)
        self.assertEqual(m.model_calls, 5)  # subagent message excluded
        self.assertEqual(m.tool_calls, 7)
        self.assertEqual(m.subagent_tool_calls, 1)
        self.assertEqual(m.tool_calls_by_family["tokensave"], 4)
        self.assertEqual(m.exact_duplicates, 1)
        self.assertEqual(m.near_duplicates, 1)
        self.assertEqual(m.revalidations, 1)
        self.assertEqual(m.tool_errors, 1)
        self.assertEqual(m.max_parallel_tools, 2)
        self.assertEqual(m.tokensave_response_bytes, 400 + 40 + 40 + 8)
        self.assertTrue(m.tokensave_connected)
        self.assertEqual(m.tokensave_tool_count, 2)

    def test_prefers_reported_usage_and_marks_missing_as_unavailable(self):
        m, _ = agent_trace.parse(sample_trace(), SALT)
        self.assertEqual((m.input_tokens, m.output_tokens, m.cache_read_tokens), (99, 33, 1000))
        self.assertEqual(m.cost_usd, 0.0123)
        m2, answer = agent_trace.parse(sample_trace(with_result=False), SALT)
        self.assertIsNone(answer)
        self.assertEqual(m2.input_tokens, 30)  # summed per message
        self.assertIsNone(m2.cost_usd)
        self.assertIsNone(m2.cache_read_tokens)

    def test_metrics_carry_no_sensitive_text(self):
        m, _ = agent_trace.parse(sample_trace(), SALT)
        blob = json.dumps(m.to_json())
        for secret in (SECRET_PATH, SECRET_QUERY, "PRIVATE_SOURCE_TEXT", "PRIVATE_ANSWER_TEXT", "api key"):
            self.assertNotIn(secret, blob)


class ProfilingTest(unittest.TestCase):
    def test_collects_only_new_process_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            old = {"schema": 1, "hour": 1, "series": {"mcp.total": {"count": 99, "sum_us": 9_000_000}}}
            (d / "1-11-aa.json").write_text(json.dumps(old))
            before = profiling.snapshot(d)
            new = {"schema": 1, "hour": 2, "series": {
                "mcp.total": {"count": 3, "errors": 1, "sum_us": 40_000},
                "tool.tokensave_context": {"count": 2, "sum_us": 30_000, "db_us": 12_000, "db_ops": 14,
                                           "response_bytes": 900,
                                           "repeated": {"db.get_outgoing_edges": {"calls": 1, "executions": 25}}},
                "tool.tokensave_search": {"count": 1, "errors": 1, "sum_us": 5_000, "db_us": 1_000, "db_ops": 2},
                "sync.total": {"count": 1, "sum_us": 7_000},
            }}
            (d / "2-22-bb.json").write_text(json.dumps(new))
            out = profiling.collect(d, before, settle_s=0)
            self.assertEqual(out["processes"], 1)
            self.assertEqual(out["mcp_calls"], 4)
            self.assertEqual(out["mcp_total_ms"], 40.0)
            self.assertEqual(out["db_ms_in_tools"], 13.0)
            self.assertEqual(out["db_ops_in_tools"], 16)
            self.assertEqual(out["repeated_db_executions"], 25)
            self.assertEqual(out["sync_ms"], 7.0)
            self.assertEqual(out["tools"]["tokensave_search"]["calls"], 2)

    def test_no_new_file_is_unavailable(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(profiling.collect(Path(tmp), set(), settle_s=0))


def record(task, arm, wall, success=True, run="r1", prof=None, ts_calls=0, dupes=0):
    m, _ = agent_trace.parse(sample_trace(), SALT)
    metrics = m.to_json()
    metrics["wall_ms"] = wall
    metrics["model_wait_ms"] = wall * 0.7
    metrics["tool_wait_ms"] = wall * 0.2
    metrics["startup_ms"] = wall * 0.05
    metrics["tail_ms"] = wall * 0.05
    metrics["tool_calls_by_family"] = {"tokensave": ts_calls}
    metrics["exact_duplicates"] = dupes
    metrics["near_duplicates"] = 0
    return {"run_id": run, "task": task, "arm": arm, "success": success,
            "outcome": "pass" if success else "wrong_answer", "metrics": metrics,
            "tokensave_profile": prof, "environment": {"os": "x"}, "requested_model": "m-1"}


class ReportTest(unittest.TestCase):
    def test_comparison_verdict_and_unavailable_fields(self):
        recs = [record("t", "baseline", w) for w in (10_000, 12_000, 11_000)]
        prof = {"mcp_total_ms": 300.0, "db_ms_in_tools": 100.0, "sync_ms": 0.0,
                "db_ops_in_tools": 10, "repeated_db_executions": 0}
        recs += [record("t", "tokensave", w, prof=prof, ts_calls=4) for w in (8_000, 9_000, 7_000)]
        agg = report.aggregate(recs)
        comp = agg["comparisons"]["t/tokensave"]
        self.assertEqual(comp["wall_ms_median_diff"], -3000)
        lo, hi = comp["wall_ms_median_diff_ci95"]
        self.assertLessEqual(lo, hi)
        self.assertIn("agent/model overhead", comp["verdict"])
        self.assertIsNone(agg["groups"]["t/baseline"]["wall_ms_p90"])  # n < 10
        md = report.render_markdown(recs)
        self.assertIn("| t | tokensave | 3 |", md)
        self.assertIn("n/a", md)

    def test_lower_success_dominates_verdict(self):
        recs = [record("t", "baseline", 10_000) for _ in range(2)]
        recs += [record("t", "tokensave", 5_000, success=False) for _ in range(2)]
        self.assertIn("quality", report.aggregate(recs)["comparisons"]["t/tokensave"]["verdict"])

    def test_redundant_calls_verdict(self):
        recs = [record("t", "baseline", 10_000)]
        recs += [record("t", "tokensave", 10_000, ts_calls=5, dupes=2)]
        self.assertIn("number of calls", report.aggregate(recs)["arm_verdicts"]["tokensave"])

    def test_arms_compare_with_their_reference_arm(self):
        recs = [record("t", "caveman", 10_000) for _ in range(2)]
        recs += [record("t", "tokensave-caveman", 6_000) for _ in range(2)]
        for r in recs[2:]:
            r["reference_arm"] = "caveman"
        recs += [record("t", "baseline", 20_000)]  # older record: no reference field
        agg = report.aggregate(recs)
        comp = agg["comparisons"]["t/tokensave-caveman"]
        self.assertEqual(comp["reference"], "caveman")
        self.assertEqual(comp["wall_ms_median_diff"], -4000)
        self.assertEqual(comp["cost_usd_median_diff"], 0)
        self.assertEqual(agg["comparisons"]["t/caveman"]["reference"], "baseline")
        self.assertEqual(agg["references"]["tokensave-caveman"], "caveman")
        md = report.render_markdown(recs)
        self.assertIn("| t/tokensave-caveman | caveman | -4.0 |", md)
        self.assertIn("**tokensave-caveman** (vs caveman)", md)

    def test_db_share_verdict(self):
        prof = {"mcp_total_ms": 3000.0, "db_ms_in_tools": 2000.0, "sync_ms": 0.0,
                "db_ops_in_tools": 10, "repeated_db_executions": 0}
        recs = [record("t", "baseline", 10_000), record("t", "tokensave", 10_000, prof=prof)]
        self.assertIn("database latency", report.aggregate(recs)["arm_verdicts"]["tokensave"])


class ConfigTest(unittest.TestCase):
    def test_task_suite_is_well_formed(self):
        tasks = agent_bench.load_json(HERE / "tasks.json")
        ids = [t["id"] for t in tasks["tasks"]]
        self.assertEqual(len(ids), len(set(ids)))
        for t in tasks["tasks"]:
            self.assertIn(t["repo"], tasks["repos"])
            self.assertTrue(t.get("checks"), t["id"])
            for pattern in t["checks"].get("answer_all", []) + t["checks"].get("answer_none", []):
                re.compile(pattern)
            for rel in t.get("setup_files", {}):
                self.assertRegex(rel, r"^[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)*$")
        for spec in tasks["repos"].values():
            if spec["source"] == "git":
                self.assertRegex(spec["ref"], r"^[0-9a-f]{40}$")  # pinned, not a branch
            else:
                self.assertTrue((HERE / spec["path"]).is_dir())

    def test_arms_differ_only_in_declared_fields(self):
        arms = agent_bench.load_json(HERE / "arms.json")
        self.assertIn("baseline", arms["arms"])
        self.assertFalse(arms["arms"]["baseline"]["tokensave"])
        cfg = agent_bench.mcp_config(arms["arms"]["baseline"], Path("ts"), Path("ws"), True)
        self.assertEqual(cfg, {"mcpServers": {}})
        cfg = agent_bench.mcp_config(arms["arms"]["tokensave"], Path("ts"), Path("ws"), True)
        self.assertEqual(cfg["mcpServers"]["tokensave"]["env"]["TOKENSAVE_PROFILE"], "light")

    def test_references_name_defined_arms(self):
        arms = agent_bench.load_json(HERE / "arms.json")["arms"]
        for name, arm in arms.items():
            self.assertIn(arm.get("reference", "baseline"), arms, name)
            self.assertNotEqual(arm.get("reference"), name)
            if "caveman_cooperation" in arm:
                self.assertTrue(arm["tokensave"] and arm["caveman_skill"], name)

    def test_caveman_arms_differ_only_in_cooperation_flag(self):
        arms = agent_bench.load_json(HERE / "arms.json")["arms"]
        home = Path("home")
        coop = agent_bench.mcp_config(arms["tokensave-caveman"], Path("ts"), Path("ws"), False, home)
        plain = agent_bench.mcp_config(arms["tokensave-caveman-nocoop"], Path("ts"), Path("ws"), False, home)
        coop_env = coop["mcpServers"]["tokensave"]["env"]
        plain_env = plain["mcpServers"]["tokensave"]["env"]
        self.assertEqual(coop_env.pop("TOKENSAVE_CAVEMAN_COMPATIBILITY"), "1")
        self.assertEqual(plain_env.pop("TOKENSAVE_CAVEMAN_COMPATIBILITY"), "0")
        self.assertEqual(coop_env, plain_env)
        self.assertEqual(coop_env["CLAUDE_CONFIG_DIR"], str(home))
        # A user's own caveman_compatibility setting cannot reach other arms.
        ts = agent_bench.mcp_config(arms["tokensave"], Path("ts"), Path("ws"), False, home)
        ts_env = ts["mcpServers"]["tokensave"]["env"]
        self.assertEqual(ts_env["TOKENSAVE_CAVEMAN_COMPATIBILITY"], "0")
        self.assertNotIn("CLAUDE_CONFIG_DIR", ts_env)

    def test_fixture_has_exactly_one_seeded_failure(self):
        proc = subprocess.run([sys.executable, "-B", "-m", "unittest", "discover", "-s", "tests"],
                              cwd=HERE / "fixtures" / "inventory", capture_output=True, text=True)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("failures=1", proc.stderr)


class EnvTest(unittest.TestCase):
    def test_calling_session_does_not_leak_into_trials(self):
        saved = dict(agent_bench.os.environ)
        try:
            agent_bench.os.environ.update({
                "CLAUDECODE": "1", "CLAUDE_CODE_SESSION_ID": "x", "CLAUDE_CODE_MESSAGING_TOKEN": "t",
                "CLAUDE_EFFORT": "high", "MCP_CONNECTION_NONBLOCKING": "1",
                "CLAUDE_CONFIG_DIR": "/cfg", "PATH_KEEP_PROBE": "kept"})
            env = agent_bench.agent_env()
        finally:
            agent_bench.os.environ.clear()
            agent_bench.os.environ.update(saved)
        for gone in ("CLAUDECODE", "CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_MESSAGING_TOKEN",
                     "CLAUDE_EFFORT", "MCP_CONNECTION_NONBLOCKING"):
            self.assertNotIn(gone, env)
        self.assertEqual(env["CLAUDE_CONFIG_DIR"], "/cfg")
        self.assertEqual(env["PATH_KEEP_PROBE"], "kept")

    def test_arms_load_no_user_or_project_settings(self):
        args = agent_bench.load_json(HERE / "arms.json")["common_args"]
        self.assertEqual(args[args.index("--setting-sources") + 1], "")


SKILL = b"---\nname: caveman\ndescription: terse\n---\nTalk like caveman.\n"

FAKE_SERVER = """
import json, sys, time
mode = sys.argv[1]
if mode == "silent":
    sys.stdin.read()  # never replies; exits when the harness closes stdin
    sys.exit(0)
req = json.loads(sys.stdin.readline())
print("not json", flush=True)
print(json.dumps({"jsonrpc": "2.0", "method": "notifications/message"}), flush=True)
result = {} if mode == "none" else {"instructions": "Use tokensave." + (
    " If Caveman is active in this session, it governs only your prose." if mode == "coop" else "")}
print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}), flush=True)
sys.stdin.read()
"""


class CavemanTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.server = self.dir / "fake_server.py"
        self.server.write_text(FAKE_SERVER, encoding="utf-8")

    def tearDown(self):
        self.tmp.cleanup()

    def skill(self, data: bytes) -> Path:
        path = self.dir / "SKILL.md"
        path.write_bytes(data)
        return path

    def test_skill_checks_match_tokensave_discovery(self):
        self.assertEqual(agent_bench.load_caveman_skill(self.skill(SKILL)), SKILL)
        for bad in (b"---\nname: other\n---\n", b"name: caveman\n\xff\xfe",
                    b"name: caveman\n" + b"x" * agent_bench.CAVEMAN_SKILL_MAX_BYTES):
            with self.assertRaises(SystemExit):
                agent_bench.load_caveman_skill(self.skill(bad))
        with self.assertRaises(SystemExit):
            agent_bench.load_caveman_skill(self.dir / "missing.md")

    def test_agent_home_holds_only_the_pinned_skill(self):
        home = agent_bench.caveman_agent_home(self.dir / "work", SKILL)
        files = [p.relative_to(home).as_posix() for p in home.rglob("*") if p.is_file()]
        self.assertEqual(files, ["skills/caveman/SKILL.md"])
        self.assertEqual((home / "skills" / "caveman" / "SKILL.md").read_bytes(), SKILL)

    def test_prompt_appendix_combines_rules_and_skill(self):
        rules = self.dir / "rules.md"
        rules.write_text("# Rules", encoding="utf-8")
        self.assertIsNone(agent_bench.prompt_appendix({"tokensave": True}, SKILL))
        self.assertIsNone(agent_bench.prompt_appendix({"caveman_skill": True}, None))
        self.assertEqual(agent_bench.prompt_appendix({"caveman_skill": True}, SKILL), SKILL.decode())
        both = agent_bench.prompt_appendix(
            {"append_system_prompt_file": str(rules), "caveman_skill": True}, SKILL)
        self.assertEqual(both, "# Rules\n\n" + SKILL.decode())
        cmd = agent_bench.build_command(Path("claude"), {"common_args": []}, both, "p", "m",
                                        Path("mcp.json"), None, None)
        self.assertEqual(cmd[-2:], ["--append-system-prompt", both])

    def test_reads_initialize_instructions(self):
        cmd = [sys.executable, str(self.server)]
        env = dict(agent_bench.os.environ)
        self.assertIn("If Caveman is active", agent_bench.mcp_instructions(cmd + ["coop"], env, self.dir))
        self.assertEqual(agent_bench.mcp_instructions(cmd + ["plain"], env, self.dir), "Use tokensave.")
        self.assertEqual(agent_bench.mcp_instructions(cmd + ["none"], env, self.dir), "")
        started = agent_bench.time.monotonic()
        self.assertIsNone(agent_bench.mcp_instructions(cmd + ["silent"], env, self.dir, timeout_s=0.5))
        self.assertLess(agent_bench.time.monotonic() - started, 10)
        self.assertIsNone(agent_bench.mcp_instructions([str(self.dir / "no-such-binary")], env, self.dir))

    def test_cooperation_mismatch_stops_the_run(self):
        def server(mode):
            return {"command": sys.executable, "args": [str(self.server), mode],
                    "env": {"TOKENSAVE_PROFILE": "light"}}

        coop = {"tokensave": True, "caveman_skill": True, "caveman_cooperation": True}
        nocoop = {"tokensave": True, "caveman_skill": True, "caveman_cooperation": False}
        self.assertTrue(agent_bench.verify_cooperation("a", coop, server("coop"), self.dir))
        self.assertFalse(agent_bench.verify_cooperation("b", nocoop, server("plain"), self.dir))
        with self.assertRaises(SystemExit):
            agent_bench.verify_cooperation("c", coop, server("plain"), self.dir)
        with self.assertRaises(SystemExit):
            agent_bench.verify_cooperation("d", nocoop, server("coop"), self.dir)


class CheckTest(unittest.TestCase):
    def test_answer_checks_and_classification(self):
        task = {"checks": {"answer_all": [r"\bload\b"], "answer_none": ["FOUND"]}}
        checks = agent_bench.check_task(task, HERE, "it is load")
        self.assertEqual(checks, {"answer_all": True, "answer_none": True})
        m, _ = agent_trace.parse(sample_trace(), SALT)
        self.assertEqual(agent_bench.classify(m, checks, 0, False), "pass")
        self.assertEqual(agent_bench.classify(m, {"answer_all": False}, 0, False), "wrong_answer")
        self.assertEqual(agent_bench.classify(m, checks, 0, True), "timeout")
        m.result_subtype = "error_max_budget_usd"
        m.is_error = True
        self.assertEqual(agent_bench.classify(m, checks, 1, False), "agent_error_max_budget_usd")


if __name__ == "__main__":
    unittest.main()
