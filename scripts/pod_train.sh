#!/usr/bin/env bash
# Fresh PPO training of the combat network on a GPU pod (Runpod, PyTorch image, Ubuntu). Run from the repo root after cloning it:
#
#   bash scripts/pod_train.sh                      # d=128 from scratch, ~6400 iterations (~314M steps)
#   ITERS=200 N_TRAIN=3000 bash scripts/pod_train.sh   # smoke run
#
# Steps: toolchain + simulator wheel -> training / held-out scenario sets (disjoint seeds) -> PPO -> checkpoint + a log you can copy back.
# Output: target/runs/$RUN/ (ckpt.pt = latest, ckpt_<it>.pt, log.jsonl). Copy ckpt.pt back and gate it with `python -m agent.improve gate`.
set -euo pipefail
RUN=${RUN:-fresh_$(date +%Y%m%d)}
ITERS=${ITERS:-6400}
N_TRAIN=${N_TRAIN:-30000}
N_EVAL=${N_EVAL:-1500}
WIDTH=${WIDTH:-128}
HOLD=${HOLD:-0.2}
ENERGY=${ENERGY:-0.15}   # share of scenarios with 4-7 energy a turn (the old training mix had almost none)
THREADS=${THREADS:-$(nproc)}
PPO_EXTRA=${PPO_EXTRA:-}   # more rl/ppo.py flags, e.g. "--adaptive 25 --adaptive-mode signal --adaptive-decay 0.8"

if ! command -v cargo >/dev/null; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
python -m venv .venv --system-site-packages
. .venv/bin/activate
pip install -q maturin numpy
maturin develop --release -m crates/sts2py/Cargo.toml
python -c "import sts2, torch; print('sts2 ok; torch', torch.__version__, 'cuda', torch.cuda.is_available())"

mkdir -p target/train target/runs
[ -f target/train/train.json ] || python tools/gen_train.py --n "$N_TRAIN" --seed 1 --energy-prob "$ENERGY" --out target/train/train.json
[ -f target/train/eval.json ]  || python tools/gen_train.py --n "$N_EVAL"  --seed 22 --out target/train/eval.json   # held-out, base mix (3 energy), seed disjoint from training
[ -f target/train/eval_energy.json ] || python tools/gen_train.py --n 600 --seed 23 --energy-prob 1.0 --out target/train/eval_energy.json  # held-out, every scenario 4-7 energy

nohup python rl/ppo.py --train target/train/train.json --eval target/train/eval.json --out "target/runs/$RUN" \
  --d "$WIDTH" --iters "$ITERS" --hold-prob "$HOLD" --threads "$THREADS" --eval-every 100 $PPO_EXTRA > "target/runs/$RUN.log" 2>&1 &
echo "training started: tail -f target/runs/$RUN.log   (checkpoints under target/runs/$RUN/)"
