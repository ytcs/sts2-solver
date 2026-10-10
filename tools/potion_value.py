#!/usr/bin/env python3
"""Run-model value of a held potion in HP-equivalents: after each non-boss fight of recorded baseline runs, paired rollouts of the
run state as played, with one potion added (a typical common), and with +H HP. V = dP(clear act | +potion) / dP(clear act | +H HP) x H.
The fight objective prices 1 HP at 0.5 / max HP, so the search's pot_cost for V HP is 0.5 x V / max HP."""
import argparse, json, os, sys
import numpy as np

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path[:0] = [os.path.join(ROOT, "rl"), ROOT, os.path.join(ROOT, "tools")]
import postmortem as PM  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("tags", nargs="+")
    ap.add_argument("--root", default=ROOT)
    ap.add_argument("--n", type=int, default=256)
    ap.add_argument("--hp", type=int, default=10)
    ap.add_argument("--potion", default="BLOCK_POTION")
    ap.add_argument("--every", type=int, default=2, help="every k-th non-boss fight")
    ap.add_argument("--out", default=os.path.join(ROOT, "evals", "potion_value.jsonl"))
    a = ap.parse_args()
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    pred = Predictor(PREDICTOR_CKPT, batch=1024)
    rows = []
    for tag in a.tags:
        root = a.root if os.path.exists(os.path.join(a.root, "evals", "baseline", f"{tag}.jsonl")) else ROOT
        for run in PM.load_runs(tag, root):
            idx = [i for i, f in enumerate(run.fights) if f["hp1"] and f["hp1"] > 0 and not f["enc"].endswith("_BOSS")][::a.every]
            for fi in idx:
                f = run.fights[fi]
                st = run.state(fi, f["hp1"])
                if len(st.potions) >= st.slots:
                    continue
                pot, hp = st.copy(), st.copy()
                pot.potions = list(pot.potions) + [a.potion]
                hp.hp = min(hp.max_hp, hp.hp + a.hp)
                if hp.hp == st.hp:
                    continue
                key = f"{run.seed}|pv{fi}"
                v0, vp, vh = PM.rollout_values(pred, [(st, None, key), (pot, None, key), (hp, None, key)], a.n)
                dp, dh = vp["act"] - v0["act"], vh["act"] - v0["act"]
                r = dict(tag=tag, seed=run.seed, fight=fi, enc=f["enc"], act=f["act"], hp=st.hp, max_hp=st.max_hp, potions=len(st.potions),
                         d_pot=float(dp.mean()), d_hp=float(dh.mean()), hp_added=hp.hp - st.hp, se_pot=float(dp.std(ddof=1) / np.sqrt(len(dp))),
                         se_hp=float(dh.std(ddof=1) / np.sqrt(len(dh))))
                rows.append(r)
                with open(a.out, "a", encoding="utf-8") as fo:
                    fo.write(json.dumps(r) + "\n")
                print(f"{run.seed} F{f['floor']} {f['enc']}: HP {st.hp}/{st.max_hp} dP pot {r['d_pot']:+.3f} hp+{r['hp_added']} {r['d_hp']:+.3f}", flush=True)
    dp = np.array([r["d_pot"] for r in rows])
    dh = np.array([r["d_hp"] / r["hp_added"] for r in rows])
    v = dp.mean() / max(1e-9, dh.mean())
    print(f"{len(rows)} states: dP(clear act) per potion {dp.mean():+.4f} +- {dp.std(ddof=1) / np.sqrt(len(dp)):.4f}; per HP {dh.mean():+.5f} +- "
          f"{dh.std(ddof=1) / np.sqrt(len(dh)):.5f}; potion = {v:.1f} HP (pot_cost {0.5 * v / 80:.3f} at 80 max HP)")


if __name__ == "__main__":
    main()
