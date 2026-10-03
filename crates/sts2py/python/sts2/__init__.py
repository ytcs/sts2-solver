"""STS2 combat RL environment.

    import sts2
    env = sts2.VecEnv(n_envs=4096, scenarios=[scenario_dict, ...], seed=0)
    obs, mask = env.reset()
    obs, mask, reward, done, info = env.step(actions)   # actions: int32 [n_envs] dense indices; mask: uint8 [n_envs, ACTIONS]

`info["outcome"]` (valid where `done`): OUTCOME_WIN 1, OUTCOME_LOSS -1, OUTCOME_TRUNCATED 2 (max_steps), OUTCOME_UNIMPLEMENTED 3
(content not ported), OUTCOME_OVERFLOW 4 (a fixed simulator capacity was exceeded, data dropped). Codes 2-4 end the episode with
reward 0: treat them as truncations, not as a win or a loss.

Scenarios use the oracle JSON format (see docs/oracle.md, tools/mk_scenario.py); every episode redraws all RNG streams.
"""
import json
import numpy as np

from ._sts2 import BatchEnv as _BatchEnv, obs_size, action_space, layout, provably_unwinnable as _provably_unwinnable  # noqa: F401
from ._sts2 import (  # noqa: F401
    OUTCOME_ONGOING, OUTCOME_WIN, OUTCOME_LOSS, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_OVERFLOW,
)

OBS_SIZE = obs_size()
ACTIONS = action_space()


def provably_unwinnable(scenario):
    """None, or a sentence proving the fight cannot be won with this deck (a relaxation that only favours the player; see
    `sts2sim::bounds`: pure damage / block cards, neutral relics, no potions, one enemy). None means nothing was proven."""
    return _provably_unwinnable(json.dumps(scenario))


class VecEnv:
    def __init__(self, n_envs, scenarios, seed=0, max_steps=2000, win=1.0, loss=-1.0, hp_bonus=0.0, step_reward=0.0):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        self.n = n_envs
        self._env = _BatchEnv(n_envs, [json.dumps(s) for s in scenarios], seed, max_steps, win, loss, hp_bonus, step_reward)
        self.obs = np.zeros((n_envs, OBS_SIZE), np.float32)
        self.mask = np.zeros((n_envs, ACTIONS), np.uint8)
        self.reward = np.zeros(n_envs, np.float32)
        self.done = np.zeros(n_envs, np.uint8)
        self.outcome = np.zeros(n_envs, np.int8)
        self.illegal = np.zeros(n_envs, np.uint8)
        self._ep = np.zeros((n_envs, 4), np.float32)

    def reset(self):
        self._env.observe_all(self.obs, self.mask)
        return self.obs, self.mask

    def step(self, actions):
        a = np.ascontiguousarray(actions, dtype=np.int32)
        self._env.step(a, self.obs, self.mask, self.reward, self.done, self.outcome, self.illegal)
        return self.obs, self.mask, self.reward, self.done, {"outcome": self.outcome, "illegal": self.illegal}

    def set_autoreset(self, on):
        """With `False` a finished episode stays finished (done=1 and the same outcome every step, actions ignored): for search."""
        self._env.set_autoreset(bool(on))

    def fork_from(self, src, src_idx, dst_idx, seeds):
        """Copy the fights `src_idx` of VecEnv `src` into this env's slots `dst_idx` and resample what a player cannot see (pile orders, RNG).
        Call `reset()` afterwards to read the observations of the copies."""
        self._env.fork_from(src._env, np.ascontiguousarray(src_idx, np.uint32), np.ascontiguousarray(dst_idx, np.uint32),
                            np.ascontiguousarray(seeds, np.uint64))

    def episode_info(self):
        """Per env, the episode that ended last (valid where `done` was set by the latest `step`): dict of arrays
        `scenario` (index into `scenarios`), `hp_lost` (fraction of max HP lost; a loss counts the HP that was left), `hp_end`
        (fraction of max HP left, 0 on a loss) and `length` (agent steps)."""
        self._env.episode_info(self._ep)
        e = self._ep
        return {"scenario": e[:, 0].astype(np.int32), "hp_lost": e[:, 1], "hp_end": e[:, 2], "length": e[:, 3].astype(np.int32)}
