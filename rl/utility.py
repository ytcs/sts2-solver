"""What a fight's ending is worth: the HP-worth curve U and everything derived from it, in ONE place.

A curve is 101 floats: U[i] = worth of ending the fight alive with HP fraction i/100 (monotone, U[100] = 1). The fight's return is then
    win  -> 1 + HP_BONUS * U[round(100 * hp_fraction)]      loss / stall -> -1      (aborted -> 0, unchanged)
which for the linear curve U[i] = i/100 is exactly the solver's original return (+1 + 0.5 x HP fraction).

Derived, all from this module (so training, the search's terminal states and the network input can never disagree):
    reward(u, frac)   PPO reward of a win (rl/ppo.py)
    table(u)          102 floats for the Rust search terminal (SearchCfg.util: [loss, win at 0%, 1%, ..., 100%])
    feats(u)          8 floats, the network's conditioning input (U at 1/8 .. 8/8 of max HP)
    sample(rng)       a training curve (shapes the route DP produces: cliffs, plateaus, near-hopeless continuations, linear)
    from_values(V)    a curve from a continuation value over HP (agent.routes.continuation_util)
"""
import numpy as np

HP_BONUS = 0.5
N = 101
FEAT_AT = np.array([12, 25, 37, 50, 62, 75, 87, 100])  # HP percent of the 8 input features
FRAC = np.arange(N) / 100.0


def linear():
    return FRAC.copy().astype(np.float32)


def normalize(u):
    """Monotone, U[100] = 1, values in [0, 1]."""
    u = np.maximum.accumulate(np.clip(np.asarray(u, np.float64), 0.0, None))
    top = u[-1]
    if top <= 1e-9:
        return linear()
    return np.clip(u / top, 0.0, 1.0).astype(np.float32)


def reward(u, frac):
    return 1.0 + HP_BONUS * float(u[min(N - 1, max(0, int(round(float(frac) * 100))))])


def table(u, loss=-1.0):
    return [float(loss)] + [1.0 + HP_BONUS * float(x) for x in u]


def feats(u):
    return np.asarray(u, np.float32)[FEAT_AT]


LINEAR_FEATS = feats(linear())


def from_values(V):
    """A curve from V(hp) over integer HP 0..max (V[max] the value at full HP): resampled at 1% steps and normalized."""
    V = np.asarray(V, np.float64)
    hp = np.arange(len(V)) / max(len(V) - 1, 1)
    return normalize(np.interp(FRAC, hp, V))


def sample(rng, p_linear=0.3):
    """A training curve. Shapes seen from the route DP: a cliff (worthless below some HP, then rising fast to a plateau), a gentle rise, a near-hopeless
    continuation (low and almost flat), a step, and the linear return."""
    r = rng.random()
    if r < p_linear:
        return linear()
    r = rng.random()
    if r < 0.55:  # sigmoid cliff: centre 10-70 % of max HP, width 2-20 %
        c, w = rng.uniform(0.10, 0.70), rng.uniform(0.02, 0.20)
        s = 1.0 / (1.0 + np.exp(-(FRAC - c) / w))
        return normalize(s - s[0])
    if r < 0.75:  # concave rise (diminishing worth of more HP): x^p, p in 0.2..0.8
        return normalize(FRAC ** rng.uniform(0.2, 0.8))
    if r < 0.88:  # near-hopeless: a small, nearly flat worth with a late bump
        c = rng.uniform(0.5, 0.95)
        return normalize(0.15 * FRAC + (FRAC > c) * (FRAC - c))
    c = rng.uniform(0.15, 0.6)  # step: only "above c" matters
    return normalize((FRAC >= c).astype(np.float64) + 1e-3 * FRAC)
