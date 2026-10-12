#!/usr/bin/env python3
"""Value-response sign gate on simulator-state edits (promotion acceptance for observation-v3/v4 checkpoints): powers, and what an enemy's death
does (Test Subject phase, a dying Waterfall Giant's blast, Axebot Stock). CPU; --parts must replay on this build (remapped parts on a v4 build).
usage: tools/power_coverage.py gate --parts GLOB --ckpt CKPT [--n 2000] --out FILE"""
import argparse, glob, json, os, sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path[:0] = [os.path.join(ROOT, "rl")]
import sts2  # noqa: E402

LAY = sts2.layout()
C = LAY["consts"]
SEC = {n: (o, s) for n, o, s in LAY["sections"]}
P, PF, EF, CF, E = C["OBS_POWERS"], C["POWER_F"], C["ENEMY_F"], C["CARD_F"], C["OBS_MAX_ENEMIES"]


def dec_powers(blk):
    """(..., P*PF) -> ids (-1 empty), amount, display"""
    b = blk.reshape(*blk.shape[:-1], P, PF)
    return b[..., 0].astype(np.int64) - 1, b[..., 1], b[..., 2]


def decode(obs):
    o, s = SEC["player"]
    pid, pam, _ = dec_powers(obs[:, o + 8:o + 8 + P * PF])
    o, s = SEC["enemies"]
    en = obs[:, o:o + s].reshape(len(obs), E, EF)
    alive = (en[..., 0] > 0.5) & (en[..., 6] > 0.5)
    eid, eam, _ = dec_powers(en[..., 8:8 + P * PF])
    o, s = SEC["hand"]
    hand = obs[:, o:o + s].reshape(len(obs), -1, CF)[..., 0].astype(np.int64) - 1
    return dict(pid=pid, pam=pam, eid=np.where(alive[..., None], eid, -1), eam=eam, hand=hand, rnd=obs[:, SEC["global"][0]].astype(np.int64))


def p_win(net, obs, bs=4096):
    import torch
    out = []
    with torch.no_grad():
        for i in range(0, len(obs), bs):
            ol = net.heads_out(torch.from_numpy(np.ascontiguousarray(obs[i:i + bs])))
            out.append(1.0 - torch.softmax(ol.float(), 1)[:, 0].numpy())
    return np.concatenate(out) if out else np.zeros(0)


def sim_states(paths, n, end_only, seed=0):
    """simulator states (copies) of single-enemy play rows without powers: trained rows, or with end_only every state where only end turn is legal"""
    out = []
    for path in paths:
        z = np.load(path)
        scen = json.loads(str(z["scenarios"]))
        dfi, dst = z["d_fight"], z["d_step"]
        for f in range(len(z["f_scen"])):
            acts = z["acts"][z["f_off"][f]:z["f_off"][f + 1]].astype(int)
            want = set(range(len(acts) + 1)) if end_only else set(dst[dfi == f].tolist())
            if not want:
                continue
            sim = sts2.Sim(json.dumps(scen[z["f_scen"][f]]), int(z["f_seed"][f]))
            ob, mk = np.zeros(sts2.OBS_SIZE, np.float32), np.zeros(sts2.ACTIONS, np.uint8)
            for t in range(len(acts) + 1):
                if t in want and sim.stage() == "play":
                    sim.observe(ob, mk)
                    d = decode(ob[None])
                    eo = SEC["enemies"][0]
                    en = ob[eo:eo + E * EF].reshape(E, EF)
                    alive = (en[:, 0] > 0.5) & (en[:, 6] > 0.5)
                    ok = alive.sum() == 1 and alive[0] and en[0, 3] <= 400 and not (d["eid"] >= 0).any() and not (d["pid"] >= 0).any()
                    if ok and end_only:
                        ok = mk[C["OFF_PLAY"]:C["OFF_DISCARD"]].sum() == 0
                    if ok:
                        out.append(sim.copy())
                if t < len(acts) and not sim.step(int(acts[t])):
                    break
            if len(out) >= 2 * n:
                break
        if len(out) >= 2 * n:
            break
    r = np.random.default_rng(seed)
    return [out[i] for i in sorted(r.choice(len(out), min(n, len(out)), replace=False))]


def enc_states(paths, enc, monster, n, seed=0):
    """simulator states (copies) at searched play rows of `enc` fights while a `monster` lives"""
    out = []
    for path in paths:
        z = np.load(path)
        scen = json.loads(str(z["scenarios"]))
        if "starts" in z:
            continue
        dfi, dst = z["d_fight"], z["d_step"]
        for f in range(len(z["f_scen"])):
            if scen[z["f_scen"][f]]["encounter"] != enc:
                continue
            acts = z["acts"][z["f_off"][f]:z["f_off"][f + 1]].astype(int)
            want = set(dst[dfi == f].tolist())
            sim = sts2.Sim(json.dumps(scen[z["f_scen"][f]]), int(z["f_seed"][f]))
            for t in range(len(acts) + 1):
                if t in want and sim.stage() == "play" and enemy_idx(sim, monster):
                    out.append(sim.copy())
                if t < len(acts) and not sim.step(int(acts[t])):
                    break
            if len(out) >= 4 * n:
                break
        if len(out) >= 4 * n:
            break
    r = np.random.default_rng(seed)
    return [out[i] for i in sorted(r.choice(len(out), min(n, len(out)), replace=False))]


def enemy_idx(s, monster):
    return [i for i, e in enumerate(json.loads(s.snapshot())["enemies"]) if e["id"] == monster and e["alive"]]


def ts_phase(phase):
    """Test Subject at its HP in `phase` (1: two revives ahead, 3: none): revive count, Adaptable, the later phases' powers cleared"""
    def edit(s):
        i = enemy_idx(s, "TEST_SUBJECT")[0]
        for pw in ("PAINFUL_STABS_POWER", "NEMESIS_POWER"):
            s.set_power("enemy", i, pw, 0)
        s.set_power("enemy", i, "ADAPTABLE_POWER", 1 if phase < 3 else 0)
        s.set_var(i, 0, phase - 1)
    return edit


def giant_dying(blast):
    """Waterfall Giant killed now with `blast(s)` Steam Eruption stacks (about to blow)"""
    def edit(s):
        i = enemy_idx(s, "WATERFALL_GIANT")[0]
        s.set_power("enemy", i, "STEAM_ERUPTION_POWER", int(blast(s)))
        s.kill(i)
    return edit


def axebot_stock(stock):
    def edit(s):
        for i in enemy_idx(s, "AXEBOT"):
            s.set_power("enemy", i, "STOCK_POWER", stock)
            s.set_var(i, 0, stock + 1)
    return edit


def lethal_blast(s):
    p = json.loads(s.snapshot())["player"]
    return p["hp"] + p["block"] + 10


DEATH_ROWS = [("TEST_SUBJECT_BOSS", "TEST_SUBJECT", "Test Subject at equal HP: phase 1 (two revives ahead) reads lower than phase 3", ts_phase(1), ts_phase(3)),
              ("WATERFALL_GIANT_BOSS", "WATERFALL_GIANT", "dying Waterfall Giant: blast >= player HP + block + 10 reads lower than blast 1",
               giant_dying(lethal_blast), giant_dying(lambda s: 1)),
              ("AXEBOTS_NORMAL", "AXEBOT", "Axebots with Stock 2 read lower than Stock 0", axebot_stock(2), axebot_stock(0))]


GATE = [("player Intangible 1 raises p_win", "player intangible 1", ">", "baseline"),
        ("enemy Intangible 1 lowers p_win", "enemy intangible 1", "<", "baseline"),
        ("enemy poison 10: player Accelerant 0 -> 1 does not lower p_win", "enemy poison 10 + player accelerant 1", ">=", "enemy poison 10"),
        ("enemy poison 10: player Accelerant 1 -> 2 does not lower p_win", "enemy poison 10 + player accelerant 2", ">=", "enemy poison 10 + player accelerant 1"),
        ("enemy poison 10: player Accelerant 2 -> 4 does not lower p_win", "enemy poison 10 + player accelerant 4", ">=", "enemy poison 10 + player accelerant 2"),
        ("player Wraith Form (card: Intangible 2 + Wraith Form 1) raises p_win", "player wraith form card", ">", "baseline"),
        ("player Buffer 1 raises p_win", "player buffer 1", ">", "baseline"),
        ("player Plating 5 raises p_win", "player plating 5", ">", "baseline")]


def sens_state(a):
    """value response to power edits made on the simulator state (the observation, v3 derived features included, is recomputed): the sign gate"""
    import model as M
    net = M.load(a.ckpt).cpu()
    paths = sorted(glob.glob(a.parts))
    res, rows = {}, {}

    def run(states, name, edits):
        obs = np.zeros((len(states), sts2.OBS_SIZE), np.float32)
        mk = np.zeros(sts2.ACTIONS, np.uint8)
        refused = 0
        for i, s in enumerate(states):
            s = s.copy()
            for side, pw, amt in edits:
                v = amt(s) if callable(amt) else amt
                refused += not s.set_power(side, 0, pw, int(v))
            s.observe(obs[i], mk)
        pw_ = p_win(net, obs)
        res[name] = dict(p_win=float(pw_.mean()), refused=refused)
        rows[name] = pw_
        return pw_

    hp = lambda s: json.loads(s.snapshot())["enemies"][0]["hp"]  # noqa: E731
    X = sim_states(paths, a.n, False)
    run(X, "baseline", [])
    run(X, "player intangible 1", [("player", "INTANGIBLE_POWER", 1)])
    run(X, "enemy intangible 1", [("enemy", "INTANGIBLE_POWER", 1)])
    run(X, "enemy poison 10", [("enemy", "POISON_POWER", 10)])
    for v in (1, 2, 4):
        run(X, f"enemy poison 10 + player accelerant {v}", [("enemy", "POISON_POWER", 10), ("player", "ACCELERANT_POWER", v)])
    run(X, "player wraith form card", [("player", "INTANGIBLE_POWER", 2), ("player", "WRAITH_FORM_POWER", 1)])
    run(X, "player wraith form power 1 alone", [("player", "WRAITH_FORM_POWER", 1)])
    run(X, "player buffer 1", [("player", "BUFFER_POWER", 1)])
    run(X, "player plating 5", [("player", "PLATING_POWER", 5)])
    gate = []
    for enc, monster, label, lo, hi in DEATH_ROWS:
        S = enc_states(paths, enc, monster, a.n_death)
        if not S:
            gate.append(dict(check=label, delta=None, se=None, passed=False, rows=0))
            continue
        pw = []
        for edit in (lo, hi):
            obs = np.zeros((len(S), sts2.OBS_SIZE), np.float32)
            mk = np.zeros(sts2.ACTIONS, np.uint8)
            for i, s in enumerate(S):
                s = s.copy()
                edit(s)
                s.observe(obs[i], mk)
            pw.append(p_win(net, obs))
        dlt = pw[0] - pw[1]
        res[label] = dict(lo=float(pw[0].mean()), hi=float(pw[1].mean()), rows=len(S))
        gate.append(dict(check=label, delta=round(float(dlt.mean()), 4), se=round(float(dlt.std() / np.sqrt(len(dlt))), 4), passed=bool(dlt.mean() < 0),
                         rows=len(S)))
    for label, x, op, y in GATE:
        dlt = rows[x] - rows[y]
        mean, se = float(dlt.mean()), float(dlt.std() / np.sqrt(len(dlt)))
        ok = mean > 0 if op == ">" else mean < 0 if op == "<" else mean >= 0
        gate.append(dict(check=label, delta=round(mean, 4), se=round(se, 4), passed=bool(ok)))
    Y = sim_states(paths, a.n, True, seed=1)
    if Y:
        run(Y, "end-turn-only: baseline", [])
        d9 = run(Y, "end-turn-only: enemy doom hp-1", [("enemy", "DOOM_POWER", lambda s: max(1, hp(s) - 1))])
        d0 = run(Y, "end-turn-only: enemy doom = hp", [("enemy", "DOOM_POWER", hp)])
        dlt = d0 - d9
        gate.append(dict(check="end-turn-only: doom = hp vs hp-1 separates (mean > 0.05, > half the rows > 0.05)", delta=round(float(dlt.mean()), 4),
                         se=round(float(dlt.std() / np.sqrt(len(dlt))), 4), share_gt_005=round(float((dlt > 0.05).mean()), 3),
                         passed=bool(dlt.mean() > 0.05 and (dlt > 0.05).mean() > 0.5)))
    out = dict(ckpt=a.ckpt, obs_version=net.obs, rows=len(X), end_turn_rows=len(Y), p_win=res, gate=gate, gate_pass=all(g["passed"] for g in gate))
    for g in gate:
        print(f"{'PASS' if g['passed'] else 'FAIL'}  {g['check']:72s} " + (f"{g['delta']:+.4f} +- {g['se']:.4f}" if g["delta"] is not None else "no states"))
    print("SIGN GATE " + ("PASS" if out["gate_pass"] else "FAIL"))
    json.dump(out, open(a.out, "w"), indent=1)


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    g = sp.add_parser("gate", help="value-response sign gate on simulator-state edits (promotion acceptance for observation-v3 checkpoints)")
    g.add_argument("--parts", required=True)
    g.add_argument("--ckpt", required=True)
    g.add_argument("--n", type=int, default=2000)
    g.add_argument("--n-death", type=int, default=300, help="states per death-effect row")
    g.add_argument("--out", required=True)
    sens_state(ap.parse_args())


if __name__ == "__main__":
    main()
