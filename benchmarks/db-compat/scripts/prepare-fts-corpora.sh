#!/usr/bin/env bash
# Builds the FTS comparison corpora listed in ../corpora.tsv.
#
#   scripts/prepare-fts-corpora.sh <cache-dir>
#
# Clones each repository at its pinned ref into <cache-dir>, indexes it with
# the installed `tokensave` (results were produced with 7.15.0), and exports
# the FTS-relevant rows to target/corpora/<name>.jsonl. The `tokensave`
# corpus is this repository at the pinned commit, without vendor/ and
# codegraph/. Prints whether each export matches the recorded SHA-256.
set -euo pipefail

cache=${1:?usage: prepare-fts-corpora.sh <cache-dir>}
crate=$(cd "$(dirname "$0")/.." && pwd)
repo_root=$(cd "$crate/../.." && pwd)
out="$crate/target/corpora"
mkdir -p "$cache" "$out"

version=$(tokensave --version | awk '{print $2}')
[ "$version" = "7.15.0" ] || echo "warning: tokensave $version (results used 7.15.0); hashes may differ" >&2

(cd "$crate" && cargo build --release --bin fts_compare)
bin="$crate/target/release/fts_compare"

grep -v '^#' "$crate/corpora.tsv" | while IFS=$'\t' read -r name _lang _size source ref commit sha; do
    dir="$cache/$name"
    if [ ! -d "$dir/.tokensave" ]; then
        rm -rf "$dir"
        if [ "$name" = tokensave ]; then
            mkdir -p "$dir"
            git -C "$repo_root" archive "$commit" | tar -x -C "$dir"
            rm -rf "$dir/vendor" "$dir/codegraph"
            git -C "$dir" init -q
        else
            git -c advice.detachedHead=false clone -q --depth 1 --branch "$ref" "$source" "$dir"
            [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ] || { echo "$name: $ref is not $commit" >&2; exit 1; }
        fi
        tokensave init --no-git-hook "$dir" < /dev/null > "$cache/$name.init.log" 2>&1
    fi
    "$bin" export "$dir/.tokensave/tokensave.db" "$out/$name.jsonl"
    got=$(sha256sum "$out/$name.jsonl" | awk '{print $1}')
    if [ "$got" = "$sha" ]; then echo "$name: sha256 matches"; else echo "$name: sha256 differs ($got)"; fi
done
