import collections
import hashlib
import json

from support import MAP_A2, MAP_SCREEN_A2, FakeBridge, FakeEngine, deck, events, fight_starts, golden, make_harness, ok, screen


def engine_digest(eng):
    out = []
    for c in eng.log:
        cnt = collections.Counter((s[1], ",".join(s[3])) for s in c["scen"])
        out.append(f"solve attempts={c['attempts']} n={len(c['scen'])} sha={hashlib.sha1(json.dumps(c['scen']).encode()).hexdigest()[:12]}")
        out += [f"  {e} [{p}] x{n}" for (e, p), n in sorted(cnt.items())]
    return "\n".join(out) + "\n"


def run(monkeypatch, tmp_path, screen_text, cmd, name):
    eng = FakeEngine()
    fake = FakeBridge(screen_text, deck_json=deck(), map_text=MAP_A2)
    h = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18), engine=eng)
    out = ok(h.handle(cmd))
    golden(name + ".txt", out)
    golden(name + ".engine.txt", engine_digest(eng))
    return h, out, fake


def test_reward(monkeypatch, tmp_path):
    h, out, fake = run(monkeypatch, tmp_path, screen("card_reward_a2"), "reward --attempts 16", "reward")
    assert h.reward_screen == ("A2 F19", ("Hemokinesis", "Cruelty", "Stone Armor"))
    assert h.priced["reward"] == "A2 F19"
    e = [x for x in events(h) if x["kind"] == "reward_eval"][-1]
    assert e["options"] == ["Hemokinesis", "Cruelty", "Stone Armor"] and e["boss"] == ["KNOWLEDGE_DEMON_BOSS"]
    golden("reward.event.json", json.dumps(dict(result=e["result"]), sort_keys=True, indent=0) + "\n")


def test_reward_unmapped_card(monkeypatch, tmp_path):
    s = screen("card_reward_a2").replace("1 Cruelty(1)", "1 Mystery Card(1)")
    h, out, fake = run(monkeypatch, tmp_path, s, "reward --attempts 16", "reward_unmapped")
    assert "not evaluated (no simulator id for the display name): Mystery Card" in out


def test_rmcalc(monkeypatch, tmp_path):
    run(monkeypatch, tmp_path, screen("shop_a2"), "rmcalc --attempts 16 --hp 50", "rmcalc")


def test_eval_variants(monkeypatch, tmp_path):
    h, out, fake = run(monkeypatch, tmp_path, screen("shop_a2"), 'eval --boss --smooth --attempts 16 --v "+setup|add=SETUP_STRIKE" --v "-strike|remove=STRIKE_IRONCLAD"', "eval_smooth")
    e = [x for x in events(h) if x["kind"] == "eval"][-1]
    assert e["spec"]["_ctx"] == dict(seen=["TUNNELER_WEAK", "EXOSKELETONS_WEAK", "OVICOPTER_NORMAL", "MYTES_NORMAL", "LOUSE_PROGENITOR_NORMAL"], bosses=["KNOWLEDGE_DEMON_BOSS"])
    assert e["spec"]["hold"] == "all" and e["spec"]["encounters"] == ["KNOWLEDGE_DEMON_BOSS"]
    golden("eval_smooth.event.json", json.dumps(e["result"], sort_keys=True, indent=0) + "\n")


def test_table_seed(monkeypatch, tmp_path):
    h, out, fake = run(monkeypatch, tmp_path, screen("shop_a2"), "eval --boss --attempts 8", "eval_seed0")
    s0 = h.engine.table_seed
    ok(h.handle("eval --boss --attempts 8"))
    assert h.engine.table_seed == s0
    ok(h.handle("eval --boss --attempts 8 --seed 3"))
    assert h.engine.table_seed == s0 + 3


def test_eval_pools(monkeypatch, tmp_path):
    run(monkeypatch, tmp_path, screen("shop_a2"), 'eval --elites --attempts 8 --hp full --v "up|upgrade=BASH"', "eval_elites")
    run(monkeypatch, tmp_path, screen("shop_a2"), "eval --next --attempts 8", "eval_next")
    run(monkeypatch, tmp_path, screen("shop_a2"), "eval --future --attempts 8", "eval_future")
    run(monkeypatch, tmp_path, screen("shop_a2"), "eval --pool Hive:regular:3 --attempts 8 --hp 40", "eval_pool")
    run(monkeypatch, tmp_path, screen("shop_a2"), "eval --pool Hive:elite --all --attempts 8", "eval_pool_all")
    run(monkeypatch, tmp_path, screen("shop_a2"), 'eval --enc NIBBITS_WEAK,KNOWLEDGE_DEMON_BOSS --attempts 8 --v "pots|potions=FIRE_POTION,BLOCK_POTION"', "eval_enc")
    run(monkeypatch, tmp_path, screen("shop_a2"), json.dumps(dict(encounters=["MYTES_NORMAL"], variants=[dict(name="b")], attempts=8)).join(["eval ", ""]), "eval_json")


def test_route(monkeypatch, tmp_path):
    h, out, fake = run(monkeypatch, tmp_path, screen("shop_a2"), "route M E R S ? B --hp 40 --act Hive --attempts 8", "route")
    assert h.priced["route"] == "A2 F23"
    run(monkeypatch, tmp_path, screen("shop_a2"), "route W M --exclude NIBBITS_WEAK --attempts 8", "route_default_act")


def test_routes(monkeypatch, tmp_path):
    h, out, fake = run(monkeypatch, tmp_path, MAP_SCREEN_A2, "routes --attempts 8", "routes")
    assert h.priced["routes"] == "A2 F16"
    run(monkeypatch, tmp_path, MAP_SCREEN_A2, "routes --attempts 8 --hp 70 --pf 0.3 --w E=4,M=1.5", "routes_whatif")
    run(monkeypatch, tmp_path, screen("shop_a2"), "routes --attempts 8", "routes_offscreen")


def test_pickplan(monkeypatch, tmp_path):
    s = screen("card_reward_a2").replace("1 Cruelty(1)", "1 Mystery Card(1)").replace("2 Stone Armor(1)", "2 Bash(2)")
    h, out, fake = run(monkeypatch, tmp_path, s, "pickplan --attempts 8 --screens 2 --slots 4", "pickplan")
    assert "Bash +nan skip" in out


def test_brief(monkeypatch, tmp_path):
    run(monkeypatch, tmp_path, screen("shop_a2"), "brief", "brief")


def test_context_read_once_per_command(monkeypatch, tmp_path):
    from agent import runctx
    reads = []
    real = runctx.encounters_met
    monkeypatch.setattr(runctx, "encounters_met", lambda path, act: reads.append(act) or real(path, act))
    fake = FakeBridge(screen("shop_a2"), deck_json=deck(), map_text=MAP_A2, on_action=[screen("shop_a2")])
    h = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18))
    ok(h.handle("eval --boss --smooth --attempts 4 --v x|add=BASH"))
    ok(h.handle('eval --elites --next --future --attempts 4 --v "x|add=BASH"'))
    assert fake.calls.count("m") == 2 and reads == [1, 1]
    ok(h.handle("routes --attempts 4"))
    assert fake.calls.count("m") == 3 and len(reads) == 3
    h._context()
    h._send("a 0")
    assert h._rc is None


def test_context_logs_bridge_errors(monkeypatch, tmp_path):
    outs = []
    for i, m in enumerate(("no map\n", "ERR bridge down: the game is not running or the mod is not loaded\n")):
        fake = FakeBridge(screen("shop_a2"), deck_json=deck(), map_text=m)
        h = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18), run_id=f"r{i}")
        outs.append((ok(h.handle("brief")), ok(h.handle("eval --boss --attempts 4"))))
        errs = [e for e in events(h) if e["kind"] == "harness_error"]
        assert len(errs) == 2 * i, errs
    assert outs[0] == outs[1]
    assert errs[0]["where"] == "context: map" and "ERR bridge down" in errs[0]["error"]
    fake = FakeBridge("ERR bridge connection lost (OSError)\n", deck_json=deck(), map_text="no map\n")
    h = make_harness(monkeypatch, tmp_path, fake, run_id="r3")
    assert h._cur_act(dict(bosses=[])) == 0
    assert [e["where"] for e in events(h) if e["kind"] == "harness_error"] == ["context: act"]


def test_context_from_record(monkeypatch, tmp_path):
    fake = FakeBridge(screen("shop_a2"), deck_json=deck(), map_text=MAP_A2.replace("KNOWLEDGE_DEMON_BOSS", "KNOWLEDGE_DEMON_BOSS + KAISER_CRAB_BOSS"))
    evs = fight_starts(upto=18) + fight_starts(upto=15)[-1:]
    h = make_harness(monkeypatch, tmp_path, fake, events=evs)
    ctx = h._ctx()
    assert ctx == dict(seen=["TUNNELER_WEAK", "EXOSKELETONS_WEAK", "OVICOPTER_NORMAL", "MYTES_NORMAL", "LOUSE_PROGENITOR_NORMAL"], bosses=["KNOWLEDGE_DEMON_BOSS", "KAISER_CRAB_BOSS"])
    hz = h._horizon()
    assert hz["boss"] == ["KNOWLEDGE_DEMON_BOSS", "KAISER_CRAB_BOSS"]
    assert sorted(hz["elites"]) == ["DECIMILLIPEDE_ELITE", "ENTOMANCER_ELITE", "INFESTED_PRISMS_ELITE"]
    assert hz["next"] == ["KNIGHTS_ELITE", "MECHA_KNIGHT_ELITE", "SOUL_NEXUS_ELITE", "QUEEN_BOSS", "TEST_SUBJECT_BOSS", "AEONGLASS_BOSS"]
    fake.map = "no map\n"
    fake.screen = screen("shop")
    h2 = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=8), run_id="a1")
    hz = h2._horizon()
    assert hz["ctx"] == dict(seen=["NIBBITS_WEAK", "SHRINKER_BEETLE_WEAK", "FUZZY_WURM_CRAWLER_WEAK", "PHROG_PARASITE_ELITE", "BYRDONIS_ELITE"], bosses=[])
    assert len(hz["boss"]) == 6 and "VANTOM_BOSS" in hz["boss"] and "WATERFALL_GIANT_BOSS" in hz["boss"]
    fut = h2._future_encounters()
    assert "PHROG_PARASITE_ELITE" not in fut and "BYRDONIS_ELITE" not in fut
    assert {"BYGONE_EFFIGY_ELITE", "TERROR_EEL_ELITE", "VANTOM_BOSS", "KAISER_CRAB_BOSS", "AEONGLASS_BOSS", "SOUL_NEXUS_ELITE"} <= set(fut)
    assert len(fut) == 1 + 3 + 6 + 6 + 6
    fake.map = "boss: 16 VANTOM_BOSS\n"
    h3 = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=8), run_id="a1b")
    hz = h3._horizon()
    assert hz["boss"] == ["VANTOM_BOSS"] and sorted(hz["elites"]) == ["BYGONE_EFFIGY_ELITE"]
    fake.screen = "REWARDS\n0 proceed\n"
    fake.map = MAP_A2
    h4 = make_harness(monkeypatch, tmp_path, fake, events=fight_starts(upto=18), run_id="a2")
    assert h4._cur_act(dict(bosses=["KNOWLEDGE_DEMON_BOSS"])) == 1
    assert h4._ctx()["seen"][0] == "TUNNELER_WEAK"
