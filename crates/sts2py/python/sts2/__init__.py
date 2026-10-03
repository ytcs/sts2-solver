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

from ._sts2 import BatchEnv as _BatchEnv, obs_size, action_space  # noqa: F401
from ._sts2 import (  # noqa: F401
    OUTCOME_ONGOING, OUTCOME_WIN, OUTCOME_LOSS, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_OVERFLOW,
)

OBS_SIZE = obs_size()
ACTIONS = action_space()


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

    def reset(self):
        self._env.observe_all(self.obs, self.mask)
        return self.obs, self.mask

    def step(self, actions):
        a = np.ascontiguousarray(actions, dtype=np.int32)
        self._env.step(a, self.obs, self.mask, self.reward, self.done, self.outcome, self.illegal)
        return self.obs, self.mask, self.reward, self.done, {"outcome": self.outcome, "illegal": self.illegal}
