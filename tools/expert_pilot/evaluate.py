"""Per-decision comparison of the expert's action with the live player and a large-budget reference.

usage: python evaluate.py spec.py out.jsonl [--ref-k 256] [--seeds 16] [--live-budget 2.0] [--only i,j]
Decision states come from the builder (simulator synced to the frames before each of his actions).
live: Engine() (models/current.json, M=5 K=32, cover) decide(); reference: Engine(K=ref_k) FastSearch rounds, one per seed,
q per legal action; his-vs-best gaps are paired over seeds.
"""
import argparse, importlib.util, json, math, os, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")))
import numpy as np  # noqa: E402
from build import Builder  # noqa: E402


def load_spec(p):
    m = importlib.util.spec_from_file_location("spec", p)
    mod = importlib.util.module_from_spec(m)
    m.loader.exec_module(mod)
    return mod.SPEC


def decision_states(spec):
    b = Builder(spec)
    out = []
    orig = b.act

    def act(a, nxt, where):
        before = b.sim.copy()
        n = len(b.log)
        orig(a, nxt, where)
        j = b.log[n]
        if a[0] == "pot":
            k = next(i for i, p in enumerate(spec["scenario"]["potions"]) if p["slot"] == a[1])
            j = json.loads(json.dumps(j))
            j["use_potion"]["slot"] = k
        out.append(dict(where=where, sim=before, action=j, kind=a[0]))

    b.act = act
    b.run()
    return b.scenario, out


def match(sim, j):
    hits = []
    for idx, t in sim.legal():
        try:
            aj = json.loads(sim.action_json(idx))
        except Exception:  # noqa: BLE001
            continue
        if "choose" in j:
            if "pick" in aj and aj["pick"] in j["choose"]:
                hits.append(idx)
        elif aj == j or (("use_potion" in j) and "use_potion" in aj and aj["use_potion"].get("slot") == j["use_potion"].get("slot")
                         and aj["use_potion"].get("target") == j["use_potion"].get("target")):
            hits.append(idx)
    return hits


def classes(sim):
    hand = json.loads(sim.snapshot()).get("hand", [])
    out = {}
    for idx, t in sim.legal():
        try:
            aj = json.loads(sim.action_json(idx))
        except Exception:  # noqa: BLE001
            aj = {}
        if "play" in aj:
            c = hand[aj["play"]["hand_pos"]]
            key = ("play", c["id"], c.get("upgrade"), json.dumps(c.get("enchantment"), sort_keys=True), aj["play"].get("target"))
        elif t.startswith("pick"):
            key = ("pick", t.split("(")[-1])
        else:
            key = (t,)
        out[idx] = key
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("spec")
    ap.add_argument("out")
    ap.add_argument("--ref-k", type=int, default=256)
    ap.add_argument("--seeds", type=int, default=16)
    ap.add_argument("--live-budget", type=float, default=2.0)
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    spec = load_spec(a.spec)
    sc, dec = decision_states(spec)
    only = {int(x) for x in a.only.split(",") if x}
    from agent.engine import Engine
    import torch
    live = Engine()
    rows = []
    for i, d in enumerate(dec):
        if only and i not in only:
            continue
        sim = d["sim"]
        legal = sim.legal()
        mine = match(sim, d["action"])
        row = dict(i=i, where=d["where"], action=d["action"], n_legal=len(legal), his_idx=mine)
        if len(legal) < 2 or not mine:
            row["note"] = "forced" if len(legal) < 2 else "his action not matched to a legal simulator action"
            rows.append(row)
            continue
        t0 = time.time()
        cls = classes(sim)
        ld = live.decide(sc, sim.copy(), a.live_budget, tol_hp=0.5, keep_potions=True)
        row.update(live=ld["text"], live_idx=ld["action"], live_agree=cls.get(ld["action"]) in {cls[x] for x in mine}, live_rounds=ld["rounds"],
                   live_q={o["text"]: o["q"] for o in ld["options"]})
        rows.append(row)
        print(i, d["where"], "his", [t for x, t in legal if x in mine], "live", ld["text"], "agree", row["live_agree"], f"{time.time() - t0:.1f}s", flush=True)
    del live
    torch.cuda.empty_cache()
    ref = Engine(K=a.ref_k)
    for row in rows:
        if "live" not in row:
            continue
        sim = dec[row["i"]]["sim"]
        text = dict(sim.legal())
        per = {}
        t0 = time.time()
        for s in range(a.seeds):
            r = ref.fs.decide(sc, sim.copy(), seed=1000 + s)
            if not r["searched"]:
                break
            for x, q, ok in zip(r["opts"], r["q"], r["legal"]):
                if ok and not np.isnan(q):
                    per.setdefault(x, {})[s] = float(q)
        if not per:
            row["ref_note"] = "not searched"
            continue
        seeds = sorted(set.intersection(*[set(v) for v in per.values()]))
        cls = classes(sim)
        groups = {}
        for x in per:
            groups.setdefault(cls.get(x, (str(x),)), []).append(x)
        q = {k: {s_: float(np.mean([per[x][s_] for x in xs])) for s_ in seeds} for k, xs in groups.items()}
        name = {k: text.get(xs[0], str(xs[0])) for k, xs in groups.items()}
        his_k = cls.get(row["his_idx"][0])
        live_k = cls.get(row["live_idx"])
        mean = {k: float(np.mean(list(v.values()))) for k, v in q.items()}

        def gap(x, y, ss):
            dif = np.array([q[x][s_] - q[y][s_] for s_ in ss])
            return float(dif.mean()), (float(dif.std(ddof=1) / math.sqrt(len(dif))) if len(dif) > 1 else float("nan"))

        even, odd = seeds[0::2], seeds[1::2]
        sel = max(q, key=lambda k: np.mean([q[k][s_] for s_ in even]))
        best = max(mean, key=mean.get)
        row.update(ref_seeds=len(seeds), ref_best=name[best], ref_best_agree=best == his_k,
                   ref_q={name[k]: round(m, 4) for k, m in sorted(mean.items(), key=lambda kv: -kv[1])})
        if his_k in q:
            g, se = gap(best, his_k, seeds)
            row.update(gap_best_minus_his=g, gap_se=se)
            g, se = gap(sel, his_k, odd)
            row.update(split_best=name[sel], split_gap=g, split_se=se)
        if his_k in q and live_k in q:
            g2, se2 = gap(his_k, live_k, seeds)
            row.update(gap_his_minus_live=g2, gap2_se=se2)
        print(row["i"], row["where"], "his", name.get(his_k), "live", row["live"], "ref best", name[best],
              f"his-live {row.get('gap_his_minus_live', float('nan')):+.4f}+-{row.get('gap2_se', float('nan')):.4f}",
              f"best-his(split) {row.get('split_gap', float('nan')):+.4f}+-{row.get('split_se', float('nan')):.4f}", f"{time.time() - t0:.1f}s", flush=True)
    with open(a.out, "w") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")


if __name__ == "__main__":
    main()
