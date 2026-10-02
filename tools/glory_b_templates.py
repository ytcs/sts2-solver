#!/usr/bin/env python3
"""Generate the glory-b sweep templates (Act 3 elites/bosses x characters x HP 500/80) into oracle/templates/.

  tools/glory_b_templates.py && for t in oracle/templates/glory_b_*.json; do tools/diff_sweep.py $t --n 40 --jobs 4; done
"""
import subprocess, os

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ENC = ["SOUL_NEXUS_ELITE", "MECHA_KNIGHT_ELITE", "KNIGHTS_ELITE", "QUEEN_BOSS", "AEONGLASS_BOSS", "TEST_SUBJECT_BOSS"]
DECKS = {
    "ir": ("IRONCLAD", "MANGLE,MOLTEN_FIST,ONE_TWO_PUNCH+,OUTRAGE,PERFECTED_STRIKE,RAMPAGE,SPITE,STOMP,TEAR_ASUNDER,THRASH+,UPPERCUT,"
           "WHIRLWIND,INFERNO,TANK,UNMOVABLE,VICIOUS,STAMPEDE,HAVOC,CASCADE,BATTLE_TRANCE,FEED,FIEND_FIRE,BREAKTHROUGH"),
    "si": ("SILENT", "BLADE_DANCE+,ACCURACY,FOOTWORK+,NOXIOUS_FUMES,SERPENT_FORM,STRANGLE,REFLEX+,TACTICIAN,UP_MY_SLEEVE,TOOLS_OF_THE_TRADE,"
           "BURST,FAN_OF_KNIVES,ENVENOM,DEADLY_POISON+,LEG_SWEEP,SKEWER,MURDER"),
    "ne": ("NECROBINDER", "SHROUD,SLEIGHT_OF_FLESH+,COUNTDOWN,REAPER_FORM,CALCIFY,NECRO_MASTERY,LETHALITY,DEBILITATE,OBLIVION,DEVOUR_LIFE+,"
           "HAUNT,POKE,DIRGE,SACRIFICE,BONE_SHARDS,SQUEEZE,PROTECTOR+,RATTLE,FLATTEN,SEANCE,SOUL_STORM"),
    "re": ("REGENT", "COMET,GAMMA_BLAST,METEOR_SHOWER+,DEVASTATE,ASTRAL_PULSE,CELESTIAL_MIGHT,SEVEN_STARS,GUIDING_STAR+,SOLAR_STRIKE,"
           "SHINING_STRIKE,KNOCKOUT_BLOW,HEGEMONY,THE_SMITH,BLACK_HOLE,CHILD_OF_THE_STARS,FURNACE,MONARCHS_GAZE,PARRY+"),
}
for e in ENC:
    for k, (ch, deck) in DECKS.items():
        for hp in (500, 80, 1200 if e == 'TEST_SUBJECT_BOSS' else 3000):
            out = os.path.join(ROOT, f"oracle/templates/glory_b_{k}_{e.lower()}_{hp}.json")
            args = ["python3", os.path.join(ROOT, "tools/mk_scenario.py"), "--encounter", e, "--character", ch, "--starter",
                    "--hp", str(hp), "--act", "2"]
            if deck:
                args += ["--deck", deck]
            open(out, "w").write(subprocess.run(args, capture_output=True, text=True, check=True).stdout)
