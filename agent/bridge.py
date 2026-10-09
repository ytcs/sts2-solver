#!/usr/bin/env python3
import os, socket, sys, time

from agent.skillgate import BRIDGE_READ_ONLY

PORT = int(os.environ.get("STS2_BRIDGE_PORT", 15555))
READ_ONLY = tuple(sorted(BRIDGE_READ_ONLY))
OVERRIDE = None  # host:port set by one process driving several headless servers (tools/baseline.py); wins over STS2_BRIDGE


def endpoint():
    """OVERRIDE or STS2_BRIDGE=host:port (a headless `OracleCombat serve`), else the real game's bridge (STS2_BRIDGE_PORT, default 15555)."""
    ep = (OVERRIDE or os.environ.get("STS2_BRIDGE", "")).strip()
    if not ep:
        return "127.0.0.1", PORT, False
    host, _, port = ep.rpartition(":")
    return host or "127.0.0.1", int(port), True


def _once(line, addr, timeout):
    with socket.create_connection(addr, timeout=timeout) as s:
        s.sendall((line + "\n").encode())
        chunks = []
        while True:
            b = s.recv(65536)
            if not b:
                break
            chunks.append(b)
    return b"".join(chunks).decode()


def call(line, port=None, timeout=120):
    host, p, explicit = endpoint()
    if port is not None and not explicit:
        p = port
    read_only = (line.split() or ["s"])[0] in READ_ONLY
    deadline = time.time() + 20
    while True:
        try:
            return _once(line, (host, p), timeout)
        except ConnectionRefusedError:
            if explicit and time.time() > deadline:
                return f"ERR bridge down: STS2_BRIDGE={host}:{p} is not reachable (headless server not running); never falling back to the real game\n"
            if not explicit and time.time() > deadline:
                return "ERR bridge down: the game is not running or the mod is not loaded (bash mods/AgentBridge/dev.sh relaunches it; the run resumes from its save)\n"
        except (ConnectionError, socket.timeout, OSError) as e:
            if not read_only or time.time() > deadline:
                return f"ERR bridge connection lost ({type(e).__name__}); the command may or may not have been applied: read the state with `s` before anything else\n"
        time.sleep(1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    print(call(" ".join(sys.argv[1:]) or "s"), end="")
