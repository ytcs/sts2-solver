#!/usr/bin/env bash
# Fine-tune one solver network for the HP-worth input (rl/utility.py) on a GPU pod. From the repo root:
#   NET=b ITERS=1500 bash scripts/pod_util_ft.sh            # b | c | d: warm start from models/solver_<NET>128.pt
#   NET=b SMOKE=1 bash scripts/pod_util_ft.sh               # 100 iterations, then a small gate
# Output: target/runs/u$NET/ (ckpt.pt, log.jsonl) and target/runs/u$NET.log.
set -euo pipefail
NET=${NET:-b}
ITERS=${ITERS:-1500}
UTIL=${UTIL:-0.7}
LR=${LR:-1e-4}
THREADS=${THREADS:-$(nproc)}
if [ "${SMOKE:-0}" = "1" ]; then ITERS=100; fi
if ! command -v cargo >/dev/null; then curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null 2>&1; fi
. "$HOME/.cargo/env"
[ -d .venv ] || python -m venv .venv --system-site-packages
. .venv/bin/activate
pip install -q maturin numpy
maturin develop --release -m crates/sts2py/Cargo.toml 2>&1 | tail -1
export STS2_DEVICE=cuda
mkdir -p target/train target/runs
# the original training mix (seed 1, 15 % high energy), plus a copy of half of it at a spread start HP (20-100 % of max): the curve has to bind
[ -f target/train/train.json ] || python tools/gen_train.py --n 30000 --seed 1 --energy-prob 0.15 --out target/train/train.json 2>&1 | tail -1
[ -f target/train/train_u.json ] || python - <<'EOF'
import json, random
s = json.load(open("target/train/train.json"))
r = random.Random(5)
extra = []
for x in s[::2]:
    y = dict(x); y["hp"] = max(1, int(round(x["max_hp"] * r.uniform(0.2, 1.0)))); y["name"] = x["name"] + "_hp"
    extra.append(y)
json.dump(s + extra, open("target/train/train_u.json", "w"))
print(len(s) + len(extra), "training scenarios")
EOF
python rl/ppo.py --train target/train/train_u.json --eval data/train/eval.json --out "target/runs/u$NET" --d 128 --iters "$ITERS" \
  --hold-prob 0.2 --util-prob "$UTIL" --resume "models/solver_${NET}128.pt" --warm --lr "$LR" --eval-every 100 --threads "$THREADS" > "target/runs/u$NET.log" 2>&1
echo "TRAIN_DONE $NET"
if [ "${SMOKE:-0}" = "1" ]; then
  python tools/gate_util.py --new "target/runs/u$NET/ckpt.pt" --limit 600 --attempts 4 2>&1 | grep -v -i warn | tail -6
  echo SMOKE_DONE
fi
