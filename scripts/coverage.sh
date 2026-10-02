#!/bin/sh
set -eu

# The binary entrypoint is process/platform glue; all behavior it invokes lives in
# covered library modules. It is the sole production-source coverage exclusion.
RUSTUP_BIN="${RUSTUP_BIN:-/opt/homebrew/opt/rustup/bin}"
export PATH="$RUSTUP_BIN:/opt/homebrew/bin:$PATH"

mkdir -p coverage
cargo +nightly llvm-cov clean --workspace
cargo +nightly llvm-cov \
  --all-features \
  --workspace \
  --branch \
  --json \
  --output-path coverage/coverage.json \
  --ignore-filename-regex 'src/main\.rs$' \
  --fail-under-lines 90

algorithm_metrics="$(jq -r '
  [.data[].files[]
    | select(.filename | endswith("/src/algorithm.rs"))
    | [.summary.lines.percent, .summary.branches.percent]]
  | first
  | @tsv
' coverage/coverage.json)"

algorithm_lines="$(printf '%s' "$algorithm_metrics" | cut -f1)"
algorithm_branches="$(printf '%s' "$algorithm_metrics" | cut -f2)"

jq -e --argjson lines "$algorithm_lines" --argjson branches "$algorithm_branches" \
  '($lines >= 100) and ($branches >= 100)' >/dev/null <<'JSON'
null
JSON

printf 'Coverage gates passed: overall lines >= 90%%; algorithm lines=%s%% branches=%s%%\n' \
  "$algorithm_lines" "$algorithm_branches"
