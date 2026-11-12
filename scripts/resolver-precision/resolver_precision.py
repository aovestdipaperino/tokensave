#!/usr/bin/env python3
"""Compare tokensave rename sites with pinned language-server reference results."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from typing import Any
from urllib.parse import unquote, urlparse


ROOT = Path(__file__).resolve().parent
MANIFEST = ROOT / "manifest.json"


class HarnessError(RuntimeError):
    pass


def executable_command(command: list[str]) -> list[str]:
    executable = shutil.which(command[0])
    if executable is None:
        raise HarnessError(f"required executable not found on PATH: {command[0]}")
    resolved = [executable, *command[1:]]
    if os.name == "nt" and Path(executable).suffix.lower() in (".cmd", ".bat"):
        return [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/s", "/c", *resolved]
    return resolved


def fixture_digest(path: Path) -> str:
    digest = hashlib.sha256()
    for item in sorted(p for p in path.rglob("*") if p.is_file()):
        relative = item.relative_to(path).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(item.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def canonical_site(path: str, line: int, column: int) -> tuple[str, int, int]:
    return (Path(path).as_posix(), int(line), int(column))


def score_sites(
    predicted: list[dict[str, Any]], oracle: set[tuple[str, int, int]]
) -> dict[str, Any]:
    located = {
        canonical_site(site["file"], site["line"], site["column"])
        for site in predicted
        if site.get("column", 0) > 0 and site.get("kind") != "definition"
    }
    true_positive = len(located & oracle)
    false_positive = len(located - oracle)
    false_negative = len(oracle - located)
    return {
        "predicted": len(located),
        "oracle": len(oracle),
        "true_positive": true_positive,
        "false_positive": false_positive,
        "false_negative": false_negative,
        "precision": ratio(true_positive, true_positive + false_positive),
        "recall": ratio(true_positive, true_positive + false_negative),
    }


def ratio(numerator: int, denominator: int) -> float | None:
    return None if denominator == 0 else round(numerator / denominator, 6)


def aggregate(rows: list[dict[str, Any]], keys: tuple[str, ...]) -> list[dict[str, Any]]:
    buckets: dict[tuple[str, ...], dict[str, Any]] = {}
    for row in rows:
        key = tuple(str(row[name]) for name in keys)
        bucket = buckets.setdefault(
            key,
            {**dict(zip(keys, key)), "predicted": 0, "oracle": 0, "true_positive": 0,
             "false_positive": 0, "false_negative": 0},
        )
        for name in ("predicted", "oracle", "true_positive", "false_positive", "false_negative"):
            bucket[name] += int(row[name])
    result = []
    for bucket in buckets.values():
        bucket["precision"] = ratio(
            bucket["true_positive"], bucket["true_positive"] + bucket["false_positive"]
        )
        bucket["recall"] = ratio(
            bucket["true_positive"], bucket["true_positive"] + bucket["false_negative"]
        )
        result.append(bucket)
    return sorted(result, key=lambda row: tuple(row[name] for name in keys))


class LspClient:
    def __init__(self, command: list[str], cwd: Path):
        self.process = subprocess.Popen(
            executable_command(command),
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if self.process.stdin is None or self.process.stdout is None:
            raise HarnessError("language server did not expose pipes")
        self.next_id = 1
        self.responses: dict[int, dict[str, Any]] = {}
        self.notifications: list[dict[str, Any]] = []
        self.stderr: list[bytes] = []
        self.reader = threading.Thread(target=self._read_loop, daemon=True)
        self.reader.start()
        if self.process.stderr is not None:
            threading.Thread(target=self._stderr_loop, daemon=True).start()

    def _stderr_loop(self) -> None:
        assert self.process.stderr is not None
        self.stderr.append(self.process.stderr.read())

    def _read_loop(self) -> None:
        assert self.process.stdout is not None
        while True:
            headers: dict[str, str] = {}
            while True:
                line = self.process.stdout.readline()
                if not line:
                    return
                if line in (b"\r\n", b"\n"):
                    break
                key, _, value = line.decode("ascii", errors="replace").partition(":")
                headers[key.lower()] = value.strip()
            length = int(headers.get("content-length", "0"))
            message = json.loads(self.process.stdout.read(length))
            if "id" in message:
                self.responses[int(message["id"])] = message
            else:
                self.notifications.append(message)

    def notify(self, method: str, params: dict[str, Any]) -> None:
        self._write({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method: str, params: dict[str, Any], timeout: float = 60.0) -> Any:
        import time

        request_id = self.next_id
        self.next_id += 1
        self._write({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            response = self.responses.pop(request_id, None)
            if response is not None:
                if "error" in response:
                    raise HarnessError(f"{method} failed: {response['error']}")
                return response.get("result")
            if self.process.poll() is not None:
                error = b"".join(self.stderr).decode(errors="replace")
                raise HarnessError(f"language server exited during {method}: {error}")
            time.sleep(0.01)
        raise HarnessError(f"language server timed out during {method}")

    def _write(self, message: dict[str, Any]) -> None:
        assert self.process.stdin is not None
        payload = json.dumps(message, separators=(",", ":")).encode()
        self.process.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
        self.process.stdin.flush()

    def close(self) -> None:
        try:
            self.request("shutdown", {}, timeout=10)
            self.notify("exit", {})
            self.process.wait(timeout=10)
        except (HarnessError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait()


def uri_to_path(uri: str, root: Path) -> str:
    parsed = urlparse(uri)
    path = unquote(parsed.path)
    if parsed.netloc:
        path = f"//{parsed.netloc}{path}"
    if re.match(r"^/[A-Za-z]:/", path):
        path = path[1:]
    return Path(path).resolve().relative_to(root.resolve()).as_posix()


def lsp_column_to_byte(file: Path, zero_line: int, utf16_column: int) -> int:
    line = file.read_text(encoding="utf-8").splitlines()[zero_line]
    units = 0
    characters = 0
    for character in line:
        width = len(character.encode("utf-16-le")) // 2
        if units + width > utf16_column:
            break
        units += width
        characters += 1
    return len(line[:characters].encode("utf-8")) + 1


def byte_column_to_lsp(file: Path, one_line: int, one_byte_column: int) -> int:
    line = file.read_text(encoding="utf-8").splitlines()[one_line - 1]
    prefix = line.encode("utf-8")[: one_byte_column - 1].decode("utf-8")
    return len(prefix.encode("utf-16-le")) // 2


def locations(result: Any, root: Path, definition: tuple[str, int, int]) -> set[tuple[str, int, int]]:
    if result is None:
        return set()
    if isinstance(result, dict):
        result = [result]
    found = set()
    for item in result:
        uri = item.get("uri") or item.get("targetUri")
        position = (item.get("range") or item.get("targetSelectionRange"))["start"]
        relative = uri_to_path(uri, root)
        site = canonical_site(
            relative,
            position["line"] + 1,
            lsp_column_to_byte(root / relative, position["line"], position["character"]),
        )
        if site != definition:
            found.add(site)
    return found


def normalized_locations(result: Any, root: Path) -> list[dict[str, Any]]:
    if result is None:
        return []
    if isinstance(result, dict):
        result = [result]
    normalized = []
    for item in result:
        uri = item.get("uri") or item.get("targetUri")
        position = (item.get("range") or item.get("targetSelectionRange"))["start"]
        relative = uri_to_path(uri, root)
        normalized.append(
            {
                "file": relative,
                "line": position["line"] + 1,
                "column": lsp_column_to_byte(
                    root / relative, position["line"], position["character"]
                ),
            }
        )
    return sorted(normalized, key=lambda site: (site["file"], site["line"], site["column"]))


def run_checked(command: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode != 0:
        raise HarnessError(
            f"command failed ({result.returncode}): {' '.join(command)}\n{result.stdout}{result.stderr}"
        )
    return result


def parse_tokensave_result(stdout: str) -> dict[str, Any]:
    envelope = json.loads(stdout)
    text = envelope["content"][0]["text"]
    return json.loads(text)


def rename_sites(plan: dict[str, Any]) -> list[dict[str, Any]]:
    sites = [site for group in plan.get("files", []) for site in group.get("sites", [])]
    sites.extend(plan.get("text_only", []))
    return sites


def verify_version(command: list[str], expected: str, cwd: Path) -> str:
    result = subprocess.run(executable_command(command), cwd=cwd, text=True, capture_output=True)
    actual = (result.stdout + result.stderr).strip()
    if result.returncode != 0 or expected not in actual:
        raise HarnessError(f"expected version containing {expected!r}, got {actual!r}")
    return actual


def evaluate_language(
    language: dict[str, Any], tokensave: Path, temp: Path, base_env: dict[str, str]
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    fixture = ROOT / language["fixture"]
    expected_digest = language["fixture_sha256"]
    actual_digest = fixture_digest(fixture)
    if actual_digest != expected_digest:
        raise HarnessError(
            f"{language['name']} fixture digest is {actual_digest}; update manifest intentionally"
        )
    project = temp / language["name"]
    shutil.copytree(fixture, project)
    env = dict(base_env)
    env["TOKENSAVE_STATE_DIR"] = str(temp / "state")
    run_checked([str(tokensave), "init", str(project)], project, env)

    server = [part.format(project=str(project)) for part in language["server_command"]]
    version_command = [part.format(project=str(project)) for part in language["version_command"]]
    version = verify_version(version_command, language["server_version"], project)
    dependency_versions = []
    for dependency in language.get("dependency_versions", []):
        command = [part.format(project=str(project)) for part in dependency["command"]]
        dependency_versions.append(
            verify_version(command, dependency["version"], project)
        )
    client = LspClient(server, project)
    root_uri = project.resolve().as_uri()
    initialize_params: dict[str, Any] = {
            "processId": os.getpid(),
            "rootUri": root_uri,
            "capabilities": {},
            "workspaceFolders": [{"uri": root_uri, "name": project.name}],
    }
    if language["name"] == "typescript":
        tsc = shutil.which("tsc")
        if tsc is None:
            client.close()
            raise HarnessError("tsc not found beside the pinned TypeScript server")
        initialize_params["initializationOptions"] = {
            "tsserver": {
                "path": str(
                    Path(tsc).resolve().parent.parent / "typescript" / "lib" / "tsserver.js"
                )
            }
        }
    try:
        client.request("initialize", initialize_params)
    except Exception:
        client.close()
        raise
    client.notify("initialized", {})
    probes = json.loads((project / "probes.json").read_text(encoding="utf-8"))
    opened: set[str] = set()
    rows: list[dict[str, Any]] = []
    raw_probes = []
    try:
        for probe in probes:
            relative = probe["file"]
            source = project / relative
            uri = source.resolve().as_uri()
            if relative not in opened:
                client.notify(
                    "textDocument/didOpen",
                    {"textDocument": {"uri": uri, "languageId": language["language_id"],
                                      "version": 1, "text": source.read_text(encoding="utf-8")}},
                )
                opened.add(relative)
            position = {
                "line": probe["line"] - 1,
                "character": byte_column_to_lsp(source, probe["line"], probe["column"]),
            }
            definition = canonical_site(relative, probe["line"], probe["column"])
            reference_params = {
                "textDocument": {"uri": uri},
                "position": position,
                "context": {"includeDeclaration": True},
            }
            deadline = time.monotonic() + 60
            refs_raw = None
            while not refs_raw and time.monotonic() < deadline:
                try:
                    refs_raw = client.request("textDocument/references", reference_params)
                except HarnessError as error:
                    if "-32801" not in str(error):
                        raise
                if refs_raw:
                    break
                time.sleep(0.25)
            if not refs_raw:
                raise HarnessError(
                    f"{language['name']} returned no references for fixture symbol {probe['symbol']}"
                )
            defs_raw = client.request(
                "textDocument/definition", {"textDocument": {"uri": uri}, "position": position}
            )
            oracle = locations(refs_raw, project, definition)
            definition_sites = locations(defs_raw, project, ("", -1, -1))
            rename = run_checked(
                [str(tokensave), "tool", "rename", "--project", str(project), "--symbol",
                 probe["symbol"], "--dry-run", "true", "--json"],
                project,
                env,
            )
            plan = parse_tokensave_result(rename.stdout)
            sites = rename_sites(plan)
            score = score_sites(sites, oracle)
            rows.append({"language": language["name"], "edge_kind": probe["edge_kind"],
                         "symbol": probe["symbol"], **score})
            for confidence in ("exact", "heuristic", "ambiguous", "text_only"):
                classified = [site for site in sites if site.get("confidence") == confidence]
                class_score = score_sites(classified, oracle)
                rows.append({"language": language["name"], "edge_kind": "rename_class",
                             "symbol": probe["symbol"], "rename_class": confidence, **class_score})
            raw_probes.append({"probe": probe,
                               "lsp_references": normalized_locations(refs_raw, project),
                               "lsp_definitions": normalized_locations(defs_raw, project),
                               "oracle_references": sorted(oracle),
                               "oracle_definitions": sorted(definition_sites), "tokensave": plan})
    finally:
        client.close()
    return rows, {"language": language["name"], "fixture_sha256": actual_digest,
                  "server_version_output": version,
                  "dependency_version_outputs": dependency_versions,
                  "probes": raw_probes}


def markdown_report(report: dict[str, Any]) -> str:
    lines = ["# Resolver precision result", "", f"Generated from manifest revision `{report['manifest_sha256']}`.", "",
             "## References by language and edge kind", "",
             "| Language | Edge kind | Predicted | Oracle | TP | FP | FN | Precision | Recall |",
             "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    for row in report["by_language_edge_kind"]:
        lines.append("| {language} | {edge_kind} | {predicted} | {oracle} | {true_positive} | "
                     "{false_positive} | {false_negative} | {precision} | {recall} |".format(**row))
    lines.extend(["", "## Rename classes", "",
                  "| Language | Class | Sites | Share | Oracle | TP | FP | Precision |",
                  "|---|---|---:|---:|---:|---:|---:|---:|"])
    for row in report["by_language_rename_class"]:
        lines.append("| {language} | {rename_class} | {predicted} | {share} | {oracle} | "
                     "{true_positive} | {false_positive} | {precision} |".format(**row))
    lines.extend(["", "The language servers are comparison oracles, not proof of semantic truth. See the harness README for limits.", ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tokensave", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--markdown", required=True, type=Path)
    parser.add_argument("--language", action="append", choices=("rust", "typescript", "python"))
    args = parser.parse_args()
    tokensave = args.tokensave.resolve()
    if not tokensave.is_file():
        parser.error(f"tokensave binary not found: {tokensave}")
    manifest_bytes = MANIFEST.read_bytes()
    manifest = json.loads(manifest_bytes)
    selected = set(args.language or ("rust", "typescript", "python"))
    base_env = dict(os.environ)
    base_env.update(TOKENSAVE_SKIP_AGENT_MAINTENANCE="1", TOKENSAVE_AUTO_TRACK="0")
    rows: list[dict[str, Any]] = []
    raw = []
    with tempfile.TemporaryDirectory(prefix="tokensave-resolver-precision-") as name:
        temp = Path(name)
        for language in manifest["languages"]:
            if language["name"] in selected:
                language_rows, language_raw = evaluate_language(language, tokensave, temp, base_env)
                rows.extend(language_rows)
                raw.append(language_raw)
    reference_rows = [row for row in rows if row["edge_kind"] != "rename_class"]
    class_rows = [row for row in rows if row["edge_kind"] == "rename_class"]
    rename_classes = aggregate(class_rows, ("language", "rename_class"))
    class_totals: dict[str, int] = {}
    for row in rename_classes:
        class_totals[row["language"]] = class_totals.get(row["language"], 0) + row["predicted"]
    for row in rename_classes:
        row["share"] = ratio(row["predicted"], class_totals[row["language"]])
    report = {
        "schema_version": 1,
        "manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
        "by_language_edge_kind": aggregate(reference_rows, ("language", "edge_kind")),
        "by_language_rename_class": rename_classes,
        "probes": reference_rows,
        "raw": raw,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.markdown.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    args.markdown.write_text(markdown_report(report), encoding="utf-8")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except HarnessError as error:
        print(f"resolver-precision: {error}", file=sys.stderr)
        raise SystemExit(1) from error
