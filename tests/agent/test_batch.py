"""Batch mode (`python -m agent - <<EOF`): stop rules, the bare-number refusal, read-only commands; the read-only command lists."""
import contextlib
import io

import agent.__main__ as cli
from agent import bridge, skillgate


def run_batch(monkeypatch, replies, lines, keep_going=False):
    sent = []

    def ask(line, timeout=900):
        sent.append(line)
        r = replies.get(line.split()[0], "ok\n") if isinstance(replies, dict) else replies.pop(0)
        return r(line) if callable(r) else r
    monkeypatch.setattr(cli, "up", lambda: True)
    monkeypatch.setattr(cli, "start_daemon", lambda: (_ for _ in ()).throw(AssertionError("no daemon in tests")))
    monkeypatch.setattr(cli, "ask", ask)
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        cli.batch(lines, keep_going=keep_going)
    return sent, buf.getvalue()


def test_stops():
    assert cli._stops("ERR x")
    assert cli._stops("  play\nREFUSED: y")
    assert cli._stops("  Strike\nPOTION ALERT (turn 2): throwing now")
    assert cli._stops("x\nSIMULATOR DESYNC (e)")
    assert cli._stops("x\nSIMULATOR DIFFERS FROM THE GAME: y")
    assert cli._stops("x\nSIMULATOR CHOICE DIFFERS: z")
    assert not cli._stops("COMBAT\n  ERR inside a line\n")
    assert not cli._stops("")


def test_batch_runs_and_skips_comments(monkeypatch):
    sent, out = run_batch(monkeypatch, {"s": "STATE\n", "brief": "BRIEF"}, ["s", "", "# a comment", "  brief  "])
    assert sent == ["s", "brief"]
    assert out == ">>> s\nSTATE\n>>> brief\nBRIEF\n"


def test_batch_stop_rules(monkeypatch):
    for reply in ("ERR bad\n", "REFUSED: no\n", "SHOP\n[chain stopped before `x`: COMBAT]\n", "  Strike\nPOTION ALERT (turn 1): y\n", "  a\nSIMULATOR DESYNC (q)\n"):
        sent, out = run_batch(monkeypatch, {"a": reply}, ["a 0", "s"])
        assert sent == ["a 0"], reply
        assert out.endswith(">>> batch stopped here\n")
    sent, out = run_batch(monkeypatch, {"a": "ERR bad\n"}, ["a 0", "s"], keep_going=True)
    assert sent == ["a 0", "s"]


def test_batch_bare_number_after_action(monkeypatch):
    sent, out = run_batch(monkeypatch, {"a": "SHOP\n"}, ["a ~gold", "a 3 -- why"])
    assert sent == ["a ~gold"]
    assert out == (">>> a ~gold\nSHOP\n>>> a 3 -- why\nREFUSED: a numbered option after an earlier action in the same batch: the screen has changed and the number may now be another "
                   "option. Use `a ~text`, or send it in its own call after reading the screen.\n>>> batch stopped here\n")
    sent, out = run_batch(monkeypatch, {"s": "S\n", "eval": "table\n", "a": "SHOP\n"}, ["s", "eval --boss", "a 3"])
    assert sent == ["s", "eval --boss", "a 3"]  # read-only commands do not count as an action
    sent, out = run_batch(monkeypatch, {"a": "REFUSED: x\n"}, ["a 2", "a 3"], keep_going=True)
    assert sent == ["a 2", "a 3"]  # a refused action changed nothing
    sent, out = run_batch(monkeypatch, {"a": "SHOP\n", "hold": "held\n"}, ["hold X", "a 3", "a ~x", "a 1"])
    assert sent == ["hold X", "a 3", "a ~x"]
    sent, out = run_batch(monkeypatch, {"a": "SHOP\n", "combat": "played\n"}, ["combat", "a 3"])
    assert sent == ["combat"]
    sent, out = run_batch(monkeypatch, {"a": "SHOP\n"}, ["a ~x", "a ~y; 3", "a 12x"])
    assert sent == ["a ~x", "a ~y; 3", "a 12x"]  # a later step of a chain is the harness's check; `12x` is not a bare number


def test_read_only_lists():
    assert set(bridge.READ_ONLY) == skillgate.BRIDGE_READ_ONLY
    assert cli.READ_ONLY is skillgate.READ_ONLY
    for c in ("s", "peek", "brief", "eval", "reward", "route", "routes", "rmcalc", "pickplan", "adv", "m", "d", "p", "status", "relics", "hold", "budget", "note", "potions"):
        assert c in skillgate.READ_ONLY, c
    for c in ("a", "turn", "combat", "x", "draw", "f", "do"):
        assert c not in skillgate.READ_ONLY, c
    assert "f" in bridge.READ_ONLY  # safe to resend after a dropped connection; still an action for the skill gate


def test_classify():
    assert skillgate.classify("ls") == "none"
    assert skillgate.classify(".venv/Scripts/python.exe -m agent s") == "read"
    assert skillgate.classify(".venv/Scripts/python.exe -m agent a 0 -- why") == "act"
    assert skillgate.classify("python -m agent - <<'EOF'\ns\nbrief\nEOF") == "read"
    assert skillgate.classify("python -m agent - <<'EOF'\ns\na 1\nEOF") == "act"
