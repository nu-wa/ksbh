#!/usr/bin/env bash
# Resolve the ksbh-bench binary the same way the runner does:
#   1. $KSBH_BENCH_BIN if set
#   2. PATH lookup
#   3. crates/target/debug/ksbh-bench (the `mise run build-rust` output)
# Then exec it with the passed args.
set -euo pipefail

KSBH_BENCH_BIN="${KSBH_BENCH_BIN:-}"
if [[ -n "$KSBH_BENCH_BIN" ]]; then
  if [[ ! -x "$KSBH_BENCH_BIN" ]]; then
    echo "error: KSBH_BENCH_BIN points to a non-executable: $KSBH_BENCH_BIN" >&2
    exit 3
  fi
elif command -v ksbh-bench >/dev/null 2>&1; then
  KSBH_BENCH_BIN="ksbh-bench"
elif [[ -x "crates/target/debug/ksbh-bench" ]]; then
  KSBH_BENCH_BIN="crates/target/debug/ksbh-bench"
else
  echo "error: ksbh-bench not found" >&2
  echo "  run 'mise run build-rust' first, install it on PATH, or set KSBH_BENCH_BIN=/path/to/ksbh-bench" >&2
  exit 3
fi

exec "$KSBH_BENCH_BIN" "$@"
