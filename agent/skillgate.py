"""The skill gate: no game action before the governing skills are loaded, and the screen's skill before a decision on that screen.

Why: CLAUDE.md is the only file a new agent is certain to see; the strategy book (`.claude/skills`) loads through the Skill tool at the moment it applies. A rule in a
file can be skipped, so two layers enforce it (stdlib only, shared by both):

1. Claude Code hooks (`.claude/settings.json` -> `scripts/skill_gate.py`):
     PostToolUse on Skill   `record`   remembers which skills this SESSION has loaded (`target/skill_state/<session>.json`)
     PreToolUse on Bash     `check`    refuses a harness command that acts on the game (or any direct bridge access) until the core skills are loaded;
                                       marks this session ACTIVE so the daemon knows who is calling
     PreToolUse on Write    `check`    refuses edits of the state files (a record cannot be forged by an agent)
     SessionStart           `start`    reminds a new session what to load; a compaction or clear forgets the record (the skill text left the context)
2. The harness daemon (`Harness._handle`): refuses `a`, `turn`, `combat`, ... unless the core skills AND the skills of the current screen were loaded by the ACTIVE
   session (pathing on the map and at Neow, deckbuilding at rewards / shop / rest / upgrade, the character and act skills). The message names the missing skill.

Read-only commands (`s`, `brief`, `eval`, `reward`, `route`, `adv`, `m`, `d`, `p`, `status` ...) are always allowed: looking is not acting.
Off switch for a human at a terminal or for harness tests: environment variable STS2_SKILL_GATE=off when the daemon starts (the hooks refuse commands that set it).
"""
import json
import os
import re
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
STATE = os.path.join(ROOT, "target", "skill_state")
SKILLS = os.path.join(ROOT, ".claude", "skills")
CORE = ("sts2", "sts2-harness", "sts2-strategy")
READ_ONLY = {"price", "plans", "s", "peek", "status", "brief", "m", "d", "p", "eval", "reward", "route", "routes", "rmcalc", "pickplan", "potions", "adv", "relics", "note", "budget", "hold", "quit", "mods", "snap", "fight", "deck.json", "newrun", ""}
BRIDGE_READ_ONLY = {"s", "peek", "d", "p", "m", "fight", "snap", "deck.json", "mods", "f"}
# screen kinds the bridge emits for deck decisions (Decisions.cs: reward screens, CHOOSE_*, and the room type for shop / rest site / treasure)
DECK_SCREENS = {"CARD_REWARD", "REWARDS", "CHOOSE_CARD", "CHOOSE_RELIC", "CHOOSE_BUNDLE", "SHOP", "RESTSITE", "TREASURE"}
COMBAT_LINE = re.compile(r"^T\d+ E\d+/\d+", re.M)


def exists(skill):
    return os.path.exists(os.path.join(SKILLS, skill, "SKILL.md"))


def _path(session):
    return os.path.join(STATE, re.sub(r"[^A-Za-z0-9_.-]", "_", session or "none") + ".json")


def loaded(session):
    try:
        return set(json.load(open(_path(session), encoding="utf-8")).get("skills", {}))
    except (OSError, ValueError):
        return set()


def record(session, skill):
    os.makedirs(STATE, exist_ok=True)
    name = skill.split(":")[-1].strip().lstrip("/")
    data = {"skills": {}}
    try:
        data = json.load(open(_path(session), encoding="utf-8"))
    except (OSError, ValueError):
        pass
    data.setdefault("skills", {})[name] = round(time.time(), 1)
    json.dump(data, open(_path(session), "w", encoding="utf-8"))


def reset(session):
    try:
        os.remove(_path(session))
    except OSError:
        pass


def set_active(session):
    os.makedirs(STATE, exist_ok=True)
    open(os.path.join(STATE, "ACTIVE"), "w").write(session or "")


def active():
    try:
        return open(os.path.join(STATE, "ACTIVE")).read().strip()
    except OSError:
        return ""


def missing_core(session):
    have = loaded(session)
    return [s for s in CORE if s not in have and exists(s)]


# ---------------------------------------------------------------------------------------------------------------------- hook side: what does this Bash command do?

def classify(command):
    """'none' = does not touch the game, 'read' = looks only, 'act' = may act (also anything unparseable that reaches the harness or the bridge)."""
    c = command or ""
    touches_bridge = bool(re.search(r"agent\.bridge|STS2_BRIDGE_PORT|15555|from agent import bridge", c))
    harness = [m for m in re.finditer(r"-m\s+agent(?![\w.])\s*([^\n|&;]*)", c)]
    if not harness and not touches_bridge:
        return "none"
    if touches_bridge and not harness:
        m = re.search(r"agent\.bridge\s+(\S+)", c)
        return "read" if m and m.group(1) in BRIDGE_READ_ONLY and "import" not in c else "act"
    worst = "read"
    for m in harness:
        arg = m.group(1).strip().strip("\"'")
        if arg.startswith("-") and arg.split()[0] == "-":  # batch mode: the lines of the heredoc that follow are the commands
            body = c[m.end():]
            lines = [l.strip() for l in body.splitlines()[1:]]
            for l in lines:
                if not l or l.startswith("#") or l.upper() in ("EOF", "'EOF'") or re.fullmatch(r"[A-Z_]+", l):
                    continue
                if l.split()[0] not in READ_ONLY:
                    return "act"
            continue
        first = arg.split()[0] if arg.split() else ""
        if first.startswith(("'", '"')):
            first = first.strip("\"'")
        if first == "serve":
            continue
        if first not in READ_ONLY:
            return "act"
    return worst


def tamper(command):
    """A command that would switch the gate off or forge its record (reading or mentioning the files is fine)."""
    c = command or ""
    if re.search(r"STS2_SKILL_GATE\s*=|STS2_SKILL_GATE[\"']?\]?\s*=|setenv.*STS2_SKILL_GATE|environ.*STS2_SKILL_GATE", c):
        return True
    write = r"(>|>>|\brm\b|\bmv\b|\bcp\b|\btee\b|sed\s+-i|Set-Content|Out-File|Remove-Item|json\.dump|\.write\(|os\.remove|shutil)"
    return bool(re.search(r"skill_state", c) and re.search(write, c)) or bool(re.search(r"(skill_gate\.py|skillgate\.py)", c) and re.search(r"(>|>>|\bsed\s+-i|\btee\b|\brm\b|\bmv\b)\s*\S*(skill_gate|skillgate)", c))


# ---------------------------------------------------------------------------------------------------------------------- harness side: what does this screen require?

def screen_skills(state_text):
    """Skills a decision on this screen needs, beyond the core (only skills that exist)."""
    first = (state_text or "").split("\n", 1)[0].split(" ")[0]
    head = next((l for l in (state_text or "").split("\n") if re.search(r"\bA\d+ F\d+\b", l)), "")
    need = []
    m = re.search(r"\bA(\d+) F(\d+)\s+([A-Z]+)", head)
    floor = int(m.group(2)) if m else 0
    if m:
        char = m.group(3).lower()
        need += [f"sts2-{char}", f"sts2-{char}-act{m.group(1)}"]
    in_combat = bool(COMBAT_LINE.search(state_text or ""))
    if first == "MAP":
        need.append("sts2-pathing")
    elif first == "EVENT":  # Neow and the ancients choose a route as well as a boon; later events are mechanics
        need += ["sts2-pathing"] + (["sts2-mechanics"] if floor > 1 else [])
    elif first in DECK_SCREENS or (first == "SELECT" and not in_combat):
        need.append("sts2-deckbuilding")
    return [s for s in dict.fromkeys(need) if exists(s)]


def gate_message(session, state_text=None):
    """None when the action may proceed, else the refusal text (names the skills to load)."""
    if os.environ.get("STS2_SKILL_GATE", "").lower() == "off":
        return None
    if not session:
        return ("no Claude Code session has announced itself (the project hooks in .claude/settings.json write it before every harness command). "
                "If you are a human at a terminal, start the daemon with STS2_SKILL_GATE=off")
    have = loaded(session)
    need = [s for s in CORE if exists(s)] + (screen_skills(state_text) if state_text is not None else [])
    miss = [s for s in dict.fromkeys(need) if s not in have]
    if miss:
        core = [s for s in miss if s in CORE]
        return ("skills not loaded in this session: " + ", ".join(miss) + ". Invoke "
                + ("the governing skills first (`sts2`, then `sts2-harness`, `sts2-strategy`)" if core else "them with the Skill tool")
                + " and read them, then repeat the command. No game action happens before that.")
    return None
