#!/usr/bin/env bash
# One round of search distillation on a GPU pod:
#   1. NPROC parallel miners play fights with network+search and save every searched decision (rl/mine.py --save-all) plus the confirmed disagreements
#   2. rl/train_soft.py fine-tunes the network on all of it (soft targets over the searched options, value targets, anchor to the old policy)
#   3. greedy and search evaluation of the new network next to the old one
#   tools/runpod_distill.sh ROUND BASE_CKPT ROOTS CHUNKS      (env: NPROC=4, TRAIN=target/train/mine_mix.json)
set -u
cd "$(dirname "$0")/.."
PY=.venv/bin/python
R=${1:-1}; BASE=${2:-models/solver_b128.pt}; ROOTS=${3:-500}; CHUNKS=${4:-6}; NPROC=${NPROC:-4}
TRAIN=${TRAIN:-target/train/mine_mix.json}
export STS2_DEVICE=${STS2_DEVICE:-cuda} RAYON_NUM_THREADS=$(( ($(nproc) > 16 ? 16 : $(nproc)) / NPROC + 1 )) OMP_NUM_THREADS=2
mkdir -p target/exit
log() { echo "[$(date +%H:%M:%S)] $*" | tee -a target/exit/distill.log; }
log "round $R: mining with $BASE ($NPROC procs x $CHUNKS chunks x $ROOTS fights) on $TRAIN"
pids=()
for p in $(seq 1 $NPROC); do
  $PY -u rl/mine.py --ckpt $BASE --train $TRAIN --roots $ROOTS --chunks $CHUNKS --confirm 32 --pmin 0.03 --threads 2 --seed $((R * 1000 + p)) \
      --out target/exit/c${R}_$p.npz --save-all target/exit/all${R}_$p.npz > target/exit/m${R}_$p.log 2>&1 &
  pids+=($!)
done
wait "${pids[@]}"
log "mined: $($PY -c "import numpy as np,sys; print(sum(len(np.load(f)['z']) for f in sys.argv[1:]), 'searched decisions;', sum(len(np.load(f)['search']) for f in sys.argv[1:]) if False else '')" target/exit/all${R}_*.npz)"
$PY -u rl/train_soft.py --ckpt $BASE --data target/exit/all${R}_*.npz --mined target/exit/c${R}_*.npz --out models/solver_s$R.pt --epochs 3 --lr 1e-4 --mb 1024 --tau 0.1 --anchor 0.3 --min-gap 0.05 2>&1 | grep -v Warn | tee -a target/exit/distill.log
for m in $BASE models/solver_s$R.pt; do
  log "greedy eval of $m"
  $PY rl/baselines.py --eval target/train/mid.json --policies ckpt:$m --envs 786 --per-env 4 --threads 4 2>&1 | grep -v Warn | cut -c1-150 | tee -a target/exit/distill.log
  $PY rl/baselines.py --eval target/train/eval.json --policies ckpt:$m --envs 500 --per-env 3 --threads 4 2>&1 | grep -v Warn | cut -c1-150 | tee -a target/exit/distill.log
done
for m in $BASE models/solver_s$R.pt; do
  log "search eval of $m (mid set, 300 paired fights)"
  $PY -u rl/bench_search.py --ckpt $m --eval target/train/mid.json --roots 300 --threads 4 --greedy --configs "5,8,0,0,0,0,0" 2>&1 | grep -E "^M=|^greedy" | tee -a target/exit/distill.log
done
log "DISTILL_DONE $R"
