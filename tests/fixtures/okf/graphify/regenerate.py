"""Regenerate the Graphify OKF fixture with a real Graphify checkout.

Usage: python3 regenerate.py <graphify-okf checkout> <new output directory>

Requires the checkout's dependencies (networkx, PyYAML). The output directory
must not exist. Compare it with ``bundle/`` using ``diff -r``.
"""

import json
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    checkout, output = Path(sys.argv[1]).resolve(), Path(sys.argv[2])
    if output.exists():
        print(f"{output} already exists", file=sys.stderr)
        return 2
    sys.path.insert(0, str(checkout))
    from graphify.build import build_from_json
    from graphify.export import to_okf_obsidian

    extraction = checkout / "tests" / "fixtures" / "extraction.json"
    graph = build_from_json(json.loads(extraction.read_text(encoding="utf-8")))
    communities = {0: ["n_transformer", "n_attention"], 1: ["n_layernorm", "n_concept_attn"]}
    labels = {0: "Model Architecture", 1: "Paper Concepts"}
    notes = to_okf_obsidian(graph, communities, str(output), community_labels=labels)
    print(f"wrote {notes} notes to {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
