import json
import re


def game_slot(scenario, i):
    pots = scenario.get("potions", [])
    return pots[i].get("slot", i) if 0 <= i < len(pots) else i


def sim_index(scenario, slot):
    for i, p in enumerate(scenario.get("potions", [])):
        if p.get("slot", i) == slot:
            return i
    return None


def to_game_action(scenario, j):
    a = json.loads(j)
    if "use_potion" not in a:
        return j
    a["use_potion"]["slot"] = game_slot(scenario, a["use_potion"]["slot"])
    return json.dumps(a)


def to_sim_action(scenario, act):
    a = json.loads(act)
    if "use_potion" not in a:
        return act
    i = sim_index(scenario, a["use_potion"]["slot"])
    if i is not None:
        a["use_potion"]["slot"] = i
    return json.dumps(a)


def live_slots(scenario, sim):
    pots = scenario.get("potions", [])
    slots = sorted({int(m.group(1)) for _, t in sim.legal() for m in [re.match(r"potion (\d+)", t)] if m})
    return [(n, pots[n]["id"] if n < len(pots) else f"POTION_{n}") for n in slots]


def text_index(text):
    m = re.match(r"potion (\d+)", str(text))
    return int(m.group(1)) if m else None


def held_indices(scenario, keep):
    pots = scenario.get("potions", [])
    if keep is True:
        return list(range(len(pots)))
    if isinstance(keep, (set, frozenset)):
        return [i for i, p in enumerate(pots) if p["id"] in keep]
    return []


def parse_names(rest):
    return set() if rest.strip() in ("", "none") else {x.strip().upper().replace(" ", "_") for x in rest.split(",")}
