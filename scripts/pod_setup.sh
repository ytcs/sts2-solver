# Sourced by the pod scripts from the repo root: Rust, venv over the image's torch, sts2 bindings, CUDA check.
if ! command -v cargo >/dev/null; then
  [ -f "$HOME/.cargo/env" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi
[ -d .venv ] || python -m venv .venv --system-site-packages
. .venv/bin/activate
# observation v4 by default (live net solver_r9_v4: 9 potions, action space 322); STS2_OBS=3 for v3 checkpoints
want=${STS2_OBS:-4}
if ! python -c "import sts2, sys; sys.exit(sts2.OBS_VERSION != $want)" 2>/dev/null; then
  pip install -q maturin numpy
  maturin develop --release -m crates/sts2py/Cargo.toml $([ "$want" -ge 3 ] && echo --features obs_v$want)
fi
python -c "import sts2, torch; print('sts2 ok; torch', torch.__version__, 'cuda', torch.cuda.is_available()); assert torch.cuda.is_available()"
export STS2_DEVICE=cuda
