#!/usr/bin/env bash
# Unattended improvement loop for a GPU pod (DAgger-style): mine disagreements with the current network, fine-tune from the BASE network on
# everything mined so far, evaluate, repeat.
#   tools/runpod_loop.sh ROUNDS ROOTS_PER_CHUNK CHUNKS   (env: NPROC=3 parallel miners, BASE=models/solver_base.pt)
# Logs: target/exit/loop.log. Archives: target/exit/r<round>_<proc>.npz. Models: models/solver_r<round>.pt.
set -u
cd "$(dirname "$0")/.."
PY=.venv/bin/python
ROUNDS=${1:-3}; ROOTS=${2:-600}; CHUNKS=${3:-4}; NPROC=${NPROC:-3}
BASE=${BASE:-models/solver_base.pt}
TRAIN=${TRAIN:-target/train/mid_train.json}   # mining scenarios: must be disjoint from the evaluation sets (mid.json, eval.json)
CK=$BASE
ARCH=""
export STS2_DEVICE=${STS2_DEVICE:-cuda} RAYON_NUM_THREADS=$(( ($(nproc) > 16 ? 16 : $(nproc)) / NPROC + 1 )) OMP_NUM_THREADS=2
mkdir -p target/exit
log() { echo "[$(date +%H:%M:%S)] $*" | tee -a target/exit/loop.log; }
evalmodel() {
  log "greedy eval of $1"
  $PY rl/baselines.py --eval target/train/mid.json --policies ckpt:$2 --envs 786 --per-env 2 --threads 4 2>&1 | grep -v Warn | cut -c1-160 | tee -a target/exit/loop.log
  $PY rl/baselines.py --eval target/train/eval.json --policies ckpt:$2 --envs 500 --per-env 3 --threads 4 2>&1 | grep -v Warn | cut -c1-160 | tee -a target/exit/loop.log
}
searcheval() {
  log "search eval of $1 (mid-difficulty set, 300 paired fights)"
  $PY -u rl/bench_search.py --ckpt $2 --eval target/train/mid.json --roots 300 --threads 4 --configs "5,8,0,0,0,0,0" 2>&1 | grep "^M=" | tee -a target/exit/loop.log
}
log "start: rounds=$ROUNDS roots=$ROOTS chunks=$CHUNKS nproc=$NPROC base=$BASE"
evalmodel base $CK
for r in $(seq 1 $ROUNDS); do
  log "round $r: mining with $CK"
  pids=()
  for p in $(seq 1 $NPROC); do
    $PY -u rl/mine.py --ckpt $CK --train $TRAIN --roots $ROOTS --chunks $CHUNKS --confirm 32 --pmin 0.03 --threads 2 --seed $((r * 100 + p)) --out target/exit/r${r}_$p.npz > target/exit/r${r}_$p.log 2>&1 &
    pids+=($!)
  done
  wait "${pids[@]}"
  ARCH="$ARCH $(ls target/exit/r${r}_*.npz 2>/dev/null | tr '\n' ' ')"
  log "round $r mined: $($PY -c "import numpy as np,sys; print(sum(len(np.load(f)['search']) for f in sys.argv[1:]), 'confirmed states in', len(sys.argv)-1, 'archives')" $ARCH)"
  (unset STS2_DEVICE; $PY -u rl/train_mined.py --ckpt $BASE --mined $ARCH --states $TRAIN --out models/solver_r$r.pt --epochs 12 --lr 1e-4 --anchors 30000 --threads 12) 2>&1 | grep -v Warn | tail -4 | tee -a target/exit/loop.log
  CK=models/solver_r$r.pt
  evalmodel r$r $CK
done
searcheval base $BASE
searcheval final $CK
log "LOOP_DONE"
