"""STS2 combat RL environment.

    import sts2
    env = sts2.VecEnv(n_envs=4096, scenarios=[scenario_dict, ...], seed=0)
    obs, mask = env.reset()
    obs, mask, reward, done, info = env.step(actions)   # actions: int32 [n_envs] dense indices; mask: uint8 [n_envs, ACTIONS]

`info["outcome"]` (valid where `done`): OUTCOME_WIN 1, OUTCOME_LOSS -1, OUTCOME_TRUNCATED 2 (max_steps), OUTCOME_UNIMPLEMENTED 3
(content not ported), OUTCOME_OVERFLOW 4 (a fixed simulator capacity was exceeded, data dropped). Codes 2-4 end the episode with
reward 0: treat them as truncations, not as a win or a loss.

With `round_robin=True` env `i` always plays `scenarios[i % len(scenarios)]` (n_envs = k x len gives k attempts of each).
Scenarios use the oracle JSON format (see docs/oracle.md, tools/mk_scenario.py); every episode redraws all RNG streams.
"""
import json
import numpy as np

from ._sts2 import Sim, replay as _replay, SearchEnginePy as _SearchEngine, BatchEnv as _BatchEnv, obs_size, action_space, layout, names, provably_unwinnable as _provably_unwinnable, set_relic_mask, set_look_legacy  # noqa: F401
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
    def __init__(self, n_envs, scenarios, seed=0, max_steps=2000, win=1.0, loss=-1.0, hp_bonus=0.0, step_reward=0.0, round_robin=False, turn_cap=0):
        if isinstance(scenarios, dict):
            scenarios = [scenarios]
        self.n = n_envs
        self._env = _BatchEnv(n_envs, [json.dumps(s) for s in scenarios], seed, max_steps, win, loss, hp_bonus, step_reward, round_robin, turn_cap)
        self.obs = np.zeros((n_envs, OBS_SIZE), np.float32)
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
        # pot_used: bit k = the potion in belt slot k before this step is gone after it (thrown, discarded, consumed), measured before an auto-reset
        return self.obs, self.mask, self.reward, self.done, {"outcome": self.outcome, "illegal": self.illegal, "pot_used": self.pot_used}

    def set_turn_cap(self, cap):
        """A fight still running after `cap` player turns ends as a loss, from the next step on (0 = no cap)."""
        self._env.set_turn_cap(int(cap))

    def set_weights(self, w):
        """Pool sources (the default, not `round_robin`): episodes starting from now draw scenario i with probability w[i] / sum(w)."""
        self._env.set_weights(np.ascontiguousarray(w, np.float32))

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
        (fraction of max HP left, 0 on a loss), `length` (agent steps), `hp_end_abs` (HP left, 0 on a loss), `max_hp_end` and `turns` (player turns).
        `turn_cap` > 0: a fight still running after that many player turns ends as a loss."""
        self._env.episode_info(self._ep)
        e = self._ep
        return {"scenario": e[:, 0].astype(np.int32), "hp_lost": e[:, 1], "hp_end": e[:, 2], "length": e[:, 3].astype(np.int32),
                "hp_end_abs": e[:, 4].astype(np.int32), "max_hp_end": e[:, 5].astype(np.int32), "turns": e[:, 6].astype(np.int32)}


def replay(scenario, seed, actions):
    """Replays a fight recorded by the search engine (`SearchEngine.moves`): `(obs [n + 1, OBS_SIZE], mask [n + 1, ACTIONS])` before every action
    and after the last one. `seed` is the job seed of the fight; the real fight depends on nothing else."""
    return _replay(json.dumps(scenario), int(seed), np.ascontiguousarray(actions, np.int32))
