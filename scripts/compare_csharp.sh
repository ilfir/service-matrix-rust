#!/bin/sh
set -eu

CSHARP_URL="${1:-http://127.0.0.1:18081}"
RUST_URL="${2:-http://127.0.0.1:18082}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/service-matrix-diff.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

request='{"maxLength":20,"minLength":3,"maxWords":10,"lettersMatrix":[["а","б","а","з","и"],["н","м","е","л","е"],["н","и","р","т","ь"],["в","у","л","у","ч"],["з","п","о","в","е"]]}'

curl -fsS -H 'content-type: application/json' --data "$request" \
  "$CSHARP_URL/words/Search" | jq -S . >"$WORK_DIR/csharp-search.json"
curl -fsS -H 'content-type: application/json' --data "$request" \
  "$RUST_URL/words/Search" | jq -S . >"$WORK_DIR/rust-search.json"
diff -u "$WORK_DIR/csharp-search.json" "$WORK_DIR/rust-search.json"

for include in true false; do
  curl -fsS "$CSHARP_URL/words/List?include=$include" | jq 'sort' >"$WORK_DIR/csharp-list-$include.json"
  curl -fsS "$RUST_URL/words/List?include=$include" | jq 'sort' >"$WORK_DIR/rust-list-$include.json"
  diff -u "$WORK_DIR/csharp-list-$include.json" "$WORK_DIR/rust-list-$include.json"
done

# Location labels intentionally differ: Rust reports the real source. Compare words.
curl -fsSG --data-urlencode 'word=абазин' --data 'exactMatch=true' "$CSHARP_URL/words/LookupWord" \
  | jq '[.[].word] | sort' >"$WORK_DIR/csharp-lookup.json"
curl -fsSG --data-urlencode 'word=абазин' --data 'exactMatch=true' "$RUST_URL/words/LookupWord" \
  | jq '[.[].word] | sort' >"$WORK_DIR/rust-lookup.json"
diff -u "$WORK_DIR/csharp-lookup.json" "$WORK_DIR/rust-lookup.json"

printf 'C# and Rust read-only contract comparison passed.\n'
