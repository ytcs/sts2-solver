"""Potions: the two numberings and the policy, in one place. Stdlib only.

Numbering. The fight scenario lists the belt's potions in slot order, each with its game `slot`; the simulator packs them into 0..n-1 in that list order (an
empty first slot makes the two differ: a lone potion in the second slot is game slot 1, simulator potion 0). The simulator's actions and texts
(`potion 0 -> e1`, `{"use_potion": {"slot": 0}}`) use the packed index; the bridge, the header's `pots[...]` and the logged actions use game slots.

Policy. The solver may PROPOSE any potion; I answer at the gate (`combat ok / skip / go`, `agent.live`). What the live search may use (`search_keep`):
the potions I `hold` are off the table, and after `combat go` all of them for the rest of that fight. What pricing sees (`priced_view`): the run snapshot
without the held potions, so no priced fight (the boss included) spends a potion I keep for something else; non-boss fights are priced without
potions anyway (`hold="all"` in `macro.evaluate`, a lower bound) and the boss with the belt.
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


def search_keep(hold, declined_fight, fight_id):
    """keep_potions for the live search: True (no potion) once I declined potions for this fight (`combat go`), else the ids I hold. Searching as if a
    declined potion will be thrown next turn picked worse lines (Living Fog, run 20261005-160158, 99th percentile). Until I decline, the solver is NOT limited."""
    return True if declined_fight == fight_id else set(hold)


def priced_view(deck, hold):
    """The run snapshot as every calculator prices it: the potions I hold are taken out of the belt (a Foul Potion kept for a merchant must not be thrown
    by the simulated boss fight)."""
    if not hold:
        return deck
    return dict(deck, potions=[p for p in deck.get("potions", []) if p["id"] not in hold])


def parse_hold(rest):
    """`hold fire potion, Block_Potion` -> {'FIRE_POTION', 'BLOCK_POTION'}; `hold` / `hold none` -> empty."""
    return set() if rest.strip() in ("", "none") else {x.strip().upper().replace(" ", "_") for x in rest.split(",")}
