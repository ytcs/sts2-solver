import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from common import AL, SOURCE, deck

SK = "SKITTISH_POWER"
_deck = deck(("S", 5), ("D", 4), ("N", 1), ("SV", 1), ("AB", 1), ("DS+", 1), ("DA", 1), ("HD", 1), ("CL", 1), ("SB", 1), ("ST", 1),
             ("NF", 1), ("OB", 1), ("DF", 1))
_deck.append({"id": "DEFEND_SILENT", "upgrade": 0, "enchantment": {"id": "SPIRAL", "amount": 1}})
SPEC = dict(
    id="hMrQSndDvPc_F15_PHANTASMAL_GARDENERS_ELITE",
    aliases=AL,
    enemy_ids=["PHANTASMAL_GARDENER"] * 4,
    hand_enchanted="DEFEND_SILENT",
    source=dict(SOURCE, floor=15, video_span="11:07-13:41"),
    scenario=dict(name="baalorlord", ascension=10, encounter="PHANTASMAL_GARDENERS_ELITE", character="SILENT", hp=30, max_hp=70, max_energy=3,
                  gold=177, max_potion_slots=2, base_orb_slots=0, seed="placeholder", total_floor=15, act=0, deck=_deck,
                  relics=[{"id": "RING_OF_THE_SNAKE"}, {"id": "SILKEN_TRESS", "props": {"IsUsed": True}}, {"id": "CENTENNIAL_PUZZLE"},
                          {"id": "PEN_NIB", "counter": 1}, {"id": "WHITE_STAR"}, {"id": "PANTOGRAPH"}, {"id": "LASTING_CANDY", "counter": 0}],
                  potions=[]),
    turns=[
        dict(obs=dict(hp=30, block=0, energy=3, hand=["HD", "SB", "OB", "D", "AB", "DF", "D"], draw_n=15, relics={"PEN_NIB": 1},
                      e=[dict(hp=29, powers={SK: 7}, intent=[("Attack", 1, 3)]), dict(hp=31, powers={SK: 7}, intent=[("Attack", 5, 1)]),
                         dict(hp=30, powers={SK: 7}, intent=[("Attack", 7, 1)]), dict(hp=32, powers={SK: 7}, intent=[("Buff",)])]),
             times=["11:13", "11:40", "11:44", "11:46", "11:47", "11:49", "11:51", "11:52"],
             acts=[("p", "DF", None, dict(block=4)), ("p", "D*", None, dict(block=14)), ("p", "SB", 3), ("p", "HD"), ("c", "OB", "D"),
                   ("p", "SH", 3, dict(e=[dict(hp=29), dict(hp=31), dict(hp=30), dict(hp=28, block=7)])),
                   ("p", "SH", 0, dict(e=[dict(hp=25, block=7), dict(hp=31), dict(hp=30), dict(hp=28, block=7)])), ("e",)]),
        dict(obs=dict(hp=29, block=0, energy=3, hand=["SV", "CL", "S", "ST", "N", "DA", "S"], draw_n=7, discard_n=6, relics={"PEN_NIB": 3},
                      e=[dict(hp=25, intent=[("Buff",)]), dict(hp=31, intent=[("Attack", 7, 1)]), dict(hp=30, intent=[("Attack", 1, 3)]),
                         dict(hp=21, powers={SK: 7, "POISON_POWER": 6, "STRENGTH_POWER": 3}, intent=[("Attack", 8, 1)])]),
             times=["12:26", "12:28", "12:39", "12:41"],
             acts=[("p", "ST", 3, dict(e=[dict(hp=25), dict(hp=31), dict(hp=30), dict(hp=13, block=7)])),
                   ("p", "DA", 3, dict(block=10, e=[dict(hp=25), dict(hp=31), dict(hp=30), dict(hp=8)])),
                   ("p", "N", 0, dict(e=[dict(hp=22, block=7), dict(hp=31), dict(hp=30), dict(hp=6)])), ("e",)]),
        dict(obs=dict(hp=29, block=0, energy=3, hand=["D", "D", "S", "S", "NF"], draw=["D", "DS+"], relics={"PEN_NIB": 6},
                      e=[dict(hp=22, intent=[("Attack", 8, 1)]), dict(hp=31, intent=[("Attack", 1, 3)]), dict(hp=30, intent=[("Buff",)])]),
             times=["12:47", "12:49", "13:01", "13:03"],
             acts=[("p", "D", None, dict(block=5)), ("p", "D", None, dict(block=10)),
                   ("p", "S", 0, dict(e=[dict(hp=16, block=7), dict(hp=31), dict(hp=30)])), ("e",)]),
        dict(obs=dict(hp=28, block=0, energy=3, hand=["DS+", "D", "S", "DF", "D"], draw_n=15, relics={"PEN_NIB": 7},
                      e=[dict(hp=16, intent=[("Attack", 10, 1)]), dict(hp=31, intent=[("Buff",)]), dict(hp=30, intent=[("Attack", 8, 1)])]),
             times=["13:32", "13:33", "13:37"],
             acts=[("p", "S", 2, dict(e=[dict(hp=16), dict(hp=31), dict(hp=24, block=7)])), ("p", "DS+", None, dict(e=[dict(hp=2)])), ("e",)]),
        dict(obs=dict(hp=28, block=0, energy=3, hand=["S", "D", "S", "S", "SB"], relics={"PEN_NIB": 0},
                      e=[dict(hp=2, intent=[("Attack", 8, 1)])]),
             times=["13:40"],
             acts=[("p", "S", 0)]),
    ],
)
