import re

# A live Sim's belt index is the game's slot (Sim() places scenario potions by slot, sync follows the game).


def live_slots(sim):
    ids = dict(sim.belt())
    slots = sorted({int(m.group(1)) for _, t in sim.legal() for m in [re.match(r"potion (\d+)", t)] if m})
    return [(n, ids.get(n, f"POTION_{n}")) for n in slots]


def text_index(text):
    m = re.match(r"potion (\d+)", str(text))
    return int(m.group(1)) if m else None


def held_indices(sim, keep):
    if keep is True:
        return [i for i, _ in sim.belt()]
    if isinstance(keep, (set, frozenset)):
        return [i for i, pid in sim.belt() if pid in keep]
    return []


def parse_names(rest):
    return set() if rest.strip() in ("", "none") else {x.strip().upper().replace(" ", "_") for x in rest.split(",")}
