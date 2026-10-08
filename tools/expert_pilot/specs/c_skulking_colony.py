import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from common import AL, SOURCE, deck

SHELL = "HARDENED_SHELL_POWER"
SPEC = dict(
    id="hMrQSndDvPc_F12_SKULKING_COLONY_ELITE",
    aliases=AL,
    enemy_ids=["SKULKING_COLONY"],
    source=dict(SOURCE, floor=12, video_span="8:39-10:03"),
    scenario=dict(name="baalorlord", ascension=10, encounter="SKULKING_COLONY_ELITE", character="SILENT", hp=17, max_hp=70, max_energy=3,
                  gold=151, max_potion_slots=2, base_orb_slots=0, seed="placeholder", total_floor=12, act=0,
                  deck=deck(("S", 5), ("D", 5), ("N", 1), ("SV", 1), ("AB", 1), ("DS+", 1), ("DA", 1), ("HD", 1), ("CL", 1), ("SB", 1),
                            ("ST", 1), ("NF", 1)),
                  relics=[{"id": "RING_OF_THE_SNAKE"}, {"id": "SILKEN_TRESS", "props": {"IsUsed": True}}, {"id": "CENTENNIAL_PUZZLE"},
                          {"id": "PEN_NIB", "counter": 9}, {"id": "WHITE_STAR"}, {"id": "PANTOGRAPH"}],
                  potions=[]),
    turns=[
        dict(obs=dict(hp=17, block=0, energy=3, hand=["ST", "S", "SV", "S", "S", "DA", "HD"], draw_n=13, relics={"PEN_NIB": 9},
                      e=[dict(hp=80, powers={SHELL: 20}, intent=[("Attack", 16, 1)])]),
             times=["8:46", "8:50", "8:53", "8:57", "8:58", "8:59", "8:59", "9:00"],
             acts=[("p", "DA", 0, dict(block=10, e=[dict(hp=60)])), ("p", "HD"), ("c", "S", "S"), ("p", "SV"), ("c", "S", dict(block=18)),
                   ("p", "SH", 0), ("p", "SH", 0, dict(e=[dict(hp=60)])), ("e",)]),
        dict(obs=dict(hp=17, block=0, energy=3, hand=["N", "S", "S", "D", "CL"], draw_n=8, relics={"PEN_NIB": 2},
                      e=[dict(hp=60, powers={SHELL: 20}, intent=[("Attack", 16, 1)])]),
             times=["9:03", "9:05", "9:06", "9:07", "9:08"],
             acts=[("p", "N", 0, dict(e=[dict(hp=57)])), ("p", "S", 0), ("p", "S", 0, dict(e=[dict(hp=45)])), ("p", "D", None, dict(block=5)), ("e",)]),
        dict(obs=dict(hp=10, block=0, energy=3, hand=["NF", "AB", "SB", "D", "D", "DS+", "D"], draw_n=0, relics={"PEN_NIB": 5},
                      e=[dict(hp=45, powers={SHELL: 20}, intent=[("Attack", 11, 1), ("Buff",)])]),
             times=["9:22", "9:24", "9:25", "9:26"],
             acts=[("p", "DS+", None, dict(e=[dict(hp=25)])), ("p", "D"), ("p", "D", None, dict(block=10)), ("e",)]),
        dict(obs=dict(hp=9, block=0, energy=3, hand=["SB", "D", "N", "S", "HD", "S"], draw_n=12, relics={"PEN_NIB": 7},
                      e=[dict(hp=25, powers={SHELL: 20, "STRENGTH_POWER": 4}, intent=[("Attack", 12, 2)])]),
             times=["9:31", "9:48", "9:50", "9:52", "9:53", "9:55", "9:58", "10:01"],
             acts=[("p", "N", 0, dict(e=[dict(hp=22)])), ("p", "HD"), ("c", "D", "S"), ("p", "SH", 0, dict(e=[dict(hp=18)])),
                   ("p", "S", 0, dict(e=[dict(hp=6)])), ("p", "SH", 0, dict(e=[dict(hp=5)])), ("p", "SB", 0), ("e",)]),
    ],
)
