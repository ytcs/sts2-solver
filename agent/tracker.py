"""Public run counters the game keeps hidden but that follow from its code and what this run showed (`docs/rebuild.md`: only the exact random state
is hidden). Recomputed from the run record (`runs/<id>/events.jsonl`), so a daemon restart loses nothing.

  potion drop   chance of a potion reward after the next fight: starts 0.40; -0.10 after a fight that dropped one, +0.10 after one that did not;
                an elite adds 0.125 (`Odds/PotionRewardOdds.cs:54-72`; the code's 0.25 * 0.5, not the comment's 25%)
  rare offset   added to a card's rare chance (base 1.49% hallway, 5% elite at A7+): starts -0.05, +0.005 per non-rare card of a fight's card reward,
                back to -0.05 after a rare, capped at +0.40; event, relic and shop cards leave it alone (`Odds/CardRarityOdds.cs:13-41, 69-134`)
  unknown room  per act from base monster 0.10, treasure 0.02, shop 0.03 (else an event): the type rolled goes back to its base, every other type
                gains its base (`Odds/UnknownMapPointOdds.cs:21-175`)
  removal       shop card removal 100 + 50 per removal bought (A6 `MerchantCardRemovalEntry.cs:20-32`)
"""
import json
import os
import re

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
        self._fight_kind = None  # the last fight's kind while its rewards are still unseen
        self._cards_seen = set()

    def fight_start(self, encounter):
        self._fight_kind = "elite" if encounter.endswith("_ELITE") else "boss" if encounter.endswith("_BOSS") else "hallway"

    def screen(self, state, choice, result, rarity):
        """One macro step: the screen before it, the option chosen, the first line of the screen after."""
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
    """The counters after every step of a run record."""
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
