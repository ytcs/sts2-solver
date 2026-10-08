import json
import os
import re

# counters follow from public play only; the game's random state is never read
POTION_START, POTION_STEP, POTION_ELITE = 0.40, 0.10, 0.125
OFFSET_START, OFFSET_STEP, OFFSET_CAP = -0.05, 0.005, 0.40
RARE_BASE = {"hallway": 0.0149, "elite": 0.05, "shop": 0.045}
UNKNOWN_BASE = {"monster": 0.10, "treasure": 0.02, "shop": 0.03}
REMOVAL_BASE, REMOVAL_STEP = 100, 50
_ROOM = {"COMBAT": "monster", "TREASURE": "treasure", "SHOP": "shop"}


def _rarity():
    from agent import macro
    cat = json.load(open(os.path.join(os.path.dirname(__file__), "..", "data", "catalog.json")))["cards"]
    by_id = {c["id"]: c["rarity"] for pool in cat.values() for c in pool}
    return lambda name: by_id.get(macro.card_from_name(name)[0] or "")


class Tracker:
    def __init__(self):
        self.potion, self.offset, self.removals, self.act = POTION_START, OFFSET_START, 0, None
        self.unknown = dict(UNKNOWN_BASE)
        self._fight_kind = None
        self._cards_seen = set()

    def fight_start(self, encounter):
        self._fight_kind = "elite" if encounter.endswith("_ELITE") else "boss" if encounter.endswith("_BOSS") else "hallway"

    def screen(self, state, choice, result, rarity):
        lines = state.split("\n")
        m = re.match(r"A(\d+) ", lines[1] if len(lines) > 1 else "")
        if m and int(m.group(1)) != self.act:
            self.act, self.unknown = int(m.group(1)), dict(UNKNOWN_BASE)
        kind = lines[0].split()[0] if lines and lines[0] else ""
        if kind == "REWARDS" and self._fight_kind:
            dropped = any(re.match(r"^\d+ potion ", x) for x in lines)
            self.potion += -POTION_STEP if dropped else POTION_STEP
            self._card_source, self._fight_kind = self._fight_kind, None
        elif kind == "CARD_REWARD" and getattr(self, "_card_source", None):
            names = tuple(m.group(1) for x in lines for m in [re.match(r"^\d+ (.+?)\(", x)] if m)
            key = (lines[1] if len(lines) > 1 else "", names)
            if key not in self._cards_seen:
                self._cards_seen.add(key)
                for n in names:
                    if rarity(n.strip().rstrip("+")) == "Rare":
                        self.offset = OFFSET_START
                    else:
                        self.offset = min(OFFSET_CAP, self.offset + OFFSET_STEP)
            self._card_source = None
        elif kind == "MAP" and choice.split()[:1] and choice.split()[0].isdigit():
            opt = next((x for x in lines if x.startswith(choice.split()[0] + " ")), "")
            if " Unknown " in f" {opt} ":
                rolled = _ROOM.get(result.split()[0] if result else "")
                for t, b in UNKNOWN_BASE.items():
                    self.unknown[t] = b if t == rolled else self.unknown[t] + b
        elif kind == "SHOP" and choice.split()[:1] and choice.split()[0].isdigit():
            opt = next((x for x in lines if x.startswith(choice.split()[0] + " ")), "")
            if "remove a card" in opt and not result.startswith("ERR"):
                self.removals += 1

    def line(self):
        e = 1 - sum(self.unknown.values())
        u = " ".join(f"{t[0].upper()} {v:.0%}" for t, v in self.unknown.items())
        return (f"public odds: potion after next fight {min(1, self.potion):.0%} (elite {min(1, self.potion + POTION_ELITE):.0%}) | rare chance per card "
                f"hallway {max(0, RARE_BASE['hallway'] + self.offset):.1%} elite {max(0, RARE_BASE['elite'] + self.offset):.1%} shop {max(0, RARE_BASE['shop'] + self.offset):.1%}"
                f" | unknown room {u} E {max(0, e):.0%} | removal {REMOVAL_BASE + REMOVAL_STEP * self.removals}g")


def from_record(events_path):
    t, rarity = Tracker(), _rarity()
    if not os.path.exists(events_path):
        return t
    with open(events_path, encoding="utf-8") as f:
        for raw in f:
            e = json.loads(raw)
            if e.get("kind") == "fight_start":
                t.fight_start(e.get("encounter", ""))
            elif e.get("kind") == "macro":
                t.screen(e.get("state", ""), e.get("choice", ""), e.get("result", ""), rarity)
    return t
