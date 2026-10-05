"""`python -m agent <command>`: the client of the harness daemon (starts it when it is not running); `python -m agent serve` runs the daemon.

The daemon keeps the networks and the aligned simulator in memory, so each command is quick once it is up (loading takes about a minute).
Commands: see `agent/harness.py`.
"""
import os
import socket
import subprocess
import sys
import threading
import time

PORT = int(os.environ.get("STS2_AGENT_PORT", 15556))


def serve():
    from agent.harness import Harness
    h = Harness()
    h.gate = os.environ.get("STS2_SKILL_GATE", "").lower() != "off"  # no game action before the governing skills are loaded (agent/skillgate.py)
    lock = threading.Lock()
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", PORT))
    srv.listen(4)
    threading.Thread(target=h.eng, daemon=True).start()  # load the networks while the first decisions are made
    print(f"agent harness on 127.0.0.1:{PORT}", flush=True)

    def conn(c):
        try:
            _conn(c)
        except Exception as e:  # noqa: BLE001  one bad request must not take the daemon down
            try:
                c.sendall(f"ERR daemon: {type(e).__name__}: {e}\n".encode())
            except OSError:
                pass

    def _conn(c):
        with c:
            data = b""
            while not data.endswith(b"\n"):
                b = c.recv(65536)
                if not b:
                    break
                data += b
            line = data.decode().strip()
            if line == "quit":
                c.sendall(b"bye\n")
                os._exit(0)
            with lock:
                out = h.handle(line)
            c.sendall(out.encode())

    while True:
        c, _ = srv.accept()
        threading.Thread(target=conn, args=(c,), daemon=True).start()


def ask(line, timeout=900):
    with socket.create_connection(("127.0.0.1", PORT), timeout=timeout) as s:
        s.sendall((line + "\n").encode())
        chunks = []
        while True:
            b = s.recv(65536)
            if not b:
                break
            chunks.append(b)
    return b"".join(chunks).decode()


def up():
    try:
        socket.create_connection(("127.0.0.1", PORT), timeout=1).close()
        return True
    except OSError:
        return False


def start_daemon():
    env = dict(os.environ, STS2_DEVICE=os.environ.get("STS2_DEVICE", "cuda"), PYTHONUNBUFFERED="1")
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    os.makedirs(os.path.join(root, "target"), exist_ok=True)
    log = open(os.path.join(root, "target", "agent.log"), "a")
    # no console window: run under pythonw.exe (when present) with CREATE_NO_WINDOW; output goes to target/agent.log
    exe = sys.executable
    pw = os.path.join(os.path.dirname(exe), "pythonw.exe")
    if os.name == "nt" and os.path.exists(pw):
        exe = pw
    flags = getattr(subprocess, "CREATE_NO_WINDOW", 0) | getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
    subprocess.Popen([exe, "-m", "agent", "serve"], cwd=root, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log, creationflags=flags, close_fds=True)
    for _ in range(120):
        time.sleep(1)
        if up():
            return
    raise SystemExit("the harness daemon did not start; see target/agent.log")


def batch(lines, keep_going=False):
    """`python -m agent - <<'EOF' ... EOF`: one command per line, run in order, each output printed under `>>> command`. Blank lines and `#` comments are skipped.
    A line whose output starts with ERR / REFUSED / `[chain stopped` ends the batch (the later lines assumed the screen it was meant to leave), unless --keep-going.
    The text is literal, so `-- why` needs no shell quoting; read-only commands (`s`, `eval`, `brief` ...) can share one call with the decision that follows them."""
    if not up():
        start_daemon()
    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        print(">>> " + line)
        out = ask(line)
        print(out, end="" if out.endswith(chr(10)) else chr(10))
        if not keep_going and (out.startswith(("ERR", "REFUSED")) or "[chain stopped" in out):
            print(">>> batch stopped here")
            return


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    if len(sys.argv) > 1 and sys.argv[1] == "serve":
        return serve()
    if sys.argv[1:2] == ["-"]:
        return batch(sys.stdin.read().splitlines(), keep_going="--keep-going" in sys.argv[2:])
    line = " ".join(sys.argv[1:]) or "s"
    if not up():
        if line == "quit":
            return
        start_daemon()
    try:
        print(ask(line), end="")
    except (ConnectionError, OSError) as e:
        if line == "quit":
            print("bye")
            return
        # the daemon died under this command: restart it; repeat only commands that change nothing in the game
        if line.split()[0] in ("s", "d", "p", "m", "adv", "status", "relics", "budget", "route", "eval"):
            start_daemon()
            print(ask(line), end="")
        else:
            print(f"ERR the harness daemon dropped the connection ({type(e).__name__}); the command may have been applied. Run `s` (restarts the daemon) before anything else.")


if __name__ == "__main__":
    main()
