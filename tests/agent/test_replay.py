"""`agent.fight.Replayer` on a recorded fight: Fabricator (run 20261005-201805) after 34 actions. Stampede's random target once killed the 8-HP Fabricator in
the simulator while the game went on; a resample that ends the fight while the game fights on must be rejected."""
import json

from support import fixture_json

from agent.fight import RANDOM_PREFIXES, Replayer


def truncated():
    d = fixture_json("fabricator_34.json")
    assert len(d["log"]) == 34 and len(d["states"]) == 35
    return dict(d, state=d["states"][34])


def test_fabricator_replay_stays_in_play():
    for seed in range(20):
        f = truncated()
        r = Replayer(f, seed=seed)
        assert r.advance(f) is True, seed
        assert r.sim.stage() == "play", seed
        alive = [e["id"] for e in json.loads(r.sim.snapshot())["enemies"] if e.get("alive", True)]
        assert alive == ["GUARDBOT", "FABRICATOR", "STABBOT"], (seed, alive)
        assert not r.errors, (seed, r.errors)
        assert r.applied == 34


def test_replay_synced_after_advance():
    f = truncated()
    r = Replayer(f, seed=3)
    r.advance(f)
    bad = [l for l in r.sim.diff(json.dumps(f["state"])) if not l.startswith(RANDOM_PREFIXES) and "props.Skin" not in l]
    assert bad == []
    assert r.stats["end_turn_matched"] >= 1


def test_map_potion():
    f = truncated()
    r = Replayer(f, seed=0)
    r.scenario = dict(r.scenario, potions=[dict(id="A", slot=1)])
    assert json.loads(r._map_potion('{"use_potion": {"slot": 1}}')) == {"use_potion": {"slot": 0}}
    r.scenario = dict(r.scenario, potions=[dict(id="A", slot=0), dict(id="B", slot=2)])
    assert json.loads(r._map_potion('{"use_potion": {"slot": 2, "target": 1}}')) == {"use_potion": {"slot": 1, "target": 1}}
    assert json.loads(r._map_potion('{"use_potion": {"slot": 1}}')) == {"use_potion": {"slot": 1}}  # an unknown slot is left alone
    assert r._map_potion('{"end_turn": true}') == '{"end_turn": true}'
    r.scenario = dict(r.scenario, potions=[dict(id="A"), dict(id="B")])
    assert json.loads(r._map_potion('{"use_potion": {"slot": 1}}')) == {"use_potion": {"slot": 1}}
