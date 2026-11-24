#!/usr/bin/env bash
# Regenerate tests/fixtures/scip/ambiguous/index.scip (#1100).
#
# Requires a rust-analyzer with the `scip` subcommand (the committed fixture
# was produced by rust-analyzer 1.88.0). Set RUST_ANALYZER to choose a binary,
# e.g. RUST_ANALYZER="$(rustup which --toolchain 1.88 rust-analyzer)".
# The crate is indexed in a disposable copy; the committed index has its
# project_root replaced by file:///scip-fixture so it holds no local paths.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
fixture="$repo/tests/fixtures/scip/ambiguous"
ra="${RUST_ANALYZER:-rust-analyzer}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cp "$fixture/Cargo.toml.fixture" "$work/Cargo.toml"
cp -R "$fixture/src" "$work/"
(cd "$work" && "$ra" scip . --output "$work/raw.scip")
rm -f "$fixture/index.scip"
python3 -I "$repo/scripts/scip/normalize_index.py" "$work/raw.scip" "$fixture/index.scip"
"$ra" --version
echo "wrote $fixture/index.scip"
