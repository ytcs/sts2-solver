#!/usr/bin/env python3
"""Community runs (spire-codex.com bulk export, /api/exports/runs) as a jump-start prior, never a target: mixed skill, correlational; the export
carries no player identity (single-player id is 1), so the comparison is won vs lost runs.
table: one streaming pass over a version dump -> compact single-player standard A10 runs. decks: deck entering the first final boss, won vs lost.
usage: tools/codex.py table target/codex/runs-v0.111.0.jsonl.gz --out target/codex/a10.jsonl.gz
       tools/codex.py decks target/codex/a10.jsonl.gz [--character ironclad]
       tools/codex.py replays --out target/codex/replays   (public API: listing + replay logs, <= 1 request/s, resumable; index.jsonl, players.json)"""
import argparse, collections, gzip, json, os, re, time

STARTERS = {"STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "STRIKE_SILENT", "DEFEND_SILENT", "STRIKE_DEFECT", "DEFEND_DEFECT", "STRIKE_NECROBINDER",
            "DEFEND_NECROBINDER", "STRIKE_REGENT", "DEFEND_REGENT"}


def _id(x):
    return re.sub(r"^[A-Z]+\.", "", x or "")


def _card(c):
    return [_id(c["id"]), int(c.get("current_upgrade_level", c.get("upgrade_level", 0)) or 0), _id((c.get("enchantment") or {}).get("id"))]


def _floor(pt):
    ps = (pt.get("player_stats") or [{}])[0]
    rooms = pt.get("rooms") or [{}]
    return dict(t=pt.get("map_point_type"), enc=[_id(r.get("model_id")) for r in rooms if r.get("model_id")], hp=ps.get("current_hp"), mx=ps.get("max_hp"),
                gold=ps.get("current_gold"), dmg=ps.get("damage_taken"), turns=sum(r.get("turns_taken") or 0 for r in rooms),
                offer=[[_id(c["card"]["id"]), int(c["card"].get("current_upgrade_level", 0) or 0), int(bool(c.get("was_picked")))] for c in ps.get("card_choices") or []],
                gained=[_id(c["id"]) for c in ps.get("cards_gained") or []], removed=[_id(c["id"] if isinstance(c, dict) else c) for c in ps.get("cards_removed") or []],
                upgraded=[_id(c) for c in ps.get("upgraded_cards") or []], rest=ps.get("rest_site_choices") or [],
                relics=[[_id(r.get("choice")), int(bool(r.get("was_picked")))] for r in ps.get("relic_choices") or []],
                ancient=[[a.get("TextKey"), int(bool(a.get("was_chosen")))] for a in ps.get("ancient_choice") or []],
                pot_used=[_id(p) for p in ps.get("potion_used") or []], bought=[_id(c) for c in (ps.get("bought_colorless") or []) + (ps.get("bought_potions") or [])])


def table(src, out):
    n, kept, why = 0, 0, collections.Counter()
    from agent.macro import card_id_set
    known = card_id_set()
    with gzip.open(src, "rt", encoding="utf-8") as f, gzip.open(out, "wt", encoding="utf-8") as g:
        for line in f:
            n += 1
            try:
                d = json.loads(line)
            except ValueError:
                why["bad json"] += 1
                continue
            ps = d.get("players") or []
            cards = {_id(c.get("id")) for c in (ps[0].get("deck") or [])} if ps else set()
            for test, k in ((len(ps) == 1, "multiplayer"), (cards <= known, "modded cards"), (d.get("game_mode") == "standard", "mode"), (not d.get("modifiers"), "modifiers"),
                            (d.get("ascension") == 10, "ascension"), (not d.get("was_abandoned"), "abandoned")):
                if not test:
                    why[k] += 1
                    break
            else:
                p = ps[0]
                floors = [[_floor(pt) for pt in act] for act in d.get("map_point_history") or []]
                g.write(json.dumps(dict(hash=d.get("run_hash"), char=_id(p.get("character")).lower(), win=bool(d.get("win")),
                                        seed=d.get("seed"), beta=bool(d.get("is_beta")), acts=[_id(a) for a in d.get("acts") or []],
                                        killed_by=_id(d.get("killed_by_encounter")), run_time=d.get("run_time"), deck=[_card(c) + [c.get("floor_added_to_deck")] for c in p.get("deck") or []],
                                        relics=[[_id(r["id"]), r.get("floor_added_to_deck")] for r in p.get("relics") or []], floors=floors)) + "\n")
                kept += 1
            if n % 50000 == 0:
                print(f"  {n} read, {kept} kept", flush=True)
    print(f"{n} runs read, {kept} kept (single-player standard A10, not abandoned); dropped: {dict(why)}")


def rows(path, character=None):
    with gzip.open(path, "rt", encoding="utf-8") as f:
        for l in f:
            r = json.loads(l)
            if character is None or r["char"] == character:
                yield r


def walk(r):
    """per floor: (act index, index in act, floor g from 1, floor record, card ids entering it, relic ids owned entering it, upgrade counts by id);
    cards: the deck simulated through gained / removed, united with final-deck cards added before the floor (ancient cards are missing from gained);
    upgrades: rest / event upgrades plus upgraded picks"""
    seq = [f for a in r["floors"] for f in a]
    deck = collections.Counter(c[0] for c in r["deck"] if (c[3] or 0) <= 1) + collections.Counter(c for f in seq for c in f["removed"] if c in STARTERS or c == "BASH")
    up, g = collections.Counter(), 0
    for ai, act in enumerate(r["floors"]):
        for i, f in enumerate(act):
            g += 1
            yield ai, i, g, f, +deck | collections.Counter(c[0] for c in r["deck"] if (c[3] or 0) < g), {x for x, fx in r["relics"] if (fx or 0) < g}, +up
            for c in f["gained"] if g > 1 else []:
                deck[c] += 1
            for c in f["removed"]:
                deck[c] -= 1
            up.update(f["upgraded"])
            up.update(c for c, u, pk in f["offer"] if u and pk)


def deck_at_pair(r):
    """the deck entering the first final boss: final deck minus cards added from that floor on (removals after it are not undone)"""
    act3 = r["floors"][2] if len(r["floors"]) >= 3 else []
    start = sum(len(a) for a in r["floors"][:2])
    boss = next((start + i + 1 for i, pt in enumerate(act3) if pt["t"] == "boss"), None)
    if boss is None:
        return None, None
    return [c for c in r["deck"] if (c[3] or 0) < boss], boss


def decks(path, character):
    agg = collections.defaultdict(list)
    for r in rows(path, character):
        d, _ = deck_at_pair(r)
        if d is not None:
            agg[r["win"]].append((len(d), sum(c[0] in STARTERS for c in d), sum(c[1] > 0 for c in d)))
    print("deck entering the first final boss (runs that reached it): n, mean size, starters left, upgraded cards")
    for w, v in sorted(agg.items()):
        n = len(v)
        print(f"  {'won ' if w else 'lost'} n {n:6d}  size {sum(x[0] for x in v) / n:5.1f}  starters {sum(x[1] for x in v) / n:4.1f}  upgraded {sum(x[2] for x in v) / n:4.1f}")


API = "https://spire-codex.com/api"
_last = [0.0]


def _get(url, tries=8):
    """<= 1 request/s; backs off on 429 / 5xx / network errors; None on 404"""
    import urllib.error, urllib.request
    for k in range(tries):
        time.sleep(max(0.0, _last[0] + 1.05 - time.time()))
        _last[0] = time.time()
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "sts2-solver-research/1.0"}), timeout=90) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            if e.code != 429 and e.code < 500:
                raise
            time.sleep(float(e.headers.get("Retry-After") or 0) or min(300, 10 * 2 ** k))
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            time.sleep(min(300, 10 * 2 ** k))
    raise RuntimeError(f"giving up on {url}")


def replay_events(path):
    with gzip.open(path, "rt", encoding="utf-8") as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


CHARACTERS = {"ironclad", "silent", "defect", "necrobinder", "regent"}


def replay_flags(path):
    """per replay: reloads / resumes / in-fight retries (combat_id "#n") (kept as flags), console, gameplay mods, multiplayer, final end; replay_version and the fight fields it carries"""
    x = dict(reloads=0, resumes=0, retries=0, console=0, full_console=False, mods=set(), players=1, seed=None, character=None, end=None, floors=0, combats=0, events=0,
             version=set(), fields=collections.Counter())
    for e in replay_events(path):
        x["events"] += 1
        t = e["t"]
        if t == "header":
            x["seed"], x["character"] = e.get("seed"), (e.get("character") or "").lower()
            x["version"].add(e.get("replay_version"))
            x["reloads"] = max(x["reloads"], e.get("reloads") or 0)
            x["full_console"] |= bool(e.get("full_console"))
            x["players"] = max(x["players"], e.get("player_count") or 1)
            x["mods"] |= {m["id"] for m in e.get("mods") or [] if m.get("affects_gameplay")}
        elif t == "resume":
            x["resumes"] += 1
        elif t == "console":
            x["console"] += 1
        elif t == "combat_start":
            x["combats"] += 1
            x["retries"] += "#" in str(e.get("combat_id"))
            x["fields"]["rng_state"] += "rng_state" in e
        elif t in ("draw_order", "intent", "move") or t == "shuffle" and "order_c" in e:
            x["fields"][t] += 1
        elif t == "end":
            x["end"], x["floors"] = e.get("terminal_reason"), e.get("floors") or x["floors"]
    x["mods"], x["version"] = sorted(x["mods"]), sorted(v for v in x["version"] if v is not None)
    return x


def replays(out_dir, build, ascension, skip_list=False, index_only=False, limit=100):
    """list + fetch A10 single-player standard replays of one build into out_dir/<run_hash>.jsonl.gz (resumable), then index.jsonl and players.json"""
    os.makedirs(out_dir, exist_ok=True)
    lpath = os.path.join(out_dir, "listing.jsonl")
    rows = {}
    if os.path.exists(lpath):
        rows = {r["run_hash"]: r for r in map(json.loads, open(lpath, encoding="utf-8"))}
    if not (skip_list or index_only):
        page, new = 1, 0
        while True:
            d = json.loads(_get(f"{API}/replays?build_id={build}&ascension={ascension}&players=1&game_mode=standard&page={page}&limit={limit}"))
            for r in d.get("runs") or []:
                new += r["run_hash"] not in rows
                rows[r["run_hash"]] = r
            print(f"  listing page {page}: {len(rows)} rows ({new} new)", flush=True)
            if len(d.get("runs") or []) < limit:
                break
            page += 1
        with open(lpath + ".tmp", "w", encoding="utf-8") as f:
            for r in rows.values():
                f.write(json.dumps(r) + "\n")
        os.replace(lpath + ".tmp", lpath)
    todo = [] if index_only else [h for h, r in rows.items() if r.get("has_replay") and not os.path.exists(os.path.join(out_dir, h + ".jsonl.gz"))]
    print(f"{len(rows)} listed, {len(todo)} to fetch", flush=True)
    for i, h in enumerate(todo):
        data = _get(f"{API}/runs/{h}/replay")
        if data is None:
            continue
        if data[:2] != b"\x1f\x8b":
            data = gzip.compress(data)
        p = os.path.join(out_dir, h + ".jsonl.gz")
        with open(p + ".tmp", "wb") as f:
            f.write(data)
        os.replace(p + ".tmp", p)
        if (i + 1) % 100 == 0:
            print(f"  fetched {i + 1}/{len(todo)}", flush=True)
    index, size = [], 0
    for h, r in rows.items():
        p = os.path.join(out_dir, h + ".jsonl.gz")
        if not os.path.exists(p):
            continue
        size += os.path.getsize(p)
        try:
            x = replay_flags(p)
        except (OSError, ValueError, EOFError) as ex:
            x = dict(error=repr(ex)[:80], clean=False)
        x.update(run_hash=h, username=r.get("username"), player_token=r.get("player_token"), win=bool(r.get("win")), killed_by=r.get("killed_by"),
                 abandoned=bool(r.get("was_abandoned")), floors_reached=r.get("floors_reached"), base_seed=str(x.get("seed") or "").startswith("BASE"))
        x["complete"] = x.get("end") == "death" or x["win"]
        x["clean"] = (x["complete"] and not x["abandoned"] and not x["base_seed"] and x.get("character") in CHARACTERS and not x.get("console")
                      and not x.get("full_console") and not x.get("mods") and x.get("players", 1) == 1)
        index.append(x)
    # strong cohort: >= STRONG_RUNS clean runs and win rate shrunk toward the pooled rate (prior weight SHRINK runs) above the mean of that group
    STRONG_RUNS, SHRINK = 8, 10
    players = collections.defaultdict(lambda: dict(runs=0, wins=0, all=0, chars=collections.Counter(), names=set()))
    for x in index:
        p = players[x["player_token"]]
        p["all"] += 1
        p["names"].add(x["username"])
        if x["clean"]:
            p["runs"] += 1
            p["wins"] += x["win"]
            p["chars"][x.get("character")] += 1
    p0 = sum(p["wins"] for p in players.values()) / max(1, sum(p["runs"] for p in players.values()))
    ps = {k: dict(runs=v["runs"], wins=v["wins"], shrunk=round((v["wins"] + SHRINK * p0) / (v["runs"] + SHRINK), 4), all=v["all"], chars=dict(v["chars"]),
                  names=sorted(n for n in v["names"] if n)) for k, v in players.items()}
    cohort = [v["shrunk"] for v in ps.values() if v["runs"] >= STRONG_RUNS]
    bar = sum(cohort) / max(1, len(cohort))
    for v in ps.values():
        v["strong"] = v["runs"] >= STRONG_RUNS and v["shrunk"] > bar
    for x in index:
        x["strong"] = ps[x["player_token"]]["strong"]
    with open(os.path.join(out_dir, "index.jsonl"), "w", encoding="utf-8") as f:
        for x in index:
            f.write(json.dumps({k: (dict(v) if isinstance(v, collections.Counter) else v) for k, v in x.items()}) + "\n")
    json.dump(dict(pooled_win_rate=p0, strong_runs=STRONG_RUNS, shrink=SHRINK, strong_bar=bar, players=ps), open(os.path.join(out_dir, "players.json"), "w", encoding="utf-8"), indent=0)
    n = len(index)
    by = collections.Counter(x.get("character") for x in index)
    fl = collections.Counter(k for x in index for k in ("reloads", "resumes", "retries", "console", "full_console", "mods", "abandoned", "base_seed") if x.get(k))
    fl.update(k for x in index for k, bad in (("multiplayer", x.get("players", 1) > 1), ("incomplete", not x.get("complete")), ("modded character", x.get("character") not in CHARACTERS)) if bad)
    clean = [x for x in index if x["clean"]]
    print(f"{n} replays, {size / 1e6:.0f} MB gz; by character {dict(by)}; flagged {dict(fl)}")
    print(f"clean {len(clean)} (wins {sum(x['win'] for x in clean)}, with reloads {sum(bool(x['reloads']) for x in clean)}); by character {dict(collections.Counter(x['character'] for x in clean))}")
    print(f"replay_version {dict(collections.Counter(v for x in index for v in x.get('version') or []))}; fight fields in clean replays: " +
          str(dict(sum((collections.Counter(x.get("fields") or {}) for x in clean), collections.Counter()))) + f", combats {sum(x['combats'] for x in clean)}")
    strong = [v for v in ps.values() if v["strong"]]
    print(f"{len(ps)} players (pooled clean win rate {p0:.3f}); >= {STRONG_RUNS} clean runs: {len(cohort)} (mean shrunk {bar:.3f}); strong {len(strong)} players, "
          f"{sum(v['runs'] for v in strong)} runs, {sum(v['wins'] for v in strong)} wins; strong clean replays {sum(x['strong'] for x in clean)}")

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["table", "decks", "replays"])
    ap.add_argument("src", nargs="?")
    ap.add_argument("--out")
    ap.add_argument("--character")
    ap.add_argument("--build", default="v0.111.0")
    ap.add_argument("--ascension", type=int, default=10)
    ap.add_argument("--skip-list", action="store_true")
    ap.add_argument("--index-only", action="store_true")
    a = ap.parse_args()
    if a.cmd == "table":
        table(a.src, a.out)
    elif a.cmd == "replays":
        replays(a.out or "target/codex/replays", a.build, a.ascension, a.skip_list, a.index_only)
    else:
        decks(a.src, a.character)


if __name__ == "__main__":
    main()
