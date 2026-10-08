import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from common import AL, SOURCE, deck

SPEC = dict(
    id="hMrQSndDvPc_F11_GREMLIN_MERC_NORMAL",
    aliases=AL,
    enemy_ids=["GREMLIN_MERC"],
    source=dict(SOURCE, floor=11, video_span="7:29-8:19"),
    scenario=dict(name="baalorlord", ascension=10, encounter="GREMLIN_MERC_NORMAL", character="SILENT", hp=18, max_hp=70, max_energy=3,
                  gold=141, max_potion_slots=2, base_orb_slots=0, seed="placeholder", total_floor=11, act=0,
                  deck=deck(("S", 5), ("D", 5), ("N", 1), ("SV", 1), ("AB", 1), ("DS+", 1), ("DA", 1), ("HD", 1), ("CL", 1), ("SB", 1),
                            ("ST", 1)),
                  relics=[{"id": "RING_OF_THE_SNAKE"}, {"id": "SILKEN_TRESS", "props": {"IsUsed": True}}, {"id": "CENTENNIAL_PUZZLE"},
                          {"id": "PEN_NIB", "counter": 8}, {"id": "WHITE_STAR"}, {"id": "PANTOGRAPH"}],
                  potions=[]),
    turns=[
        dict(obs=dict(hp=18, block=0, energy=3, hand=["D", "D", "S", "SB", "D", "D", "HD"], draw_n=12, relics={"PEN_NIB": 8},
                      e=[dict(hp=53, powers={"SURPRISE_POWER": 1, "THIEVERY_POWER": 20}, intent=[("Attack", 8, 2)])]),
             times=["7:36", "7:37", "7:38", "7:39", "7:39", "7:41", "7:44"],
             acts=[("p", "D@0", None, dict(block=5)), ("p", "D", None, dict(block=10)), ("p", "D", None, dict(block=15)), ("p", "HD"),
                   ("c", "S", "D"), ("p", "SH", 0, dict(e=[dict(hp=49)])), ("e",)]),
        dict(obs=dict(hp=17, block=0, energy=3, hand=["SB", "D", "S", "ST", "S", "N", "DS+", "SV", "AB"], draw=["S", "S", "DA", "CL"],
                      relics={"PEN_NIB": 9}, e=[dict(hp=49)]),
             times=["7:49", "7:51", "7:52", "8:07", "8:08"],
             acts=[("p", "ST", 0, dict(e=[dict(hp=33)])), ("p", "N", 0, dict(e=[dict(hp=28)])), ("p", "DS+", None),
                   ("p", "S", 1, dict(e=[dict(hp=12, max_hp=12), dict(hp=12)])), ("e",)]),
        dict(obs=dict(hp=17, block=0, energy=3, hand=["SB", "S", "CL", "DA", "S", "D"], relics={"PEN_NIB": 4},
                      e=[dict(hp=12, intent=[("Attack", 10, 1)]), dict(hp=12)]),
             times=["8:11", "8:13", "8:14"],
             acts=[("p", "DA", 1, dict(block=10, e=[dict(hp=12), dict(hp=2)])), ("p", "S", 1), ("e",)]),
        dict(obs=dict(hp=17, block=0, energy=3, hand=["SB", "S", "SV", "S", "SH", "D"], relics={"PEN_NIB": 6},
                      e=[dict(hp=12, intent=[("Attack", 10, 1)])]),
             times=["8:17", "8:18", "8:18"],
             acts=[("p", "SH", 0, dict(e=[dict(hp=8)])), ("p", "S", 0), ("p", "S", 0)]),
    ],
)
