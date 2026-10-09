"""Expert re-enactment pipeline (procedure: skill `expert-reenact`).

fetch URL                     video -> frames + transcript (git-ignored), video deleted
build RECORD [--out DIR]      fight transcriptions -> fight records (simulator synced to every observation)
validate RECORD               schema, shapes, fight records replay in the simulator
seedcheck|replay RECORD --dry-run   the action sequence the live harness commands would send
compare RECORD [--replay]     per decision: live player + references -> verdicts (divergences JSON)
report RECORD                 notes skeleton + divergence table from a compare output
compact RECORD                transcription record -> <id>.compact.jsonl (actions only; the replayable run)
check-trace RECORD TRACE      frame obs vs an oracle run-replay trace (hand, hp, enemy hp per turn)
Live seedcheck / replay are harness commands: `python -m agent seedcheck|replay RECORD`.
"""
import argparse
import copy
import ctypes
import html
import json
import math
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, ROOT)
from agent import reenact as RE  # noqa: E402

WIN, HPB = 1.0, 0.5
SCREENS = {"EVENT", "MAP", "COMBAT", "SELECT", "REWARDS", "CARD_REWARD", "CHOOSE_CARD", "CHOOSE_RELIC", "CHOOSE_BUNDLE", "SHOP", "RESTSITE", "TREASURE"}
ACTS = {"p": (2, 3), "c": (2, 9), "pot": (2, 4), "e": (1, 1)}
CAP = 4000
MAX_RSS, MIN_FREE = float(os.environ.get("EXPERT_MAX_RSS_GB", 6)) * 1e9, 8e9


class _MS(ctypes.Structure):
    _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong), ("ullTotalPhys", ctypes.c_ulonglong), ("ullAvailPhys", ctypes.c_ulonglong),
                ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong), ("ullTotalVirtual", ctypes.c_ulonglong),
                ("ullAvailVirtual", ctypes.c_ulonglong), ("ullAvailExtendedVirtual", ctypes.c_ulonglong)]


class _PMC(ctypes.Structure):
    _fields_ = [("cb", ctypes.c_ulong), ("PageFaultCount", ctypes.c_ulong)] + [(n, ctypes.c_size_t) for n in (
        "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
        "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage")]


def memory():
    if os.name != "nt":
        return 0.0, float("inf")
    ms = _MS()
    ms.dwLength = ctypes.sizeof(_MS)
    ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(ms))
    pmc = _PMC()
    pmc.cb = ctypes.sizeof(_PMC)
    k32, psapi = ctypes.windll.kernel32, ctypes.windll.psapi
    k32.GetCurrentProcess.restype = ctypes.c_void_p
    psapi.GetProcessMemoryInfo.argtypes = [ctypes.c_void_p, ctypes.POINTER(_PMC), ctypes.c_ulong]
    psapi.GetProcessMemoryInfo(k32.GetCurrentProcess(), ctypes.byref(pmc), pmc.cb)
    return float(pmc.WorkingSetSize), float(ms.ullAvailPhys)


def watchdog():
    rss, free = memory()
    if free < MIN_FREE:
        sys.exit(f"refused: {free / 1e9:.1f} GB free RAM (< {MIN_FREE / 1e9:.0f} GB)")

    def loop():
        while True:
            rss, free = memory()
            if rss > MAX_RSS or free < MIN_FREE:
                print(f"\nWATCHDOG: abort (process {rss / 1e9:.1f} GB, free {free / 1e9:.1f} GB)", file=sys.stderr, flush=True)
                os._exit(3)
            time.sleep(1.0)
    threading.Thread(target=loop, daemon=True).start()


def record_path(p):
    return p if os.path.isabs(p) else os.path.join(ROOT, p)


def slug(s):
    return re.sub(r"[^a-z0-9]+", "", str(s).lower()) or "unknown"


def paragraphs(rows, span=30):
    out, cur, start = [], [], 0.0
    for r in rows:
        if cur and r["start"] - start >= span:
            out.append((start, " ".join(cur)))
            cur = []
        if not cur:
            start = r["start"]
        cur.append(html.unescape(r["text"]).replace("\n", " "))
    if cur:
        out.append((start, " ".join(cur)))
    return "".join(f"[{int(s) // 3600}:{int(s) % 3600 // 60:02d}:{int(s) % 60:02d}] {t}\n" for s, t in out)


def json3_rows(path):
    d = json.load(open(path, encoding="utf-8"))
    rows = []
    for e in d.get("events", []):
        text = "".join(s.get("utf8", "") for s in e.get("segs") or []).strip()
        if text:
            rows.append(dict(start=e.get("tStartMs", 0) / 1000.0, text=text))
    return rows


TRANSCRIPT_API = ("import json,sys\nfrom youtube_transcript_api import YouTubeTranscriptApi as Y\nv=sys.argv[1]\n"
                  "try:\n r=Y().fetch(v).to_raw_data()\nexcept AttributeError:\n r=Y.get_transcript(v)\nprint(json.dumps([dict(start=x['start'],text=x['text']) for x in r]))")


def transcript_api(video_id):
    try:
        import youtube_transcript_api  # noqa: F401
        exe = [sys.executable]
    except ImportError:
        exe = ["py", "-3.13"] if shutil.which("py") else None
    if exe is None:
        return None, "youtube-transcript-api missing: install it into a scratch venv and rerun `fetch --transcript-only` with that interpreter"
    p = subprocess.run(exe + ["-c", TRANSCRIPT_API, video_id], capture_output=True, text=True, encoding="utf-8")
    if p.returncode:
        return None, f"youtube-transcript-api failed ({' '.join(exe)}): {p.stderr.strip().splitlines()[-1] if p.stderr.strip() else p.returncode}"
    return json.loads(p.stdout), None


def gitignore(creator):
    path = os.path.join(ROOT, ".gitignore")
    lines = open(path, encoding="utf-8").read().splitlines()
    want = [f"data/expert/{creator}/transcripts/", f"data/expert/{creator}/frames/"]
    new = [w for w in want if w not in lines]
    if new:
        with open(path, "a", encoding="utf-8") as f:
            f.write("".join(w + "\n" for w in new))
    return new


def fetch(a):
    url = a.url
    if a.dry_run:
        print(f"would: yt-dlp metadata for {url}; subtitles (json3, en, auto) -> transcripts/<date>_<id>.txt (30 s paragraphs), on HTTP 429 or no subs "
              f"youtube-transcript-api; best video stream (no resolution limit) -> ffmpeg {a.fps} fps -> frames/<id>/<mmss>.jpg; video deleted; "
              ".gitignore lines for the creator's frames/ and transcripts/")
        return
    import yt_dlp
    with yt_dlp.YoutubeDL(dict(quiet=True, skip_download=True)) as y:
        info = y.extract_info(url, download=False)
    vid, creator, date = info["id"], a.creator or slug(info.get("channel") or info.get("uploader")), info.get("upload_date") or time.strftime("%Y%m%d")
    d = os.path.join(ROOT, "data", "expert", creator)
    os.makedirs(os.path.join(d, "transcripts"), exist_ok=True)
    print(f"{vid} | {info.get('title')} | {creator} | {date} | {info.get('duration')} s; ignored: {gitignore(creator) or 'already'}")
    tmp = tempfile.mkdtemp(prefix="sts2x_")
    try:
        tpath = os.path.join(d, "transcripts", f"{date}_{vid}.txt")
        rows, err = None, None
        try:
            with yt_dlp.YoutubeDL(dict(quiet=True, skip_download=True, writesubtitles=True, writeautomaticsub=True, subtitleslangs=["en", "en-orig"],
                                       subtitlesformat="json3", outtmpl=os.path.join(tmp, "subs"))) as y:
                y.download([url])
            subs = [f for f in os.listdir(tmp) if f.endswith(".json3")]
            rows = json3_rows(os.path.join(tmp, subs[0])) if subs else None
        except Exception as e:  # noqa: BLE001
            err = f"yt-dlp subtitles: {e}"
        if not rows:
            rows, err2 = transcript_api(vid)
            err = err2 if rows is None else None
        if rows:
            open(tpath, "w", encoding="utf-8").write(paragraphs(rows))
            print(f"transcript: {tpath} ({len(rows)} captions)")
        else:
            print(f"transcript: none ({err})")
        if a.transcript_only:
            return
        with yt_dlp.YoutubeDL(dict(quiet=True, format="bv*", outtmpl=os.path.join(tmp, "video.%(ext)s"))) as y:
            y.download([url])
        video = next(os.path.join(tmp, f) for f in os.listdir(tmp) if f.startswith("video."))
        fdir = os.path.join(d, "frames", vid)
        os.makedirs(fdir, exist_ok=True)
        subprocess.run(["ffmpeg", "-loglevel", "error", "-i", video, "-vf", f"fps={a.fps}", "-q:v", "2", os.path.join(tmp, "f%06d.jpg")], check=True)
        n = 0
        for f in sorted(x for x in os.listdir(tmp) if x.startswith("f") and x.endswith(".jpg")):
            s = int((int(f[1:7]) - 1) / a.fps)
            dst = os.path.join(fdir, f"{s // 60:02d}{s % 60:02d}" + ("" if a.fps <= 1 else f"_{int(f[1:7]):06d}") + ".jpg")
            shutil.move(os.path.join(tmp, f), dst)
            n += 1
        print(f"frames: {n} in {fdir}; video deleted")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def _catalog_cost():
    cat = json.load(open(os.path.join(ROOT, "data", "catalog.json"), encoding="utf-8"))
    return {c["id"]: c["cost"] for pool in cat["cards"].values() for c in pool}


def _card(tok, al):
    cid, up, _e, _p = RE.token(tok, al)
    return cid, up


def _intents(e):
    return tuple((i["type"], i.get("damage"), i.get("hits")) for i in e.get("intents", []))


def _want(o):
    if "intent" not in o:
        return None
    return tuple((it[0], it[1] if len(it) > 1 else None, it[2] if len(it) > 2 else None) for it in o["intent"])


def _intents_ok(got, want):
    if len(got) != len(want):
        return False
    for g, w in zip(got, want):
        if w is None:
            continue
        if len(g) != len(w) or any(b[0] != a_[0] or (b[1] is not None and a_[1] != b[1]) or (b[2] is not None and a_[2] != b[2]) for a_, b in zip(g, w)):
            return False
    return True


def _enemies_ok(sim, obs_e):
    es = [e for e in json.loads(sim.snapshot())["enemies"] if e.get("alive", True)]
    return len(es) == len(obs_e) and all(all(e[k] == o[k] for k in ("hp", "block") if k in o) for e, o in zip(es, obs_e))


class Builder:
    def __init__(self, spec, aliases, seed_tries=40000):
        import sts2
        self.sts2 = sts2
        self.spec, self.al = spec, aliases
        self.cost = _catalog_cost()
        self.rng = random.Random(spec.get("rng", 7))
        self.report, self.log, self.states, self.times, self.observed = [], [], [], [], []
        self.scenario = dict(spec["scenario"])
        first = spec["turns"][0]["obs"]
        want = [_want(e) for e in first["e"]]
        exact_hp = len(set(spec["enemy_ids"])) < len(spec["enemy_ids"])
        best = s = None
        for _ in range(seed_tries):
            s = sts2.Sim(json.dumps(dict(self.scenario, seed=f"b{self.rng.randrange(1 << 40)}")), self.rng.randrange(1 << 62))
            snap = json.loads(s.snapshot())
            if exact_hp and [e["hp"] for e in snap["enemies"]] != [e["hp"] for e in first["e"]]:
                continue
            if [e["id"] for e in snap["enemies"]] != spec["enemy_ids"] or not _intents_ok([_intents(e) for e in snap["enemies"]], want):
                continue
            if spec.get("hand_enchanted"):
                t = s.copy()
                t.sync(json.dumps(self._patch(json.loads(t.snapshot()), first, "probe")))
                if not any(c["id"] == spec["hand_enchanted"] and c.get("enchantment") for c in json.loads(t.snapshot())["hand"]):
                    continue
            best = s
            break
        if best is None:
            self.report.append("start: no seed reproduces the opening intents; using the last simulator")
            best = s
        if best.missing():
            self.report.append(f"missing content: {best.missing()}")
        self.sim = best
        st = self._patch(json.loads(self.sim.snapshot()), first, "start")
        self._sync(st, "start")
        self.states.append(st)
        self.observed.append(sorted(first))

    def _hand_cards(self, toks, snap_hand):
        pool, out = list(snap_hand), []
        deck = self.scenario.get("deck", [])
        for tok in toks:
            cid, up, ench, _p = RE.token(tok, self.al)
            same = [c for c in deck if c["id"] == cid and c.get("upgrade", 0) == up]
            ench = ench or bool(same) and all(c.get("enchantment") for c in same)
            j = next((j for j, c in enumerate(pool) if c["id"] == cid and c["upgrade"] == up and bool(c.get("enchantment")) == ench), None)
            if j is not None:
                out.append(pool.pop(j))
                continue
            card = {"id": cid, "upgrade": up, "cost": self.cost.get(cid, 0), "keywords": []}
            src = next((c for c in same if bool(c.get("enchantment")) == ench), None)
            if src and src.get("enchantment"):
                card["enchantment"] = dict(src["enchantment"])
            out.append(card)
        return out

    def _patch(self, snap, obs, where):
        st = copy.deepcopy(snap)
        st.pop("combat_over", None)
        st.setdefault("play_pile", [])
        pots = self.spec["scenario"].get("potions", [])
        for i, p in enumerate(st.get("potions", [])):
            p.setdefault("slot", pots[i]["slot"] if i < len(pots) else i)
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
                st[key] = [dict(id=c, upgrade=u) for c, u in (_card(t, self.al) for t in obs[key])]
        if "draw" not in obs:
            st.pop("draw", None)
        if "e" in obs:
            for e, o in zip([e for e in st["enemies"] if e.get("alive", True)], obs["e"]):
                for k in ("hp", "max_hp", "block"):
                    if k in o:
                        e[k] = o[k]
                if o.get("hp") == 0:
                    e["alive"], e["powers"], e["intents"] = False, [], []
                if "powers" in o:
                    e["powers"] = [{"id": k, "amount": v} for k, v in o["powers"].items()]
                if "intent" in o and not _intents_ok([_intents(e)], [_want(o)]):
                    self.report.append(f"{where}: intent shown {_want(o)} vs simulated {_intents(e)} ({e.get('next_move')})")
        for r in st.get("relics", []):
            if r["id"] in obs.get("relics", {}):
                r["counter"] = obs["relics"][r["id"]]
                if r["id"] == "PEN_NIB":
                    r.setdefault("props", {})["AttacksPlayed"] = obs["relics"][r["id"]]
        if "potions" in obs:
            st["potions"] = [dict(slot=s, id=p) for s, p in obs["potions"]]
        for k, pile in (("hand_n", "hand"), ("draw_n", "draw"), ("discard_n", "discard")):
            if k in obs:
                n = len(snap[pile]) if pile == "draw" else len(st[pile])
                if n != obs[k]:
                    self.report.append(f"{where}: {pile} count (sim, before sync) {n} vs shown {obs[k]}")
        return st

    def _sync(self, st, where):
        pre = [ln for ln in self.sim.diff(json.dumps(st)) if not ln.startswith((".draw", ".hand", ".discard")) and "amount_on_turn_start" not in ln]
        if pre:
            self.report.append(f"{where}: sim vs frames before sync: " + " | ".join(ln[:160] for ln in pre[:12]))
        rep = json.loads(self.sim.sync(json.dumps(st)))
        if rep.get("notes"):
            self.report.append(f"{where}: sync notes {rep['notes']}")
        snap = json.loads(self.sim.snapshot())
        for k in ("hand", "draw", "discard", "exhaust"):
            st[k] = snap.get(k, [])
        post = self.sim.diff(json.dumps(st))
        if post:
            self.report.append(f"{where}: residual after sync: " + " | ".join(ln[:160] for ln in post[:12]))

    def _hand_pos(self, tok):
        hand = json.loads(self.sim.snapshot())["hand"]
        cid, up, ench, pos = RE.token(tok, self.al)
        if ench:
            return next(j for j, c in enumerate(hand) if c["id"] == cid and c["upgrade"] == up and c.get("enchantment"))
        if pos is not None:
            assert hand[pos]["id"] == cid, (tok, [c["id"] for c in hand])
            return pos
        for j, c in enumerate(hand):
            if c["id"] == cid and c["upgrade"] == up:
                return j
        raise ValueError(f"{tok} not in hand {[(c['id'], c['upgrade']) for c in hand]}")

    def act(self, a, next_obs, where):
        kind = a[0]
        if kind == "p":
            j = {"play": {"hand_pos": self._hand_pos(a[1])}}
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
            offered = {int(m.group(1)): m.group(2) for _i, t in self.sim.legal() for m in [re.match(r"pick (\d+) \((\w+)\)", t)] if m}
            idx = []
            for tok in a[1:]:
                cid, up = _card(tok, self.al)
                k = next((k for k, c in enumerate(hand) if k not in idx and c["id"] == cid and c["upgrade"] == up), None)
                if k is None:
                    k = next(k for k, t in sorted(offered.items()) if k not in idx and RE.base(t) == RE.base(cid))
                idx.append(k)
            j = {"choose": idx}
        elif kind == "e":
            j = {"end_turn": True}
        else:
            raise ValueError(a)
        if kind == "e":
            self._end_turn(next_obs, where)
        else:
            sj = copy.deepcopy(j)
            if kind == "pot":
                sj["use_potion"]["slot"] = next(i for i, p in enumerate(self.spec["scenario"]["potions"]) if p["slot"] == a[1])
            base = self.sim.copy()
            self.sim.apply(json.dumps(sj))
            want = getattr(self, "next_choice", None)
            if kind == "pot" and want and self.sim.stage() == "choice":
                for k in range(600):
                    if any(t.startswith("pick") and RE.base(t.split("(")[-1].rstrip(")")) == RE.base(want) for _i, t in self.sim.legal()):
                        break
                    s = base.copy()
                    s.determinize(self.rng.randrange(1 << 62))
                    s.apply(json.dumps(sj))
                    self.sim = s
                else:
                    self.report.append(f"{where}: no determinization offers {want} after the potion")
            elif kind in ("p", "pot") and next_obs and "e" in next_obs and not _enemies_ok(self.sim, next_obs["e"]):
                for k in range(300):
                    s = base.copy()
                    s.determinize(self.rng.randrange(1 << 62))
                    s.apply(json.dumps(sj))
                    if _enemies_ok(s, next_obs["e"]):
                        self.sim = s
                        break
                else:
                    self.report.append(f"{where}: no determinization of the play's random effects reproduces the enemies shown")
        self.log.append(j)
        snap = json.loads(self.sim.snapshot())
        st = self._patch(snap, next_obs or {}, where)
        if next_obs:
            self._sync(st, where)
        else:
            st["draw"] = snap.get("draw", [])
        self.states.append(st)
        self.observed.append(sorted(next_obs or {}))

    def _plays_ok(self, s, obs, acts):
        saved = (self.sim, self.log, self.states, self.observed, self.times, len(self.report), getattr(self, "next_choice", None))
        self.sim, self.log, self.states, self.observed, self.next_choice = s.copy(), [], [], [], None
        try:
            st = self._patch(json.loads(self.sim.snapshot()), obs, "probe")
            self.sim.sync(json.dumps(st))
            for a in acts:
                if a[0] == "e":
                    break
                self.act([x for x in a if not isinstance(x, dict)], None, "probe")
            return True
        except Exception:  # noqa: BLE001
            return False
        finally:
            self.sim, self.log, self.states, self.observed, self.times, n, self.next_choice = saved
            del self.report[n:]

    def _end_turn(self, obs, where):
        want = [_want(e) for e in obs.get("e", [])] if obs else None
        acts = getattr(self, "next_acts", None)
        first = fallback = None
        for k in range(600):
            s = self.sim.copy()
            if k:
                s.determinize(self.rng.randrange(1 << 62))
            s.apply('{"end_turn":true}')
            first = first or s
            if s.stage() == "over" or not want:
                break
            if _intents_ok([_intents(e) for e in json.loads(s.snapshot())["enemies"] if e.get("alive", True)], want):
                if not acts or "hand" not in obs or self._plays_ok(s, obs, acts):
                    self.sim = s
                    return
                fallback = fallback or s
        if fallback is not None:
            self.report.append(f"{where}: end turn: no determinization lets the next turn's plays apply (hidden per-draw state, e.g. Bound)")
            self.sim = fallback
            return
        if want:
            self.report.append(f"{where}: end turn: no determinization reproduces intents {want}")
        self.sim = first

    def run(self):
        turns = self.spec["turns"]
        for ti, t in enumerate(turns):
            for ai, a in enumerate(t["acts"]):
                when = (t.get("times") or [None] * len(t["acts"]))[ai]
                where = f"T{ti + 1}.{ai + 1} {a[0]} {a[1] if len(a) > 1 and not isinstance(a[1], dict) else ''} @{when or ''}"
                if a[0] == "e":
                    nxt = turns[ti + 1]["obs"] if ti + 1 < len(turns) else t.get("end_obs")
                    self.next_acts = turns[ti + 1]["acts"] if ti + 1 < len(turns) else None
                else:
                    nxt = a[-1] if isinstance(a[-1], dict) else None
                    a = a[:-1] if isinstance(a[-1], dict) else a
                after = t["acts"][ai + 1] if ai + 1 < len(t["acts"]) else None
                self.next_choice = _card(after[1], self.al)[0] if after and after[0] == "c" else None
                self.act(a, nxt, where)
                self.times.append(when)
                if self.sim.stage() == "over":
                    break
        return json.loads(self.sim.snapshot())


def meta(rec, spec, floor):
    v = rec["video"]
    return dict(video=v["url"], channel=v.get("channel"), uploaded=v.get("uploaded"), title=v.get("title"), run_seed=rec["seed"], build=rec["build"],
                build_hash=rec.get("build_hash"), modded=rec.get("modded"), character=rec["character"], ascension=rec["ascension"], floor=floor,
                video_span=spec.get("video_span"))


def build_fight(rec, spec, floor):
    from agent.fight import Replayer
    b = Builder(spec, rec.get("aliases", {}))
    final = b.run()
    src = meta(rec, spec, floor)
    sc = dict(b.scenario, run_seed=src["run_seed"], game_build=src["build"], build_hash=src["build_hash"], modded=src["modded"], video=src["video"],
              video_span=src["video_span"])
    out = dict(id=spec["id"], encounter=sc["encounter"], hp_start=[sc["hp"], sc["max_hp"]], hp_end=[final["player"]["hp"], sc["max_hp"]], source=src,
               scenario=sc, fight=dict(scenario=sc, log=b.log, states=b.states, state=b.states[-1]), times=b.times, observed=b.observed,
               outcome=b.sim.outcome(), report=b.report)
    rp = Replayer(out["fight"], seed=1)
    ok = rp.advance(out["fight"])
    out["replay_check"] = dict(ok=ok, outcome=rp.sim.outcome(), stats=dict(rp.stats))
    return out


def fights(rec):
    return [(st["floor"], st["fight"]) for st in rec["steps"] if st.get("fight")]


def build_from_replay(rec, only=None):
    """compact record: fight records come from the game (replay-dir records, else the replay log's states); the simulator cannot redraw his hands"""
    d = os.path.join(RE.creator_dir(rec), "replay", rec["video"]["id"])
    log = d + ".jsonl"
    if not os.path.exists(log):
        sys.exit(f"no replay log {os.path.relpath(log, ROOT)}: replay the record in the game first (`python -m agent seedcheck|replay`)")
    rows = [json.loads(x) for x in open(log, encoding="utf-8")]
    openings = [r for r in rows if r["event"] == "opening"]
    by_fight = {}
    for x, row in align(RE.flatten(rec, built=None), log):
        if x.get("fight"):
            by_fight.setdefault(x["fight"], []).append((x, row))
    os.makedirs(d, exist_ok=True)
    have = {f: RE.load(os.path.join(d, f)) for f in os.listdir(d) if f.endswith(".json")}
    for floor, spec in fights(rec):
        if only and only not in spec["id"]:
            continue
        acts, p_ = by_fight[spec["id"]], os.path.join(d, spec["id"] + ".json")
        src = have.get(spec["id"] + ".json") or next((r for r in have.values() if r.get("encounter") == spec["encounter"]
                                                       and (r.get("scenario") or {}).get("total_floor") == floor and len(r["fight"]["log"]) == len(acts)), None)
        how = "replay-dir record"
        if src is None:
            missing = [x["i"] for x, row in acts if row is None]
            op = next((r for r in openings if r.get("encounter") == spec["encounter"]), None)
            if missing or op is None:
                print(f"{spec['id']}: cannot build: {'actions ' + str(missing) + ' are not in the replay log' if missing else 'no opening row'}")
                continue
            sc = dict(op["scenario"], run_seed=rec["seed"], game_build=rec["build"], video=rec["video"]["url"])
            states = [row["state"] for _x, row in acts]
            src = dict(id=spec["id"], encounter=spec["encounter"], hp_start=[sc.get("hp"), sc.get("max_hp")], scenario=sc,
                       fight=dict(scenario=sc, log=[RE.to_bridge(x, row["cmd"]) for x, row in acts], states=states, state=states[-1]), source="replay log")
            how = "replay log"
        src = dict(src, id=spec["id"])
        json.dump(src, open(p_, "w", encoding="utf-8"))
        print(f"{spec['id']}: {len(src['fight']['log'])} actions from the {how} -> {os.path.relpath(p_, ROOT)}")


def build(a):
    rec = RE.load(record_path(a.record))
    if a.record.endswith(".jsonl"):
        return build_from_replay(rec, a.fight)
    d = a.out or os.path.join(RE.creator_dir(rec), "fights")
    os.makedirs(d, exist_ok=True)
    for floor, spec in fights(rec):
        if a.fight and a.fight not in spec["id"]:
            continue
        t0 = time.time()
        out = build_fight(rec, spec, floor)
        p = os.path.join(d, spec["id"] + ".json")
        json.dump(out, open(p, "w", encoding="utf-8"), indent=1)
        print(f"{spec['id']}: {len(out['fight']['log'])} actions, outcome {out['outcome']}, replay {out['replay_check']['ok']} {out['replay_check']['stats']}, "
              f"{time.time() - t0:.0f}s -> {os.path.relpath(p, ROOT)}")
        for line in out["report"]:
            print("   ", line)


def check_record(rec):
    errs, warns = [], []
    for k in ("seed", "build", "character", "ascension", "video", "steps"):
        if k not in rec:
            errs.append(f"missing `{k}`")
    if errs:
        return errs, warns
    if not re.fullmatch(r"[0-9A-HJ-NP-Z]{10,12}", str(rec["seed"])):
        errs.append(f"seed {rec['seed']!r} is not in the game's alphabet (0-9, A-Z without I and O)")
    if not RE.build_ok(rec):
        warns.append(f"build {rec['build']!r} is not the pinned {RE.PINNED_BUILD}: no re-enactment, frame-based notes only")
    for k in ("id", "url", "creator"):
        if k not in rec["video"]:
            errs.append(f"video.{k} missing")
    al = rec.get("aliases", {})
    last_floor = 0
    for i, st in enumerate(rec["steps"]):
        where = f"step {i} (floor {st.get('floor')})"
        if not isinstance(st.get("floor"), int):
            errs.append(f"{where}: `floor` must be an int")
        elif st["floor"] < last_floor and st.get("act", 0) == 0:
            errs.append(f"{where}: floor goes backwards")
        else:
            last_floor = st["floor"]
        if st.get("screen") not in SCREENS:
            errs.append(f"{where}: screen {st.get('screen')!r} not in {sorted(SCREENS)}")
        if "gap" in st:
            continue
        if st.get("fight"):
            errs += [f"{where}: {e}" for e in check_spec(st["fight"], al)]
        elif st.get("screen") == "COMBAT":
            errs.append(f"{where}: a COMBAT step needs `fight` (the transcription) or `gap`")
        elif "pick" not in st:
            errs.append(f"{where}: `pick` missing")
        elif st["screen"] == "MAP" and not (isinstance(st["pick"], dict) or re.fullmatch(r"r\d+c\d+", str(st["pick"]))):
            errs.append(f"{where}: map pick must be {{room?, col?, row?}} or 'r<row>c<col>'")
        elif st["screen"] == "SELECT" and not isinstance(st["pick"], list):
            errs.append(f"{where}: SELECT pick must be a list of card names")
        elif isinstance(st["pick"], dict) and "discard_potion" in st["pick"]:
            if not isinstance(st["pick"]["discard_potion"], int):
                errs.append(f"{where}: discard_potion must be a slot index")
        elif isinstance(st["pick"], dict) and "potion" in st["pick"]:
            if not isinstance(st["pick"]["potion"], int) or not st["pick"].get("id"):
                errs.append(f"{where}: an out-of-combat potion pick is {{potion: slot, id}}")
        elif st["screen"] == "REWARDS" and not isinstance(st["pick"], (list, str)):
            errs.append(f"{where}: REWARDS pick must be a label or a list of labels")
    gaps = [i for i, st in enumerate(rec["steps"]) if "gap" in st]
    if gaps:
        warns.append(f"{len(gaps)} gap(s); the replay stops at step {gaps[0]} (floor {rec['steps'][gaps[0]].get('floor')}): {rec['steps'][gaps[0]]['gap']}")
    return errs, warns


def check_spec(spec, al):
    errs = []
    for k in ("id", "encounter", "turns") + (("enemy_ids", "scenario") if any("obs" in t for t in spec.get("turns", [])) else ()):
        if k not in spec:
            errs.append(f"fight transcription missing `{k}`")
    if errs:
        return errs
    import sts2
    names = sts2.names()
    known = set(names.get("card", [])) | {"SHIV"}
    for ti, t in enumerate(spec["turns"]):
        if "acts" not in t or ("scenario" in spec and "obs" not in t):
            errs.append(f"{spec['id']} T{ti + 1}: needs `obs` and `acts`")
            continue
        times = t.get("times")
        if times and len(times) != len(t["acts"]):
            errs.append(f"{spec['id']} T{ti + 1}: {len(times)} times for {len(t['acts'])} acts")
        toks = list(t.get("obs", {}).get("hand", []))
        for ai, a in enumerate(t["acts"]):
            body = [x for x in a if not isinstance(x, dict)]
            lo, hi = ACTS.get(body[0] if body else None, (0, -1))
            if not lo <= len(body) <= hi:
                errs.append(f"{spec['id']} T{ti + 1}.{ai + 1}: bad act {a}")
                continue
            if body[0] in ("p", "c"):
                toks += body[1:] if body[0] == "c" else [body[1]]
            if body[0] == "e" and ai != len(t["acts"]) - 1:
                errs.append(f"{spec['id']} T{ti + 1}.{ai + 1}: end turn before the turn's last act")
        for tok in toks:
            cid = RE.token(tok, al)[0]
            if known and cid not in known:
                errs.append(f"{spec['id']} T{ti + 1}: card token {tok!r} -> {cid} is not a simulator card id")
    return errs


def validate(a):
    from agent.fight import Replayer
    rec = RE.load(record_path(a.record))
    errs, warns = check_record(rec)
    flat = RE.flatten(rec)
    for w in warns:
        print("warn:", w)
    for e in errs:
        print("ERROR:", e)
    first_gap = next((i for i, x in enumerate(flat) if x["kind"] == "gap"), len(flat))
    print(f"{len(rec['steps'])} steps -> {len(flat)} actions; replayable prefix: {first_gap} actions")
    bad = len(errs)
    for floor, spec in fights(rec):
        built = RE.built_record(rec, spec["id"])
        n_acts = sum(len(t["acts"]) for t in spec["turns"])
        if built is None:
            print(f"{spec['id']}: not built (`build`)")
            bad += 1
            continue
        f = built["fight"]
        rp = Replayer(f, seed=1)
        ok = rp.advance(f)
        st = dict(rp.stats)
        worse = {k: v for k, v in st.items() if k.startswith(("residual", "intent unmatched", "end_turn_unmatched", "action failed", "start unmatched"))}
        stale = len(f["log"]) != n_acts
        print(f"{spec['id']}: {len(f['log'])} actions{' (STALE: transcription has ' + str(n_acts) + ')' if stale else ''}, simulator replay "
              f"{'ok' if ok else 'FAILED'}, outcome {rp.sim.outcome()}, end turns matched {st.get('end_turn_matched', 0)}/"
              f"{st.get('end_turn_matched', 0) + st.get('end_turn_unmatched', 0)}" + (f", residuals {worse}" if worse else ""))
        bad += (not ok) + stale
    sys.exit(1 if bad else 0)


def dry_run(a, seedcheck):
    rec = RE.load(record_path(a.record))
    if not RE.build_ok(rec):
        sys.exit(f"REFUSED: build {rec.get('build')!r} is not the pinned {RE.PINNED_BUILD}")
    print(f"menu: a <new run> {rec['character'].lower()} {rec['ascension']} {rec['seed']}   (standard run; `--custom` uses `custom run`)")
    print(f"map check: boss {rec.get('boss')}; rooms by floor {RE.floor_rooms(rec)}")
    built = {}
    for i, x in enumerate(RE.flatten(rec)):
        cmd = ""
        if x["kind"] in ("play", "choose", "potion", "end"):
            b = built.setdefault(x["fight"], RE.built_record(rec, x["fight"]))
            if b is None:
                cmd = "(fight not built: `build` first)"
            else:
                st = b["fight"]["states"][x["i"]]
                c, err = RE.combat_command(x, st, recorded_screen(x, st, b["scenario"]))
                cmd = f"{c:10s} do {json.dumps(RE.to_bridge(x, c))}" if c else f"?? {err}"
        elif x["kind"] == "macro":
            p = x.get("pick")
            cmd = ("a <map node consistent with the record's rooms>" if x.get("screen") == "MAP" and not p else
                   f"a <option `{p}`>" if isinstance(p, str) else f"a <{json.dumps(p)}>")
        print(f"{i:4d} F{x.get('floor', '?'):<3} {str(x.get('t') or ''):>6} {RE.describe(x):46s} {cmd}")
        if seedcheck and (x.get("fight") or x["kind"] == "gap") and (x.get("i", 0) == 0):
            print("     (seedcheck compares this fight's opening with the record, then stops)")
            break
        if x["kind"] == "gap" and not a.all:
            print("     (the replay stops here: record gap; --all lists the rest)")
            break


def recorded_screen(x, st, scenario):
    def title(c):
        return RE.base(c["id"]).replace("_", " ").title() + ("+" if c.get("upgrade") else "")
    if x["kind"] == "choose":
        return "\n".join([f"SELECT {len(x['cards'])}"] + [f"{i} {title(c)}(1) ." for i, c in enumerate(st.get("hand", []))]) + "\n"
    es = st.get("enemies", [])
    lines = ["COMBAT"] + [f"e{i} {e['id']} {e['hp']}/{e['max_hp']} b0 -> ?" for i, e in enumerate(es) if e.get("alive", True)]
    tgt = " ->e" if x.get("target") is not None else ""
    opts = [f"{title(c)}(1) ." + (tgt if x["kind"] == "play" and RE.base(c["id"]) == RE.base(x["card"]) else "") for c in st.get("hand", [])]
    opts += [f"potion {p['id'].replace('_', ' ').title()}: ." + (tgt if x["kind"] == "potion" and p["id"] == x.get("potion") else "") for p in scenario.get("potions", [])]
    return "\n".join(lines + [f"{i} {o}" for i, o in enumerate(opts + ["end turn"])]) + "\n"


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
            out[idx] = ("play", c["id"], c.get("upgrade"), json.dumps(c.get("enchantment"), sort_keys=True), aj["play"].get("target"))
        elif t.startswith("pick"):
            out[idx] = ("pick", t.split("(")[-1])
        else:
            out[idx] = (t,)
    return out


def match(sim, j):
    hits = []
    for idx, _t in sim.legal():
        try:
            aj = json.loads(sim.action_json(idx))
        except Exception:  # noqa: BLE001
            continue
        if "choose" in j:
            if "pick" in aj and aj["pick"] in j["choose"]:
                hits.append(idx)
        elif aj == j or ("use_potion" in j and "use_potion" in aj and aj["use_potion"].get("slot") == j["use_potion"].get("slot")
                         and aj["use_potion"].get("target") == j["use_potion"].get("target")):
            hits.append(idx)
    return hits


def terminal(sim, max_hp):
    o = sim.outcome()
    if o == 1:
        return WIN + HPB * min(json.loads(sim.snapshot())["player"]["hp"] / max_hp, 1.0)
    return -1.0 if o != 0 else None


def utility(sim, max_hp):
    t = terminal(sim, max_hp)
    return -1.0 if t is None else t


def enumerate_turn(sim, potions=False, cap=CAP, roots=None):
    n, seen, stack = 0, set(), [(sim, [])]
    enumerate_turn.capped = False
    while stack:
        if n >= cap:
            enumerate_turn.capped = True
            return
        s, path = stack.pop()
        if s.stage() == "over":
            n += 1
            yield s, path, True
            continue
        picks = []
        if s.stage() == "choice":
            for t in reversed(path):
                if not t.startswith("pick"):
                    break
                picks.append(t)
        key = hash((path[0] if path else "", s.snapshot(), tuple(sorted(picks))))
        if key in seen or len(path) > 30:
            continue
        seen.add(key)
        if n % 50 == 0 and memory()[1] < MIN_FREE:
            raise MemoryError("free RAM below the floor during turn enumeration")
        for idx, t in s.legal():
            if not path and roots is not None and idx not in roots:
                continue
            if t == "end turn":
                n += 1
                yield s, path, False
                continue
            if t.startswith("discard potion") or (t.startswith("potion") and not potions) or (t.startswith("pick") and t in picks):
                continue
            c = s.copy()
            try:
                c.step(idx)
            except Exception:  # noqa: BLE001
                continue
            stack.append((c, path + [t]))


def leaf_value(fs, sc, sim, dets, seed0, max_hp):
    vals, exact = [], True
    for d in range(dets):
        s = sim.copy()
        s.determinize(seed0 + 7919 * d)
        s.apply('{"end_turn":true}')
        t = terminal(s, max_hp)
        if t is not None:
            vals.append(t)
            continue
        exact = False
        while s.stage() == "choice":
            s.step(s.legal()[0][0])
        r = fs.decide(sc, s, seed=seed0 + d)
        qs = [q for q, ok in zip(r["q"], r["legal"]) if ok and q == q]
        vals.append(max(qs) if qs else float("nan"))
    return vals, exact


def paired(x, y):
    import numpy as np
    d = np.asarray(x, float) - np.asarray(y, float)
    return float(d.mean()), (float(d.std(ddof=1) / math.sqrt(len(d))) if len(d) > 1 else 0.0)


def turn_check(eng, sc, sim, his_k, live_k, dets=2, revalue=8, potions=False, max_leaves=1500):
    max_hp = sc["max_hp"]
    cls = classes(sim)
    first_of = {t: cls[x] for x, t in sim.legal()}
    best, cache, info = {}, {}, {}
    n, leaves_capped = 0, False
    roots = {x for x, k in cls.items() if k in (his_k, live_k)}
    for s_, path, over in enumerate_turn(sim.copy(), potions, roots=roots):
        n += 1
        k0 = ("end turn",) if not path else first_of[path[0]]
        if over:
            vals, exact = [terminal(s_, max_hp)] * dets, True
        else:
            key = hash(s_.snapshot())
            if key not in cache:
                if len(cache) >= max_leaves:
                    leaves_capped = True
                    break
                cache[key] = leaf_value(eng.fs, sc, s_, dets, 4242, max_hp)
            vals, exact = cache[key]
        v = sum(vals) / len(vals)
        c = info.setdefault(k0, dict(all_exact=True, exact_max=None, max=-9.0))
        c["all_exact"] &= exact
        c["max"] = max(c["max"], v)
        if exact:
            c["exact_max"] = v if c["exact_max"] is None else max(c["exact_max"], v)
        if k0 not in best or v > best[k0][0]:
            best[k0] = (v, path, s_, over, exact)
    capped = leaves_capped or getattr(enumerate_turn, "capped", False)
    rv = {}
    for k in (his_k, live_k):
        if k in best:
            v, path, s_, over, exact = best[k]
            rv[k] = [terminal(s_, max_hp)] * revalue if over else leaf_value(eng.fs, sc, s_, revalue, 9001, max_hp)[0]
    out = dict(lines=n, leaves=len(cache), capped=capped, his_line=best.get(his_k, (None, None))[1], live_line=best.get(live_k, (None, None))[1])
    if his_k in rv and live_k in rv:
        out["r2"] = dict(zip(("d", "se"), paired(rv[his_k], rv[live_k])))
    hi, li = info.get(his_k), info.get(live_k)
    if hi and li:
        ub = lambda c: c["max"] if c["all_exact"] and not capped else None  # noqa: E731
        out["exact"] = dict(his_lb=hi["exact_max"], his_ub=ub(hi), live_lb=li["exact_max"], live_ub=ub(li))
    return out


def k_search(ref, sc, sim, his_k, live_k, seeds):
    import numpy as np
    cls = classes(sim)
    per = {}
    for s in range(seeds):
        r = ref.fs.decide(sc, sim.copy(), seed=1000 + s)
        if not r["searched"]:
            return None
        for x, q, ok in zip(r["opts"], r["q"], r["legal"]):
            if ok and q == q:
                per.setdefault(cls.get(x, (str(x),)), {}).setdefault(s, []).append(float(q))
    if his_k not in per or live_k not in per:
        return None
    ss = sorted(set(per[his_k]) & set(per[live_k]))
    h = [np.mean(per[his_k][s]) for s in ss]
    lv = [np.mean(per[live_k][s]) for s in ss]
    mean = {k: float(np.mean([np.mean(v) for v in d.values()])) for k, d in per.items()}
    return dict(zip(("d", "se"), paired(h, lv)), best=str(max(mean, key=mean.get)))


def playouts(eng, sc, sim0, his, live, n, rounds):
    max_hp = sc["max_hp"]
    res = {"H": [], "L": []}
    for s in range(n):
        for arm, first in (("H", his), ("L", live)):
            sim = sim0.copy()
            sim.determinize(100003 + s)
            sim.step(first)
            steps = 0
            while sim.stage() != "over" and sim.outcome() == 0 and steps < 300:
                r = eng.decide(sc, sim, 1.0, seed=7 * s + steps, rounds=1 if sim.stage() == "choice" else rounds, tol_hp=0.5, keep_potions=False)
                sim.step(r["action"])
                steps += 1
            res[arm].append((utility(sim, max_hp), sim.outcome(), json.loads(sim.snapshot())["player"]["hp"]))
    H, L = [x[0] for x in res["H"]], [x[0] for x in res["L"]]
    d, se = paired(H, L)
    mean = lambda xs: sum(xs) / len(xs)  # noqa: E731
    return dict(d=d, se=se, n=n, his_win=mean([x[1] == 1 for x in res["H"]]), live_win=mean([x[1] == 1 for x in res["L"]]),
                his_hp=mean([x[2] for x in res["H"]]), live_hp=mean([x[2] for x in res["L"]]))


def significant(r, hp_eq):
    return r is not None and abs(r["d"]) > 2 * r["se"] and abs(r["d"]) >= hp_eq


def bounded(r, hp_eq):
    return r is not None and abs(r["d"]) + 2 * r["se"] < hp_eq


def side(d):
    return "our gap" if d > 0 else "expert error"


def verdict(refs, max_hp):
    """refs: his - live in linear q units (win 1 + 0.5 hp/max, loss -1)"""
    hp_eq = HPB / max_hp
    ex, r3, r2, r1 = (refs.get(k) for k in ("exact", "r3", "r2", "r1"))
    if ex:
        if ex.get("his_lb") is not None and ex.get("live_ub") is not None and ex["his_lb"] - ex["live_ub"] >= hp_eq:
            return "our gap", "exact", f"his class reaches {ex['his_lb']:+.4f}; every line of the live player's class ends at most {ex['live_ub']:+.4f}"
        if ex.get("live_lb") is not None and ex.get("his_ub") is not None and ex["live_lb"] - ex["his_ub"] >= hp_eq:
            return "expert error", "exact", f"the live class reaches {ex['live_lb']:+.4f}; every line of his class ends at most {ex['his_ub']:+.4f}"
        if None not in (ex.get("his_ub"), ex.get("live_ub")) and ex["his_ub"] == ex["his_lb"] and ex["live_ub"] == ex["live_lb"] and abs(ex["his_ub"] - ex["live_ub"]) < hp_eq:
            return "tie", "exact", f"both classes end exactly at {ex['his_ub']:+.4f} / {ex['live_ub']:+.4f}"
    cap = refs.get("cap")
    if ex and cap is not None:
        top = {w: ex.get(f"{w}_lb") is not None and ex[f"{w}_lb"] >= cap - 1e-9 for w in ("his", "live")}
        if top["his"] and top["live"]:
            return "tie", "exact", f"both classes contain a line that wins this turn losing no HP ({cap:+.4f}, the maximum)"
        # a class that wins this turn losing no HP is optimal: a sampled reference ranking the other class above it over-values a leaf
        drop = (lambda r: significant(r, hp_eq) and r["d"] < 0) if top["his"] else (lambda r: significant(r, hp_eq) and r["d"] > 0) if top["live"] else (lambda r: False)
        r3, r2, r1 = (None if drop(r) else r for r in (r3, r2, r1))
    if significant(r3, hp_eq):
        return side(r3["d"]), "r3", f"paired playouts {r3['d']:+.4f} ({r3['se']:.4f}), n {r3.get('n')}"
    if r3 is not None and not (bounded(r3, hp_eq) or (r3["d"] == 0 and r3["se"] == 0)):
        return "unresolved", "r3", f"paired playouts {r3['d']:+.4f} ({r3['se']:.4f}), n {r3.get('n')} neither resolve 1 HP-eq nor bound it (they outrank the turn check)"
    if r3 is not None:
        return "tie", "r3", f"{'identical outcomes' if r3['d'] == 0 and r3['se'] == 0 else 'immaterial'}: {r3['d']:+.4f} ({r3['se']:.4f}) < 1 HP-eq"
    if significant(r2, hp_eq):
        if r1 is not None and r1["d"] * r2["d"] < 0 and abs(r1["d"]) > 2 * r1["se"]:
            return "unresolved", "r2", f"turn check {r2['d']:+.4f} ({r2['se']:.4f}) and K search {r1['d']:+.4f} ({r1['se']:.4f}) disagree in sign: needs powered playouts"
        return side(r2["d"]), "r2", f"turn check {r2['d']:+.4f} ({r2['se']:.4f})"
    for name, r in (("r3", r3), ("r2", r2)):
        if r is not None and (bounded(r, hp_eq) or (r["d"] == 0 and r["se"] == 0)):
            return "tie", name, f"{('order only' if name == 'r2' else 'identical outcomes') if r['d'] == 0 and r['se'] == 0 else 'immaterial'}: {r['d']:+.4f} ({r['se']:.4f}) < 1 HP-eq"
    return "unresolved", None, "no reference resolves 1 HP-eq"


def decision_states(fight):
    from agent import potions
    from agent.fight import Replayer
    rp = Replayer(fight, seed=1)
    sc = fight["scenario"]
    for i, act in enumerate(fight["log"]):
        sim = rp.sim.copy()
        j = json.loads(potions.to_sim_action(sc, act if isinstance(act, str) else json.dumps(act)))
        yield i, sim, j
        rp.advance(dict(fight, log=fight["log"][: i + 1]))
        if rp.errors:
            return


def fight_records(rec, replay):
    if replay:
        d = os.path.join(RE.creator_dir(rec), "replay", rec["video"]["id"])
        return [RE.load(os.path.join(d, f)) for f in sorted(os.listdir(d)) if f.endswith(".json")] if os.path.isdir(d) else []
    return [r for r in (RE.built_record(rec, spec["id"]) for _f, spec in fights(rec)) if r]


def short(t):
    return re.sub(r" #\d+", "", str(t)).replace("play ", "").replace(" -> e", ">e").replace("_SILENT", "")


def compare(a):
    watchdog()
    os.environ.setdefault("STS2_DEVICE", "cpu")
    from agent import proposal
    from agent.engine import Engine
    rec = RE.load(record_path(a.record))
    only = {}
    for tok in (a.only or "").split(","):
        if tok:
            fid, _, idx = tok.partition(":")
            only.setdefault(fid, set()).update(int(x) for x in idx.split("+") if x)
    live = Engine()
    flat_ = [x for x in RE.flatten(rec, built=None)]
    kidx = {(x.get("fight"), x.get("i")): k for k, x in enumerate(flat_) if x.get("fight")}
    tmap = {(x.get("fight"), x.get("i")): x.get("t") for x in flat_ if x.get("fight")}
    tc_eng = Engine(K=a.tc_k)
    ref = Engine(K=a.ref_k) if a.ref_k else None
    rows = []
    out_path = a.out or os.path.join(RE.creator_dir(rec), f"{rec['video']['id']}.divergences.json")
    for fr in fight_records(rec, a.replay):
        fid = fr["id"]
        if a.fights and not any(x in fid for x in a.fights.split(",")):
            continue
        sc = fr["scenario"]
        worth = proposal.fight_objective(sc)[0] if a.objective == "harness" else None
        for i, sim, j in decision_states(fr["fight"]):
            key = next((k for k in only if k in fid), None)
            if only and (key is None or (only[key] and i not in only[key])):
                continue
            legal = sim.legal()
            text = dict(legal)
            row = dict(k=kidx.get((fid, i)), fight=fid, i=i, t=tmap.get((fid, i)), action=j, n_legal=len(legal))
            mine = match(sim, j)
            if len(legal) < 2 or not mine:
                row["verdict"] = "forced" if len(legal) < 2 else "unmatched"
                rows.append(row)
                continue
            cls = classes(sim)
            his_k = cls[mine[0]]
            t0 = time.time()
            d = live.decide(sc, sim.copy(), a.live_budget, seed=1, rounds=a.live_rounds, tol_hp=0.5, keep_potions=True, worth=worth)
            live_k = cls.get(d["action"])
            row.update(his=short(text[mine[0]]), live=short(d["text"]), agree=live_k in {cls[x] for x in mine})
            if row["agree"]:
                row["verdict"] = "agree"
            elif "use_potion" in j:
                row["verdict"], row["why"] = "held", "potions are the operator's call: the live player holds them by design"
            else:
                refs = {"cap": WIN + HPB * min(json.loads(sim.snapshot())["player"]["hp"] / sc["max_hp"], 1.0)}
                if ref is not None:
                    refs["r1"] = k_search(ref, sc, sim, his_k, live_k, a.ref_seeds)
                if a.tc_dets:
                    try:
                        tc = turn_check(tc_eng, sc, sim, his_k, live_k, a.tc_dets, a.tc_revalue, max_leaves=a.tc_leaves)
                        refs.update({k: tc[k] for k in ("r2", "exact") if k in tc})
                        row.update(tc_lines=tc["lines"], tc_capped=tc["capped"], his_line=tc["his_line"], live_line=tc["live_line"])
                    except MemoryError as e:
                        row["tc_error"] = str(e)
                v0 = verdict(refs, sc["max_hp"])
                if a.playouts and (v0[0] == "unresolved" or (a.force_r3 and v0[1] == "r2")):
                    refs["r3"] = playouts(live, sc, sim, mine[0], d["action"], a.playouts, a.playout_rounds)
                row["verdict"], row["by"], row["why"] = verdict(refs, sc["max_hp"])
                row["refs"] = refs
            row["secs"] = round(time.time() - t0, 1)
            rows.append(row)
            print(f"{fid[-28:]:28s} {i:3d} {str(row['t'] or ''):>6} his {row['his'][:22]:22s} live {row['live'][:22]:22s} {row['verdict']:12s} "
                  f"{row.get('by') or ''} {row.get('why', '')[:90]} ({row['secs']}s)", flush=True)
            json.dump(dict(record=os.path.relpath(record_path(a.record), ROOT), source="replay" if a.replay else "frames", settings=settings(a), rows=rows),
                      open(out_path, "w", encoding="utf-8"), indent=1)
    if a.macro:
        rows_m = compare_macro(rec, a)
        data = json.load(open(out_path, encoding="utf-8")) if os.path.exists(out_path) else dict(rows=[])
        data["macro"] = rows_m
        json.dump(data, open(out_path, "w", encoding="utf-8"), indent=1)
    print(f"-> {os.path.relpath(out_path, ROOT)}")
    summary(json.load(open(out_path, encoding="utf-8")))


def settings(a):
    return dict(live_rounds=a.live_rounds, objective=a.objective, ref_k=a.ref_k, ref_seeds=a.ref_seeds, tc_k=a.tc_k, tc_dets=a.tc_dets,
                tc_revalue=a.tc_revalue, tc_leaves=a.tc_leaves, playouts=a.playouts, playout_rounds=a.playout_rounds, device=os.environ.get("STS2_DEVICE"),
                model=json.load(open(os.path.join(ROOT, "models", "current.json"), encoding="utf-8")).get("policy"))


def summary(data):
    from collections import Counter, defaultdict
    by = defaultdict(Counter)
    for r in data["rows"]:
        by[r["fight"]][r["verdict"]] += 1
    for f, c in by.items():
        print(f"{f}: " + ", ".join(f"{k} {v}" for k, v in sorted(c.items())))
    for r in data["rows"]:
        if r["verdict"] in ("our gap", "expert error", "unresolved"):
            print(f"  {r['fight'][12:]} #{r['i']} {r.get('t')}: his {r['his']} | live {r['live']} -> {r['verdict']} ({r.get('by')}: {r.get('why')})")


def compare_macro(rec, a):
    from agent import price as PR
    from predictor import Predictor
    from solver import PREDICTOR_CKPT
    pred = Predictor(PREDICTOR_CKPT, batch=1024)
    if a.replay:
        log = os.path.join(RE.creator_dir(rec), "replay", rec["video"]["id"] + ".jsonl")
        run = next((json.loads(x)["run"] for x in open(log, encoding="utf-8") if '"start"' in x), None) if os.path.exists(log) else None
        screens = PR.recorded_screens(os.path.join(ROOT, "runs", run, "events.jsonl"), ("CARD_REWARD", "RESTSITE")) if run else []
        screens = ((f, s, st, PR._played(s, c, PR.options(st, s)), c) for f, s, st, c, _old in screens)
    else:
        screens = synthetic_screens(rec)
    rows, used = [], set()
    flat = list(enumerate(RE.flatten(rec, built=None)))
    for floor, state, st, played, shown in screens:
        opts = PR.options(st, state)
        if len(opts) < 2:
            continue
        res = PR.price(st, opts, pred, n=a.macro_n, seed=floor)
        best, horizon = PR.best(res)
        fam = [lb for lb in res if PR._family(lb) == PR._family(played)]
        mine = played if played in res else max(fam, key=lambda lb: res[lb][horizon].mean(), default=None)
        kind = state.split("\n")[0]
        k = next((k for k, x in flat if k not in used and x.get("floor") == floor and x.get("screen") == kind), None)
        used.add(k)
        row = dict(k=k, floor=floor, screen=kind, played=shown, priced_as=mine, price_best=best, horizon=horizon, n=a.macro_n)
        if mine is None:
            row["verdict"] = "unpriced"
        elif PR._family(mine) == PR._family(best) and (mine == best or not played.startswith("smith ")):
            row["verdict"] = "agree"
        else:
            d, se = paired(res[mine][horizon], res[best][horizon])
            row.update(d=d, se=se, verdict="price prefers another option" if d < -2 * se else "tie")
        rows.append(row)
        print(f"F{floor:<3d} {row['screen']:11s} played {str(shown)[:26]:26s} price {best[:26]:26s} [{horizon}] {row['verdict']}"
              + (f" {row['d']:+.3f} ({row['se']:.3f})" if "d" in row else ""), flush=True)
    return rows


def synthetic_screens(rec):
    from agent import pools
    from agent import price as PR
    from agent import runmodel as R
    from agent import tracker
    steps = rec["steps"]
    sc = deck = hp = None
    bosses = {}
    for st in steps:
        enc = (st.get("fight") or {}).get("encounter", "")
        if enc.endswith("_BOSS") and enc not in bosses.setdefault(st.get("act", 0), []):
            bosses[st.get("act", 0)].append(enc)
    act_name = lambda a, enc: next((n for n in pools.act_names(a) if any(enc in pools.pool(n, kd) for kd in ("weak", "regular", "elite", "boss"))),  # noqa: E731
                                   pools.act_names(a)[0])
    for k, st in enumerate(steps):
        if st.get("fight"):
            sc = st["fight"]["scenario"]
            built = RE.built_record(rec, st["fight"]["id"])
            deck = [dict(c) for c in sc["deck"]]
            hp = built["hp_end"][0] if built and (built.get("outcome") == 1) else None
            continue
        if st.get("screen") == "SELECT" and deck is not None and k and steps[k - 1].get("screen") == "RESTSITE" and isinstance(st.get("pick"), list):
            c = next((c for c in deck if RE.base(c["id"]) == RE.base(st["pick"][0]) and not c.get("upgrade")), None)
            if c:
                c["upgrade"] = 1
        if sc is None or hp is None or "gap" in st or st.get("screen") not in ("CARD_REWARD", "RESTSITE"):
            continue
        act = sc.get("act", 0)
        header = f"A{act + 1} F{st['floor']} {rec['character']} A{rec['ascension']} HP {hp}/{sc['max_hp']} G{sc.get('gold', 0)} pots[-]"
        if st["screen"] == "CARD_REWARD":
            opts = [f"{i} {name}(1) ." for i, name in enumerate(st.get("seen") or [])] + [f"{len(st.get('seen') or [])} Skip"]
            played = st["pick"] if st.get("pick") else "skip"
            shown = played
        else:
            opts = ["0 Rest: Heal for 30% of your Max HP.", "1 Smith: Upgrade a card in your Deck."]
            rest = str(st.get("pick")).lower().startswith("rest")
            target = None if rest or k + 1 >= len(steps) else (steps[k + 1].get("pick") or [None])[0]
            cid = PR._card_id(target)[0] if target else None
            cid = cid or next((c["id"] for c in deck if target and RE.base(c["id"]) == RE.base(target)), None)
            played = "rest" if rest else (f"smith {cid}" if cid else "smith")
            shown = "rest" if rest else f"smith {target}"
        state = f"{st['screen']}\n{header}\n" + "\n".join(opts) + "\n"
        rs = R.RunState(sc, act, act_name(act, sc["encounter"]), hp, sc["max_hp"], sc.get("gold", 0), [dict(c) for c in deck], [r["id"] for r in sc.get("relics", [])], [],
                        sc.get("max_potion_slots", 2), (tracker.POTION_START, tracker.OFFSET_START, dict(tracker.UNKNOWN_BASE), 0), {},
                        bosses.get(act, []), None, None, 0)
        yield st["floor"], state, rs, played, shown
        if st["screen"] == "CARD_REWARD" and st.get("pick"):
            cid, up = PR._card_id(st["pick"])
            if cid:
                deck.append({"id": cid, "upgrade": up})
        elif played == "rest":
            hp = min(sc["max_hp"], hp + int(R.HEAL_REST * sc["max_hp"]))


def align(flat, log):
    """pairs each record action with its replay-log decision row (None for actions the log lacks, e.g. played by hand)"""
    bare = lambda d: re.sub(r" -> e\d+", "", d)  # noqa: E731
    rows = [json.loads(x) for x in open(log, encoding="utf-8")]
    dec = [r for r in rows if r["event"] == "decision"]
    j = 0
    for x in flat:
        row = dec[j] if j < len(dec) else None
        if row is not None and bare(row["action"]) == bare(RE.describe(x)):
            j += 1
            yield x, row
        else:
            yield x, None


def compact(a):
    rec = RE.load(record_path(a.record))
    head = {k: rec[k] for k in ("video", "seed", "build", "build_hash", "modded", "character", "ascension", "boss", "aliases") if k in rec}
    if a.result:
        head["result"] = a.result
    head["format"] = ("one step per line after this header; COMBAT steps carry fight {id, encounter, turns [{acts, times}]}; act = "
                      "['p', card, target?] | ['c', card...] | ['pot', slot, target, potion] | ['e']; target = the game's enemy index; "
                      "fight states: replay log (`python -m agent replay`), then `tools/expert.py build`")
    steps, merged = [], {}
    for st in rec["steps"]:
        if "gap" in st:
            sys.exit(f"refused: the record still has a gap at floor {st.get('floor')}: {st['gap']}")
        out = {k: st[k] for k in ("floor", "act", "screen", "t", "room", "pick") if k in st}
        f = st.get("fight")
        if f:
            pots = {p.get("slot", i): p["id"] for i, p in enumerate(f["scenario"].get("potions", []))}
            turns = []
            for t in f["turns"]:
                acts = []
                for x in t["acts"]:
                    x = [y for y in x if not isinstance(y, dict)]
                    if x[0] == "pot":
                        x = [x[0], x[1], x[2] if len(x) > 2 else None, pots[x[1]]]
                    acts.append(x)
                turn = dict(acts=acts)
                if t.get("times") and any(t["times"]):
                    turn["times"] = t["times"]
                if str(t.get("note", "")).startswith("inferred"):
                    turn["inferred"] = t["note"].split(". ")[0]
                turns.append(turn)
            base_id = re.sub(r"_b$", "", f["id"])
            if base_id in merged:
                merged[base_id]["fight"]["turns"] += turns
                continue
            out["fight"] = dict(id=base_id, encounter=f["encounter"], turns=turns)
            merged[base_id] = out
        steps.append(out)
    log = os.path.join(RE.creator_dir(rec), "replay", rec["video"]["id"] + ".jsonl")
    if os.path.exists(log):
        acts = {(s_["fight"]["id"], i): x for s_ in steps if s_.get("fight") for i, x in enumerate(x for t in s_["fight"]["turns"] for x in t["acts"])}
        bare = lambda d: re.sub(r" -> e\d+", "", d)  # noqa: E731
        n_t = 0
        for x, row in align(RE.flatten(dict(head, steps=steps), built=None), log):
            if row is not None and x.get("fight") and x["kind"] in ("play", "potion"):
                m = re.search(r" e(\d+)$", row["cmd"])
                act = acts[(x["fight"], x["i"])]
                tgt = int(m.group(1)) if m else None
                if (act[2] if len(act) > 2 else None) != tgt:
                    n_t += 1
                (act.__setitem__(2, tgt) if len(act) > 2 else act.append(tgt)) if tgt is not None or len(act) > 2 else None
        print(f"targets set to the game's enemy index from the replay log ({n_t} changed)")
    dst = os.path.join(RE.creator_dir(rec), f"{rec['video']['id']}.compact.jsonl")
    with open(dst, "w", encoding="utf-8", newline="\n") as fh:
        for x in [head] + steps:
            fh.write(json.dumps(x, ensure_ascii=False, separators=(",", ":")) + "\n")
    n = sum(len(t["acts"]) for s_ in steps if s_.get("fight") for t in s_["fight"]["turns"])
    print(f"-> {os.path.relpath(dst, ROOT)}: {len(steps)} steps, {n} combat actions, {os.path.getsize(dst) / 1024:.0f} KB")


def check_trace(a):
    """working record (frame obs) vs an oracle run-replay trace: per fight turn, hand / player hp / enemy hp at turn start"""
    from collections import Counter
    rec = RE.load(record_path(a.record))
    al = rec.get("aliases", {})
    groups = []
    for line in open(a.trace, encoding="utf-8"):
        r = json.loads(line)
        if r.get("event") != "combat":
            continue
        if r.get("step") == 0 or not groups:
            groups.append({})
        groups[-1].setdefault(r["turn"], r)
    bad, specs = 0, [spec for _floor, spec in fights(rec)]
    for spec, starts in zip(specs, groups + [None] * (len(specs) - len(groups))):
        if starts is None:
            print(f"{spec['id']}: not in the trace")
            bad += 1
            continue
        for ti, t in enumerate(spec["turns"]):
            o, r, msgs = t.get("obs") or {}, starts.get(ti + 1), []
            if r is None:
                print(f"{spec['id']} T{ti + 1}: no trace turn")
                bad += 1
                break
            if "hand" in o:
                want = Counter(RE.base(RE.token(x, al)[0]) + "+" * RE.token(x, al)[1] for x in o["hand"])
                got = Counter(RE.base(c["id"]) + "+" * int(c.get("upgrade", 0) > 0) for c in r["hand"])
                if want != got:
                    msgs.append(f"hand game-only {dict(got - want)} frames-only {dict(want - got)}")
            if "hp" in o and r["player"]["hp"] != o["hp"]:
                msgs.append(f"hp game {r['player']['hp']} frames {o['hp']}")
            gh, wh = [e["hp"] for e in r["enemies"] if e.get("alive", True)], [e["hp"] for e in o.get("e", [])]
            if "e" in o and gh != wh:
                msgs.append(f"enemy hp game {gh} frames {wh}")
            if msgs:
                bad += 1
                print(f"{spec['id']} T{ti + 1}: " + "; ".join(msgs))
    print(f"{len(specs)} fights, {len(groups)} in the trace, {bad} mismatching turns")
    sys.exit(1 if bad else 0)


def report(a):
    rec = RE.load(record_path(a.record))
    p = a.input or os.path.join(RE.creator_dir(rec), f"{rec['video']['id']}.divergences.json")
    data = json.load(open(p, encoding="utf-8"))
    v = rec["video"]
    lines = [f"# {v.get('channel')} \"{v.get('title')}\" ({rec['character'].title()} A{rec['ascension']}): decisions vs the live player",
             f"- url: {v['url']} | uploaded {v.get('uploaded')} | build `{rec['build']}`{' (pinned: re-enactment allowed)' if RE.build_ok(rec) else ' (not pinned: frame-based only)'}"
             f" | {rec.get('modded') or 'unmodded'} | seed `{rec['seed']}`",
             f"- source: {data['source']}; settings {json.dumps(data['settings'])}; reference precedence exact > R3 > R2, R1 never alone; verdict > 2 se and >= 1 HP-eq.",
             "", "## Summary", "| fight | decisions (forced) | agree | tie | held | our gap | expert error | unresolved |", "|---|---|---|---|---|---|---|---|"]
    from collections import Counter, OrderedDict
    per = OrderedDict()
    for r in data["rows"]:
        per.setdefault(r["fight"], Counter())[r["verdict"]] += 1
    for f, c in per.items():
        n = sum(c.values())
        lines.append(f"| {f} | {n} ({c['forced']}) | {c['agree']} | {c['tie']} | {c['held']} | {c['our gap']} | {c['expert error']} | {c['unresolved']} |")
    lines += ["", "## Divergences", "| k | fight | # | t | his | live | R1 his-live (se) | R2 | R3 | exact | verdict |", "|---|---|---|---|---|---|---|---|---|---|---|"]
    fmt = lambda r: "" if not r else f"{r['d']:+.4f} ({r['se']:.4f})" + (f" n {r['n']}" if "n" in r else "")  # noqa: E731
    for r in data["rows"]:
        if r["verdict"] in ("agree", "forced"):
            continue
        refs = r.get("refs") or {}
        ex = refs.get("exact") or {}
        f4 = lambda x: "?" if x is None else f"{x:+.4f}"  # noqa: E731
        ex_s = "" if not any(v is not None for v in ex.values()) else f"his [{f4(ex.get('his_lb'))}, {f4(ex.get('his_ub'))}] live [{f4(ex.get('live_lb'))}, {f4(ex.get('live_ub'))}]"
        lines.append(f"| {r.get('k', '')} | {r['fight'].split('_', 1)[-1]} | {r['i']} | {r.get('t') or ''} | {r['his']} | {r['live']} | {fmt(refs.get('r1'))} | {fmt(refs.get('r2'))} | "
                     f"{fmt(refs.get('r3'))} | {ex_s} | **{r['verdict']}** ({r.get('by') or '-'}) |")
    if data.get("macro"):
        lines += ["", "## Macro vs `price`", "| k | floor | screen | played | price best | horizon | d (se) | verdict |", "|---|---|---|---|---|---|---|---|"]
        for m in data["macro"]:
            lines.append(f"| {m.get('k', '')} | {m['floor']} | {m['screen']} | {str(m['played']).split('(')[0].split(':')[0]} | {m['price_best']} | {m['horizon']} | {fmt(m) if 'd' in m else ''} | {m['verdict']} |")
    text = "\n".join(lines) + "\n"
    out = a.out
    if out:
        open(out, "w", encoding="utf-8").write(text)
        print(f"-> {out}")
    else:
        print(text)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("fetch")
    p.add_argument("url")
    p.add_argument("--creator")
    p.add_argument("--fps", type=float, default=1.0)
    p.add_argument("--transcript-only", action="store_true")
    p.add_argument("--dry-run", action="store_true")
    p = sub.add_parser("build")
    p.add_argument("record")
    p.add_argument("--fight")
    p.add_argument("--out")
    p = sub.add_parser("validate")
    p.add_argument("record")
    for name in ("seedcheck", "replay"):
        p = sub.add_parser(name)
        p.add_argument("record")
        p.add_argument("--dry-run", action="store_true")
        p.add_argument("--all", action="store_true", help="dry run: list past the first gap")
    p = sub.add_parser("compare")
    p.add_argument("record")
    p.add_argument("--replay", action="store_true", help="fight records from the live replay instead of the frame-built ones")
    p.add_argument("--fights", help="comma-separated id substrings")
    p.add_argument("--only", help="fight:i+j,... decisions only")
    p.add_argument("--live-rounds", type=int, default=8)
    p.add_argument("--live-budget", type=float, default=2.0)
    p.add_argument("--objective", choices=("harness", "linear"), default="harness")
    p.add_argument("--ref-k", type=int, default=256)
    p.add_argument("--ref-seeds", type=int, default=8)
    p.add_argument("--tc-k", type=int, default=8)
    p.add_argument("--tc-dets", type=int, default=2)
    p.add_argument("--tc-leaves", type=int, default=1500)
    p.add_argument("--tc-revalue", type=int, default=8)
    p.add_argument("--playouts", type=int, default=12)
    p.add_argument("--playout-rounds", type=int, default=1)
    p.add_argument("--force-r3", action="store_true", help="also run the playouts when the turn check alone decides")
    p.add_argument("--macro", action="store_true", help="also price the card rewards and rests (run model, `agent/price.py`)")
    p.add_argument("--macro-n", type=int, default=32)
    p.add_argument("--out")
    p = sub.add_parser("compact")
    p.add_argument("record")
    p.add_argument("--result", help="e.g. 'win, 23/77 HP after the floor-49 boss'")
    p = sub.add_parser("check-trace")
    p.add_argument("record")
    p.add_argument("trace")
    p = sub.add_parser("report")
    p.add_argument("record")
    p.add_argument("--input")
    p.add_argument("--out")
    a = ap.parse_args()
    if a.cmd == "fetch":
        return fetch(a)
    if a.cmd == "build":
        return build(a)
    if a.cmd == "validate":
        return validate(a)
    if a.cmd in ("seedcheck", "replay"):
        if not a.dry_run:
            sys.exit(f"the live {a.cmd} drives the game through the harness (skill gate applies): python -m agent {a.cmd} {a.record}")
        return dry_run(a, a.cmd == "seedcheck")
    if a.cmd == "compare":
        return compare(a)
    if a.cmd == "report":
        return report(a)
    if a.cmd == "check-trace":
        return check_trace(a)
    if a.cmd == "compact":
        return compact(a)


if __name__ == "__main__":
    main()
