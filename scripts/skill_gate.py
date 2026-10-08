#!/usr/bin/env python3
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("skillgate", os.path.join(HERE, "..", "agent", "skillgate.py"))
sg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sg)


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "check"
    try:
        d = json.load(sys.stdin)
    except ValueError:
        d = {}
    session = d.get("session_id", "")
    ti = d.get("tool_input") or {}
    if mode == "record":
        skill = ti.get("skill") or ti.get("name") or ""
        if skill:
            sg.record(session, skill)
        return 0
    if mode == "start":
        if d.get("source") in ("compact", "clear"):
            sg.reset(session)
        print("STS2 harness rule: before ANY game action, invoke the skills `sts2`, then `sts2-harness` and `sts2-strategy` (Skill tool) and read them; the hooks refuse "
              "harness action commands until they are loaded in this session" + (" (the record was reset: load them again)" if d.get("source") in ("compact", "clear") else "") +
              ". Screen skills (`sts2-pathing`, `sts2-deckbuilding`, the character and act skills) are demanded by the harness when a screen needs them.")
        return 0
    tool = d.get("tool_name", "")
    if tool in ("Write", "Edit", "NotebookEdit"):
        p = (ti.get("file_path") or ti.get("notebook_path") or "").replace("\\", "/")
        if "target/skill_state" in p or p.endswith("scripts/skill_gate.py") or p.endswith("agent/skillgate.py"):
            print("REFUSED: the skill gate's state and code are not edited by the agent that the gate constrains.", file=sys.stderr)
            return 2
        return 0
    if tool != "Bash":
        return 0
    cmd = ti.get("command") or ""
    if sg.tamper(cmd):
        print("REFUSED: the skill gate (STS2_SKILL_GATE / skill_state / skill_gate.py) is not touched by the agent that it constrains; a human sets it.", file=sys.stderr)
        return 2
    kind = sg.classify(cmd)
    if kind == "none":
        return 0
    sg.set_active(session)
    if kind == "act":
        miss = sg.missing_core(session)
        if miss:
            print("REFUSED: no game action before the governing skills are loaded. Missing in this session: " + ", ".join(miss) +
                  ". Invoke them with the Skill tool (`sts2` first), read them, then repeat. Read-only commands (s, brief, eval, reward, route, adv, m, d, p, status) are allowed meanwhile.",
                  file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
