#!/usr/bin/env python3
"""Run a Linux-only, synthetic Ossie-to-OKF retrieval comparison."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import sys
import tempfile

import tiktoken
import yaml
from ossie_nvidia_auto_ontology import (
    convert_auto_ontology_to_ossie,
    convert_auto_ontology_to_ossie_ontology,
)

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "mcp_probe"))
import probe

FIXTURE = HERE / "fixture"
# A human supplies these associations. Neither catalog format can infer them.
CODE_LINKS = {
    "NetBookedRevenue": "book_invoice",
    "ActiveSubscriber": "renew_subscription",
    "PaymentAttempt": "retry_payment",
}
TASKS = (
    ("payment-code", "retry_payment", None, None),
    ("revenue-definition-and-code", "book_invoice", "Net Booked Revenue",
     "Deduct promotional credits and exclude sales tax."),
    ("subscriber-definition-and-code", "renew_subscription", "Active Subscriber",
     "Count only paid subscriptions and exclude trial accounts."),
)


def project_fixture(bundle):
    """Project just the fixture's display text and source pointer into OKF."""
    source = (FIXTURE / "catalog.yaml").read_bytes()
    digest = hashlib.sha256(source).hexdigest()
    model = yaml.safe_load(convert_auto_ontology_to_ossie(source.decode()))
    ontology, mapping = map(yaml.safe_load, convert_auto_ontology_to_ossie_ontology(
        source.decode(), semantic_model_iri="./semantic_model.yaml", ontology_iri="./ontology.yaml",
    ))
    datasets = {item["name"]: item for item in model["datasets"]}
    mappings = {
        item["concept"]: item["object_mappings"][0]["expression"].split(".", 1)[0]
        for item in mapping["concept_mappings"]
    }
    bundle.mkdir()
    index = ['---\nokf_version: "0.2"\n---\n', "# Billing catalog\n"]
    for item in ontology["ontology"]:
        concept = item["concept"]
        dataset = datasets[mappings[concept]]
        metadata = {
            "type": "Catalog Term",
            "title": dataset["name"].replace("_", " ").title(),
            "resource": "billing.py",
            "catalog_concept": concept,
            "catalog_dataset": dataset["source"],
            "catalog_fixture_sha256": digest,
        }
        text = yaml.safe_dump(metadata, sort_keys=False)
        body = f'{item["description"]}\n\nCode entry point: `{CODE_LINKS[concept]}`.\n'
        (bundle / f"{concept}.md").write_text(f"---\n{text}---\n\n{body}", encoding="utf-8")
        index.append(f"- [{metadata['title']}]({concept}.md)")
    (bundle / "index.md").write_text("\n".join(index) + "\n", encoding="utf-8")


def tool(client, name, **arguments):
    response, duration = client.call(name, arguments)
    result = (response or {}).get("result", {})
    if "content" not in result or result.get("isError"):
        raise RuntimeError(f"{name} failed: {response}")
    return result["content"][0]["text"], duration * 1000


def run_task(client, task, enriched, encoding):
    label, symbol, term, definition = task
    calls = [tool(client, "tokensave_search", query=symbol, limit=3)]
    code = symbol in calls[0][0] and "billing.py" in calls[0][0]
    found_link = False
    if enriched and term:
        calls.append(tool(client, "tokensave_knowledge", query=term, limit=1))
        hits = json.loads(calls[-1][0])["results"]
        if hits:
            top = hits[0]
            calls.append(tool(client, "tokensave_knowledge", concept=top["id"], bundle=top["bundle"]))
            concept = json.loads(calls[-1][0])
            found_link = (
                definition in concept["body"]
                and f"`{symbol}`" in concept["body"]
                and {"path": "billing.py", "indexed": True} in concept["code_files"]
            )
    elif term:
        calls.append(tool(client, "tokensave_search", query=term, literal=True, limit=3))
    found_definition = bool(definition and any(definition in text for text, _ in calls))
    facts = int(code) + int(found_definition) + int(found_link)
    expected = 3 if term else 1
    return {
        "task": label,
        "facts": facts,
        "expected": expected,
        "calls": len(calls),
        "content_tokens": sum(len(encoding.encode_ordinary(text)) for text, _ in calls),
        "latency_ms": sum(duration for _, duration in calls),
    }


def run_arm(root, enriched, samples, encoding):
    client = probe.McpClient(str(root), "okf" if enriched else "baseline")
    try:
        first = [run_task(client, task, enriched, encoding) for task in TASKS]
        warm = [[run_task(client, task, enriched, encoding) for task in TASKS] for _ in range(samples)]
        rss = Path(f"/proc/{client.proc.pid}/status").read_text(encoding="utf-8")
        peak_kib = next(int(line.split()[1]) for line in rss.splitlines() if line.startswith("VmHWM:"))
    finally:
        client.close()
        client.proc.wait(timeout=5)
    if any(sample["facts"] != (sample["expected"] if enriched else 1)
           for trial in [first, *warm] for sample in trial):
        raise RuntimeError(f"Unexpected retrieval result for {root.name}: {first!r}")
    return {
        "peak_server_rss_kib": peak_kib,
        "first_pass": first,
        "warm_latency_ms": [
            {"task": task[0], "median": statistics.median(trial[i]["latency_ms"] for trial in warm)}
            for i, task in enumerate(TASKS)
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tokensave", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=20, choices=range(1, 101), metavar="1–100")
    args = parser.parse_args()
    probe.BIN = args.tokensave.resolve(strict=True)
    encoding = tiktoken.get_encoding("cl100k_base")

    with tempfile.TemporaryDirectory(prefix="tokensave-ontology-") as work:
        work = Path(work)
        # Isolate the CLI and MCP server from real settings, state, and hooks.
        for key in list(os.environ):
            if key.startswith(("TOKENSAVE_", "GIT_", "XDG_")):
                del os.environ[key]
        os.environ.update({
            "TOKENSAVE_STATE_DIR": "",
            "TOKENSAVE_AUTO_TRACK": "0",
            "GIT_CONFIG_NOSYSTEM": "1",
        })
        probe.STDERR_DIR = work / "logs"

        def cli(*command, cwd=work):
            result = subprocess.run([probe.BIN, *command], cwd=cwd, text=True,
                                    capture_output=True, timeout=60)
            if result.returncode:
                raise RuntimeError(f"{command[:2]} failed ({result.returncode}): {result.stderr}")
            return result.stdout.strip()

        (work / "home").mkdir()
        os.environ["HOME"] = str(work / "home")
        bundle = work / "bundle"
        project_fixture(bundle)
        cli("okf", "validate", str(bundle), "--strict")
        report = {"binary": cli("--version", "--verbose"), "warm_samples": args.samples}
        for label in ("baseline", "okf"):
            home = work / f"{label}-home"
            home.mkdir()
            os.environ["HOME"] = str(home)
            root = work / label
            root.mkdir()
            subprocess.run(["git", "init", "-q"], cwd=root, check=True, timeout=10)
            shutil.copyfile(FIXTURE / "billing.py", root / "billing.py")
            cli("init", "--no-git-hook", "--cpu-usage", "low", cwd=root)
            if label == "okf":
                cli("okf", "import", str(bundle), "--name", "billing", "--strict", cwd=root)
            arm = run_arm(root, label == "okf", args.samples, encoding)
            state = root / ".tokensave"
            arm["index_bytes"] = sum(path.stat().st_size for path in state.rglob("*")
                                     if path.is_file() and path.name.endswith((".db", ".db-wal", ".db-shm")))
            arm["okf_bytes"] = sum(path.stat().st_size for path in (state / "okf").rglob("*")
                                   if path.is_file())
            report[label] = arm
        if not report["baseline"]["index_bytes"] == report["okf"]["index_bytes"] > 0:
            raise RuntimeError("Expected equal, nonempty code indexes in both arms")
        if not report["okf"]["okf_bytes"] or report["baseline"]["okf_bytes"]:
            raise RuntimeError("Expected only the enriched arm to store an OKF bundle")
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
