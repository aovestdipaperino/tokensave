"""Privacy-preserving metrics from a Claude Code stream-json trace (#948).

The parser sees the full event stream, but returns only aggregate numbers,
tool names and keyed hashes: no prompt text, tool arguments, file paths,
tool output or answer text. Times are harness receipt times of each line, so
model timings are observable round trips, not provider inference time.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import re
from dataclasses import dataclass, field
from typing import Any, Iterable

# Tools that change files. A repeated identical retrieval after one of these
# is a legitimate revalidation, not a redundant call.
MUTATING_TOOLS = frozenset(
    {
        "Edit",
        "MultiEdit",
        "Write",
        "NotebookEdit",
        "mcp__tokensave__tokensave_str_replace",
        "mcp__tokensave__tokensave_multi_str_replace",
        "mcp__tokensave__tokensave_insert_at",
        "mcp__tokensave__tokensave_create_file",
        "mcp__tokensave__tokensave_ast_grep_rewrite",
        "mcp__tokensave__tokensave_rename",
    }
)
# Shell tools may or may not mutate; they are treated as mutating so a
# repeat after them is never counted as redundant (conservative).
SHELL_TOOLS = frozenset({"Bash", "PowerShell"})

# Argument keys that carry the "question" of a retrieval call. Two calls of
# the same tool with the same normalized question but other differing
# arguments are near-duplicates.
QUERY_KEYS = ("query", "task", "pattern", "symbol", "file_path", "path", "node_id")

BYTES_PER_TOKEN = 4  # documented estimate for MCP response size in tokens


@dataclass
class Event:
    t_ms: float
    obj: dict[str, Any]


def tool_family(name: str) -> str:
    if name.startswith("mcp__tokensave__"):
        return "tokensave"
    if name.startswith("mcp__"):
        return "other_mcp"
    return "builtin"


def _content_bytes(content: Any) -> int:
    if content is None:
        return 0
    if isinstance(content, str):
        return len(content.encode("utf-8"))
    if isinstance(content, list):
        total = 0
        for block in content:
            if isinstance(block, dict) and block.get("type") == "text":
                total += len(str(block.get("text", "")).encode("utf-8"))
            else:
                total += len(json.dumps(block, sort_keys=True).encode("utf-8"))
        return total
    return len(json.dumps(content, sort_keys=True).encode("utf-8"))


def _normalize(value: Any) -> str:
    return re.sub(r"\s+", " ", str(value)).strip().lower()


@dataclass
class _Call:
    name: str
    started_ms: float
    exact_key: bytes
    query_key: bytes | None
    parent: str | None


@dataclass
class TraceMetrics:
    """Everything the harness keeps about one trial by default."""

    wall_ms: float = 0.0
    startup_ms: float | None = None
    model_wait_ms: float = 0.0
    tool_wait_ms: float = 0.0
    tail_ms: float = 0.0
    tool_wait_summed_ms: float = 0.0
    model_calls: int = 0
    reported_turns: int | None = None
    reported_duration_ms: float | None = None
    reported_api_ms: float | None = None
    cost_usd: float | None = None
    input_tokens: int | None = None
    output_tokens: int | None = None
    cache_read_tokens: int | None = None
    cache_creation_tokens: int | None = None
    tool_calls: int = 0
    subagent_tool_calls: int = 0
    tool_errors: int = 0
    tool_calls_by_name: dict[str, int] = field(default_factory=dict)
    tool_calls_by_family: dict[str, int] = field(default_factory=dict)
    tokensave_response_bytes: int = 0
    tool_response_bytes: int = 0
    exact_duplicates: int = 0
    revalidations: int = 0
    near_duplicates: int = 0
    max_parallel_tools: int = 0
    result_subtype: str | None = None
    is_error: bool | None = None
    model: str | None = None
    tokensave_connected: bool | None = None
    tokensave_tool_count: int = 0

    @property
    def tokensave_response_tokens_est(self) -> int:
        return self.tokensave_response_bytes // BYTES_PER_TOKEN

    def to_json(self) -> dict[str, Any]:
        data = dict(self.__dict__)
        data["tokensave_response_tokens_est"] = self.tokensave_response_tokens_est
        return data


def parse(events: Iterable[Event], salt: bytes, process_end_ms: float | None = None):
    """Returns `(metrics, answer)`. `answer` is the final result text, for the
    correctness check only; callers must not persist it by default."""
    m = TraceMetrics()
    answer: str | None = None
    pending: dict[str, _Call] = {}
    seen_exact: dict[bytes, int] = {}
    seen_query: set[bytes] = set()
    mutations = 0
    usage_by_message: dict[str, dict[str, Any]] = {}
    message_ids: set[str] = set()
    last_t = 0.0
    phase = "startup"

    def advance(now: float) -> None:
        nonlocal last_t
        gap = max(0.0, now - last_t)
        if phase == "startup":
            m.startup_ms = (m.startup_ms or 0.0) + gap
        elif phase == "tail":
            m.tail_ms += gap
        elif pending:
            m.tool_wait_ms += gap
        else:
            m.model_wait_ms += gap
        last_t = now

    def key(*parts: Any) -> bytes:
        text = json.dumps(parts, sort_keys=True, default=str).encode("utf-8")
        return hmac.new(salt, text, hashlib.sha256).digest()

    for ev in events:
        obj = ev.obj
        kind = obj.get("type")
        advance(ev.t_ms)
        if kind == "system" and obj.get("subtype") == "init":
            phase = "agent"
            m.model = obj.get("model")
            tools = obj.get("tools") or []
            m.tokensave_tool_count = sum(
                1 for t in tools if isinstance(t, str) and t.startswith("mcp__tokensave__")
            )
            servers = obj.get("mcp_servers") or []
            status = [s.get("status") for s in servers if s.get("name") == "tokensave"]
            m.tokensave_connected = (status[0] == "connected") if status else False
        elif kind == "assistant":
            msg = obj.get("message") or {}
            parent = obj.get("parent_tool_use_id")
            mid = msg.get("id")
            if mid and mid not in message_ids:
                message_ids.add(mid)
                if parent is None:
                    m.model_calls += 1
            if mid and isinstance(msg.get("usage"), dict):
                usage_by_message[mid] = msg["usage"]
            for block in msg.get("content") or []:
                if not isinstance(block, dict) or block.get("type") != "tool_use":
                    continue
                name = str(block.get("name", "?"))
                args = block.get("input") or {}
                m.tool_calls += 1
                if parent is not None:
                    m.subagent_tool_calls += 1
                m.tool_calls_by_name[name] = m.tool_calls_by_name.get(name, 0) + 1
                fam = tool_family(name)
                m.tool_calls_by_family[fam] = m.tool_calls_by_family.get(fam, 0) + 1
                exact = key(name, args)
                query = None
                if isinstance(args, dict):
                    for k in QUERY_KEYS:
                        if k in args:
                            query = key(name, k, _normalize(args[k]))
                            break
                if name not in MUTATING_TOOLS and name not in SHELL_TOOLS:
                    if exact in seen_exact:
                        if seen_exact[exact] == mutations:
                            m.exact_duplicates += 1
                        else:
                            m.revalidations += 1
                    elif query is not None and query in seen_query:
                        m.near_duplicates += 1
                    seen_exact[exact] = mutations
                    if query is not None:
                        seen_query.add(query)
                else:
                    mutations += 1
                pending[str(block.get("id"))] = _Call(name, ev.t_ms, exact, query, parent)
                m.max_parallel_tools = max(m.max_parallel_tools, len(pending))
        elif kind == "user":
            msg = obj.get("message") or {}
            content = msg.get("content")
            if not isinstance(content, list):
                continue
            for block in content:
                if not isinstance(block, dict) or block.get("type") != "tool_result":
                    continue
                call = pending.pop(str(block.get("tool_use_id")), None)
                size = _content_bytes(block.get("content"))
                m.tool_response_bytes += size
                if block.get("is_error"):
                    m.tool_errors += 1
                if call is None:
                    continue
                m.tool_wait_summed_ms += max(0.0, ev.t_ms - call.started_ms)
                if tool_family(call.name) == "tokensave":
                    m.tokensave_response_bytes += size
        elif kind == "result":
            phase = "tail"
            m.result_subtype = obj.get("subtype")
            m.is_error = bool(obj.get("is_error"))
            m.reported_turns = obj.get("num_turns")
            m.reported_duration_ms = obj.get("duration_ms")
            m.reported_api_ms = obj.get("duration_api_ms")
            m.cost_usd = obj.get("total_cost_usd")
            usage = obj.get("usage")
            if isinstance(usage, dict):
                _apply_usage(m, usage)
            if isinstance(obj.get("result"), str):
                answer = obj["result"]

    if process_end_ms is not None:
        advance(process_end_ms)
    m.wall_ms = last_t
    if m.input_tokens is None and usage_by_message:
        total: dict[str, int] = {}
        for usage in usage_by_message.values():
            for k, v in usage.items():
                if isinstance(v, int):
                    total[k] = total.get(k, 0) + v
        _apply_usage(m, total)
    return m, answer


def _apply_usage(m: TraceMetrics, usage: dict[str, Any]) -> None:
    def get(name: str) -> int | None:
        value = usage.get(name)
        return value if isinstance(value, int) else None

    m.input_tokens = get("input_tokens")
    m.output_tokens = get("output_tokens")
    m.cache_read_tokens = get("cache_read_input_tokens")
    m.cache_creation_tokens = get("cache_creation_input_tokens")
