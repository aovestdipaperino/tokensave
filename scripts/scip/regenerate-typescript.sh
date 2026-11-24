#!/usr/bin/env bash
# Regenerate tests/fixtures/scip/typescript/index.scip (#1100).
#
# Requires scip-typescript (the committed fixture was produced by
# @sourcegraph/scip-typescript 0.6.1). Set SCIP_TYPESCRIPT to choose a binary,
# e.g. SCIP_TYPESCRIPT=/path/to/node_modules/.bin/scip-typescript.
# The project is indexed in a disposable copy; the committed index has its
# project_root replaced by file:///scip-fixture so it holds no local paths.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
fixture="$repo/tests/fixtures/scip/typescript"
indexer="${SCIP_TYPESCRIPT:-scip-typescript}"
work="$(mktemp -d "$fixture/.regenerate.XXXXXX")"
trap 'rm -rf "$work"' EXIT

mkdir "$work/project"
cp "$fixture/package.json" "$fixture/tsconfig.json" "$work/project/"
cp -R "$fixture/src" "$work/project/"
(cd "$work/project" && "$indexer" index --output "$work/raw.scip")
python3 -I "$repo/scripts/scip/normalize_index.py" "$work/raw.scip" "$work/index.scip"
"$indexer" --version
mv -f "$work/index.scip" "$fixture/index.scip"
echo "wrote $fixture/index.scip"
