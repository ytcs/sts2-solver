#!/usr/bin/env python3
"""Play-by-play of a won and a lost fight, as one self-contained HTML page.

  .venv/bin/python rl/trace.py --ckpt target/runs/r4/ckpt.pt --scenario target/runs/phrog.json --out target/trace.html
        [--roots 120] [--M 5] [--K 8] [--greedy] [--seed 11] [--standalone]

The model plays `roots` copies of the fight (with the play-out search unless `--greedy`), every observation and chosen action is recorded, and one
typical win and one typical loss are narrated: per turn the hand and the enemy's intent, every card played with what it did (damage, block, powers,
cards drawn / created), the enemy turn, and, where the search ran, the alternatives it weighed. Everything is decoded from the observation vector,
the same information the agent had. Without `--standalone` the file is a page fragment (title, style, markup, script) as the Artifact tool expects.
"""
import argparse, collections, json, os, sys
import numpy as np
import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sts2
from search import Searcher, load

LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}
NAMES = sts2.names()
INTENTS = {1: "Attack", 2: "Buff", 3: "Debuff", 4: "Strong debuff", 5: "Defend", 6: "Escape", 7: "Heal", 8: "Hidden", 9: "Summon", 10: "Asleep",
           11: "Stunned", 12: "Adds status cards", 13: "Card debuff", 14: "Death blow"}


CHARACTER_SUFFIX = (" Ironclad", " Silent", " Defect", " Necrobinder", " Regent")
FIXES = {"Ascenders Bane": "Ascender's Bane", "Dollys Mirror": "Dolly's Mirror", "Odds And Ends": "Odds and Ends", "Burning Blood": "Burning Blood"}


def title(s):
    t = s.replace("_", " ").title()
    for suf in CHARACTER_SUFFIX:
        if t.endswith(suf) and t not in ("Strike" + suf, "Defend" + suf) or t in ("Strike" + suf, "Defend" + suf):
            t = t[: -len(suf)]
            break
    return FIXES.get(t, t)


def name(kind, i):
    return title(NAMES[kind][i])


def sec(obs, n):
    o, s = SEC[n]
    return obs[o:o + s]


def powers(arr):
    out = []
    for k in range(C["OBS_POWERS"]):
        pid, amt = int(arr[2 * k]), int(arr[2 * k + 1])
        if pid > 0:
            out.append((name("power", pid - 1).replace(" Power", ""), amt))
    return out


def card(arr):
    cid = int(arr[0])
    return None if cid <= 0 else dict(name=name("card", cid - 1) + ("+" if arr[1] > 0 else ""), cost=int(arr[2]), playable=bool(arr[3]), dmg=int(arr[6]), blk=int(arr[7]))


def decode(obs):
    g, pl = sec(obs, "global"), sec(obs, "player")
    st = dict(round=int(g[0]), turn=int(g[1]), over=bool(g[4]), hp=int(pl[0]), max_hp=int(pl[1]), block=int(pl[2]), energy=int(pl[3]), max_energy=int(pl[4]),
              stars=int(pl[5]), powers=powers(pl[8:8 + 2 * C["OBS_POWERS"]]))
    hand = sec(obs, "hand").reshape(C["MAX_HAND"], C["CARD_F"])
    st["hand"] = [c for c in (card(h) for h in hand) if c]
    piles = {}
    for nm in ("draw", "discard", "exhaust"):
        arr = sec(obs, nm).reshape(-1, 2)
        piles[nm] = [name("card", int(a[0]) - 1) + ("+" if a[1] > 0 else "") for a in arr if a[0] > 0]
    st["piles"] = piles
    sizes = sec(obs, "pile_sizes")
    st["sizes"] = dict(draw=int(sizes[0]), discard=int(sizes[1]), exhaust=int(sizes[2]))
    ens = sec(obs, "enemies").reshape(C["OBS_MAX_ENEMIES"], C["ENEMY_F"])
    seen = collections.Counter()
    st["enemies"] = []
    for e in ens:
        if e[0] < 0.5:
            continue
        base = name("monster", int(e[2]) - 1)
        seen[base] += 1
        intents = []
        for j in range(C["OBS_INTENTS"]):
            kind, dmg, hits = int(e[40 + 3 * j]), int(e[41 + 3 * j]), int(e[42 + 3 * j])
            if kind:
                intents.append(dict(kind=INTENTS.get(kind, "?"), dmg=dmg, hits=hits, attack=kind in (1, 14)))
        st["enemies"].append(dict(cid=int(e[1]), name=base, n=seen[base], hp=int(e[3]), max_hp=int(e[4]), block=int(e[5]), alive=bool(e[6]), powers=powers(e[8:8 + 2 * C["OBS_POWERS"]]), intents=intents))
    for e in st["enemies"]:
        if seen[e["name"]] > 1:
            e["name"] += f" #{e['n']}"
    d = sec(obs, "decision")
    st["decision"] = None
    if d[0] > 0.5:
        cands = d[8:].reshape(C["OBS_MAX_CANDS"], C["CARD_F"] + 1)
        st["decision"] = dict(min=int(d[2]), max=int(d[3]), cands=[card(c[:C["CARD_F"]]) for c in cands if c[0] > 0])
    pot = sec(obs, "potions").reshape(C["MAX_POTIONS"], 2)
    st["potions"] = [name("potion", int(p[0]) - 1) if p[0] > 0 else None for p in pot]
    return st


def all_cards(st):
    c = collections.Counter(h["name"] for h in st["hand"])
    for p in st["piles"].values():
        c.update(p)
    return c


def pdiff(a, b):
    """Changes between two power lists as text."""
    da, db = dict(a), dict(b)
    out = []
    for k in db:
        if db[k] != da.get(k, 0):
            out.append(f"{k} {db[k] - da.get(k, 0):+d}" if k in da else f"{k} {db[k]}")
    for k in da:
        if k not in db:
            out.append(f"{k} gone")
    return out


def intent_text(e):
    parts = []
    for it in e["intents"]:
        parts.append(f"{it['kind']} {it['dmg']}×{it['hits']}" if it["attack"] and it["hits"] > 1 else (f"{it['kind']} {it['dmg']}" if it["attack"] else it["kind"]))
    return ", ".join(parts) if parts else "no intent"


def intended_damage(st):
    return sum(it["dmg"] * max(it["hits"], 1) for e in st["enemies"] if e["alive"] for it in e["intents"] if it["attack"])


def describe(a):
    """(kind, hand slot / target / candidate index) of a dense action index."""
    if a == 0:
        return "end", None, None
    if a < C["OFF_POTION"]:
        k = a - 1
        return "play", k // 13, k % 13
    if a < C["OFF_DISCARD"]:
        k = a - C["OFF_POTION"]
        return "potion", k // 13, k % 13
    if a < C["OFF_PICK"]:
        return "discard_potion", a - C["OFF_DISCARD"], None
    if a < C["OFF_CONFIRM"]:
        return "pick", a - C["OFF_PICK"], None
    return "confirm", None, None


def action_label(st, a):
    kind, x, t = describe(a)
    tgt = ""
    if t is not None and t < C["MAX_CREATURES"]:
        for e in st["enemies"]:
            if e["cid"] == t:
                tgt = f" → {e['name']}"
    if kind == "end":
        return "End turn"
    if kind == "play":
        h = st["hand"][x] if x < len(st["hand"]) else None
        return f"Play {h['name']}{tgt}" if h else f"Play card {x}"
    if kind == "potion":
        p = st["potions"][x] if x < len(st["potions"]) else None
        return f"Drink {p or 'potion'}{tgt}"
    if kind == "discard_potion":
        return "Discard a potion"
    if kind == "pick":
        d = st["decision"]
        c = d["cands"][x] if d and x < len(d["cands"]) else None
        return f"Choose {c['name']}" if c else f"Choose option {x}"
    return "Confirm"


def effects(before, after, a, in_play=None):
    """What an action did, read from the two observations around it."""
    out = []
    if after["hp"] != before["hp"]:
        out.append(dict(t="hp", s=f"You {after['hp'] - before['hp']:+d} HP"))
    if after["block"] != before["block"]:
        out.append(dict(t="block", s=f"Block {after['block'] - before['block']:+d}"))
    spent = before["energy"] - after["energy"]
    if spent:
        out.append(dict(t="energy", s=f"Energy {-spent:+d}"))
    for pw in pdiff(before["powers"], after["powers"]):
        out.append(dict(t="power", s=f"You: {pw}"))
    be = {e["cid"]: e for e in before["enemies"]}
    for e in after["enemies"]:
        b = be.get(e["cid"])
        if not b:
            out.append(dict(t="power", s=f"{e['name']} appears"))
            continue
        if e["hp"] != b["hp"]:
            out.append(dict(t="dmg", s=f"{e['name']} {e['hp'] - b['hp']:+d} HP" + (" (dead)" if not e["alive"] else "")))
        if e["block"] != b["block"]:
            out.append(dict(t="block", s=f"{e['name']} block {e['block'] - b['block']:+d}"))
        for pw in pdiff(b["powers"], e["powers"]):
            out.append(dict(t="power", s=f"{e['name']}: {pw}"))
    ca, cb = all_cards(after), all_cards(before)
    if in_play:  # the card being played is in no visible pile while its prompt is open: it is not a new card when it lands
        cb = cb + collections.Counter([in_play])
    new = ca - cb
    if new:
        out.append(dict(t="card", s="Added to your deck: " + ", ".join(f"{k}" + (f" ×{v}" if v > 1 else "") for k, v in new.items())))
    names_after = {e["cid"] for e in after["enemies"]}
    for e in before["enemies"]:
        if e["alive"] and e["cid"] not in names_after:
            out.append(dict(t="dmg", s=f"{e['name']} {-e['hp']:+d} HP (defeated)"))
    if after["over"] and after["hp"] > 0 and not any(e["alive"] for e in after["enemies"]):
        # the fight ended on this action: what the end of combat does (relic heals, powers wearing off) is not the card's doing
        keep = tuple(e["name"] for e in before["enemies"])
        out = [e for e in out if e["t"] not in ("hp", "power", "card") or (e["t"] == "power" and e["s"].startswith(keep))]
        out.append(dict(t="win", s="Last enemy defeated"))
    kind = describe(a)[0]
    if kind == "play":
        drew = len(after["hand"]) - (len(before["hand"]) - 1)
        if drew > 0 and not before["decision"]:
            out.append(dict(t="card", s=f"Drew {drew}"))
        ex = after["sizes"]["exhaust"] - before["sizes"]["exhaust"]
        if ex > 0:
            out.append(dict(t="card", s=f"Exhausted {ex}"))
    return out


def story(steps):
    """steps: list of dict(obs, a, info); the last has a=None (terminal)."""
    states = [decode(s["obs"]) for s in steps]
    turns, cur = [], None
    hp_series = []
    in_play = None
    for i, s in enumerate(steps):
        st = states[i]
        hp_series.append(dict(i=i, hp=st["hp"], enemy=sum(e["hp"] for e in st["enemies"] if e["alive"]), turn=st["turn"]))
        if s["a"] is None:
            break
        if cur is None or st["turn"] != cur["n"]:
            cur = dict(n=st["turn"], start=dict(hp=st["hp"], max_hp=st["max_hp"], block=st["block"], energy=st["energy"], max_energy=st["max_energy"], powers=[f"{k} {v}" for k, v in st["powers"]],
                                                 hand=st["hand"], enemies=[dict(name=e["name"], hp=e["hp"], max_hp=e["max_hp"], block=e["block"], powers=[f"{k} {v}" for k, v in e["powers"]], intent=intent_text(e), alive=e["alive"]) for e in st["enemies"]],
                                                 sizes=st["sizes"], potions=[p for p in st["potions"] if p]), plays=[], end=None)
            turns.append(cur)
        a, nxt = s["a"], states[i + 1]
        kind = describe(a)[0]
        alts = None
        info = s.get("info")
        if info and info.get("q") is not None:
            alts = [dict(label=action_label(st, ac), p=round(p, 3), q=round(q, 3), chosen=(ac == a)) for ac, p, q, lg in zip(info["acts"], info["p"], info["q"], info["legal"]) if lg]
        if kind == "end":
            taken = st["hp"] - nxt["hp"]
            intended = intended_damage(st)
            blocked = max(0, min(st["block"], intended))
            ep = []
            nb = {e["cid"]: e for e in nxt["enemies"]}
            for e in st["enemies"]:
                n_ = nb.get(e["cid"])
                if n_:
                    for pw in pdiff(e["powers"], n_["powers"]):
                        ep.append(f"{e['name']}: {pw}")
            new = all_cards(nxt) - all_cards(st)
            cur["end"] = dict(intent="; ".join(f"{e['name']}: {intent_text(e)}" for e in st["enemies"] if e["alive"]), intended=intended, blocked=blocked, hp_lost=taken, block_before=st["block"],
                              enemy_changes=ep, added=[f"{k}" + (f" ×{v}" if v > 1 else "") for k, v in new.items()], hp_after=nxt["hp"], over=nxt["over"], alts=alts)
        else:
            fx = effects(st, nxt, a, in_play if kind in ("pick", "confirm") else None)
            if kind == "play":
                hc = st["hand"][describe(a)[1]] if describe(a)[1] < len(st["hand"]) else None
                in_play = hc["name"] if (hc and nxt["decision"]) else None
            elif kind not in ("pick",) or not nxt["decision"]:
                in_play = None
            cur["plays"].append(dict(kind=kind, text=action_label(st, a), effects=fx, alts=alts, energy_left=nxt["energy"],
                                     decision=(None if not st["decision"] else [c["name"] for c in st["decision"]["cands"]])))
    last = states[-1]
    return dict(turns=turns, hp=hp_series, final=dict(hp=last["hp"], max_hp=last["max_hp"], over=last["over"], enemies=[dict(name=e["name"], hp=e["hp"], alive=e["alive"]) for e in last["enemies"]]))


HTML_HEAD = """<title>%(title)s</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:opsz,wght@12..96,600;12..96,800&family=IBM+Plex+Sans:wght@400;500;600&family=IBM+Plex+Mono:wght@400;500&display=swap" rel="stylesheet">
"""

STYLE = r"""<style>
/* Layout: one column. Fight summary on top, a win / loss switch, an HP chart, then the turns as cards, each a small table of plays in order. */
:root {
  --bg: #eef2f0; --surface: #ffffff; --surface-2: #e4ebe7; --fg: #17211d; --muted: #5a6962; --line: #cfd9d3;
  --win: #0e7a63; --loss: #b23a2b; --dmg: #b8481c; --block: #2c63a8; --energy: #8a6a00; --card: #5b4a9e; --power: #7a4b8f;
  --win-bg: #d9efe8; --loss-bg: #f6dcd6;
  --display: "Bricolage Grotesque", "Avenir Next", "Segoe UI", sans-serif; --body: "IBM Plex Sans", "Segoe UI", system-ui, sans-serif; --mono: "IBM Plex Mono", ui-monospace, Menlo, monospace;
}
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) {
  --bg: #0f1613; --surface: #17201c; --surface-2: #202b26; --fg: #e3ece7; --muted: #94a59c; --line: #2c3a33;
  --win: #4ac9a6; --loss: #ee7d6b; --dmg: #f08d5a; --block: #7ab0f0; --energy: #e0bd52; --card: #aa9be8; --power: #cb9be0;
  --win-bg: #123329; --loss-bg: #3a1d19; color-scheme: dark; } }
:root[data-theme="dark"] {
  --bg: #0f1613; --surface: #17201c; --surface-2: #202b26; --fg: #e3ece7; --muted: #94a59c; --line: #2c3a33;
  --win: #4ac9a6; --loss: #ee7d6b; --dmg: #f08d5a; --block: #7ab0f0; --energy: #e0bd52; --card: #aa9be8; --power: #cb9be0;
  --win-bg: #123329; --loss-bg: #3a1d19; color-scheme: dark; }
body { background: var(--bg); color: var(--fg); font-family: var(--body); font-size: 15px; line-height: 1.5; padding-inline: 16px; padding-block: 28px 64px; }
.wrap { max-width: 880px; margin: 0 auto; display: flex; flex-direction: column; gap: 28px; }
h1, h2, h3 { font-family: var(--display); margin: 0; text-wrap: balance; line-height: 1.15; }
h1 { font-size: clamp(28px, 5vw, 40px); font-weight: 800; letter-spacing: -0.01em; }
h2 { font-size: 22px; font-weight: 800; } h3 { font-size: 16px; font-weight: 600; }
.mono, .num { font-family: var(--mono); font-variant-numeric: tabular-nums; }
.muted { color: var(--muted); }
.head { display: flex; flex-direction: column; gap: 14px; }
.head p { margin: 0; max-width: 65ch; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; }
.chip { background: var(--surface-2); border: 1px solid var(--line); border-radius: 6px; padding: 2px 8px; font-size: 13px; white-space: nowrap; }
.chip b { font-family: var(--mono); font-weight: 500; }
.stats { display: flex; flex-wrap: wrap; gap: 8px 24px; font-size: 14px; }
.switch { display: flex; gap: 8px; flex-wrap: wrap; }
.switch button { font: inherit; font-weight: 600; border: 1.5px solid var(--line); background: var(--surface); color: var(--fg); padding: 8px 16px; border-radius: 8px; cursor: pointer; }
.switch button[aria-pressed="true"].win { background: var(--win-bg); border-color: var(--win); color: var(--win); }
.switch button[aria-pressed="true"].loss { background: var(--loss-bg); border-color: var(--loss); color: var(--loss); }
.switch button:focus-visible, summary:focus-visible { outline: 2px solid var(--block); outline-offset: 2px; }
.panel { background: var(--surface); border: 1px solid var(--line); border-radius: 10px; padding: 16px; }
.chart svg { width: 100%; height: auto; display: block; }
.chart text { fill: var(--muted); font-family: var(--mono); font-size: 11px; }
.legend { display: flex; gap: 16px; flex-wrap: wrap; font-size: 13px; color: var(--muted); margin-top: 6px; }
.legend i { display: inline-block; width: 18px; height: 3px; vertical-align: middle; margin-right: 6px; border-radius: 2px; }
.turn { background: var(--surface); border: 1px solid var(--line); border-radius: 10px; overflow: hidden; }
.turn > header { display: flex; flex-wrap: wrap; align-items: baseline; gap: 6px 16px; padding: 12px 16px; background: var(--surface-2); }
.turn > header h3 { margin-right: auto; }
.turn .body { padding: 14px 16px; display: flex; flex-direction: column; gap: 12px; }
.row { display: flex; flex-wrap: wrap; gap: 6px 12px; align-items: baseline; }
.label { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); min-width: 4.5em; }
.hand { display: flex; flex-wrap: wrap; gap: 6px; }
.hc { border: 1px solid var(--line); border-radius: 6px; padding: 2px 8px; font-size: 13px; background: var(--bg); }
.hc.off { opacity: 0.5; }
.hc .c { font-family: var(--mono); color: var(--energy); margin-right: 4px; }
.plays { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; }
.play { display: grid; grid-template-columns: 2em minmax(0, 1fr); gap: 2px 8px; padding: 8px 0; border-top: 1px solid var(--line); }
.play:first-child { border-top: 0; }
.idx { font-family: var(--mono); color: var(--muted); font-size: 13px; padding-top: 1px; }
.what { font-weight: 600; }
.fx { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: 13px; font-family: var(--mono); grid-column: 2; }
.fx .win { color: var(--win); font-weight: 500; } .fx .hp, .fx .dmg { color: var(--dmg); } .fx .block { color: var(--block); } .fx .energy { color: var(--energy); } .fx .card { color: var(--card); } .fx .power { color: var(--power); }
.enemy-turn { border-left: 3px solid var(--loss); padding: 2px 0 2px 12px; display: flex; flex-direction: column; gap: 2px; }
.enemy-turn.safe { border-left-color: var(--block); }
details.alts { grid-column: 2; font-size: 13px; }
details.alts summary { cursor: pointer; color: var(--muted); width: max-content; }
.alts table { border-collapse: collapse; margin-top: 4px; display: block; overflow-x: auto; }
.alts td { padding: 2px 14px 2px 0; white-space: nowrap; font-family: var(--mono); font-size: 12.5px; }
.alts tr.chosen td { color: var(--win); font-weight: 500; }
.result { display: flex; flex-wrap: wrap; gap: 8px 16px; align-items: baseline; padding: 14px 16px; border-radius: 10px; font-weight: 600; }
.result.win { background: var(--win-bg); color: var(--win); } .result.loss { background: var(--loss-bg); color: var(--loss); }
.bars { display: grid; grid-template-columns: minmax(7em, 11em) minmax(0, 1fr) 4.5em; gap: 6px 10px; align-items: center; font-size: 13px; }
.bars .nm { text-align: right; }
.track { position: relative; height: 16px; background: var(--surface-2); border-radius: 4px; }
.track .zero { position: absolute; top: -2px; bottom: -2px; width: 2px; background: var(--fg); opacity: .55; }
.track .fill { position: absolute; top: 2px; bottom: 2px; border-radius: 3px; }
.fill.up { background: var(--win); } .fill.down { background: var(--loss); } .fill.flat { background: var(--muted); }
.two { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr)); gap: 16px; }
.panel h2 { margin-bottom: 4px; } .panel .sub { margin: 0 0 12px; font-size: 13px; color: var(--muted); max-width: 62ch; }
.note { font-size: 13px; color: var(--muted); max-width: 70ch; }
footer { font-size: 13px; color: var(--muted); }
@media (prefers-reduced-motion: no-preference) { .turn { transition: border-color .15s; } }
</style>"""

BODY = """<div class="wrap" id="app"></div>"""

SCRIPT = r"""<script>
const DATA = %(data)s;
const $ = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
const app = document.getElementById('app');
const fmt = (n) => (n > 0 ? '+' : '') + n;
function header() {
  const s = DATA.scenario, h = $('header', 'head');
  h.append($('h1', '', DATA.title));
  const p = $('p', 'muted', DATA.subtitle); h.append(p);
  const chips = $('div', 'chips');
  const mk = (a, b) => { const c = $('span', 'chip'); c.append(document.createTextNode(a + ' ')); c.append($('b', '', b)); return c; };
  chips.append(mk('HP', s.hp + '/' + s.max_hp), mk('Ascension', String(s.ascension)), mk('Energy', String(s.energy)));
  s.relics.forEach(r => chips.append($('span', 'chip', r)));
  (s.potions || []).forEach(r => chips.append($('span', 'chip', 'Potion: ' + r)));
  h.append(chips);
  const deck = $('div', 'chips');
  s.deck.forEach(([n, k]) => { const c = $('span', 'chip', n); if (k > 1) c.append($('b', '', ' ×' + k)); deck.append(c); });
  h.append($('div', 'label', 'Deck (' + s.deck.reduce((a, b) => a + b[1], 0) + ' cards)'), deck);
  const st = $('div', 'stats');
  const o = DATA.stats;
  [['Model attempts', o.attempts], ['Wins', o.wins + ' (' + Math.round(100 * o.wins / o.attempts) + '%)'], ['Mean HP left on a win', o.hp_left_on_win.toFixed(0) + ' / ' + s.max_hp], ['Played with', o.mode]]
    .forEach(([a, b]) => { const d = $('div'); d.append($('span', 'muted', a + ': '), $('b', 'num', String(b))); st.append(d); });
  h.append(st);
  return h;
}
function chart(tr) {
  const W = 760, H = 220, L = 38, R = 14, T = 14, B = 26, hp = tr.hp;
  const n = hp.length - 1 || 1, maxP = tr.max_hp, maxE = Math.max(1, ...hp.map(d => d.enemy));
  const X = i => L + (W - L - R) * i / n, Yp = v => T + (H - T - B) * (1 - v / maxP), Ye = v => T + (H - T - B) * (1 - v / maxE);
  let s = `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="Player and enemy HP over the fight">`;
  let lastTurn = null;
  hp.forEach((d, i) => { if (d.turn !== lastTurn) { s += `<line x1="${X(i)}" x2="${X(i)}" y1="${T}" y2="${H - B}" stroke="var(--line)" stroke-dasharray="3 3"/><text x="${X(i) + 3}" y="${H - 8}">T${d.turn}</text>`; lastTurn = d.turn; } });
  [0, 0.5, 1].forEach(f => { s += `<line x1="${L}" x2="${W - R}" y1="${Yp(maxP * f)}" y2="${Yp(maxP * f)}" stroke="var(--line)"/><text x="${L - 6}" y="${Yp(maxP * f) + 4}" text-anchor="end">${Math.round(maxP * f)}</text>`; });
  const line = (f, col) => `<polyline fill="none" stroke="${col}" stroke-width="2.4" stroke-linejoin="round" points="${hp.map((d, i) => X(i).toFixed(1) + ',' + f(d).toFixed(1)).join(' ')}"/>`;
  s += line(d => Ye(d.enemy), 'var(--loss)') + line(d => Yp(d.hp), 'var(--win)');
  const e = hp[hp.length - 1];
  s += `<circle cx="${X(hp.length - 1)}" cy="${Yp(e.hp)}" r="4" fill="var(--win)"/><circle cx="${X(hp.length - 1)}" cy="${Ye(e.enemy)}" r="4" fill="var(--loss)"/>`;
  return s + '</svg>';
}
function alts(list) {
  if (!list) return null;
  const d = $('details', 'alts'), sm = $('summary', '', 'Search weighed ' + list.length + ' options');
  d.append(sm);
  const t = $('table'); const head = $('tr'); ['option', 'model chance', 'search value'].forEach(x => head.append($('td', 'muted', x))); t.append(head);
  list.slice().sort((a, b) => b.q - a.q).forEach(a => { const r = $('tr', a.chosen ? 'chosen' : ''); r.append($('td', '', (a.chosen ? '▸ ' : '  ') + a.label), $('td', '', Math.round(a.p * 100) + '%'), $('td', '', fmt(a.q))); t.append(r); });
  d.append(t); return d;
}
function turnCard(t, i) {
  const c = $('section', 'turn'), h = $('header'), s = t.start;
  h.append($('h3', '', 'Turn ' + t.n));
  const hpEnd = t.end ? t.end.hp_after : null;
  h.append($('span', 'num', 'You ' + s.hp + '/' + s.max_hp + ' HP' + (s.block ? ', ' + s.block + ' block' : '')), $('span', 'num', s.energy + '/' + s.max_energy + ' energy'));
  s.enemies.filter(e => e.alive).forEach(e => h.append($('span', 'num', e.name + ' ' + e.hp + '/' + e.max_hp + (e.block ? ' (+' + e.block + ' block)' : ''))));
  c.append(h);
  const b = $('div', 'body');
  const r1 = $('div', 'row'); r1.append($('span', 'label', 'Enemy next'));
  s.enemies.filter(e => e.alive).forEach(e => r1.append($('span', '', e.name + ': ' + e.intent + (e.powers.length ? '  [' + e.powers.join(', ') + ']' : ''))));
  b.append(r1);
  const r2 = $('div', 'row'); r2.append($('span', 'label', 'Hand'));
  const hand = $('div', 'hand');
  s.hand.forEach(k => { const x = $('span', 'hc' + (k.playable ? '' : ' off')); const cost = $('span', 'c', k.cost < 0 ? 'X' : String(k.cost)); x.append(cost, document.createTextNode(k.name + (k.dmg ? ' · ' + k.dmg + ' dmg' : '') + (k.blk ? ' · ' + k.blk + ' blk' : ''))); hand.append(x); });
  r2.append(hand); b.append(r2);
  if (s.powers.length) { const r = $('div', 'row'); r.append($('span', 'label', 'You have'), $('span', '', s.powers.join(', '))); b.append(r); }
  const ol = $('ol', 'plays');
  t.plays.forEach((p, k) => {
    const li = $('li', 'play'); li.append($('span', 'idx', String(k + 1)), $('span', 'what', p.text));
    if (p.effects.length) { const fx = $('div', 'fx'); p.effects.forEach(e => fx.append($('span', e.t, e.s))); li.append(fx); }
    const a = alts(p.alts); if (a) li.append(a);
    ol.append(li);
  });
  if (t.plays.length) b.append(ol); else b.append($('div', 'muted', 'No cards played this turn.'));
  if (t.end) {
    const e = t.end, d = $('div', 'enemy-turn' + (e.hp_lost > 0 ? '' : ' safe'));
    d.append($('b', '', 'You end the turn' + (e.block_before ? ' with ' + e.block_before + ' block' : '')));
    const txt = e.intended > 0 ? ('Enemy attacks for ' + e.intended + (e.blocked ? ', ' + e.blocked + ' blocked' : '') + ' → you lose ' + e.hp_lost + ' HP (' + e.hp_after + ' left).') : (e.hp_lost > 0 ? 'You lose ' + e.hp_lost + ' HP (' + e.hp_after + ' left).' : 'No damage taken.');
    d.append($('span', '', e.intent ? e.intent + '. ' + txt : txt));
    if (e.enemy_changes.length) d.append($('span', 'muted num', e.enemy_changes.join('; ')));
    if (e.added.length) d.append($('span', 'muted num', 'Added to your deck: ' + e.added.join(', ')));
    const a = alts(e.alts); if (a) d.append(a);
    b.append(d);
  }
  c.append(b); return c;
}
function trace(tr) {
  const box = $('div'); box.style.display = 'flex'; box.style.flexDirection = 'column'; box.style.gap = '16px';
  const res = $('div', 'result ' + (tr.outcome === 'win' ? 'win' : 'loss'));
  res.append($('span', '', tr.outcome === 'win' ? 'Victory' : 'Defeat'), $('span', 'num', 'You ' + tr.final.hp + '/' + tr.final.max_hp + ' HP'), $('span', 'num', tr.turns.length + ' turns, ' + tr.steps + ' decisions'));
  tr.final.enemies.forEach(e => res.append($('span', 'num', e.name + ' ' + e.hp + ' HP')));
  box.append(res);
  box.append($('p', 'note', tr.why));
  const cp = $('div', 'panel chart'); cp.innerHTML = chart({ hp: tr.hp, max_hp: tr.final.max_hp });
  const lg = $('div', 'legend'); lg.innerHTML = '<span><i style="background:var(--win)"></i>Your HP</span><span><i style="background:var(--loss)"></i>Enemy HP (own scale)</span><span>dashed lines mark turns</span>';
  cp.append(lg); box.append(cp);
  box.append($('p', 'note', DATA.note));
  tr.turns.forEach((t, i) => box.append(turnCard(t, i)));
  return box;
}
function render(which) {
  const slot = document.getElementById('trace'); slot.replaceChildren(trace(DATA.traces[which]));
  document.querySelectorAll('.switch button').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.k === which)));
  try { history.replaceState(null, '', '#' + which); } catch (e) {}
}
app.append(header());
const sw = $('div', 'switch');
['win', 'loss'].forEach(k => { if (!DATA.traces[k]) return; const b = $('button', k, DATA.traces[k].label); b.dataset.k = k; b.type = 'button'; b.onclick = () => render(k); sw.append(b); });
app.append(sw);
const slot = $('div'); slot.id = 'trace'; app.append(slot);
const start = (location.hash === '#loss' && DATA.traces.loss) ? 'loss' : (DATA.traces.win ? 'win' : 'loss');
render(start);
function bars(rows, base, fmtv) {
  // rows: [label, value, extra]; bars grow from the value of `base`, scaled to the largest deviation
  const g = $('div', 'bars'); const span = Math.max(0.05, ...rows.map(r => Math.abs(r[1] - base))) * 1.1;
  rows.forEach(([lab, v, extra]) => {
    const d = v - base, w = 50 * Math.abs(d) / span;
    const t = $('div', 'track'), z = $('div', 'zero'); z.style.left = 'calc(50% - 1px)'; t.append(z);
    const f = $('div', 'fill ' + (Math.abs(d) < 0.005 ? 'flat' : d > 0 ? 'up' : 'down')); f.style.width = Math.max(w, 0.4) + '%'; f.style.left = d >= 0 ? '50%' : (50 - w) + '%'; t.append(f);
    g.append($('div', 'nm', lab), t, $('div', 'num', fmtv(d, v, extra)));
  });
  return g;
}
if (DATA.analysis) {
  const A = DATA.analysis, two = $('div', 'two');
  const p1 = $('section', 'panel');
  p1.append($('h2', '', 'Which card matters'), $('p', 'sub', 'Win rate when one copy of a card is taken out of the deck, against ' + Math.round(A.base.win * 100) + '% with the full deck (' + A.n + ' attempts each, model alone). Green: the deck is better without it.'));
  const rows = A.removal.slice().sort((a, b) => b.win - a.win).map(r => [r.card + (r.copies > 1 ? ' (×' + r.copies + ')' : ''), r.win, r]);
  p1.append(bars(rows, A.base.win, (d, v) => (d >= 0 ? '+' : '') + Math.round(d * 100) + ' pts'));
  const p2 = $('section', 'panel');
  p2.append($('h2', '', 'When to split the Parasite'), $('p', 'sub', 'The Parasite bursts into Wrigglers when it dies. Win rate when the model may not kill it before turn T (T = 1 is no restriction; the model never manages it before turn 2).'));
  const rows2 = A.split.map(r => ['not before turn ' + r.T, r.win, r]);
  p2.append(bars(rows2, A.split[0].win, (d, v, r) => Math.round(v * 100) + '%'));
  const obs = Object.entries(A.split_observed || {});
  if (obs.length) p2.append($('p', 'note', 'Left alone, the model splits on ' + obs.map(([t, o]) => 'turn ' + t + ' in ' + Math.round(o.share * 100) + '% of fights (' + Math.round(o.win * 100) + '% won)').join(', ') + '.'));
  two.append(p1, p2); app.append(two);
  if (A.potion) {
    const P = A.potion, pp = $('section', 'panel');
    pp.append($('h2', '', 'When to drink the Duplicator'), $('p', 'sub', 'The model drinks it on turn 1 in every fight, whatever it holds: it ends up duplicating ' + P.dup.slice(0, 4).map(d => d.card + ' (' + Math.round(d.share * 100) + '% of fights, ' + Math.round(d.win * 100) + '% won)').join(', ') + '. Win rate under forced rules, model alone:'));
    pp.append(bars(P.rules.map(r => [r.rule, r.win, r]), P.free, (d, v) => Math.round(v * 100) + '%'));
    pp.append($('p', 'note', 'Perfected Strike is in the opening hand in ' + Math.round(P.ps_open * 100) + '% of fights. Waiting for it is worth about ' + Math.round((P.rules[0].win - P.free) * 100) + ' points over the model\'s habit.'));
    app.append(pp);
  }
  if (A.search) { const sp = $('section', 'panel'); sp.append($('h2', '', 'Checked with search'), $('p', 'sub', 'The same question with the model plus play-out search (' + A.search_roots + ' attempts each).'));
    sp.append(bars(Object.entries(A.search).map(([k, v]) => [k, v.win, v]), (A.search['full deck'] || {}).win || A.base.win, (d, v) => Math.round(v * 100) + '%')); app.append(sp); }
}
app.append($('footer', '', DATA.footer));
</script>"""


def pick(steps_by_root, rec, want_win):
    idx = [i for i, r in enumerate(rec) if (r[1] == 1) == want_win and r[1] in (1, -1)]
    if not idx:
        return None
    if want_win:
        idx.sort(key=lambda i: rec[i][4])  # HP left
    else:
        idx.sort(key=lambda i: rec[i][3])  # length
    return idx[len(idx) // 2]


def scenario_summary(scen):
    deck = collections.Counter()
    for c in scen["deck"]:
        nm = c if isinstance(c, str) else c["id"] + ("+" if c.get("upgrade") else "")
        if isinstance(c, str):
            nm = c
        deck[title(nm.rstrip("+")) + ("+" if nm.endswith("+") else "")] += 1
    return dict(potions=[title(p) for p in scen.get("potions", [])], hp=scen["hp"], max_hp=scen["max_hp"], ascension=scen.get("ascension", 0), energy=scen.get("max_energy", 3), relics=[title(r if isinstance(r, str) else r["id"]) for r in scen["relics"]],
                deck=sorted(deck.items(), key=lambda kv: (-kv[1], kv[0])))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--scenario", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--roots", type=int, default=120)
    ap.add_argument("--M", type=int, default=5)
    ap.add_argument("--K", type=int, default=8)
    ap.add_argument("--greedy", action="store_true")
    ap.add_argument("--seed", type=int, default=11)
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("--standalone", action="store_true")
    ap.add_argument("--title")
    ap.add_argument("--potion", help="JSON written by rl/potion_whatif.py")
    ap.add_argument("--analysis", help="JSON written by rl/whatif.py: adds the card-removal and split-timing panels")
    a = ap.parse_args()
    torch.set_num_threads(a.threads)
    scen = json.load(open(a.scenario))
    tmp = a.out + ".scenarios.json"
    json.dump([scen], open(tmp, "w"))
    net = load(a.ckpt)
    trace = [[] for _ in range(a.roots)]
    s = Searcher(net, a.roots, a.M, a.K, 0.0, seed=a.seed, max_steps=300, conf=(0.0 if a.greedy else 1.01))
    if a.greedy:
        s.conf = 0.0  # never search: the policy's own top action
    rec = s.play(tmp, seed=a.seed, verbose=False, with_records=True, trace=trace)
    os.remove(tmp)
    rec = [tuple(r) for r in rec]
    wins = [r for r in rec if r[1] == 1]
    mode = "the policy alone (greedy)" if a.greedy else f"policy + search ({a.M} options × {a.K} simulated futures per decision)"
    data = dict(
        title=a.title or f"{title(scen['encounter'])}: play by play",
        subtitle=f"{title(scen['character'])} against {title(scen['encounter'])}, played by the trained model. The best and the worst of {a.roots} attempts at the same fight.",
        scenario=scenario_summary(scen),
        stats=dict(attempts=len(rec), wins=len(wins), hp_left_on_win=(float(np.mean([r[4] for r in wins])) * scen["max_hp"] if wins else 0.0), mode=mode),
        note="Every line is read from what the agent saw (hand, piles, enemy intents), the same information a player has. The agent does not see the order of its draw pile or any random outcome in advance. “Search value” is the average final score of the simulated futures after that option: +1 for a win (up to +0.5 more for HP left), −1 for a loss.",
        footer="Generated by rl/trace.py from the simulator, which is checked against the game's own code.",
        traces={})
    # the extremes: the win that keeps the most HP (ties: fewer decisions); the loss that leaves the most enemy HP standing (ties: shorter)
    wins_i = [i for i, r in enumerate(rec) if r[1] == 1]
    loss_i = [i for i, r in enumerate(rec) if r[1] == -1 or r[1] == 2]
    chosen = {}
    if wins_i:
        chosen["win"] = max(wins_i, key=lambda i: (rec[i][4], -rec[i][3]))
    if loss_i:
        def left(i):
            return sum(e["hp"] for e in decode(trace[i][-1]["obs"])["enemies"] if e["alive"])
        chosen["loss"] = max(loss_i, key=lambda i: (left(i), -rec[i][3]))
    if a.analysis:
        w = json.load(open(a.analysis))
        data["analysis"] = dict(base=w["base"], n=w["base"]["n"], removal=[dict(card=title(r["card"].rstrip("+")) + ("+" if r["card"].endswith("+") else ""), copies=r["copies"], win=r["win"]) for r in w["removal"]],
                                split=[dict(T=r["T"], win=r["win"]) for r in w["split"]], split_observed=w.get("split_observed", {}),
                                search=({k: v for k, v in w["search"].items()} if w.get("search") else None), search_roots=240)
    if a.potion and "analysis" in data:
        pw = json.load(open(a.potion))
        rules = [dict(rule=k, win=v) for k, v in pw.items() if k not in ("free", "dup", "ps_open")]
        data["analysis"]["potion"] = dict(free=pw["free"], ps_open=pw.get("ps_open", 0.34), rules=rules,
                                          dup=[dict(card=title(k.rstrip("+")) + ("+" if k.endswith("+") else ""), share=v["share"], win=v["win"]) for k, v in pw["dup"].items()])
    for key, i in chosen.items():
        st = story(trace[i])
        st.update(outcome=key, steps=len(trace[i]) - 1)
        if key == "win":
            st.update(label="Best line", why=f"The win that kept the most HP of the {len(wins_i)} wins: {st['final']['hp']} of {st['final']['max_hp']}.")
        else:
            rem = sum(e["hp"] for e in st["final"]["enemies"] if e["alive"])
            st.update(label="Worst line", why=f"The loss that left the most enemy HP standing of the {len(loss_i)} losses: {rem} HP still on the board when you died.")
        data["traces"][key] = st
    head = HTML_HEAD.replace("%(title)s", data["title"]) + STYLE
    script = SCRIPT.replace("%(data)s", json.dumps(data, ensure_ascii=False))
    if a.standalone:
        body = ("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
                + head + "</head><body>" + BODY + script + "</body></html>")
    else:
        body = head + "\n" + BODY + "\n" + script
    open(a.out, "w").write(body)
    print(f"wrote {a.out}: {len(rec)} attempts, {len(wins)} wins; traces: {list(data['traces'])}")


if __name__ == "__main__":
    main()
