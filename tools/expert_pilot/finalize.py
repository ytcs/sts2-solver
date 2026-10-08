"""Copy fight records into the repo (with run seed / build metadata in the scenario) and print the per-decision tables."""
import glob, json, math, os, sys

sys.path.insert(0, r"C:\Users\steve\sts2\sts2-solver")
import sts2  # noqa: E402
from agent.fight import Replayer  # noqa: E402

S = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DST = r"C:\Users\steve\sts2\sts2-solver\data\expert\baalorlord\fights"
FIGHTS = dict(a="a_terror_eel", b="b_gremlin_merc", c="c_skulking_colony", d="d_gardeners", e="e_lagavulin")


def records():
    os.makedirs(DST, exist_ok=True)
    for k, name in FIGHTS.items():
        rec = json.load(open(os.path.join(S, "out", f"{k}.json")))
        src = rec["source"]
        meta = dict(run_seed=src["run_seed"], game_build=src["build"], build_hash=src["build_hash"], modded=src["modded"],
                    video=src["video"], video_span=src["video_span"])
        for sc in (rec["scenario"], rec["fight"]["scenario"]):
            sc.update(meta)
        sts2.Sim(json.dumps(rec["scenario"]), 0)
        rp = Replayer(rec["fight"], seed=1)
        ok = rp.advance(rec["fight"])
        rec["replay_check"] = dict(ok=ok, outcome=rp.sim.outcome(), stats=dict(rp.stats))
        out = os.path.join(DST, f"{rec['id']}.json")
        json.dump(rec, open(out, "w"), indent=1)
        print(out, ok, rp.sim.outcome(), dict(rp.stats))


def short(t):
    import re
    if t is None:
        return ""
    t = re.sub(r" #\d+", "", t).replace("play ", "").replace(" -> e", ">e").replace("_SILENT", "")
    return t.replace("pick ", "discard ").replace("potion 0", "potion slot0").replace("potion 1", "potion slot1")


def tables():
    tc = {}
    p = os.path.join(S, "out", "turncheck.jsonl")
    if os.path.exists(p):
        for l in open(p):
            r = json.loads(l)
            tc[(r["spec"].replace(".py", ""), r["i"])] = r
    for k, name in FIGHTS.items():
        p = os.path.join(S, "out", f"eval_{name}.jsonl")
        rows = [json.loads(l) for l in open(p)]
        print(f"{chr(10)}{name}: i | t | his | live | ag | K256 his-live (se) | K256 best")
        for r in rows:
            if "live" not in r:
                print(f"{r['i']} | {r['where'].split('@')[-1]} | {r['where'].split(' ')[1]} {r['where'].split(' ')[2] if len(r['where'].split(' ')) > 3 else ''} | forced")
                continue
            g, se = r.get("gap_his_minus_live"), r.get("gap2_se")
            gs = "" if r["live_agree"] or g is None else f"{g:+.4f} ({se:.4f})"
            print(f"{r['i']} | {r['where'].split('@')[-1]} | {short(r['where'].split(' @')[0].split(' ', 1)[1])} | {short(r['live'])} | {'Y' if r['live_agree'] else 'N'} | {gs} | {short(r.get('ref_best'))}")


if __name__ == "__main__":
    if "records" in sys.argv:
        records()
    if "tables" in sys.argv:
        tables()
