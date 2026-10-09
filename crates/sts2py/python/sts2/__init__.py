import json
import numpy as np

from ._sts2 import Sim, replay as _replay, replay_rows as _replay_rows, SearchEnginePy as _SearchEngine, BatchEnv as _BatchEnv, FightStartsPy as _FightStarts, obs_size, action_space, layout, names, provably_unwinnable as _provably_unwinnable  # noqa: F401

OBS_SIZE = obs_size()
ACTIONS = action_space()
_ENC = json.JSONEncoder(check_circular=False, separators=(",", ":"))


def provably_unwinnable(scenario):
    return _provably_unwinnable(json.dumps(scenario))


class VecEnv:
    def __init__(self, n_envs, scenarios, seed=0, max_steps=2000, win=1.0, loss=-1.0, hp_bonus=0.0, round_robin=False, turn_cap=0):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        self.n = n_envs
        self._env = _BatchEnv(n_envs, [s if isinstance(s, str) else json.dumps(s) for s in scenarios], seed, max_steps, win, loss, hp_bonus, round_robin, turn_cap)
        self.obs = np.zeros((n_envs, OBS_SIZE), np.float32)
        self.mask = np.zeros((n_envs, ACTIONS), np.uint8)
        self.reward = np.zeros(n_envs, np.float32)
        self.done = np.zeros(n_envs, np.uint8)
        self.outcome = np.zeros(n_envs, np.int8)
        self.illegal = np.zeros(n_envs, np.uint8)
        self._ep = np.zeros((n_envs, 7), np.float32)

    def reset(self):
        self._env.observe_all(self.obs, self.mask)
        return self.obs, self.mask

    def step(self, actions):
        a = np.ascontiguousarray(actions, dtype=np.int32)
        self._env.step(a, self.obs, self.mask, self.reward, self.done, self.outcome, self.illegal)
        return self.obs, self.mask, self.reward, self.done, {"outcome": self.outcome, "illegal": self.illegal}

    def set_weights(self, w):
        self._env.set_weights(np.ascontiguousarray(w, np.float32))

    def episode_info(self):
        self._env.episode_info(self._ep)
        e = self._ep
        return {"scenario": e[:, 0].astype(np.int32), "hp_lost": e[:, 1], "hp_end": e[:, 2], "length": e[:, 3].astype(np.int32),
                "hp_end_abs": e[:, 4].astype(np.int32), "max_hp_end": e[:, 5].astype(np.int32), "turns": e[:, 6].astype(np.int32)}


class FightStarts:
    """First observations of scenarios parsed once: observe(seed) equals VecEnv(len, scenarios, seed, round_robin=True).reset()."""

    def __init__(self, scenarios):
        self._st = _FightStarts([s if isinstance(s, str) else _ENC.encode(s) for s in scenarios])
        self.n = len(self._st)
        self.obs = np.zeros((self.n, OBS_SIZE), np.float32)
        self.mask = np.zeros((self.n, ACTIONS), np.uint8)

    def observe(self, seed):
        self._st.observe(int(seed), self.obs, self.mask)
        return self.obs, self.mask


def replay(scenario, seed, actions):
    return _replay(json.dumps(scenario), int(seed), np.ascontiguousarray(actions, np.int32))


def replay_rows(scenarios, scen, seeds, actions, off, steps, soff):
    return _replay_rows([json.dumps(s) for s in scenarios], [int(x) for x in scen], [int(x) for x in seeds], np.ascontiguousarray(actions, np.int32),
                        [int(x) for x in off], [int(x) for x in steps], [int(x) for x in soff])
