#!/usr/bin/env bash
# Round r10 (observation v4) stages, run by scripts/pod_job.sh as $INPUTS/job.sh:  STAGE=collect|train RUN=<name> bash scripts/pod_r10.sh
# collect: $INPUTS/pool.json (+ optional $INPUTS/starts.json, one entry per fight) with $INPUTS/init.pt -> target/round/$RUN_NNN.npz, collect.log
# train:   $INPUTS/init.pt + $INPUTS/base/*.npz (remapped r9 parts) + $INPUTS/new/*.npz -> target/round/gen2_$RUN.pt, train.log; then the gate
#          (death bench, sign gate, score; GATE=full adds play + near-miss) for r9 vs the new net -> target/round/gate/
set -euo pipefail
cd "$(dirname "$0")/.."
export STS2_OBS=4
. scripts/pod_setup.sh
python -c "import sts2; assert sts2.OBS_VERSION == 4 and sts2.ACTIONS == 322, (sts2.OBS_VERSION, sts2.ACTIONS)"
INPUTS=${INPUTS:-/root/inputs}
RUN=${RUN:?RUN names the shard or the round}
INIT=${INIT:-$INPUTS/init.pt}
mkdir -p target/round
case "${STAGE:?collect or train}" in
collect)
  POOL=${POOL:-$INPUTS/pool.json}
  STARTS=${STARTS:-$INPUTS/starts.json}
  gpu_mb=$(nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits | head -1)
  ram_gb=$(free -g | awk '/^Mem:/{print $2}')
  lim=$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo max)
  if [[ $lim =~ ^[0-9]+$ ]] && [ $((lim >> 30)) -lt "$ram_gb" ]; then ram_gb=$((lim >> 30)); fi
  if [ -z "${ROOTS:-}" ]; then
    if [ "$gpu_mb" -ge 30000 ] && [ "$ram_gb" -ge 56 ]; then ROOTS=2048
    elif [ "$gpu_mb" -ge 11000 ] && [ "$ram_gb" -ge 24 ]; then ROOTS=1024
    else ROOTS=512; fi
  fi
  CHUNK=${CHUNK:-$((2 * ROOTS))}
  CPP=$(( 12288 / CHUNK > 0 ? 12288 / CHUNK : 1 ))
  echo "$(date -u +%FT%TZ) collect $RUN: $(python -c "import json,sys; print(len(json.load(open(sys.argv[1]))))" "$POOL") fights, GPU ${gpu_mb} MB, RAM ${ram_gb} GB, $(nproc) CPUs, roots $ROOTS, chunk $CHUNK" | tee -a target/round/collect.log
  bash tools/collect.sh --ckpt "$INIT" --fights "$POOL" $([ -f "$STARTS" ] && echo --starts "$STARTS") --out "target/round/$RUN.npz" --cover --K "${K:-32}" \
    --futures 0 --attempts "${ATTEMPTS:-1}" --chunk "$CHUNK" --roots "$ROOTS" --chunks-per-process "$CPP" --chunk-timeout 3 --max-minutes 100000 \
    --seed "${SEED:-110}" >>target/round/collect.log 2>&1
  ;;
train)
  OUT=target/round/gen2_$RUN.pt
  python -u rl/exit.py train --init "$INIT" --data "$INPUTS"/base/*.npz "$INPUTS"/new/*.npz --out "$OUT" --epochs "${EPOCHS:-3}" --value-target td \
    --lam 0.95 --lam-death "${LAM_DEATH:-1.0}" --pol 3 --c 2 --lr 1e-4 --lr-floor 0.1 --chunk 2048 --mb 2048 >target/round/train.log 2>&1
  grep holdout target/round/train.log
  G=target/round/gate
  mkdir -p "$G"
  REF=${REF:-models/solver_r9.pt}
  ( python tools/death_gate.py --ckpt "$OUT" --ref "$REF" --out "$G/death.json" || true ) >"$G/death.log" 2>&1
  for ck in "$REF" "$OUT"; do
    python tools/power_coverage.py gate --parts "$INPUTS/base/r9_00[0-5].npz" --ckpt "$ck" --out "$G/sign_$(basename "$ck" .pt).json" >>"$G/sign.log" 2>&1
  done
  python tools/bench.py score "$REF" "$OUT" >"$G/score.log" 2>&1
  if [ "${GATE:-quick}" = full ]; then
    python tools/bench.py play "$REF" "$OUT" --cover >"$G/play.log" 2>&1
    python tools/nearmiss_bench.py eval "$REF" "$OUT" >"$G/nearmiss.log" 2>&1
  fi
  ;;
esac
echo "$(date -u +%FT%TZ) pod_r10 $STAGE $RUN done"
