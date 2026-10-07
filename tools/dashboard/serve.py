"""Live dashboard server: `python tools/dashboard/serve.py [--port 8777] [--runs DIR] [--assets DIR]`, then open http://localhost:8777

Parasitic and read-only: it serves `index.html`, the art cache (`target/dashboard/assets`, see `extract_assets.py`) and the run record files the
harness writes anyway. It never talks to the game bridge (15555) or the harness daemon (15556). Each request is a stat and, only when the file
grew or changed, a read of the new bytes; the page polls about once a second.

  GET /                          the page
  GET /api/runs                  run ids, newest first, and the current one (runs/CURRENT)
  GET /api/live?run=ID           runs/<ID>/live.json (304 when unchanged: send `If-None-Match`)
  GET /api/events?run=ID&from=N  complete lines of runs/<ID>/events.jsonl from byte N: {"run", "start", "next", "size", "events": [...]}
                                 (a line still being written waits for the next poll; start < N means the file was replaced)
  GET /api/fights?run=ID         the saved fight exports (runs/<ID>/fights/*.json): names only
  GET /api/fight?run=ID&name=F   one export, without its per-action `states` list unless `&states=1`
  GET /assets/<path>             the art cache
"""
import argparse
import json
import os
import re
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, unquote, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
RUN_ID = re.compile(r"^[\w.-]{1,80}$")
CHUNK = 1 << 20  # at most 1 MB of events per reply; the page asks again from `next`
TYPES = {".html": "text/html; charset=utf-8", ".js": "text/javascript; charset=utf-8", ".json": "application/json", ".css": "text/css",
         ".png": "image/png", ".webp": "image/webp", ".jpg": "image/jpeg", ".atlas": "text/plain; charset=utf-8", ".skel": "application/octet-stream",
         ".txt": "text/plain; charset=utf-8", ".svg": "image/svg+xml"}


class Handler(BaseHTTPRequestHandler):
    runs = os.path.join(ROOT, "runs")
    assets = os.path.join(ROOT, "target", "dashboard", "assets")
    page = os.path.join(HERE, "index.html")

    def log_message(self, *a):  # quiet: one line per poll would flood the terminal
        pass

    # ------------------------------------------------------------ replies

    def _send(self, code, body=b"", ctype="application/json", headers=None):
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-cache")
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _json(self, obj, code=200):
        self._send(code, json.dumps(obj, default=str).encode())

    def _file(self, path, cache=False):
        try:
            st = os.stat(path)
        except OSError:
            return self._send(404, b"not found", "text/plain")
        etag = f'"{st.st_mtime_ns:x}-{st.st_size:x}"'
        if self.headers.get("If-None-Match") == etag:
            return self._send(304, headers={"ETag": etag})
        with open(path, "rb") as f:
            body = f.read()
        h = {"ETag": etag}
        if cache:
            h["Cache-Control"] = "max-age=86400"
        self.send_response(200)
        self.send_header("Content-Type", TYPES.get(os.path.splitext(path)[1].lower(), "application/octet-stream"))
        self.send_header("Content-Length", str(len(body)))
        for k, v in h.items():
            self.send_header(k, v)
        if not cache:
            self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    # ------------------------------------------------------------ run record

    def _current(self):
        try:
            with open(os.path.join(self.runs, "CURRENT"), encoding="utf-8") as f:
                cur = f.read().strip()
            return cur if RUN_ID.match(cur) else None
        except OSError:
            return None

    def _run_dir(self, q):
        run = (q.get("run") or [""])[0] or self._current()
        if not run or not RUN_ID.match(run):
            return None, None
        d = os.path.join(self.runs, run)
        return (run, d) if os.path.isdir(d) else (run, None)

    def _events(self, q, run, d):
        p = os.path.join(d, "events.jsonl")
        try:
            start = max(0, int((q.get("from") or ["0"])[0]))
        except ValueError:
            start = 0
        try:
            size = os.path.getsize(p)
        except OSError:
            return self._json(dict(run=run, start=0, next=0, size=0, events=[]))
        if start > size:  # the file was replaced (a new record under the same id): start over
            start = 0
        events, nxt = [], start
        if size > start:
            with open(p, "rb") as f:
                f.seek(start)
                buf = f.read(min(size - start, CHUNK))
            end = buf.rfind(b"\n") + 1  # complete lines only; a line still being written waits for the next poll
            if end == 0 and len(buf) == CHUNK:  # one line longer than a chunk: read it whole
                with open(p, "rb") as f:
                    f.seek(start)
                    buf = f.readline()
                end = len(buf) if buf.endswith(b"\n") else 0
            for line in buf[:end].splitlines():
                try:
                    events.append(json.loads(line))
                except ValueError:
                    events.append(dict(kind="unparsed", text=line[:200].decode("utf-8", "replace")))
            nxt = start + end
        return self._json(dict(run=run, start=start, next=nxt, size=size, events=events))

    def _runs(self):
        try:
            ids = sorted((n for n in os.listdir(self.runs) if RUN_ID.match(n) and os.path.isdir(os.path.join(self.runs, n))), reverse=True)
        except OSError:
            ids = []
        return self._json(dict(current=self._current(), runs=ids[:200]))

    def _fights(self, run, d):
        fd = os.path.join(d, "fights")
        names = sorted(n for n in os.listdir(fd) if n.endswith(".json")) if os.path.isdir(fd) else []
        return self._json(dict(run=run, fights=names))

    def _fight(self, q, d):
        name = (q.get("name") or [""])[0]
        if not re.match(r"^[\w.-]+\.json$", name):
            return self._send(400, b"bad name", "text/plain")
        p = os.path.join(d, "fights", name)
        try:
            with open(p, encoding="utf-8") as f:
                data = json.load(f)
        except (OSError, ValueError):
            return self._send(404, b"not found", "text/plain")
        if (q.get("states") or ["0"])[0] != "1" and isinstance(data.get("fight"), dict):
            data["fight"].pop("states", None)
        return self._json(data)

    # ------------------------------------------------------------ routing

    def do_HEAD(self):
        self.do_GET()

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        path = unquote(u.path)
        try:
            if path in ("/", "/index.html"):
                return self._file(self.page)
            if path.startswith("/assets/"):
                rel = os.path.normpath(path[len("/assets/"):]).replace("\\", "/")
                if rel.startswith("..") or os.path.isabs(rel) or ":" in rel:
                    return self._send(403, b"forbidden", "text/plain")
                return self._file(os.path.join(self.assets, rel), cache=True)
            if path == "/api/runs":
                return self._runs()
            if path.startswith("/api/"):
                run, d = self._run_dir(q)
                if d is None:
                    return self._json(dict(run=run, error="no run record"), 404)
                if path == "/api/live":
                    return self._file(os.path.join(d, "live.json"))
                if path == "/api/events":
                    return self._events(q, run, d)
                if path == "/api/fights":
                    return self._fights(run, d)
                if path == "/api/fight":
                    return self._fight(q, d)
            return self._send(404, b"not found", "text/plain")
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
            pass


def make_server(port=8777, runs=None, assets=None, host="127.0.0.1"):
    """The server (not started). Tests use port 0 and a fixture runs directory."""
    attrs = {}
    if runs:
        attrs["runs"] = os.path.abspath(runs)
    if assets:
        attrs["assets"] = os.path.abspath(assets)
    handler = type("DashboardHandler", (Handler,), attrs)
    srv = ThreadingHTTPServer((host, port), handler)
    srv.daemon_threads = True
    return srv


def main(argv=None):
    ap = argparse.ArgumentParser(description="STS2 live dashboard (read-only)")
    ap.add_argument("--port", type=int, default=8777)
    ap.add_argument("--runs", default=None, help="run records directory (default: <repo>/runs)")
    ap.add_argument("--assets", default=None, help="art cache (default: <repo>/target/dashboard/assets)")
    a = ap.parse_args(argv)
    srv = make_server(a.port, a.runs, a.assets)
    h = srv.RequestHandlerClass
    print(f"dashboard on http://localhost:{srv.server_address[1]}  (runs: {h.runs}; assets: {h.assets}"
          + ("" if os.path.exists(os.path.join(h.assets, "manifest.json")) else " -- missing: run tools/dashboard/extract_assets.py") + ")", flush=True)
    try:
        srv.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    sys.exit(main())
