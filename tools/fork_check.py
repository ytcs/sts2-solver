#!/usr/bin/env python3
"""price vs routes at recorded map forks: replay a headless run to a MAP screen it priced (tools/branch.py machinery), run `routes` there,
and print price's ranking next to routes' P(win boss) per option (k >= 0 elites).
usage: tools/fork_check.py <events.jsonl> <MAP floor key, e.g. "A2 F30"> [...more keys] [--port 15960]"""
import argparse, json, os, re, shutil, socket, subprocess, sys, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path[:0] = [ROOT, os.path.join(ROOT, "rl"), os.path.join(ROOT, "tools")]


def _server(seed, port, sd):
    import baseline as B
    env = dict(os.environ, APPDATA=os.path.join(sd, "appdata"), LOCALAPPDATA=os.path.join(sd, "localappdata"), TEMP=os.path.join(sd, "tmp"), TMP=os.path.join(sd, "tmp"))
    for k in ("APPDATA", "LOCALAPPDATA", "TEMP"):
        os.makedirs(env[k], exist_ok=True)
    p = subprocess.Popen([B.DOTNET, os.path.abspath(B.DLL), "serve", "--port", str(port), "--state-dir", os.path.join(sd, "saves"), "--seed", seed,
                          "--character", "ironclad", "--ascension", "10"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, cwd=sd, env=env)
    for _ in range(600):
        try:
            socket.create_connection(("127.0.0.1", port), timeout=1).close()
            return p
        except OSError:
            time.sleep(0.1)
    raise RuntimeError("server did not start")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("events")
    ap.add_argument("floors", nargs="+")
    ap.add_argument("--port", type=int, default=15960)
    a = ap.parse_args()
    from postmortem import replay_prefix
    from agent import bridge
    from agent.harness import Harness
    from agent.runlog import RunLog
    ev = [json.loads(l) for l in open(a.events, encoding="utf-8")]
    seed = os.path.basename(os.path.dirname(a.events))
    h = None
    for key in a.floors:
        idx = next(i for i, e in enumerate(ev) if e["kind"] == "macro" and e.get("screen") == "MAP" and key in (e.get("state") or "").split("\n")[1])
        pr = next((e for e in ev[idx:] if e["kind"] == "price" and e.get("screen") == "MAP"), None)
        sd = os.path.join(ROOT, "target", "fork_check", f"{seed}_{key.replace(' ', '')}")
        shutil.rmtree(sd, ignore_errors=True)
        os.makedirs(sd)
        proc = _server(seed, a.port, sd)
        try:
            bridge.OVERRIDE = f"127.0.0.1:{a.port}"
            s, bad = replay_prefix(ev, idx)
            log = RunLog(os.path.join(sd, "run"))
            os.makedirs(log.dir, exist_ok=True)
            with open(os.path.join(log.dir, "events.jsonl"), "w", encoding="utf-8") as fh:
                fh.write("".join(json.dumps(e) + "\n" for e in ev[:idx]))
            if h is None:
                h = Harness(log=log)
            else:
                h.log = log
            out = h.handle("routes")
            rows = re.findall(r"^(\S+)\s+([0-9.]+)\s+[0-9.]+", out.split("per option on offer")[1].split("representative")[0], re.M) if "per option on offer" in out else []
            chosen = (ev[idx].get("choice"), pr.get("best") if pr else None)
            print(f"{seed} {key}: mismatches {len(bad)}; price best {chosen[1]} ranked by {pr.get('ranked_by') if pr else None}: "
                  + (", ".join(f"{k} {v.get(pr['ranked_by'], 0):.3f}" for k, v in pr["result"].items()) if pr else "-")
                  + " | routes P(win boss): " + ", ".join(f"{o} {p}" for o, p in rows), flush=True)
        finally:
            bridge.OVERRIDE = None
            proc.kill()


if __name__ == "__main__":
    main()
