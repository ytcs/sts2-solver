"""Potions: the two numberings and the policy, in one place. Stdlib only.

Numbering. The fight scenario lists the belt's potions in slot order, each with its game `slot`; the simulator packs them into 0..n-1 in that list order (an
empty first slot makes the two differ: a lone potion in the second slot is game slot 1, simulator potion 0). The simulator's actions and texts
(`potion 0 -> e1`, `{"use_potion": {"slot": 0}}`) use the packed index; the bridge, the header's `pots[...]` and the logged actions use game slots.

Policy (`agent.proposal`): the solver sees every potion and proposes; only the operator commits one (`potion use`, `a <i>`), one per commit. The live
card-play search plans without potions (`held_indices(..., True)`). Potions set aside for the boss (`potion aside`) are named with `parse_names`.
"""
import json
import re


def game_slot(scenario, i):
    """Simulator potion index -> game slot (an index past the scenario's belt is returned unchanged)."""
    pots = scenario.get("potions", [])
    return pots[i].get("slot", i) if 0 <= i < len(pots) else i


def sim_index(scenario, slot):
    """Game slot -> simulator potion index, None when no scenario potion sits in that slot."""
    for i, p in enumerate(scenario.get("potions", [])):
        if p.get("slot", i) == slot:
            return i
    return None


def to_game_action(scenario, j):
    """A simulator action (JSON text) for the bridge's `do`: `use_potion` gets the game slot; anything else is returned as is."""
    a = json.loads(j)
    if "use_potion" not in a:
        return j
    a["use_potion"]["slot"] = game_slot(scenario, a["use_potion"]["slot"])
    return json.dumps(a)


def to_sim_action(scenario, act):
    """A logged game action (JSON text) for the simulator: `use_potion` gets the packed index (an unknown slot is left alone)."""
    a = json.loads(act)
    if "use_potion" not in a:
        return act
    i = sim_index(scenario, a["use_potion"]["slot"])
    if i is not None:
        a["use_potion"]["slot"] = i
    return json.dumps(a)


def live_slots(scenario, sim):
    """[(slot, id)] of the potions the simulator can use now: its `potion N` actions number the fight's slots (the scenario's order), which stay fixed when one
    is thrown (its slot empties; the snapshot's potion list is packed and shifts). A potion made mid-fight has no scenario entry: id `POTION_N`."""
    pots = scenario.get("potions", [])
    slots = sorted({int(m.group(1)) for _, t in sim.legal() for m in [re.match(r"potion (\d+)", t)] if m})
    return [(n, pots[n]["id"] if n < len(pots) else f"POTION_{n}") for n in slots]


def text_index(text):
    """The simulator potion index of an action text (`potion 1 -> e0` -> 1), else None."""
    m = re.match(r"potion (\d+)", str(text))
    return int(m.group(1)) if m else None


def held_indices(scenario, keep):
    """Simulator indices of the potions the search may not use: every one for `keep is True`, else those whose id is in the set `keep`."""
    pots = scenario.get("potions", [])
    if keep is True:
        return list(range(len(pots)))
    if isinstance(keep, (set, frozenset)):
        return [i for i, p in enumerate(pots) if p["id"] in keep]
    return []


def parse_names(rest):
    """`fire potion, Block_Potion` -> {'FIRE_POTION', 'BLOCK_POTION'}; '' / 'none' -> empty."""
    return set() if rest.strip() in ("", "none") else {x.strip().upper().replace(" ", "_") for x in rest.split(",")}
