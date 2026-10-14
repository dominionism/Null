"""The bench's sites, served on this Mac only, and what each run did on them.

    python3 Mini/Bench/server.py --port 8790

Six made-up sites on one address, a path each: /mail, /docs, /chat, /shop,
/calendar and /settings, and /home, a page of the person's own that stands for
the tab they are looking at. Each site's page asks `/api/<site>/<what>` to see
something and posts to `/api/<site>/<action>` to do something; the world they
work on is `world.World`, made afresh for each run.

A site is signed in only in a browser that has been to `/__bench/signin` with the
server's key: the bench's own Chrome, set up by the runner. Any other browser,
the harness's own among them, gets the sign-in page, as it would on a real site.

The runner, and only the runner, holds the key for `/__bench/*`: it starts a run
(`reset`, with the task's troubles), reads how it ended (`end`), and signs its
Chrome in. Two of those need no key: `/__bench/open`, which opens an address in
the bench's Chrome for the `open` stand-in, and `/__bench/seen`, where the
person's own tab says whether it is still in view.
"""

import argparse
import json
import mimetypes
import os
import secrets
import threading
import time
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from world import World

HERE = os.path.dirname(os.path.abspath(__file__))
PAGES = os.path.join(HERE, "Pages")
SITES = ("mail", "docs", "chat", "shop", "calendar", "settings")
COOKIE = "bench_session"


class Bench:
    """What the server holds between requests: the run's world and its record."""

    def __init__(self, key, chrome_port=None):
        self.key = key
        self.session = secrets.token_hex(16)
        self.lock = threading.Lock()
        self.world = World()
        self.task = None
        self.slow = {}
        self.popups = []
        self.seen = []
        self.opened = []
        self.chrome_port = chrome_port

    def reset(self, task, trouble, base=None):
        import datetime
        with self.lock:
            day = datetime.date.fromisoformat(base) if base else None
            self.world = World(base=day, trouble={k: trouble[k] for k in ("expire_after", "fail_once") if k in trouble})
            self.task = task
            self.slow = trouble.get("slow", {})
            self.popups = list(trouble.get("popup", []))
            self.seen, self.opened = [], []


def make_handler(bench):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *args):
            pass

        def send(self, status, body, kind="application/json", headers=()):
            data = body if isinstance(body, bytes) else json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            for name, value in headers:
                self.send_header(name, value)
            self.end_headers()
            self.wfile.write(data)

        def signed_in(self):
            return f"{COOKIE}={bench.session}" in (self.headers.get("Cookie") or "")

        def runner(self, query):
            return secrets.compare_digest(query.get("key", ""), bench.key)

        def body(self):
            n = int(self.headers.get("Content-Length") or 0)
            return json.loads(self.rfile.read(n) or b"{}") if n else {}

        def page(self, name):
            with open(os.path.join(PAGES, name), "rb") as f:
                self.send(200, f.read(), "text/html; charset=utf-8")

        def do_GET(self):
            url = urllib.parse.urlparse(self.path)
            query = dict(urllib.parse.parse_qsl(url.query))
            parts = [p for p in url.path.split("/") if p]
            if not parts:
                return self.send(302, b"", headers=[("Location", "/home/")])
            if parts[0] == "__bench":
                return self.control(parts[1:], query, None)
            if parts[0] == "static" and len(parts) == 2 and "/" not in parts[1] and not parts[1].startswith("."):
                path = os.path.join(PAGES, parts[1])
                if os.path.isfile(path):
                    with open(path, "rb") as f:
                        return self.send(200, f.read(), mimetypes.guess_type(path)[0] or "application/octet-stream")
                return self.send(404, {"error": "not found"})
            if parts[0] == "home":
                return self.page("home.html")
            if parts[0] in SITES and len(parts) == 1:
                if not self.signed_in() or bench.world.signed_out:
                    return self.page("signin.html")
                return self.page(f"{parts[0]}.html")
            if parts[0] == "api" and len(parts) >= 3 and parts[1] in SITES:
                return self.api_view(parts[1], parts[2], query)
            if parts[0] in SITES and len(parts) >= 2 and parts[1] == "export" and parts[0] == "docs":
                # A document's own download address, as an online editor has one.
                if not self.signed_in():
                    return self.send(302, b"", headers=[("Location", "/docs")])
                with bench.lock:
                    status, got = bench.world.view("docs", "export", {"id": query.get("id")})
                if status != 200:
                    return self.send(status, got)
                return self.send(200, got["text"].encode(), "text/plain; charset=utf-8", [("Content-Disposition", f'inline; filename="{got["title"]}.txt"')])
            return self.send(404, {"error": "not found"})

        def do_POST(self):
            url = urllib.parse.urlparse(self.path)
            query = dict(urllib.parse.parse_qsl(url.query))
            parts = [p for p in url.path.split("/") if p]
            try:
                body = self.body()
            except ValueError:
                return self.send(400, {"error": "not JSON"})
            if parts and parts[0] == "__bench":
                return self.control(parts[1:], query, body)
            if len(parts) == 3 and parts[0] == "api" and (parts[1] in SITES or parts[1] == "signin"):
                if parts[1] != "signin" and not self.signed_in():
                    return self.send(401, {"signed_out": True})
                with bench.lock:
                    status, got = bench.world.act(parts[1], parts[2], body)
                return self.send(status, got)
            return self.send(404, {"error": "not found"})

        def api_view(self, site, what, query):
            if not self.signed_in():
                return self.send(401, {"signed_out": True})
            delay = bench.slow.get(site)
            if delay and what in ("list", "state", "search"):
                time.sleep(float(delay))
            with bench.lock:
                status, got = bench.world.view(site, what, query)
                popup = site in bench.popups
                if popup and what in ("list", "state", "folders"):
                    bench.popups.remove(site)
            if status == 200 and isinstance(got, dict):
                got = dict(got, popup=popup)
            return self.send(status, got)

        def control(self, parts, query, body):
            what = parts[0] if parts else ""
            if what == "seen":
                bench.seen.append({"at": time.time(), **{k: query.get(k) for k in ("event", "state", "url")}})
                return self.send(204, b"")
            if what == "open":
                return self.open_in_chrome(query.get("url", ""), body)
            if not self.runner(query):
                return self.send(403, {"error": "the runner's key is needed"})
            if what == "signin":
                return self.send(302, b"", headers=[("Location", "/home/"), ("Set-Cookie", f"{COOKIE}={bench.session}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400")])
            if what == "reset" and body is not None:
                bench.reset(body.get("task"), body.get("trouble", {}), body.get("base"))
                return self.send(200, {"base": bench.world.base})
            if what == "end":
                with bench.lock:
                    return self.send(200, dict(bench.world.end(), seen=bench.seen, opened=bench.opened, task=bench.task))
            if what == "chrome" and body is not None:
                bench.chrome_port = body.get("port")
                return self.send(200, {})
            return self.send(404, {"error": "no such control"})

        def open_in_chrome(self, address, body):
            """Open an address in a new tab of the bench's Chrome and bring it forward, as
            `open -a "Google Chrome" <address>` does in a person's own."""
            address = address or (body or {}).get("url", "")
            if not address.startswith(("http://", "https://")) or not bench.chrome_port:
                bench.opened.append({"url": address, "ok": False})
                return self.send(400, {"error": "only a web address can be opened, in the bench's Chrome"})
            base = f"http://127.0.0.1:{bench.chrome_port}"
            try:
                # The whole address is encoded, or Chrome drops what follows a #.
                made = json.loads(urllib.request.urlopen(urllib.request.Request(f"{base}/json/new?{urllib.parse.quote(address, safe='')}", method="PUT"), timeout=10).read())
                urllib.request.urlopen(f"{base}/json/activate/{made['id']}", timeout=10).read()
            except OSError as e:
                bench.opened.append({"url": address, "ok": False})
                return self.send(502, {"error": f"the bench's Chrome did not answer: {e}"})
            bench.opened.append({"url": address, "ok": True})
            return self.send(200, {"opened": address})

    return Handler


def serve(port, key, chrome_port=None):
    bench = Bench(key, chrome_port)
    server = ThreadingHTTPServer(("127.0.0.1", port), make_handler(bench))
    server.daemon_threads = True
    return server, bench


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=8790)
    parser.add_argument("--key", default=os.environ.get("NULL_BENCH_KEY") or secrets.token_hex(16))
    args = parser.parse_args()
    server, _ = serve(args.port, args.key)
    print(f"the bench's sites on http://localhost:{args.port}/home/ (sign in a browser at /__bench/signin?key={args.key})", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
