"""Build a harness fight record from a frame-read spec: sim-evolved states patched with what the frames show.

usage: python build.py spec_module out.json
The spec's observations are synced into the simulator; before each sync the simulator's own prediction is diffed
against the observation, and those diffs are the reconstruction report (written next to the record as .report.txt).
"""
import importlib.util, json, os, random, sys, copy

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
sys.path.insert(0, ROOT)
import sts2  # noqa: E402

CAT = json.load(open(os.path.join(ROOT, "data", "catalog.json")))
COST = {c["id"]: c["cost"] for pool in CAT["cards"].values() for c in pool}


def card(tok, aliases):
    up = tok.endswith("+")
    base = tok.rstrip("+")
    cid = aliases.get(base, base)
    return cid, int(up)


def intents_tuple(e):
    return tuple((i["type"], i.get("damage"), i.get("hits")) for i in e.get("intents", []))


def want_intents(obs_e):
    if "intent" not in obs_e:
        return None
    out = []
    for it in obs_e.get("intent", []):
        t = (it[0], it[1] if len(it) > 1 else None, it[2] if len(it) > 2 else None)
        out.append(t)
    return tuple(out)


class Builder:
    def __init__(self, spec):
        self.spec = spec
        self.al = spec.get("aliases", {})
        self.rng = random.Random(spec.get("rng", 7))
        self.report = []
        self.log, self.states, self.times, self.observed = [], [], [], []
        sc = dict(spec["scenario"])
        self.scenario = sc
        first = spec["turns"][0]["obs"]
        want = [want_intents(e) for e in first["e"]]
        best = None
        exact_hp = len(set(spec["enemy_ids"])) < len(spec["enemy_ids"])
        for k in range(40000):
            sc2 = dict(sc, seed=f"b{self.rng.randrange(1 << 40)}")
            s = sts2.Sim(json.dumps(sc2), self.rng.randrange(1 << 62))
            snap = json.loads(s.snapshot())
            got = [intents_tuple(e) for e in snap["enemies"]]
            if exact_hp and [e["hp"] for e in snap["enemies"]] != [e["hp"] for e in first["e"]]:
                continue
            if [e["id"] for e in snap["enemies"]] == spec["enemy_ids"] and self._intents_ok(got, want):
                if "hand_enchanted" in spec:
                    t = s.copy()
                    t.sync(json.dumps(self._patch(json.loads(t.snapshot()), first, "probe")))
                    hand = json.loads(t.snapshot())["hand"]
                    if not any(c["id"] == spec["hand_enchanted"] and c.get("enchantment") for c in hand):
                        continue
                best = s
                break
        if best is None:
            self.report.append("start: no seed reproduces the opening intents; using the last sim")
            best = s
        if best.missing():
            self.report.append(f"missing content: {best.missing()}")
        self.sim = best
        st = self._patch(json.loads(self.sim.snapshot()), first, "start")
        self._sync(st, "start")
        self.states.append(st)
        self.observed.append(sorted(first.keys()))

    @staticmethod
    def _intents_ok(got, want):
        if len(got) != len(want):
            return False
        for g, w in zip(got, want):
            if w is None:
                continue
            if len(g) != len(w):
                return False
            for a, b in zip(g, w):
                if b[0] != a[0] or (b[1] is not None and a[1] != b[1]) or (b[2] is not None and a[2] != b[2]):
                    return False
        return True

    def _hand_cards(self, toks, snap_hand):
        pool = list(snap_hand)
        out = []
        for tok in toks:
            cid, up = card(tok, self.al)
            j = next((j for j, c in enumerate(pool) if c["id"] == cid and c["upgrade"] == up), None)
            if j is not None:
                out.append(pool.pop(j))
            else:
                out.append({"id": cid, "upgrade": up, "cost": COST.get(cid, 0), "keywords": []})
        return out

    def _patch(self, snap, obs, where):
        st = copy.deepcopy(snap)
        st.pop("combat_over", None)
        st.setdefault("play_pile", [])
        for i, p in enumerate(st.get("potions", [])):
            p.setdefault("slot", self.spec["scenario"]["potions"][i]["slot"] if i < len(self.spec["scenario"]["potions"]) else i)
        st["potion_slots"] = self.spec["scenario"].get("max_potion_slots", 3)
        pl = st["player"]
        for k in ("hp", "block"):
            if k in obs:
                pl[k] = obs[k]
        if "pp" in obs:
            pl["powers"] = [{"id": k, "amount": v} for k, v in obs["pp"].items()]
        if "energy" in obs:
            st["energy"] = obs["energy"]
        if "hand" in obs:
            st["hand"] = self._hand_cards(obs["hand"], snap["hand"])
        for key in ("discard", "exhaust", "draw"):
            if key in obs:
                st[key] = [dict(id=c, upgrade=u) for c, u in (card(t, self.al) for t in obs[key])]
        if "draw" not in obs:
            st.pop("draw", None)
        if "e" in obs:
            alive = [e for e in st["enemies"] if e.get("alive", True)]
            for e, o in zip(alive, obs["e"]):
                for k in ("hp", "max_hp", "block"):
                    if k in o:
                        e[k] = o[k]
                if o.get("hp") == 0:
                    e["alive"] = False
                    e["powers"], e["intents"] = [], []
                if "powers" in o:
                    e["powers"] = [{"id": k, "amount": v} for k, v in o["powers"].items()]
                if "intent" in o and not self._intents_ok([intents_tuple(e)], [want_intents(o)]):
                    self.report.append(f"{where}: intent shown {want_intents(o)} vs simulated {intents_tuple(e)} ({e.get('next_move')})")
        for r in st.get("relics", []):
            if r["id"] in obs.get("relics", {}):
                r["counter"] = obs["relics"][r["id"]]
                if r["id"] == "PEN_NIB":
                    r.setdefault("props", {})["AttacksPlayed"] = obs["relics"][r["id"]]
        if "potions" in obs:
            st["potions"] = [dict(slot=s, id=p) for s, p in obs["potions"]]
        for k in ("hand_n", "draw_n", "discard_n"):
            if k in obs:
                pile = {"hand_n": "hand", "draw_n": "draw", "discard_n": "discard"}[k]
                n = len(snap[pile]) if pile == "draw" else len(st[pile])
                if n != obs[k]:
                    self.report.append(f"{where}: {pile} count (sim, before sync) {n} vs shown {obs[k]}")
        return st

    def _sync(self, st, where):
        pre = [l for l in self.sim.diff(json.dumps(st)) if not l.startswith((".draw", ".hand", ".discard")) and "amount_on_turn_start" not in l]
        if pre:
            self.report.append(f"{where}: sim vs frames before sync: " + " | ".join(l[:160] for l in pre[:12]))
        rep = json.loads(self.sim.sync(json.dumps(st)))
        if rep.get("notes"):
            self.report.append(f"{where}: sync notes {rep['notes']}")
        snap = json.loads(self.sim.snapshot())
        for k in ("hand", "draw", "discard", "exhaust"):
            st[k] = snap[k]
        post = self.sim.diff(json.dumps(st))
        if post:
            self.report.append(f"{where}: residual after sync: " + " | ".join(l[:160] for l in post[:12]))

    def _hand_pos(self, tok):
        hand = json.loads(self.sim.snapshot())["hand"]
        if tok.endswith("*"):
            cid, up = card(tok[:-1], self.al)
            return next(j for j, c in enumerate(hand) if c["id"] == cid and c["upgrade"] == up and c.get("enchantment"))
        if "@" in tok:
            tok, pos = tok.split("@")
            cid, up = card(tok, self.al)
            assert hand[int(pos)]["id"] == cid, (tok, pos, [c["id"] for c in hand])
            return int(pos)
        cid, up = card(tok, self.al)
        for j, c in enumerate(hand):
            if c["id"] == cid and c["upgrade"] == up:
                return j
        raise ValueError(f"{tok} not in hand {[(c['id'], c['upgrade']) for c in hand]}")

    def act(self, a, next_obs, where):
        kind = a[0]
        if kind == "p":
            pos = self._hand_pos(a[1])
            j = {"play": {"hand_pos": pos}}
            if len(a) > 2 and a[2] is not None:
                j["play"]["target"] = a[2]
        elif kind == "pot":
            j = {"use_potion": {"slot": a[1]}}
            if len(a) > 2 and a[2] is not None:
                j["use_potion"]["target"] = a[2]
            else:
                j["use_potion"]["target_ally"] = 0
        elif kind == "c":
            hand = json.loads(self.sim.snapshot())["hand"]
            idx, used = [], set()
            for tok in a[1:]:
                cid, up = card(tok, self.al)
                k = next(k for k, c in enumerate(hand) if k not in used and c["id"] == cid and c["upgrade"] == up)
                used.add(k)
                idx.append(k)
            j = {"choose": idx}
        elif kind == "e":
            j = {"end_turn": True}
        else:
            raise ValueError(a)
        js = json.dumps(j)
        if kind == "e":
            self._end_turn(next_obs, where)
        else:
            sim_js = js
            if kind == "pot":
                k = next(i for i, p in enumerate(self.spec["scenario"]["potions"]) if p["slot"] == a[1])
                sj = json.loads(js)
                sj["use_potion"]["slot"] = k
                sim_js = json.dumps(sj)
            self.sim.apply(sim_js)
        self.log.append(j)
        snap = json.loads(self.sim.snapshot())
        st = self._patch(snap, next_obs or {}, where)
        if next_obs:
            self._sync(st, where)
        else:
            st["draw"] = snap.get("draw", [])
        self.states.append(st)
        self.observed.append(sorted((next_obs or {}).keys()))

    def _end_turn(self, obs, where):
        want = [want_intents(e) for e in obs.get("e", [])] if obs else None
        first = None
        for k in range(600):
            s = self.sim.copy()
            if k:
                s.determinize(self.rng.randrange(1 << 62))
            s.apply('{"end_turn":true}')
            if first is None:
                first = s
            if s.stage() == "over" or not want:
                break
            snap = json.loads(s.snapshot())
            alive = [e for e in snap["enemies"] if e.get("alive", True)]
            if self._intents_ok([intents_tuple(e) for e in alive], want):
                self.sim = s
                return
        if want:
            self.report.append(f"{where}: end turn: no determinization reproduces intents {want}")
        self.sim = first

    def run(self):
        turns = self.spec["turns"]
        for ti, t in enumerate(turns):
            acts = t["acts"]
            for ai, a in enumerate(acts):
                where = f"T{ti + 1}.{ai + 1} {a[0]} {a[1] if len(a) > 1 else ''} @{t.get('times', [None] * len(acts))[ai] if t.get('times') else ''}"
                if a[0] == "e":
                    nxt = turns[ti + 1]["obs"] if ti + 1 < len(turns) else t.get("end_obs")
                else:
                    nxt = a[-1] if isinstance(a[-1], dict) else None
                    a = a[:-1] if isinstance(a[-1], dict) else a
                try:
                    self.act(a, nxt, where)
                except Exception:
                    snap = json.loads(self.sim.snapshot())
                    print("FAILED at", where, "stage", self.sim.stage(), "hand", [(c["id"], c["upgrade"]) for c in snap.get("hand", [])], "legal", self.sim.legal())
                    print(chr(10).join(self.report))
                    raise
                self.times.append(t["times"][ai] if t.get("times") else None)
                if self.sim.stage() == "over":
                    break
        final = json.loads(self.sim.snapshot())
        return final


def main():
    spec_path, out = sys.argv[1], sys.argv[2]
    m = importlib.util.spec_from_file_location("spec", spec_path)
    mod = importlib.util.module_from_spec(m)
    m.loader.exec_module(mod)
    spec = mod.SPEC
    b = Builder(spec)
    final = b.run()
    sc = b.scenario
    rec = dict(id=spec["id"], encounter=sc["encounter"], hp_start=[sc["hp"], sc["max_hp"]],
               hp_end=[final["player"]["hp"], sc["max_hp"]], source=spec["source"], scenario=sc,
               fight=dict(scenario=sc, log=b.log, states=b.states, state=b.states[-1]),
               times=b.times, observed=b.observed, outcome=b.sim.outcome())
    json.dump(rec, open(out, "w"), indent=None)
    with open(out.replace(".json", ".report.txt"), "w") as f:
        f.write("\n".join(b.report) + "\n")
    print(f"{len(b.log)} actions, outcome {b.sim.outcome()}, stage {b.sim.stage()}, final hp {final['player']['hp']}, enemies "
          f"{[(e['id'], e['hp']) for e in final['enemies']]}")
    print("\n".join(b.report))


if __name__ == "__main__":
    main()
