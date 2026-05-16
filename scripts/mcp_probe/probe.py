#!/usr/bin/env python3
"""Drive `tokensave serve` over stdio against every repo listed in repos.toml
and exercise every tool with 5 query variants per language.

Per-call status lines go to /tmp/tokensave_matrix.log (override with
$TOKENSAVE_PROBE_LOG). Run build_matrix.py afterwards to render the table.

The driver:
  * spawns one MCP server per repo (cwd=repo), initialize handshake,
  * runs a `discover` pass to harvest real node ids / qualified names /
    files from `tokensave_search` + `tokensave_node`,
  * pulls per-language probe sets from `tools/<lang>.py::probes_for(discovered)`,
  * matches responses by JSON-RPC id (so a slow call cannot poison later ones),
  * times each call, classifies the response, logs.

Exit code is 0 even if some tools fail — that is the matrix's whole point.
Use build_matrix.py to surface the bad cells.
"""
from __future__ import annotations

import importlib
import json
import os
import select
import subprocess
import sys
import time
from pathlib import Path

try:
    import tomllib  # py3.11+
except ImportError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

ROOT = Path(__file__).resolve().parent
REPO_ROOT = ROOT.parents[1]
DEFAULT_BIN = REPO_ROOT / "target" / "release" / "tokensave"
DEFAULT_LOG = "/tmp/tokensave_matrix.log"
DEFAULT_REPOS = ROOT / "repos.toml"
DEFAULT_STDERR_DIR = "/tmp/tokensave_matrix_stderr"

BIN = Path(os.environ.get("TOKENSAVE_PROBE_BIN", str(DEFAULT_BIN)))
LOG_PATH = os.environ.get("TOKENSAVE_PROBE_LOG", DEFAULT_LOG)
REPOS_CONF = Path(os.environ.get("TOKENSAVE_PROBE_REPOS", str(DEFAULT_REPOS)))
TIMEOUT = float(os.environ.get("TOKENSAVE_PROBE_TIMEOUT", "25"))
STDERR_DIR = Path(os.environ.get("TOKENSAVE_PROBE_STDERR_DIR", DEFAULT_STDERR_DIR))


class McpInitError(RuntimeError):
    """Raised when a freshly-spawned MCP server fails to complete handshake.

    Carries the path of the per-repo stderr capture so the caller can point
    a human at it instead of swallowing the diagnosis."""

    def __init__(self, repo: str, stderr_path: Path, hint: str):
        super().__init__(f"MCP init failed for {repo}: {hint} (stderr: {stderr_path})")
        self.repo = repo
        self.stderr_path = stderr_path
        self.hint = hint


class McpClient:
    """Thin JSON-RPC-over-stdio client. Responses are routed back to the
    awaiting caller by id, so a late response from a previously-timed-out
    request cannot be misread as the response to a later request."""

    def __init__(self, repo_path: str, repo_name: str = "unknown"):
        # Capture stderr per-repo. Earlier revisions piped to DEVNULL, which
        # made server crashes during init look like a BrokenPipeError from
        # the probe driver — totally unactionable. Real causes (missing
        # .tokensave/tokensave.db, unreadable DB, OOM on large repos like
        # chromium) showed up on stderr but were silently discarded.
        STDERR_DIR.mkdir(parents=True, exist_ok=True)
        self.stderr_path = STDERR_DIR / f"{repo_name}.stderr"
        self.stderr_file = open(self.stderr_path, "wb")
        self.proc = subprocess.Popen(
            [str(BIN), "serve"],
            cwd=repo_path,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self.stderr_file,
            bufsize=0,
        )
        self._id = 0
        self._buf = b""
        self._stale: set[int] = set()  # ids whose original caller already gave up

        # Initialize handshake — fail loud, fail early. Earlier the code
        # ignored a missing/error response and tried to push the
        # "initialized" notification regardless; if the server had already
        # exited, that second write blew up the whole probe with a bare
        # BrokenPipeError instead of skipping the repo.
        try:
            self._send({
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "tokensave-probe", "version": "1"},
                },
                "id": self._next(),
            })
        except (BrokenPipeError, OSError) as exc:
            raise McpInitError(repo_name, self.stderr_path,
                               f"server died before initialize ({exc})") from exc

        resp = self._recv_id(self._id, timeout=15)
        if resp is None:
            raise McpInitError(repo_name, self.stderr_path,
                               "server closed stdout during initialize")
        if isinstance(resp, dict) and resp.get("_timeout"):
            raise McpInitError(repo_name, self.stderr_path,
                               "initialize timed out after 15s")
        if isinstance(resp, dict) and "error" in resp:
            err = resp["error"]
            msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
            raise McpInitError(repo_name, self.stderr_path,
                               f"initialize returned error: {msg[:200]}")

        try:
            self._send({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
        except (BrokenPipeError, OSError) as exc:
            raise McpInitError(repo_name, self.stderr_path,
                               f"server died after initialize ({exc})") from exc

    # ------------------------------------------------------------------ low-level

    def _next(self) -> int:
        self._id += 1
        return self._id

    def _send(self, msg: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write((json.dumps(msg) + "\n").encode())
        self.proc.stdin.flush()

    def _readline_blocking(self, timeout: float) -> dict | None:
        """Read one JSON-RPC message, respecting `timeout`. Returns
        `{"_timeout": True}` on deadline, `None` on EOF, or the parsed message."""
        assert self.proc.stdout is not None
        fd = self.proc.stdout.fileno()
        deadline = time.time() + timeout
        while True:
            if b"\n" in self._buf:
                line, _, rest = self._buf.partition(b"\n")
                self._buf = rest
                try:
                    return json.loads(line.decode())
                except Exception:
                    return {"raw": line.decode()[:200]}
            remaining = deadline - time.time()
            if remaining <= 0:
                return {"_timeout": True}
            r, _, _ = select.select([fd], [], [], remaining)
            if not r:
                return {"_timeout": True}
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                return None
            if not chunk:
                return None
            self._buf += chunk

    def _recv_id(self, want_id: int, timeout: float) -> dict:
        """Wait for the response with `want_id`. Late responses for ids in
        `self._stale` are silently dropped on the way."""
        deadline = time.time() + timeout
        while True:
            remaining = max(0.0, deadline - time.time())
            msg = self._readline_blocking(remaining)
            if msg is None:
                return {"error": {"message": "server closed stdout"}}
            if msg.get("_timeout"):
                return msg
            mid = msg.get("id")
            if mid == want_id:
                return msg
            if mid in self._stale:
                self._stale.discard(mid)
                continue
            # Unknown id (e.g. server-pushed notification). Skip.
            if mid is None:
                continue
            # Out-of-order response for a request we no longer care about.
            self._stale.discard(mid)

    # ------------------------------------------------------------------ public

    def call(self, tool: str, args: dict, timeout: float = TIMEOUT) -> tuple[dict | None, float]:
        rid = self._next()
        self._send({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {"name": tool, "arguments": args},
            "id": rid,
        })
        t0 = time.time()
        resp = self._recv_id(rid, timeout=timeout)
        dt = time.time() - t0
        if isinstance(resp, dict) and resp.get("_timeout"):
            # Caller is giving up — mark id stale so a late reply is dropped.
            self._stale.add(rid)
        return resp, dt

    def close(self) -> None:
        try:
            if self.proc.stdin is not None:
                self.proc.stdin.close()
            self.proc.wait(timeout=3)
        except Exception:
            self.proc.kill()
        try:
            self.stderr_file.close()
        except Exception:
            pass


# ---------------------------------------------------------------------------- helpers

def text_of(resp) -> str | None:
    """Return the first content block that parses as JSON, else content[0]."""
    if not isinstance(resp, dict):
        return None
    res = resp.get("result")
    if not isinstance(res, dict):
        return None
    content = res.get("content") or []
    for block in content:
        t = block.get("text", "")
        try:
            json.loads(t)
            return t
        except Exception:
            continue
    return content[0].get("text") if content else None


def discover(cli: McpClient) -> dict:
    """Harvest real ids / qualified names / files for the open repo."""
    out: dict = {"ids": [], "qnames": [], "names": [], "files": []}
    for query in ["main", "new", "Error", "Config", "Display"]:
        resp, _ = cli.call("tokensave_search", {"query": query}, timeout=15)
        txt = text_of(resp)
        if not txt:
            continue
        try:
            parsed = json.loads(txt)
        except Exception:
            continue
        if isinstance(parsed, list):
            items = parsed
        elif isinstance(parsed, dict):
            items = parsed.get("results") or parsed.get("items") or parsed.get("matches") or []
        else:
            items = []
        for item in items[:20]:
            if not isinstance(item, dict):
                continue
            if item.get("id"):
                out["ids"].append(item["id"])
            if item.get("qualified_name"):
                out["qnames"].append(item["qualified_name"])
            fp = item.get("file") or item.get("file_path")
            if fp:
                out["files"].append(fp)
            if item.get("name"):
                out["names"].append(item["name"])

    for key in out:
        out[key] = list(dict.fromkeys(out[key]))[:5]

    # Fill qualified names from `tokensave_node` if search didn't expose them.
    for nid in list(out["ids"]):
        if len(out["qnames"]) >= 5:
            break
        resp, _ = cli.call("tokensave_node", {"node_id": nid}, timeout=10)
        txt = text_of(resp)
        if not txt:
            continue
        try:
            parsed = json.loads(txt)
            if isinstance(parsed, dict) and parsed.get("qualified_name"):
                out["qnames"].append(parsed["qualified_name"])
        except Exception:
            pass
    out["qnames"] = list(dict.fromkeys(out["qnames"]))[:5]

    def pad(lst, fallback):
        while len(lst) < 5:
            lst.append(fallback)
        return lst

    out["ids"] = pad(out["ids"], "__missing__")
    out["qnames"] = pad(out["qnames"], "__missing__")
    out["names"] = pad(out["names"], "main")
    out["files"] = pad(out["files"], "src/lib.rs")
    return out


def classify(resp, dt) -> tuple[str, str]:
    if resp is None:
        return "BAD", "no response"
    if isinstance(resp, dict) and resp.get("_timeout"):
        return "TIMEOUT", f"{dt:.1f}s"
    if "error" in resp:
        err = resp["error"]
        msg = err.get("message", "") if isinstance(err, dict) else str(err)
        return "ERROR", msg[:100]
    res = resp.get("result")
    if res is None:
        return "BAD", "no result"
    if res.get("isError"):
        c = res.get("content") or [{}]
        return "ERROR", (c[0].get("text") or "")[:100]
    c = res.get("content")
    if not c:
        return "EMPTY", "no content"
    txt = c[0].get("text", "")
    if not txt.strip():
        return "EMPTY", "blank"
    if dt > 10:
        return "SLOW", f"{dt:.1f}s"
    return "OK", f"{dt:.2f}s"


# ---------------------------------------------------------------------------- main

def load_repos() -> list[dict]:
    with open(REPOS_CONF, "rb") as f:
        return tomllib.load(f).get("repos", [])


def load_probe_set(languages: list[str], discovered: dict) -> dict:
    merged: dict = {}
    for lang in languages:
        try:
            mod = importlib.import_module(f"tools.{lang}")
        except ImportError:
            print(f"warning: no probe module for language '{lang}'", file=sys.stderr)
            continue
        merged.update(mod.probes_for(discovered))
    return merged


def main() -> int:
    if not BIN.exists():
        print(f"error: tokensave binary not found at {BIN}", file=sys.stderr)
        print(f"hint: run `cargo build --release` first", file=sys.stderr)
        return 2

    sys.path.insert(0, str(ROOT))
    repos = load_repos()
    if not repos:
        print(f"error: no repos in {REPOS_CONF}", file=sys.stderr)
        return 2

    log = open(LOG_PATH, "w")
    for repo in repos:
        name = repo["name"]
        path = repo["path"]
        if not Path(path).exists():
            print(f"skip {name}: {path} not found", file=sys.stderr)
            continue
        print(f"=== {name} ({path}) ===", file=sys.stderr, flush=True)
        try:
            cli = McpClient(path, repo_name=name)
        except McpInitError as exc:
            # Skip the repo but record the failure in the matrix so it
            # surfaces in build_matrix.py instead of just vanishing.
            print(f"skip {name}: {exc.hint} — see {exc.stderr_path}", file=sys.stderr)
            log.write(f"{name}\t_init_\t{{}}\tBAD\t{exc.hint}\n")
            log.flush()
            continue
        try:
            discovered = discover(cli)
            probes = load_probe_set(repo.get("languages", []), discovered)
            for tool, queries in probes.items():
                for query in queries:
                    resp, dt = cli.call(tool, query, timeout=TIMEOUT)
                    st, detail = classify(resp, dt)
                    log.write(f"{name}\t{tool}\t{json.dumps(query)[:120]}\t{st}\t{detail}\n")
                    log.flush()
        finally:
            cli.close()
    log.close()
    print(f"log written to {LOG_PATH}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
