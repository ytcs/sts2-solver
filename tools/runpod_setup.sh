#!/usr/bin/env bash
set -euo pipefail
cd /workspace 2>/dev/null || cd /root
if ! command -v cargo >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
source "$HOME/.cargo/env"
[ -d sts2-solver ] || git clone --depth 1 -b sim-rebuild https://github.com/ytcs/sts2-solver.git
cd sts2-solver
git pull -q || true
uv venv --system-site-packages .venv
uv pip install --python .venv/bin/python maturin numpy
(cd crates/sts2py && VIRTUAL_ENV=$PWD/../../.venv ../../.venv/bin/maturin develop --release 2>&1 | tail -3)
tools/setup_data.sh
.venv/bin/python -c "import sts2; print('sts2 import ok, obs', sts2.OBS_SIZE)"
echo SETUP_DONE
