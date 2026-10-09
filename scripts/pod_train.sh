#!/usr/bin/env bash
# Usage: [ITERS=200 N_TRAIN=3000] bash scripts/pod_train.sh  (from the repo root; output: target/runs/$RUN/)
set -euo pipefail
RUN=${RUN:-fresh_$(date +%Y%m%d)}
ITERS=${ITERS:-6400}
N_TRAIN=${N_TRAIN:-30000}
N_EVAL=${N_EVAL:-1500}
WIDTH=${WIDTH:-128}
HOLD=${HOLD:-0.2}
ENERGY=${ENERGY:-0.15}
THREADS=${THREADS:-$(nproc)}
PPO_EXTRA=${PPO_EXTRA:-}

. scripts/pod_setup.sh

mkdir -p target/train target/runs
[ -f target/train/train.json ] || python tools/gen_train.py --n "$N_TRAIN" --seed 1 --energy-prob "$ENERGY" --out target/train/train.json
[ -f target/train/eval.json ]  || python tools/gen_train.py --n "$N_EVAL"  --seed 22 --out target/train/eval.json
[ -f target/train/eval_energy.json ] || python tools/gen_train.py --n 600 --seed 23 --energy-prob 1.0 --out target/train/eval_energy.json

nohup python rl/ppo.py --train target/train/train.json --eval target/train/eval.json --out "target/runs/$RUN" \
  --d "$WIDTH" --iters "$ITERS" --hold-prob "$HOLD" --threads "$THREADS" --eval-every 100 $PPO_EXTRA > "target/runs/$RUN.log" 2>&1 &
echo "training started: tail -f target/runs/$RUN.log   (checkpoints under target/runs/$RUN/)"
