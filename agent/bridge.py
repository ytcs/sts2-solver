#!/usr/bin/env python3
"""Client for the AgentBridge mod (mods/AgentBridge): one command per call, plain-text reply.

  python -m agent.bridge s              current state + numbered legal actions
  python -m agent.bridge a 3            take action 3, wait for the game to settle, print the new state
  python -m agent.bridge a 3 1          action 3 aimed at target 1 (combat: enemy index)
  python -m agent.bridge a pick 0 2     answer a card-choice prompt with options 0 and 2
  python -m agent.bridge d | p draw | m full deck / a pile / the whole map
  python -m agent.bridge x fight CULTISTS_NORMAL   dev-console command
"""
import os, socket, sys, time

PORT = int(os.environ.get("STS2_BRIDGE_PORT", 15555))
# commands that change nothing: safe to repeat when the connection drops
READ_ONLY = ("s", "peek", "d", "p", "m", "fight", "snap", "deck.json", "mods", "f")


def _once(line, port, timeout):
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as s:
        s.sendall((line + "\n").encode())
        chunks = []
        while True:
            b = s.recv(65536)
            if not b:
                break
            chunks.append(b)
    return b"".join(chunks).decode()


def call(line, port=PORT, timeout=120):
    """One bridge command. A refused connection (the game is starting, loading a scene or briefly stuck) is retried for ~20 s; a dropped connection is retried only for
    read-only commands (an action may already have been applied). Failures come back as an `ERR bridge ...` reply instead of a traceback."""
    read_only = (line.split() or ["s"])[0] in READ_ONLY
    deadline = time.time() + 20
    while True:
        try:
            return _once(line, port, timeout)
        except ConnectionRefusedError:
            if time.time() > deadline:
                return "ERR bridge down: the game is not running or the mod is not loaded (bash mods/AgentBridge/dev.sh relaunches it; the run resumes from its save)\n"
        except (ConnectionError, socket.timeout, OSError) as e:
            if not read_only or time.time() > deadline:
                return f"ERR bridge connection lost ({type(e).__name__}); the command may or may not have been applied: read the state with `s` before anything else\n"
        time.sleep(1)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    print(call(" ".join(sys.argv[1:]) or "s"), end="")
