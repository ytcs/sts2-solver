import json
import numpy as np

from ._sts2 import Sim, replay as _replay, replay_rows as _replay_rows, SearchEnginePy as _SearchEngine, BatchEnv as _BatchEnv, FightStartsPy as _FightStarts, obs_size, action_space, layout, names, provably_unwinnable as _provably_unwinnable, set_relic_mask  # noqa: F401
from ._sts2 import obs_version, obs_version as _default_obs_version, set_obs_version  # noqa: F401
from ._sts2 import (  # noqa: F401
    OUTCOME_ONGOING, OUTCOME_WIN, OUTCOME_LOSS, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_OVERFLOW,
)

OBS_SIZE = obs_size(1)
ACTIONS = action_space()
_ENC = json.JSONEncoder(check_circular=False, separators=(",", ":"))


def provably_unwinnable(scenario):
    return _provably_unwinnable(json.dumps(scenario))


class VecEnv:
    def __init__(self, n_envs, scenarios, seed=0, max_steps=2000, win=1.0, loss=-1.0, hp_bonus=0.0, step_reward=0.0, round_robin=False, turn_cap=0,
                 obs_version=None):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        self.n = n_envs
        self._env = _BatchEnv(n_envs, [s if isinstance(s, str) else json.dumps(s) for s in scenarios], seed, max_steps, win, loss, hp_bonus, step_reward, round_robin, turn_cap,
                              obs_version)
        self.obs_version = self._env.obs_version()
        self.obs_size = self._env.obs_size()
        self.obs = np.zeros((n_envs, self.obs_size), np.float32)
        self.mask = np.zeros((n_envs, ACTIONS), np.uint8)
        self.reward = np.zeros(n_envs, np.float32)
        self.done = np.zeros(n_envs, np.uint8)
        self.outcome = np.zeros(n_envs, np.int8)
        self.illegal = np.zeros(n_envs, np.uint8)
        self.pot_used = np.zeros(n_envs, np.uint8)
        self._ep = np.zeros((n_envs, 7), np.float32)

    def reset(self):
        self._env.observe_all(self.obs, self.mask)
        return self.obs, self.mask

    def step(self, actions):
        a = np.ascontiguousarray(actions, dtype=np.int32)
        self._env.step(a, self.obs, self.mask, self.reward, self.done, self.outcome, self.illegal)
        self._env.potion_used(self.pot_used)
        return self.obs, self.mask, self.reward, self.done, {"outcome": self.outcome, "illegal": self.illegal, "pot_used": self.pot_used}

    def loops(self):
        return int(self._env.loops())

    def set_turn_cap(self, cap):
        self._env.set_turn_cap(int(cap))

    def set_weights(self, w):
        self._env.set_weights(np.ascontiguousarray(w, np.float32))

    def set_autoreset(self, on):
        self._env.set_autoreset(bool(on))

    def fork_from(self, src, src_idx, dst_idx, seeds):
        self._env.fork_from(src._env, np.ascontiguousarray(src_idx, np.uint32), np.ascontiguousarray(dst_idx, np.uint32),
                            np.ascontiguousarray(seeds, np.uint64))

    def episode_info(self):
        self._env.episode_info(self._ep)
        e = self._ep
        return {"scenario": e[:, 0].astype(np.int32), "hp_lost": e[:, 1], "hp_end": e[:, 2], "length": e[:, 3].astype(np.int32),
                "hp_end_abs": e[:, 4].astype(np.int32), "max_hp_end": e[:, 5].astype(np.int32), "turns": e[:, 6].astype(np.int32)}


class FightStarts:
    """First observations of scenarios parsed once: observe(seed) equals VecEnv(len, scenarios, seed, round_robin=True).reset()."""

    def __init__(self, scenarios, obs_version=None):
        self._st = _FightStarts([s if isinstance(s, str) else _ENC.encode(s) for s in scenarios])
        self.n = len(self._st)
        self.obs_version = _default_obs_version() if obs_version is None else obs_version
        self.obs = np.zeros((self.n, obs_size(self.obs_version)), np.float32)
        self.mask = np.zeros((self.n, ACTIONS), np.uint8)

    def observe(self, seed):
        self._st.observe(int(seed), self.obs_version, self.obs, self.mask)
        return self.obs, self.mask


def replay(scenario, seed, actions, obs_version=None):
    return _replay(json.dumps(scenario), int(seed), np.ascontiguousarray(actions, np.int32), obs_version)


def replay_rows(scenarios, scen, seeds, actions, off, steps, soff, obs_version=None):
    return _replay_rows([json.dumps(s) for s in scenarios], [int(x) for x in scen], [int(x) for x in seeds], np.ascontiguousarray(actions, np.int32),
                        [int(x) for x in off], [int(x) for x in steps], [int(x) for x in soff], obs_version)
