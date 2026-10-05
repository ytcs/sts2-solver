#!/usr/bin/env python3
"""Client for the AgentBridge mod (mods/AgentBridge): one command per call, plain-text reply.

  python -m agent.bridge s              current state + numbered legal actions
  python -m agent.bridge a 3            take action 3, wait for the game to settle, print the new state
  python -m agent.bridge a 3 1          action 3 aimed at target 1 (combat: enemy index)
  python -m agent.bridge a pick 0 2     answer a card-choice prompt with options 0 and 2
  python -m agent.bridge d | p draw | m full deck / a pile / the whole map
  python -m agent.bridge x fight CULTISTS_NORMAL   dev-console command
"""
import os, socket, sys


def call(line, port=int(os.environ.get("STS2_BRIDGE_PORT", 15555)), timeout=120):
    with socket.create_connection(("127.0.0.1", port), timeout=timeout) as s:
        s.sendall((line + "\n").encode())
        chunks = []
        while True:
            b = s.recv(65536)
            if not b:
                break
            chunks.append(b)
    return b"".join(chunks).decode()


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    print(call(" ".join(sys.argv[1:]) or "s"), end="")
