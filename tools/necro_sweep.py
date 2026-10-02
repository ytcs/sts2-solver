#!/usr/bin/env python3
"""Necrobinder validation: run diff sweeps over groups of cards (starter deck + group, A10, NIBBITS_WEAK by default).

  tools/necro_sweep.py [--n 30] [--jobs 6] [--upgrade] [--encounter X] [group ...]

Groups are defined below; `all` runs every group. Mismatching runs keep their scenario/trace under /tmp/sts2sweep_<group>.
"""
import argparse, json, os, subprocess, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

GROUPS = {
    "osty1": "AFTERLIFE REANIMATE CLEANSE DIRGE INVOKE PULL_AGGRO SPUR NECRO_MASTERY",
    "osty2": "POKE SNAP FLATTEN FETCH RATTLE SIC_EM HIGH_FIVE BONE_SHARDS",
    "osty3": "RIGHT_HAND_HAND SQUEEZE PROTECTOR SACRIFICE UNLEASH BODYGUARD CALCIFY",
    "doom1": "BLIGHT_STRIKE DEATHBRINGER DEATHS_DOOR END_OF_DAYS NEGATIVE_PULSE NO_ESCAPE OBLIVION SCOURGE",
    "doom2": "TIMES_UP COUNTDOWN NEUROSURGE REAPER_FORM SHROUD DEBILITATE LETHALITY SLEIGHT_OF_FLESH",
    "souls1": "CAPTURE_SPIRIT REAVE SEVERANCE GRAVE_WARDEN SEANCE SOUL_STORM HAUNT DEVOUR_LIFE",
    "ethereal1": "DEFILE DEFY FEAR PARSE VEILPIERCER PULL_FROM_BELOW BANSHEES_CRY EIDOLON",
    "ethereal2": "ENFEEBLING_TOUCH SCULPTING_STRIKE PAGESTORM SPIRIT_OF_ASH CALL_OF_THE_VOID DEMESNE SENTRY_MODE",
    "misc1": "BORROWED_TIME BURY DEATH_MARCH DELAY DRAIN_POWER DREDGE GRAVEBLAST HANG",
    "misc2": "MELANCHOLY MISERY PUTREFY REAP SOW SHARED_FATE TRANSFIGURE UNDEATH",
    "combo1": "SHROUD SLEIGHT_OF_FLESH COUNTDOWN REAPER_FORM CALCIFY NECRO_MASTERY LETHALITY DEBILITATE OBLIVION DEVOUR_LIFE HAUNT POKE DIRGE",
    "combo2": "DEMESNE FRIENDSHIP SPIRIT_OF_ASH PAGESTORM VEILPIERCER DANSE_MACABRE CALL_OF_THE_VOID SENTRY_MODE NEUROSURGE DEFILE PARSE FEAR",
    "combo3": "SACRIFICE BONE_SHARDS SQUEEZE PROTECTOR RATTLE FLATTEN FETCH SIC_EM SPUR CLEANSE SEANCE SOUL_STORM REAVE",
    "rare1": "END_OF_DAYS BANSHEES_CRY DEMESNE DEATHS_DOOR FLATTEN CLEANSE REAPER_FORM SENTRY_MODE SLEIGHT_OF_FLESH PARSE DEFILE FEAR",
    "misc3": "THE_SCYTHE ERADICATE WISP FRIENDSHIP DANSE_MACABRE FORBIDDEN_GRIMOIRE DEFEND_NECROBINDER STRIKE_NECROBINDER",
}


def deck_spec(cards, upgrade):
    return ",".join(c + ("+" if upgrade else "") for c in cards.split())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("groups", nargs="*", default=["all"])
    ap.add_argument("--n", type=int, default=30)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--upgrade", action="store_true", help="upgrade every card of the group")
    ap.add_argument("--mixed", action="store_true", help="each card once base + once upgraded")
    ap.add_argument("--encounter", default="NIBBITS_WEAK")
    ap.add_argument("--extra", default="", help="extra deck spec appended (e.g. CARD:2)")
    ap.add_argument("--tag", default="s")
    ap.add_argument("--energy", type=int, default=0, help="override max energy (and 3x hp) to reach deeper combinations")
    a = ap.parse_args()
    names = list(GROUPS) if a.groups == ["all"] else a.groups
    worst = 0
    for g in names:
        cards = GROUPS.get(g, g)
        if a.mixed:
            spec = deck_spec(cards, False) + "," + deck_spec(cards, True)
        else:
            spec = deck_spec(cards, a.upgrade)
        if a.extra:
            spec += "," + a.extra
        tpl = f"/tmp/necro_tpl_{g}{'_up' if a.upgrade else ''}{'_mx' if a.mixed else ''}.json"
        out = subprocess.run([sys.executable, os.path.join(ROOT, "tools/mk_scenario.py"), "--character", "NECROBINDER", "--starter",
                              "--encounter", a.encounter, "--deck", spec], capture_output=True, text=True)
        if out.returncode != 0:
            print(g, "mk_scenario failed", out.stderr)
            continue
        tj = json.loads(out.stdout)
        if a.energy:
            tj["max_energy"] = a.energy
            tj["hp"] = tj["max_hp"] = 200
        open(tpl, "w").write(json.dumps(tj, indent=1))
        keep = f"/tmp/sts2sweep_{g}{'_up' if a.upgrade else ''}{'_mx' if a.mixed else ''}"
        r = subprocess.run([sys.executable, os.path.join(ROOT, "tools/diff_sweep.py"), tpl, "--n", str(a.n), "--jobs", str(a.jobs),
                            "--keep", keep, "--tag", a.tag], capture_output=True, text=True)
        lines = [l for l in r.stdout.strip().splitlines()]
        print(f"=== {g}{' (upgraded)' if a.upgrade else ''}{' (mixed)' if a.mixed else ''}: {lines[-1] if lines else r.stderr[-200:]}")
        for l in lines[:-1][:14]:
            print("   ", l)
        worst = max(worst, r.returncode)
    sys.exit(worst)


main()
