#!/usr/bin/env bash
# Combat-loop round on a fresh pod, from the repo root: nohup bash scripts/pod_round.sh >/dev/null 2>&1 &
# Builds, then waits for $INPUTS/READY (upload pool.json, one init .pt, optional extra .npz parts, then touch READY),
# collects with the cover search, trains value TD(lam) from the init checkpoint. Logs and the DONE / FAILED flag in target/round/.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p target/round
exec >>target/round/pod.log 2>&1
trap 'echo "FAILED at line $LINENO: $BASH_COMMAND"; echo "line $LINENO: $BASH_COMMAND" >target/round/FAILED' ERR

INPUTS=${INPUTS:-/root/inputs}
RUN=${RUN:-r5}
K=${K:-32}
FUTURES=${FUTURES:-0}
ATTEMPTS=${ATTEMPTS:-1}
SEED=${SEED:-105}
EPOCHS=${EPOCHS:-3}
LAM=${LAM:-0.8}
TRAIN_CHUNK=${TRAIN_CHUNK:-2048}
COLLECT_TRIES=${COLLECT_TRIES:-3}
export STS2_DEVICE=cuda
echo "== $(date -u +%FT%TZ) pod_round $RUN start"

if ! command -v cargo >/dev/null; then
  [ -f "$HOME/.cargo/env" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
[ -d .venv ] || python -m venv .venv --system-site-packages
. .venv/bin/activate
if ! python -c "import sts2" 2>/dev/null; then
  pip install -q maturin numpy
  maturin develop --release -m crates/sts2py/Cargo.toml
fi
python -c "import sts2, torch; print('sts2 ok; torch', torch.__version__, 'cuda', torch.cuda.is_available()); assert torch.cuda.is_available()"

echo "waiting for $INPUTS/READY"
until [ -f "$INPUTS/READY" ]; do sleep 30; done
POOL=${POOL:-$INPUTS/pool.json}
CKPT=${CKPT:-$(ls "$INPUTS"/*.pt | head -1)}
EXTRA=${EXTRA:-$(ls "$INPUTS"/*.npz 2>/dev/null | tr '\n' ' ' || true)}
OUT_CKPT=${OUT_CKPT:-target/round/gen2_$RUN.pt}

gpu_mb=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits | head -1)
ram_gb=$(free -g | awk '/^Mem:/{print $2}')
lim=$(cat /sys/fs/cgroup/memory.max 2>/dev/null || cat /sys/fs/cgroup/memory/memory.limit_in_bytes 2>/dev/null || echo max)
if [[ $lim =~ ^[0-9]+$ ]] && [ $((lim >> 30)) -lt "$ram_gb" ]; then ram_gb=$((lim >> 30)); fi
# E32: a d256 cover search at 1024 roots takes ~12 GB of GPU memory and ~20 GB of host RAM; past 2048 roots there is no gain (docs/solver.md)
if [ -z "${ROOTS:-}" ]; then
  if [ "$gpu_mb" -ge 30000 ] && [ "$ram_gb" -ge 56 ]; then ROOTS=2048
  elif [ "$gpu_mb" -ge 11000 ] && [ "$ram_gb" -ge 24 ]; then ROOTS=1024
  else ROOTS=512; fi
fi
n_fights=$(python -c "import json, sys; print(len(json.load(open(sys.argv[1]))))" "$POOL")
# a chunk caps the live roots (one engine per group gets chunk / 2 jobs); a chunk above the roots refills slots as fights end.
# Fresh process every ~12k fights: E6 saw a long-lived search process slow down 3x after ~9k.
sizes() {
  CHUNK=${CHUNK_SET:-$((2 * ROOTS))}
  CPP=${CHUNKS_PER_PROCESS:-$(( 12288 / CHUNK > 0 ? 12288 / CHUNK : 1 ))}
  n_parts=$(( (n_fights * ATTEMPTS + CHUNK - 1) / CHUNK ))
}
CHUNK_SET=${CHUNK:-}
sizes
echo "inputs: pool $POOL ($n_fights fights), init $CKPT, extra [${EXTRA}]; GPU ${gpu_mb} MB, RAM ${ram_gb} GB, $(nproc) CPUs -> roots $ROOTS, chunk $CHUNK, $n_parts parts, $CPP chunks per process; search cover K=$K futures=$FUTURES"

parts() { find target/round -maxdepth 1 -name "${RUN}_[0-9][0-9][0-9].npz" | wc -l; }
for try in $(seq 1 "$COLLECT_TRIES"); do
  [ "$(parts)" -ge "$n_parts" ] && break
  echo "collect try $try ($(parts)/$n_parts parts present)"
  rc=0
  skip=$([ "$try" -gt 1 ] && echo --skip-stuck || true)
  bash tools/collect.sh --ckpt "$CKPT" --fights "$POOL" --out "target/round/$RUN.npz" --cover --K "$K" --futures "$FUTURES" --attempts "$ATTEMPTS" \
    --chunk "$CHUNK" --roots "$ROOTS" --chunks-per-process "$CPP" --chunk-timeout "${CHUNK_TIMEOUT:-3}" --max-minutes 100000 --seed "$SEED" $skip >>target/round/collect.log 2>&1 || rc=$?
  echo "collect exit $rc"
  if [ "$rc" -ne 0 ] && [ "$rc" -ne 2 ] && [ "$rc" -ne 3 ] && [ "$(parts)" -eq 0 ] && [ "$ROOTS" -gt 256 ]; then
    ROOTS=$((ROOTS / 2)); CHUNK_SET=${CHUNK_SET:+$((CHUNK_SET / 2))}; sizes
    echo "no part saved (out of memory?): retrying at roots $ROOTS, chunk $CHUNK, $n_parts parts"
  fi
done
have=$(parts)
status=complete
[ "$have" -ge "$n_parts" ] || status="partial ($have of $n_parts parts; stuck chunks in target/round/${RUN}_stuck_*.json)"
echo "collection $status: $(grep -c '^chunk' target/round/collect.log || true) chunk lines in collect.log"
[ "$have" -gt 0 ]

python -u rl/exit.py train --init "$CKPT" --data target/round/"$RUN"_[0-9][0-9][0-9].npz $EXTRA --out "$OUT_CKPT" --epochs "$EPOCHS" \
  --value-target td --lam "$LAM" --chunk "$TRAIN_CHUNK" >target/round/train.log 2>&1
grep holdout target/round/train.log
echo "collection $status; $OUT_CKPT; $(grep holdout target/round/train.log | tail -1)" >target/round/DONE
echo "== $(date -u +%FT%TZ) pod_round $RUN done"
