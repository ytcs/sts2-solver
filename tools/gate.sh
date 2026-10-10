#!/usr/bin/env bash
# Accuracy gate: exit 0 = pass; only a deliberate behaviour change may update the checksums.
set -u
cd "$(dirname "$0")/.."
fail=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2' want '$3'"; fail=1; fi; }
sum() { grep -o "checksum [0-9a-f]*" | head -1 | cut -d' ' -f2; }

check "search" "$(cargo run --release -q -p sts2env --example searchprof -- data/bench/mix.json 128 8 64 5 32 2>/dev/null | sum)" effb395521d20429
check "search cap" "$(STS2_HP_CAP=1 cargo run --release -q -p sts2env --example searchprof -- data/bench/mix.json 128 8 64 5 32 2>/dev/null | sum)" a293f34d31e01ad3
check "env" "$(cargo run --release -q -p sts2env --example envprof -- data/train/eval.json 256 100 600 2>/dev/null | sum)" 0112d5c3d8f99835
# observation v3 (feature obs_v3: the v2 bytes + a tail; the stub policy hashes the whole row)
check "search v3" "$(cargo run --release -q -p sts2env --features obs_v3 --example searchprof -- data/bench/mix.json 128 8 64 5 32 2>/dev/null | sum)" 098a65da24ad9b75
check "env v3" "$(cargo run --release -q -p sts2env --features obs_v3 --example envprof -- data/train/eval.json 256 100 600 2>/dev/null | sum)" 3ae361cf467427fa
if cargo test --release -q -p sts2sim --features obs_v3 --test observe >/dev/null 2>&1; then echo "ok   cargo sts2sim/observe v3"; else echo "FAIL cargo sts2sim/observe v3"; fail=1; fi

for t in "sts2diff regression" "sts2sim observe" "sts2sim rng_golden" "sts2sim sync" "sts2env lookahead_cache"; do
  set -- $t
  if cargo test --release -q -p "$1" --test "$2" >/dev/null 2>&1; then echo "ok   cargo $1/$2"; else echo "FAIL cargo $1/$2"; fail=1; fi
done

[ $fail -eq 0 ] && echo "GATE PASS" || echo "GATE FAIL"
exit $fail
