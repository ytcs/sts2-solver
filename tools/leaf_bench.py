#!/usr/bin/env python3
"""Value-head bench at fight states whose value hides HP behind an enemy's death (Test Subject revives, Waterfall Giant's explosion):
the value head vs solver play-outs (lower bounds) from the same state. data/bench/leaf_values.json holds each state as a recipe
(scenario, seed, ops) and its play-out target.

usage: python tools/leaf_bench.py build [--n 256] [--only NAME,...]   # play-outs -> data/bench/leaf_values.json
       python tools/leaf_bench.py check [--ckpt models/solver_rN.pt]  # value head vs targets; exit 1 if any state is off by > 2 se
"""
import argparse, json, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
sys.path.insert(0, os.path.join(ROOT, "rl"))
import numpy as np  # noqa: E402
import torch  # noqa: E402

BENCH = os.path.join(ROOT, "data", "bench", "leaf_values.json")


def card(cid, up=0, cost=None, ench=None):
    d = {"id": cid, "upgrade": up}
    if cost is not None:
        d["cost"] = cost
    if ench:
        d["enchantment"] = {"id": ench[0], "amount": ench[1]}
    return d


def play(i, t=None):
    return ["apply", {"play": {"hand_pos": i, "target": t}} if t is not None else {"play": {"hand_pos": i}}]


END = ["apply", {"end_turn": True}]


def ts_items():
    """Run 2 (BASE0122, E100), Test Subject phase 1, reconstructed from the replay log (decision ids 659-664)."""
    sc = {"name": "run2_test_subject", "ascension": 10, "encounter": "TEST_SUBJECT_BOSS", "character": "IRONCLAD", "hp": 53, "max_hp": 80, "max_energy": 3,
          "gold": 62, "max_potion_slots": 2, "base_orb_slots": 0, "seed": "BASE0122", "total_floor": 48, "act": 2,
          "deck": [card("STRIKE_IRONCLAD", 1), card("STRIKE_IRONCLAD", 1), card("STRIKE_IRONCLAD"), card("DEFEND_IRONCLAD"), card("DEFEND_IRONCLAD"),
                   card("DEFEND_IRONCLAD"), card("ASCENDERS_BANE"), card("MOLTEN_FIST"), card("BULLY", 1), card("BLUDGEON", 1, ench=("PERFECT_FIT", 1)),
                   card("TREMBLE"), card("DEMON_FORM", 1), card("BREAK", 1), card("BATTLE_TRANCE", 1), card("METAMORPHOSIS"),
                   card("TEAR_ASUNDER", ench=("SHARP", 2)), card("UPPERCUT")],
          "relics": [{"id": r} for r in ("BURNING_BLOOD", "PRECISE_SCISSORS", "AMETHYST_AUBERGINE", "ANCHOR", "ARCHAIC_TOOTH", "TUNGSTEN_ROD", "WHETSTONE",
                                         "RED_SKULL")]
                    + [{"id": "PEN_NIB", "props": {"AttacksPlayed": 5}, "counter": 5}, {"id": "JUZU_BRACELET"}, {"id": "DELICATE_FROND"},
                       {"id": "FORGOTTEN_SOUL"}, {"id": "HAPPY_FLOWER", "props": {"TurnsSeen": 0}, "counter": 0}, {"id": "MERCURY_HOURGLASS"}],
          "potions": [{"id": "WEAK_POTION", "slot": 0}]}
    S, D, DF, AB, MG = card("STRIKE_IRONCLAD", 1), card("DEFEND_IRONCLAD"), card("DEMON_FORM", 1), card("ASCENDERS_BANE"), card("MANGLE", 0, 0)
    HB, BR = card("HEADBUTT", 0, 0), card("BREAKTHROUGH", 0, 0)
    ops = [["sync", {"hand": [card("UPPERCUT"), card("BLUDGEON", 1, ench=("PERFECT_FIT", 1)), D, card("BREAK", 1), card("MOLTEN_FIST")], "energy": 3,
                     "potions": [{"id": "WEAK_POTION", "slot": 0}, {"id": "BLOCK_POTION", "slot": 1}]}],
           play(0, 0), ["apply", {"use_potion": {"slot": 0, "target": 0}}], play(2, 0), END,
           ["sync", {"hand": [D, S, card("BULLY", 1), card("TEAR_ASUNDER", ench=("SHARP", 2)), card("TREMBLE")]}], END,
           ["sync", {"hand": [S, card("METAMORPHOSIS"), card("BATTLE_TRANCE", 1), DF, D, AB]}], play(1), play(1),
           ["sync", {"hand": [S, DF, D, MG, AB, HB, BR], "energy": 2}]]
    at = {"ts659": list(ops)}
    ops += [play(6), ["sync", {"hand": [S, DF, D, MG, AB, HB], "energy": 2}]]
    at["ts660"] = list(ops)
    ops += [play(5, 0), ["pick_if_choice", 1], ["sync", {"hand": [S, DF, D, MG, AB], "energy": 2, "potions": []}]]
    at["ts663"] = list(ops)
    ops += [play(2), ["sync", {"hand": [S, DF, MG, AB], "energy": 1, "potions": []}]]
    at["ts664"] = list(ops)
    at["ts664_end"] = ops + [END]
    at["ts664_mangle_end"] = ops + [play(2, 0), END]
    return [dict(name=k, scenario=sc, seed=1, ops=v) for k, v in at.items()]


def giant_items():
    """Waterfall Giant (act-1 Underdocks boss) at 6 HP: before the kill, after it (ABOUT_TO_BLOW queued), and with EXPLODE pending."""
    S, D, SH, IW = card("STRIKE_IRONCLAD"), card("DEFEND_IRONCLAD"), card("SHRUG_IT_OFF"), card("IRON_WAVE")
    sc = {"name": "giant", "ascension": 10, "encounter": "WATERFALL_GIANT_BOSS", "character": "IRONCLAD", "hp": 60, "max_hp": 80, "max_energy": 3,
          "gold": 99, "max_potion_slots": 2, "base_orb_slots": 0, "seed": "giant", "total_floor": 17, "act": 0,
          "deck": [S, S, S, S, D, D, D, D, card("BASH"), SH, card("POMMEL_STRIKE"), IW, card("INFLAME"), card("TWIN_STRIKE")],
          "relics": [{"id": "BURNING_BLOOD"}], "potions": []}
    out = []

    def pre(hp, steam, ehp=6):
        return [["sync", {"hand": [S, S, D, D, D], "energy": 3, "player_hp": hp, "enemy_hp": [ehp]}], ["set_power", "enemy", 0, "STEAM_ERUPTION_POWER", steam]]

    for hp, steam in ((40, 30), (25, 40), (15, 30)):
        out.append(dict(name=f"giant_pre_hp{hp}_steam{steam}", ops=pre(hp, steam)))
        out.append(dict(name=f"giant_killed_hp{hp}_steam{steam}", ops=pre(hp, steam) + [play(0, 0)]))
    out.append(dict(name="giant_alive30_hp40_steam30", ops=pre(40, 30, ehp=30)))
    for hp, steam, hand, tag in ((40, 30, [S, S, D, D, D], "ssddd"), (25, 40, [S, S, D, D, D], "ssddd"), (25, 40, [S, D, D, D, SH], "sddd_shrug"),
                                 (15, 30, [S, S, D, D, D], "ssddd"), (15, 30, [S, D, D, SH, IW], "sdd_shrug_ironwave"), (60, 50, [S, S, D, D, D], "ssddd")):
        out.append(dict(name=f"giant_explode_hp{hp}_steam{steam}_{tag}", ops=pre(hp, steam) + [play(0, 0), END, ["sync", {"hand": hand, "energy": 3}]]))
    for it in out:
        it.update(scenario=sc, seed=1)
    return out


def build_state(it):
    import sts2
    sim = sts2.Sim(json.dumps(it["scenario"]), it["seed"])
    for op in it["ops"]:
        if op[0] == "apply":
            sim.apply(json.dumps(op[1]))
        elif op[0] == "set_power":
            assert sim.set_power(*op[1:]), op
        elif op[0] == "kill":
            sim.kill(op[1])
        elif op[0] == "settle":
            for _ in range(20):
                if sim.stage() != "choice":
                    break
                legal = sim.legal()
                sim.step(next((j for j, t in legal if t == "confirm"), legal[0][0]))
        elif op[0] == "pick_if_choice":
            if sim.stage() == "choice":
                legal = sim.legal()
                sim.step(legal[min(op[1], len(legal) - 1)][0])
        elif op[0] == "sync":
            real = json.loads(sim.snapshot())
            p = op[1]
            real.pop("draw", None)
            for k in ("hand", "energy", "potions"):
                if k in p:
                    real[k] = p[k]
            if "player_hp" in p:
                real["player"]["hp"] = p["player_hp"]
            for e, hp in zip(real["enemies"], p.get("enemy_hp", [])):
                e["hp"] = hp
            sim.sync(json.dumps(real))
        else:
            raise ValueError(op)
    return sim


def net_values(net, sims, max_hp):
    import sts2
    import heads
    o = np.zeros((len(sims), sts2.OBS_SIZE), np.float32)
    m = np.zeros(sts2.ACTIONS, np.uint8)
    for i, s in enumerate(sims):
        s.observe(o[i], m)
    with torch.no_grad():
        p = torch.softmax(net.heads_out(torch.from_numpy(o).to(next(net.parameters()).device)).float(), 1).cpu().numpy()
    u = heads.utility(torch.tensor(max_hp)).numpy()
    return p[:, 1:].sum(1), (p * u).sum(1)


def describe(sim):
    sn = json.loads(sim.snapshot())
    es = " ".join(f"{e['id']} {e['hp']}/{e['max_hp']} {e.get('next_move')} {[(p['id'], p['amount']) for p in e.get('powers', [])]}" for e in sn["enemies"])
    return f"player {sn['player']['hp']}+{sn['player']['block']} round {sn.get('round')} | {es}"


def se(x):
    return float(x.std(ddof=1) / np.sqrt(len(x))) if len(x) > 1 else 0.0


def playouts(eng, it, n):
    """the item's state, play-out target (live search with the encounter's live objective sans pair table, no potion reserve at a final-act
    boss as at the live pair; n attempts, hidden info re-sampled) and the value head at build, in place"""
    from agent.proposal import LAST_ACT, fight_objective
    sim = build_state(it)
    assert sim.outcome() == 0, it["name"]
    starts = []
    for k in range(n):
        s = sim.copy()
        s.determinize(1000 + k)
        starts.append(s)
    sc = it["scenario"]
    pair = sc.get("act") == LAST_ACT and str(sc.get("encounter", "")).endswith("_BOSS")
    out = eng.play_on(sc, starts, list(range(5000, 5000 + n)), worth=fight_objective(sc)[0], live=True, pot_cost=0.0 if pair else None)
    res = np.array([o[0] for o in out])
    frac = np.array([o[1] for o in out])
    win = (res == 1).astype(np.float64)
    obj = np.where(res == 1, 1.0 + 0.5 * np.minimum(frac, 1.0), -1.0)
    pw, v = net_values(eng.solver.net, [sim], [it["scenario"]["max_hp"]])
    it["state"] = describe(sim)
    it["target"] = dict(n=n, win=float(win.mean()), win_se=se(win), obj=float(obj.mean()), obj_se=se(obj))
    it["net_at_build"] = dict(p_win=round(float(pw[0]), 4), value=round(float(v[0]), 4))
    t = it["target"]
    print(f"{it['name']:40s} net P(win) {pw[0]:.3f} value {v[0]:+.3f} | play-outs win {t['win']:.3f} +- {t['win_se']:.3f} obj {t['obj']:+.3f} +- {t['obj_se']:.3f} | {it['state']}",
          flush=True)
    return it


def cmd_build(a):
    os.environ.setdefault("STS2_DEVICE", "cpu")
    from agent.engine import Engine
    eng = Engine()
    items = ts_items() + giant_items()
    if a.only:
        items = [it for it in items if it["name"] in a.only.split(",")]
    old = {it["name"]: it for it in json.load(open(a.out))["items"]} if os.path.exists(a.out) else {}
    for it in items:
        old[it["name"]] = playouts(eng, it, a.n)
    doc = ("Value-head bench at states where an enemy's death hides HP (tools/leaf_bench.py, E102). target: live-search play-outs (Engine.play_on "
           "live=True: M=5 K=32 cover, exact turn, HP cap; act-1/2 bosses win-only with reserve 0.03, final-act bosses linear with reserve 0 (no pair table); "
           "hidden info re-sampled per attempt; lower bounds; E113). check: the value head's P(win) within 2 se of target win (se floored at 1/n).")
    model = json.load(open(os.path.join(ROOT, "models", "current.json")))["policy"]
    json.dump(dict(_doc=doc, model=model, items=list(old.values())), open(a.out, "w"))


def cmd_check(a):
    from model import load
    net = load(a.ckpt)
    items = json.load(open(BENCH))["items"]
    bad = 0
    for it in items:
        sim = build_state(it)
        assert describe(sim) == it["state"], (it["name"], describe(sim), it["state"])
        pw, v = net_values(net, [sim], [it["scenario"]["max_hp"]])
        t = it["target"]
        s = max(t["win_se"], 1.0 / t["n"])
        ok = abs(pw[0] - t["win"]) <= 2 * s
        bad += not ok
        print(f"{'ok  ' if ok else 'OFF '} {it['name']:40s} net P(win) {pw[0]:.3f} value {v[0]:+.3f} | play-outs win {t['win']:.3f} +- {t['win_se']:.3f} obj {t['obj']:+.3f}")
    print(f"{len(items) - bad}/{len(items)} within 2 se")
    sys.exit(1 if bad else 0)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build")
    b.add_argument("--n", type=int, default=256)
    b.add_argument("--only", default="")
    b.add_argument("--out", default=BENCH)
    c = sub.add_parser("check")
    c.add_argument("--ckpt", default=os.path.join(ROOT, "models", json.load(open(os.path.join(ROOT, "models", "current.json")))["policy"]))
    a = ap.parse_args()
    torch.set_num_threads(max(1, (os.cpu_count() or 2) // 2))
    {"build": cmd_build, "check": cmd_check}[a.cmd](a)


if __name__ == "__main__":
    main()
