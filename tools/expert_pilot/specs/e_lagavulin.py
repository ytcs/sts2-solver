import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from common import AL, SOURCE, deck

_deck = deck(("S", 5), ("D", 4), ("N", 1), ("SV", 1), ("AB", 1), ("DS+", 1), ("DA", 1), ("HD", 1), ("CL", 1), ("SB+", 1), ("ST", 1),
             ("NF", 1), ("OB", 2), ("DF", 1))
_deck.append({"id": "DEFEND_SILENT", "upgrade": 0, "enchantment": {"id": "SPIRAL", "amount": 1}})
SPEC = dict(
    id="hMrQSndDvPc_F17_LAGAVULIN_MATRIARCH_BOSS",
    aliases=AL,
    enemy_ids=["LAGAVULIN_MATRIARCH"],
    hand_enchanted="DEFEND_SILENT",
    source=dict(SOURCE, floor=17, video_span="14:30-15:08 (turns 1-3; the video cuts inside turn 4)"),
    scenario=dict(name="baalorlord", ascension=10, encounter="LAGAVULIN_MATRIARCH_BOSS", character="SILENT", hp=28, max_hp=70, max_energy=3,
                  gold=209, max_potion_slots=2, base_orb_slots=0, seed="placeholder", total_floor=17, act=0, deck=_deck,
                  relics=[{"id": "RING_OF_THE_SNAKE"}, {"id": "SILKEN_TRESS", "props": {"IsUsed": True}}, {"id": "CENTENNIAL_PUZZLE"},
                          {"id": "PEN_NIB", "counter": 1}, {"id": "WHITE_STAR"}, {"id": "PANTOGRAPH"}, {"id": "LASTING_CANDY", "counter": 1},
                          {"id": "FESTIVE_POPPER"}],
                  potions=[{"id": "ENERGY_POTION", "slot": 0}]),
    turns=[
        dict(obs=dict(hp=53, block=0, energy=3, hand=["S", "D", "N", "D", "NF", "S", "DS+"], draw_n=16, relics={"PEN_NIB": 1},
                      e=[dict(hp=233, block=3, powers={"PLATING_POWER": 12, "ASLEEP_POWER": 3}, intent=[("Sleep",)])]),
             times=["14:37", "14:38", "14:40"],
             acts=[("p", "NF"), ("p", "N", 0), ("e",)]),
        dict(obs=dict(hp=53, block=0, energy=3, hand=["D", "SV", "CL", "S", "SB+"], draw=["D", "D", "S", "S", "DF", "DA", "HD", "ST", "OB", "OB", "AB"],
                      relics={"PEN_NIB": 2}, e=[dict(hp=233, block=12, intent=[("Sleep",)])]),
             times=["14:49", "14:51", "14:52"],
             acts=[("p", "SB+", 0), ("p", "S", 0), ("e",)]),
        dict(obs=dict(hp=53, block=0, energy=3, hand=["OB", "S", "HD", "OB", "D"], relics={"PEN_NIB": 3},
                      e=[dict(hp=221, intent=[("Attack", 21, 1)])]),
             times=["15:00", "15:02", "15:03", "15:04", "15:06", "15:07"],
             acts=[("p", "OB"), ("p", "HD"), ("c", "S", "OB"), ("p", "SH", 0), ("p", "SH", 0), ("e",)],
             end_obs=dict(hp=32, e=[dict(hp=170)]),
             ),
    ],
)
