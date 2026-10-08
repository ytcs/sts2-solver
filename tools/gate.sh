#!/usr/bin/env bash
# Accuracy gate: every change must pass this unchanged. Exit 0 = pass.
# Checksums are bit-identity of the search engine and the PPO env (v1 and v2 observations); a deliberate behaviour change updates them here.
set -u
cd "$(dirname "$0")/.."
PY=.venv/Scripts/python.exe; [ -x "$PY" ] || PY=.venv/bin/python
fail=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2' want '$3'"; fail=1; fi; }
sum() { grep -o "checksum [0-9a-f]*" | head -1 | cut -d' ' -f2; }

check "search v1" "$(cargo run --release -q -p sts2env --example searchprof -- data/bench/mix.json 128 8 64 5 32 1 2>/dev/null | sum)" 17264c97764b59ec
check "search v2" "$(cargo run --release -q -p sts2env --example searchprof -- data/bench/mix.json 128 8 64 5 32 2 2>/dev/null | sum)" effb395521d20429
check "env v1" "$(cargo run --release -q -p sts2env --example envprof -- data/train/eval.json 256 100 1 600 2>/dev/null | sum)" 9a698c8be8c964fc
check "env v2" "$(cargo run --release -q -p sts2env --example envprof -- data/train/eval.json 256 100 2 600 2>/dev/null | sum)" 0112d5c3d8f99835

# real-game oracle traces, RNG goldens, information contract, live sync, look-ahead cache exactness
for t in "sts2diff regression" "sts2sim observe" "sts2sim rng_golden" "sts2sim sync" "sts2env lookahead_cache"; do
  set -- $t
  if cargo test --release -q -p "$1" --test "$2" >/dev/null 2>&1; then echo "ok   cargo $1/$2"; else echo "FAIL cargo $1/$2"; fail=1; fi
done

if "$PY" tests/rl/test_obs_version.py >/dev/null 2>&1; then echo "ok   obs v1 identity"; else echo "FAIL obs v1 identity"; fail=1; fi
[ $fail -eq 0 ] && echo "GATE PASS" || echo "GATE FAIL"
exit $fail
