"""Card costs and card text for the dashboard, generated once (not part of the polling loop):

    python tools/dashboard/card_data.py [--pck PATH] [--check]

Writes `data/dashboard_cards.json` (committed, so the page works without the game installed):
  {"cards": {ID: {"cost", "up_cost", "x", "star", "up_star", "max_upgrade", "type", "target", "text": [level 0, level 1]}}, ...}
  cost        canonical energy cost (-1 = none: curses, statuses); upgraded = max(0, cost + up_cost * level), never for X cards
  star        star cost (Regent): -1 none, -2 X stars (STARDUST: the game's `HasStarCostX`), else the cost; up_star is the per-level delta
  text        the card's full text at each upgrade level, out of combat: keyword lines, `[gold]...[/gold]`, and icon tokens
              `[energy:N]`, `[star:N]`, `[star]` the page draws with the game's icons
Costs, keywords and vars come from the simulator's definitions (crates/sts2sim/src/content/gen_cards.rs, generated from the game's
source); the text templates from the game's localization (localization/eng/cards.json in the pack, read as untrusted data).
Also copies the game's star icon to target/dashboard/assets/ui/star_icon.png and records it in manifest.json (`ui.star_icon`).
`--check` compares the generated text with every card line the game printed outside combat in runs/*/events.jsonl.
"""
import argparse
import glob
import io
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
import pck  # noqa: E402
from extract_assets import DEFAULT_PCK, ids_table  # noqa: E402

GEN = os.path.join(ROOT, "crates", "sts2sim", "src", "content", "gen_cards.rs")
OUT = os.path.join(ROOT, "data", "dashboard_cards.json")
ASSETS = os.path.join(ROOT, "target", "dashboard", "assets")
KW = {"EXHAUST": 1, "ETHEREAL": 2, "INNATE": 4, "UNPLAYABLE": 8, "RETAIN": 16, "SLY": 32, "ETERNAL": 64}
KW_BEFORE = ("UNPLAYABLE", "INNATE", "ETHEREAL", "RETAIN", "SLY")  # the game's order on card text (screen lines: "Unplayable. Ethereal. Eternal.")
KW_AFTER = ("EXHAUST", "ETERNAL")
VAR_NAMES = {"CalcBase": "CalculationBase", "CalcExtra": "CalculationExtra", "CalcDamage": "CalculatedDamage", "CalcBlock": "CalculatedBlock"}


def pascal(snake):
    return "".join(w.capitalize() for w in snake.lower().split("_"))


def kw_bits(expr):
    return sum(KW[k] for k in re.findall(r"kw::(\w+)", expr))


def parse_defs(src, power_classes):
    named = {int(n): pascal(k) for k, n in re.findall(r"pub const (\w+): u16 = (\d+);", src.split("pub mod var_name", 1)[1].split("}", 1)[0])}
    cards = {}
    for line in src.splitlines():
        m = re.match(r"\s*CardDef::new\(ids::card::(\w+), (-?\d+), CardType::(\w+), CardRarity::(\w+), TargetType::(\w+)\)(.*)$", line)
        if not m:
            continue
        cid, rest = m.group(1), m.group(6)
        num = lambda name, d=0: int(re.search(rf"\.{name}\((-?\d+)\)", rest).group(1)) if re.search(rf"\.{name}\((-?\d+)\)", rest) else d  # noqa: E731
        kwm = lambda name: kw_bits(re.search(rf"\.{name}\(([^)]*)\)", rest).group(1)) if re.search(rf"\.{name}\(", rest) else 0  # noqa: E731
        variables = {}
        vm = re.search(r"\.vars\(&\[(.*?)\]\)", rest)
        for f, a in re.findall(r"(var_p|var|power_var|named_var)\(([^()]*)\)", vm.group(1) if vm else ""):
            args = [s.strip() for s in a.split(",")]
            if f in ("var_p", "var"):
                kind = args[0].split("::")[1]
                name = VAR_NAMES.get(kind, kind)
            elif f == "power_var":
                name = power_classes.get(args[0].split("::")[-1], pascal(args[0].split("::")[-1]))
            else:
                name = named.get(int(args[0]), f"Named{args[0]}")
            variables[name] = (int(args[1]), int(args[2]))
        cards[cid] = dict(cost=int(m.group(2)), up_cost=num("up_cost"), x=".x_cost()" in rest, star=num("star_cost", -1), up_star=num("up_star_cost"),
                          max_upgrade=num("max_upgrade", 1), type=m.group(3), rarity=m.group(4), target=m.group(5),
                          kw=kwm("kw"), up_add_kw=kwm("up_add_kw"), up_remove_kw=kwm("up_remove_kw"), vars=variables)
    return cards


# ------------------------------------------------------------------ SmartFormat subset (the game's card text templates)

def split_top(s, sep="|"):
    """Split at `sep` outside braces."""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    out.append(cur)
    return out


class Text:
    def __init__(self, card, level):
        self.c, self.level = card, level

    def value(self, name):
        v = self.c["vars"].get(name)
        if name in ("CalculatedDamage", "CalculatedBlock") and "CalculationBase" in self.c["vars"]:
            v = self.c["vars"]["CalculationBase"]  # out of combat the game shows the base of a calculated value
        return None if v is None else v[0] + v[1] * self.level

    def render(self, s, cur=None):
        out, i = "", 0
        while i < len(s):
            ch = s[i]
            if ch != "{":
                out += ch
                i += 1
                continue
            depth, j = 0, i
            while j < len(s):
                depth += {"{": 1, "}": -1}.get(s[j], 0)
                if depth == 0:
                    break
                j += 1
            out += self.place(s[i + 1:j], cur)
            i = j + 1
        return out

    def place(self, body, cur):
        if body == "":
            return "" if cur is None else str(cur)
        if body == "singleStarIcon":
            return "[star]"
        name, _, fmt = body.partition(":")
        v = self.value(name)
        if name == "IfUpgraded" and fmt.startswith("show:"):
            parts = split_top(fmt[5:]) + [""]
            return self.render(parts[0] if self.level > 0 else parts[1], cur)
        if fmt in ("diff()", "inverseDiff()", ""):
            if v is None and fmt == "" and name not in self.c["vars"]:
                return "X"
            return "X" if v is None else str(abs(v))
        if fmt.startswith("energyIcons("):
            return "[energy]" if v is None and fmt != "energyIcons()" else f"[energy:{'X' if v is None else v}]"  # energyIcons(1) on a non-var: one bare icon
        if fmt == "starIcons()":
            return f"[star:{'X' if v is None else v}]"
        if fmt in ("percentMore()", "percentLess()"):
            return "X" if v is None else str(v * 100 if abs(v) < 10 else v)
        if fmt.startswith("plural:"):
            parts = split_top(fmt[7:]) + [""]
            return self.render(parts[0] if v == 1 else parts[1], v)
        if fmt.startswith("cond:"):
            c = fmt[5:]
            m = re.match(r"([<>]=?|==|!=)(-?\d+)\?(.*)$", c, re.S)
            parts = split_top(m.group(3) if m else c) + [""]
            if m:
                ok = v is not None and eval(f"{v}{m.group(1)}{m.group(2)}", {}, {})  # noqa: S307 (two ints and a fixed operator)
            else:
                ok = bool(v)
            return self.render(parts[0] if ok else parts[1], v)
        m = re.match(r"choose\(([^)]*)\):(.*)$", fmt, re.S)
        if m:
            opts, parts = m.group(1).split("|"), split_top(m.group(2))
            val = {"CardType": self.c["type"], "TargetType": self.c["target"]}.get(name, None if v is None else str(v))
            k = opts.index(val) if val in opts else len(opts)
            return self.render(parts[k] if k < len(parts) else "", cur)
        # a combat-only condition ({InCombat:a|b}, {HasRider:...}): out of combat it is false
        parts = split_top(fmt)
        return self.render(parts[1] if len(parts) > 1 else "", cur) if v is None else self.render(parts[0], cur)


def card_text(card, template, kw_titles, level):
    kw = (card["kw"] | (card["up_add_kw"] if level else 0)) & ~(card["up_remove_kw"] if level else 0)
    body = Text(card, level).render(template or "").strip()
    body = re.sub(r"\[(?!/?gold]|energy|star)[^\]]*\]", "", body)  # other markup (colors, effects)
    before = [kw_titles.get(k, k.title()) + "." for k in KW_BEFORE if kw & KW[k]]
    after = [kw_titles.get(k, k.title()) + "." for k in KW_AFTER if kw & KW[k]]
    return "\n".join(x for x in [" ".join(before), body, " ".join(after)] if x)


# ------------------------------------------------------------------ build / check

def build(pack_path):
    src = open(GEN, encoding="utf-8").read()
    ids = ids_table(os.path.join(ROOT, "crates", "sts2sim", "src", "ids.rs"))
    cards = parse_defs(src, dict(ids.get("power", [])))
    loc, kwt, P = {}, {}, None
    try:
        P = pck.Pack(pack_path)
        loc = json.loads(P.text("localization/eng/cards.json"))
        kwt = {k.split(".")[0]: v for k, v in json.loads(P.text("localization/eng/card_keywords.json")).items() if k.endswith(".title")}
    except Exception as e:  # noqa: BLE001
        print(f"no pack ({e}): costs only", file=sys.stderr)
    out = {}
    for cid, c in cards.items():
        tpl = loc.get(f"{cid}.description")
        levels = range(0, max(1, c["max_upgrade"]) + 1) if c["max_upgrade"] else [0]
        out[cid] = dict({k: c[k] for k in ("cost", "up_cost", "x", "star", "up_star", "max_upgrade", "type", "target")},
                        text=[card_text(c, tpl, kwt, lv) for lv in levels] if tpl is not None else None)
    if P is not None:
        save_star_icon(P)
        P.close()
    return dict(source="tools/dashboard/card_data.py from gen_cards.rs and the game's localization", cards=out)


def save_star_icon(P):
    from extract_assets import Extractor
    try:
        dest = Extractor(P, ASSETS).save("images/packed/sprite_fonts/star_icon.png", "ui/star_icon.png")
    except Exception as e:  # noqa: BLE001
        print(f"star icon: {e}", file=sys.stderr)
        dest = None
    if not dest:
        print("star icon: not found in the pack", file=sys.stderr)
        return
    man = os.path.join(ASSETS, "manifest.json")
    if os.path.exists(man):
        with open(man, encoding="utf-8") as f:
            m = json.load(f)
        m.setdefault("ui", {})["star_icon"] = dest
        with open(man, "w", encoding="utf-8") as f:
            json.dump(m, f, indent=0)


def plain(t):
    t = re.sub(r"\[/?gold\]", "", t)
    t = re.sub(r"\[energy:(\w+)\]", r"\1E", t)
    t = re.sub(r"\[star:(\d+)\]", lambda m: "*" * int(m.group(1)), t).replace("[star]", "*").replace("[energy]", "E")  # the game draws N star icons
    return re.sub(r"\s+", " ", t).strip()


def check(data, titles):
    """Generated text vs every card line printed outside combat (`Name(cost) text`)."""
    by_title = {}
    for cid, t in titles.items():
        by_title.setdefault(t.lower(), []).append(cid)
    seen, ok, bad = set(), 0, []
    for p in glob.glob(os.path.join(ROOT, "runs", "*", "events.jsonl")):
        for line in open(p, encoding="utf-8", errors="replace"):
            try:
                e = json.loads(line)
            except ValueError:
                continue
            st = e.get("state") if e.get("kind") == "macro" else None
            if not st or st.startswith("COMBAT"):
                continue
            for l in st.split("\n"):
                m = re.match(r"^\d+ (?:\d+g (?:card )?)?(?:\(x\) )?([^()]+?)(\+\d*)?\(([^()]*)\) (.*)$", l)
                if not m or (m.group(1), m.group(2), m.group(4)) in seen:
                    continue
                seen.add((m.group(1), m.group(2), m.group(4)))
                ids = [i for i in by_title.get(m.group(1).strip().lower(), []) if i in data["cards"] and data["cards"][i]["text"]]
                if not ids:
                    continue
                lv = 1 if m.group(2) else 0
                game = re.sub(r"(\s*(\(can't afford\)|SALE))+$", "", m.group(4)).strip()
                mine = [plain(data["cards"][i]["text"][min(lv, len(data["cards"][i]["text"]) - 1)]) for i in ids]
                if game in mine:
                    ok += 1
                else:
                    bad.append((m.group(1) + (m.group(2) or ""), game, mine[0]))
    print(f"text check: {ok} of {ok + len(bad)} distinct card lines match")
    for b in bad[:60]:
        print(f"  {b[0]}\n    game: {b[1]}\n    mine: {b[2]}")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--pck", default=DEFAULT_PCK)
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args(argv)
    data = build(a.pck)
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=0, sort_keys=True)
    n = sum(1 for c in data["cards"].values() if c["text"])
    print(f"{OUT}: {len(data['cards'])} cards, {n} with text")
    if a.check:
        man = os.path.join(ASSETS, "manifest.json")
        titles = {k: v["title"] for k, v in json.load(open(man, encoding="utf-8"))["cards"].items()} if os.path.exists(man) else {}
        check(data, titles)


if __name__ == "__main__":
    main()
