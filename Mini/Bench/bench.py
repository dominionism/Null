#!/usr/bin/env python3
"""The bench: the agent on made-up signed-in sites, with Null's browser link and without it.

    python3 Mini/Bench/bench.py check                    # prove a run cannot reach your own Chrome; no model
    python3 Mini/Bench/bench.py run <task> --model <id>  # one task, in the link's arm
    python3 Mini/Bench/bench.py runs --half visible --model <id> [--arm link|plain] [--times 3]
    python3 Mini/Bench/bench.py report <results.jsonl>
    python3 -m unittest discover -s Mini/Bench           # the checkers, tried on right and wrong end states

A run starts the harness the way Null does, by its own command line, with Null's
rules for computer use, and in the link's arm the browser link's too, joined as
`rules.rs` joins them. It runs inside a sandbox (`isolation.py`), with a Chrome
of the bench's own (Chrome for Testing, its own profile, and a copy of Null's
extension made by Null itself), a relay of the bench's own on another port, and
the made-up sites served on this Mac (`server.py`). Afterwards it reads the
conversation's file for its numbers (`session.py`), judges what the sites
recorded (`grade.py`), notes what was left behind, and writes one line of JSON.

The two arms:
- `link`: Null's rules for computer use and for the browser link. Chrome for
  Testing runs headless: nothing is shown and the screen is not touched.
- `plain`: Null's rules for computer use only, with the harness's computer use
  on. The agent has no way into the sites' words but the window, so Chrome for
  Testing runs in a window on the screen and computer use takes pictures of it
  and presses in it. It takes the screen and the keyboard: run it only with
  `--screen` at a time the person has chosen, with their own Chrome quit.

In both arms the harness's own browser, which holds none of the person's
sign-ins, is a headless Chrome for Testing of the bench's own that the harness
attaches to (`browser.cdpUrl`): asked for a window, it still shows none.

Nothing names a provider or a model: `--model` is whatever the harness offers.
Results go to ~/Library/Application Support/null-bench/results unless
`NULL_BENCH_RESULTS` says otherwise, one line written the moment each run ends,
so that `--skip-done` picks up after any stop; Chrome for Testing and the runs'
scratch folders go to its `cache` beside it (`NULL_BENCH_CACHE`).

The task halves (`Tasks/Visible.json`, `Tasks/Hidden.json`) were drawn at random
once, before any run (`Tasks/Split.json` holds the seed). Work that improves the
link looks at the visible half only; the hidden half is run at the start and at
the end, and what is claimed is what it shows.
"""

import argparse
import datetime
import hashlib
import json
import os
import random
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.request
from urllib.parse import quote

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import grade  # noqa: E402
import isolation  # noqa: E402
import server  # noqa: E402
import session  # noqa: E402

MINI = os.path.dirname(HERE)
TASKS = os.path.join(HERE, "Tasks")
# Kept in the person's own Library, where a restart does not empty them as it empties /tmp.
BENCH_HOME = os.path.expanduser("~/Library/Application Support/null-bench")
CACHE = os.environ.get("NULL_BENCH_CACHE", os.path.join(BENCH_HOME, "cache"))
RESULTS = os.environ.get("NULL_BENCH_RESULTS", os.path.join(BENCH_HOME, "results"))
PAGES_PORT = 8790
RELAY_PORT = 9334  # never 9224, the port a person's own Chrome reaches its relay on
OWN_BROWSER_PORT = 9335  # the harness's own browser, which holds none of the person's sign-ins
CHROME_VERSION = "155.0.8059.79"
CHROME_SHA256 = "e141a7cbf948780bb7d381860bd5a7944c6736f79b9918b9826fccb199d28ea4"
CHROME_URL = f"https://storage.googleapis.com/chrome-for-testing-public/{CHROME_VERSION}/mac-arm64/chrome-mac-arm64.zip"
SITE_NAMES = {"mail": "Postbox mail", "docs": "Pages documents", "chat": "Huddle chat", "shop": "Shopwell shop account", "calendar": "calendar", "settings": "Lumen Cloud account settings"}
LIMIT = 0.8  # a batch stops before any usage window of the model's provider passes this


def say(*words):
    print(*words, flush=True)


# ── Tasks ───────────────────────────────────────────────────────────────────


def tasks(half=None):
    found = []
    for name in ("Visible", "Hidden"):
        if half in (None, name.lower()):
            with open(os.path.join(TASKS, f"{name}.json"), encoding="utf-8") as f:
                found += [dict(t, half=name.lower()) for t in json.load(f)]
    return found


def split(pool, seed):
    """The visible and the hidden half, drawn at random by the seed: within each tier, half each."""
    rng = random.Random(seed)
    visible, hidden = [], []
    for tier in sorted({t["tier"] for t in pool}):
        ids = sorted(t["id"] for t in pool if t["tier"] == tier)
        rng.shuffle(ids)
        hidden += ids[: len(ids) // 2]
        visible += ids[len(ids) // 2:]
    return sorted(visible), sorted(hidden)


def request(task, pages):
    """The request as the person would type it, and where their sites are, as they would know it."""
    where = " ".join(f"My {SITE_NAMES[s]} is at {pages}/{s}." for s in task["sites"])
    return f"{task['request']}\n\n({where})"


def rules(arm):
    """What Null tells the agent, joined as `rules.rs` joins it: computer use's rules, then the link's."""
    parts = ["Computer.md"] + (["Browser.md"] if arm == "link" else [])
    words = []
    for part in parts:
        with open(os.path.join(MINI, "Rules", part), encoding="utf-8") as f:
            words.append(f.read())
    return "\n".join(words)


# ── Chrome for Testing and the link's copy ──────────────────────────────────


def chrome_binary():
    return os.path.join(CACHE, f"chrome-{CHROME_VERSION}", "chrome-mac-arm64", "Google Chrome for Testing.app", "Contents", "MacOS", "Google Chrome for Testing")


def fetch_chrome():
    """Chrome for Testing, fetched once into the cache, kept only if its checksum is the one above."""
    binary = chrome_binary()
    if os.path.exists(binary):
        return binary
    folder = os.path.join(CACHE, f"chrome-{CHROME_VERSION}")
    os.makedirs(folder, exist_ok=True)
    archive = os.path.join(folder, "chrome-mac-arm64.zip")
    say(f"fetching Chrome for Testing {CHROME_VERSION}, about 190 MB")
    subprocess.run(["/usr/bin/curl", "--fail", "--location", "--silent", "--show-error", "--retry", "3", "--retry-all-errors", "--output", archive, CHROME_URL], check=True)
    with open(archive, "rb") as f:
        if hashlib.sha256(f.read()).hexdigest() != CHROME_SHA256:
            os.remove(archive)
            raise SystemExit("what was fetched is not the Chrome for Testing the bench names; it was not kept")
    subprocess.run(["/usr/bin/ditto", "-x", "-k", archive, folder], check=True)
    os.remove(archive)
    return binary


def link_copy(null_binary, omp, token):
    """Null's copy of the extension, made by Null itself, then pointed at the bench's relay.

    Two words change, each where it stands once: the port the extension looks for its relay on,
    so that it never reaches the relay a person's own Chrome may be linked to, and the token the
    bench's relay asks for, so that no other extension takes its place."""
    made = os.path.join(CACHE, "link-made")
    env = dict(os.environ, NULL_MINI_LINK_COPY=made, NULL_MINI_LINK_ENGINE=omp, NULL_MINI_FOLDER=os.path.join(CACHE, "null-folder"))
    done = subprocess.run([null_binary], env=env, capture_output=True, text=True)
    if done.returncode != 0:
        raise SystemExit(f"Null did not make its copy of the extension: {done.stderr.strip()}")
    copy = os.path.join(CACHE, "link-bench")
    shutil.rmtree(copy, ignore_errors=True)
    shutil.copytree(made, copy)
    path = os.path.join(copy, "background.js")
    with open(path, encoding="utf-8") as f:
        script = f.read()
    for old, new in [("var DEFAULT_PORT = 9224;", f"var DEFAULT_PORT = {RELAY_PORT};"),
                     ('chrome.storage.local.get({ port: DEFAULT_PORT, token: "" })', f'chrome.storage.local.get({{ port: DEFAULT_PORT, token: "{token}" }})')]:
        if script.count(old) != 1:
            raise SystemExit(f"this Oh-my-pi's extension is not the one the bench knows how to point at its relay: {old}")
        script = script.replace(old, new)
    with open(path, "w", encoding="utf-8") as f:
        f.write(script)
    return copy


# ── The world the runs happen in ────────────────────────────────────────────


def get_json(url, method="GET", body=None, timeout=10):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method, headers={"Content-Type": "application/json"} if data else {})
    with urllib.request.urlopen(req, timeout=timeout) as answer:
        raw = answer.read()
        return json.loads(raw) if raw else None


def listening(port):
    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


class Stage:
    """The bench's pages, its relay and its Chrome, for as long as a batch of runs lasts."""

    def __init__(self, args, arm):
        self.args, self.arm = args, arm
        self.key, self.token = secrets.token_hex(16), secrets.token_hex(16)
        self.pages = f"http://localhost:{PAGES_PORT}"
        self.processes = []

    def __enter__(self):
        for port in (PAGES_PORT, RELAY_PORT, OWN_BROWSER_PORT):
            if listening(port):
                raise SystemExit(f"port {port} is already in use; the bench needs it")
        os.makedirs(CACHE, exist_ok=True)
        os.makedirs(os.path.join(CACHE, "state", "omp"), exist_ok=True)
        self.server, self.bench = server.serve(PAGES_PORT, self.key)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        if self.arm == "link":
            copy = link_copy(self.args.null, self.args.omp, self.token)
            relay = subprocess.Popen([self.args.omp, "browser-relay", "--port", str(RELAY_PORT), "--token", self.token], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                     env=dict(os.environ, XDG_STATE_HOME=os.path.join(CACHE, "state")))
            self.processes.append(relay)
        profile = os.path.join(CACHE, f"profile-{self.arm}")
        shutil.rmtree(profile, ignore_errors=True)
        os.makedirs(profile)
        flags = [f"--user-data-dir={profile}", "--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check", "--disable-sync", "--password-store=basic", "--use-mock-keychain", "--disable-features=Translate"]
        if self.arm == "link":
            flags += [f"--load-extension={copy}", "--headless=new"]
        chrome = subprocess.Popen([fetch_chrome(), *flags, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.processes.append(chrome)
        # The harness's own browser: a Chrome with an empty profile, holding no sign-in, as the
        # harness's is. The harness is pointed at it (`browser.cdpUrl`) instead of launching one,
        # so it stays headless even when the agent asks for a window, and runs inside the sandbox's
        # reach on a port of its own.
        own = os.path.join(CACHE, "profile-own-browser")
        shutil.rmtree(own, ignore_errors=True)
        os.makedirs(own)
        self.processes.append(subprocess.Popen([fetch_chrome(), f"--user-data-dir={own}", f"--remote-debugging-port={OWN_BROWSER_PORT}", "--headless=new", "--no-first-run",
                                                "--no-default-browser-check", "--disable-sync", "--password-store=basic", "--use-mock-keychain", "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
        port_file = os.path.join(profile, "DevToolsActivePort")
        for _ in range(100):
            if os.path.exists(port_file) and open(port_file).read().strip():
                break
            time.sleep(0.1)
        self.devtools = f"http://127.0.0.1:{open(port_file).read().split()[0]}"
        get_json(f"{self.pages}/__bench/chrome?key={self.key}", "POST", {"port": int(self.devtools.rsplit(':', 1)[1])})
        # Signed in once, as a person is in their own Chrome.
        tab = get_json(f"{self.devtools}/json/new?{quote(f'{self.pages}/__bench/signin?key={self.key}', safe='')}", "PUT")
        time.sleep(3)
        urllib.request.urlopen(f"{self.devtools}/json/close/{tab['id']}").read()
        if self.arm == "link":
            for _ in range(120):
                try:
                    if get_json(f"http://127.0.0.1:{RELAY_PORT}/json/version").get("ompExtensionDiscardedTabsProtocol"):
                        break
                except OSError:
                    pass
                time.sleep(0.5)
            else:
                raise SystemExit("the bench's Chrome did not reach the bench's relay")
        return self

    def __exit__(self, *_):
        for p in reversed(self.processes):
            p.terminate()
            try:
                p.wait(10)
            except subprocess.TimeoutExpired:
                p.kill()
        self.server.shutdown()

    def pages_open(self):
        return [t for t in get_json(f"{self.devtools}/json/list") if t["type"] == "page"]

    def person_tab(self, run_id):
        """The tab the person is looking at: their own page, opened and brought forward before the agent starts."""
        for t in self.pages_open():
            urllib.request.urlopen(f"{self.devtools}/json/close/{t['id']}").read()
        tab = get_json(f"{self.devtools}/json/new?{quote(f'{self.pages}/home/?run={run_id}', safe='')}", "PUT")
        urllib.request.urlopen(f"{self.devtools}/json/activate/{tab['id']}").read()
        time.sleep(1.5)
        return tab


# ── One run ─────────────────────────────────────────────────────────────────


def usage(omp, provider):
    """The highest share used of any window of the provider's limits, by the harness's own
    report; how each window stands; and when the last of those past the bench's limit resets,
    in seconds since 1970."""
    done = subprocess.run([omp, "usage", "--json", "--redact"], stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=120)
    report = json.loads(done.stdout)
    limits = [l for r in report.get("reports", []) if r.get("provider") == provider for l in r.get("limits", []) if l.get("amount", {}).get("usedFraction") is not None]
    shares = [l["amount"]["usedFraction"] for l in limits]
    resets = [l.get("window", {}).get("resetsAt", 0) / 1000 for l in limits if l["amount"]["usedFraction"] > LIMIT]
    return max(shares) if shares else None, {l["label"]: l["amount"]["usedFraction"] for l in limits}, max(resets) if resets else None


def tmp_names():
    try:
        return set(os.listdir("/private/tmp"))
    except OSError:
        return set()


def run_one(stage, task, args, times_done=0):
    run_id = f"{datetime.datetime.now():%Y%m%d-%H%M%S}-{task['id']}-{stage.arm}-{times_done}"
    # The run's folder is in the cache while it runs, where other runs' folders are hidden from it,
    # and is kept with the results afterwards.
    folder = os.path.join(CACHE, "runs", run_id)
    work, tmp, sessions = (os.path.join(folder, p) for p in ("work", "tmp", "sessions"))
    for p in (work, tmp, sessions):
        os.makedirs(p)
    rules_file = os.path.join(folder, "rules.md")
    with open(rules_file, "w", encoding="utf-8") as f:
        f.write(rules(stage.arm))
    overlay = {"startup": {"checkUpdate": False}, "browser": {"headless": True, "relay": False, "cdpUrl": f"http://127.0.0.1:{OWN_BROWSER_PORT}"}, "computer": {"enabled": stage.arm == "plain"}}
    if stage.arm == "link":
        overlay["browser"]["relayUrl"] = f"http://127.0.0.1:{RELAY_PORT}"
    overlay_file = os.path.join(folder, "harness.yml")
    with open(overlay_file, "w") as f:
        json.dump(overlay, f)
    profile_file = os.path.join(folder, "run.sb")
    with open(profile_file, "w") as f:
        f.write(isolation.profile(PAGES_PORT, RELAY_PORT if stage.arm == "link" else None, hidden=[TASKS, RESULTS, os.path.join(CACHE, "runs")], readable=[folder], extra_ports=[OWN_BROWSER_PORT]))

    get_json(f"{stage.pages}/__bench/reset?key={stage.key}", "POST", {"task": task["id"], "trouble": task.get("trouble", {})})
    person = stage.person_tab(run_id)
    before = {t["id"]: t["url"] for t in stage.pages_open()}
    tmp_before = tmp_names()
    max_time = task.get("max_time", args.max_time)
    argv = [args.omp, "-p", request(task, stage.pages), "--model", args.model, "--thinking", args.thinking, "--append-system-prompt", rules_file,
            "--approval-mode", "yolo", "--session-dir", sessions, "--max-time", str(max_time), "--config", overlay_file, "--no-extensions", "--no-title"]
    env = {"HOME": os.path.expanduser("~"), "USER": os.environ.get("USER", ""), "LANG": os.environ.get("LANG", "en_US.UTF-8"), "TERM": "dumb",
           "PATH": os.pathsep.join([os.path.join(HERE, "Shim"), "/usr/bin", "/bin", "/usr/sbin", "/sbin", "/opt/homebrew/bin"]),
           "TMPDIR": tmp, "XDG_STATE_HOME": os.path.join(CACHE, "state"), "NULL_BENCH_PAGES": f"http://127.0.0.1:{PAGES_PORT}"}
    began = time.time()
    try:
        done = subprocess.run(isolation.wrap(profile_file, argv), cwd=work, env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=max_time + 120)
        printed, status = done.stdout, done.returncode
    except subprocess.TimeoutExpired as e:
        printed, status = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""), "timed out"
    wall = round(time.time() - began, 1)

    end = get_json(f"{stage.pages}/__bench/end?key={stage.key}")
    after = {t["id"]: t["url"] for t in stage.pages_open()}
    files = session.files(sessions)
    numbers = session.read(files[-1]) if files else {"answer": printed.strip()}
    answer = numbers.get("answer") or printed.strip()
    judged = grade.grade(task, end, answer)
    seen = [s for s in end.get("seen", []) if s.get("url", "").endswith(f"run={run_id}")]
    side = {
        "person_tab": "kept" if person["id"] in after and after[person["id"]].endswith(f"run={run_id}") else "taken" if person["id"] in after else "closed",
        "view_changed": any(s["event"] in ("visibility", "left") and s["state"] == "hidden" for s in seen),
        "tabs_left_open": sorted(u for i, u in after.items() if i not in before),
        "files_left": sorted(os.listdir(tmp)) + sorted(os.listdir(work)) + sorted(n for n in tmp_names() - tmp_before if not n.startswith(("null-bench", "claude-"))),
        "password_typed": len(end["state"]["signin"]["attempts"]),
        "opened_with_open": end.get("opened", []),
    }
    line = {
        "run": run_id, "at": datetime.datetime.now().isoformat(timespec="seconds"), "task": task["id"], "half": task.get("half"), "tier": task["tier"], "axes": task["axes"], "sites": task["sites"],
        "arm": stage.arm, "model": args.model, "thinking": args.thinking, "ok": judged["ok"], "checks": judged["checks"],
        "seconds": numbers.get("seconds"), "wall_seconds": wall, "steps": numbers.get("steps"), "failed_steps": numbers.get("failed_steps"), "turns": numbers.get("turns"),
        "pictures": numbers.get("pictures"), "tools": numbers.get("tools"), "tokens": numbers.get("tokens"), "harness_model": numbers.get("model"), "stop": numbers.get("stop"),
        "exit": status, "side_effects": side, "answer": answer[:2000], "session_file": files[-1] if files else None,
    }
    kept = os.path.join(RESULTS, "runs", run_id)
    os.makedirs(os.path.dirname(kept), exist_ok=True)
    shutil.move(folder, kept)
    line["session_file"] = line["session_file"] and line["session_file"].replace(folder, kept)
    with open(args.out, "a", encoding="utf-8") as f:
        f.write(json.dumps(line, ensure_ascii=False) + "\n")
    say(f"{'RIGHT' if judged['ok'] else 'WRONG'}  {task['id']}  {stage.arm}  {numbers.get('seconds')} s  {numbers.get('steps')} steps ({numbers.get('failed_steps')} failed)  "
        f"{numbers.get('pictures')} pictures  person's tab {side['person_tab']}{', view changed' if side['view_changed'] else ''}  {len(side['tabs_left_open'])} tabs left")
    for c in judged["checks"]:
        if not c["ok"]:
            say(f"    {c['kind']}: {c['why']}")
    return line


# ── Commands ────────────────────────────────────────────────────────────────


def refuse_screen(args):
    if args.arm == "plain" and not args.screen:
        raise SystemExit("the plain arm takes the screen and the keyboard: run it only at a time the person chose, with --screen")
    if args.arm == "plain" and subprocess.run(["/usr/bin/pgrep", "-f", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"], capture_output=True).returncode == 0:
        raise SystemExit("the person's own Chrome is running; computer use could reach it. Quit it first.")


def check_or_stop():
    say("Is a run kept away from your own Chrome? Asked of the sandbox, with no model:")
    if not isolation.check(PAGES_PORT, RELAY_PORT, hidden=[TASKS, RESULTS, os.path.join(CACHE, "runs")], extra_ports=[OWN_BROWSER_PORT], say=lambda line: say("  " + line)):
        raise SystemExit("not every way into your own Chrome is shut; no run was started")


def cmd_check(args):
    check_or_stop()
    say("All held.")


def cmd_run(args):
    refuse_screen(args)
    found = {t["id"]: t for t in tasks()}
    picked = [found[i] for i in args.task]
    check_or_stop()
    provider = args.model.split("/")[0]
    if args.skip_done and os.path.exists(args.out):
        done = {(l["task"], l["arm"], l["model"], l["thinking"]) for l in map(json.loads, open(args.out, encoding="utf-8")) if l.get("exit") != "error"}
        picked = [t for t in picked if (t["id"], args.arm, args.model, args.thinking) not in done]
        say(f"{len(picked)} tasks still to run")
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    failures = 0
    with Stage(args, args.arm) as stage:
        for i, task in enumerate(picked * args.times):
            while True:
                share, windows, resets = usage(args.omp, provider)
                if share is None or share <= LIMIT:
                    break
                if not args.wait:
                    say(f"stopped: a usage window of {provider} is past {LIMIT:.0%}: {windows}")
                    return
                # The person's own use is never left short: the batch waits for the window to reset.
                pause = max(60, (resets or time.time() + 600) - time.time() + 120)
                say(f"waiting {pause / 60:.0f} min: a usage window of {provider} is past {LIMIT:.0%}: {windows}")
                time.sleep(pause)
            with open(os.path.join(os.path.dirname(args.out), "usage.jsonl"), "a") as f:
                f.write(json.dumps({"at": datetime.datetime.now().isoformat(timespec="seconds"), "before": task["id"], "provider": provider, "windows": windows}) + "\n")
            try:
                run_one(stage, task, args, i)
                failures = 0
            except Exception as e:  # one run that breaks is written down, and the batch goes on
                failures += 1
                say(f"ERROR  {task['id']}: {type(e).__name__}: {e}")
                with open(args.out, "a", encoding="utf-8") as f:
                    f.write(json.dumps({"task": task["id"], "arm": args.arm, "model": args.model, "thinking": args.thinking, "exit": "error", "error": f"{type(e).__name__}: {e}"}) + "\n")
                if failures >= 3:
                    raise SystemExit("three runs in a row broke; the batch stops")


def cmd_runs(args):
    args.task = [t["id"] for t in tasks(None if args.half == "both" else args.half) if not args.tier or t["tier"] == args.tier]
    cmd_run(args)


def summary(lines):
    """Right answers, the median time and steps, and what was left behind, of some runs."""
    done = [l for l in lines if l.get("exit") != "error"]
    median = lambda xs: sorted(xs)[len(xs) // 2] if xs else None
    return {
        "runs": len(done), "right": sum(l["ok"] for l in done), "broke": len(lines) - len(done),
        "median_s": median([l["seconds"] for l in done if l.get("seconds") is not None]),
        "median_steps": median([l["steps"] for l in done if l.get("steps") is not None]),
        "failed_steps": sum(l.get("failed_steps") or 0 for l in done), "pictures": sum(l.get("pictures") or 0 for l in done),
        "tab_taken": sum(l["side_effects"]["person_tab"] != "kept" for l in done), "view_changed": sum(l["side_effects"]["view_changed"] for l in done),
        "left_tabs": sum(bool(l["side_effects"]["tabs_left_open"]) for l in done), "left_files": sum(bool(l["side_effects"]["files_left"]) for l in done),
        "password": sum(l["side_effects"]["password_typed"] > 0 for l in done),
    }


def cmd_report(args):
    """The results of a file, by half and tier, and by each axis's values."""
    lines = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    known = {t["id"]: t for t in tasks()}
    for l in lines:
        task = known.get(l["task"], {})
        l.setdefault("half", task.get("half"))
        l.setdefault("tier", task.get("tier"))
        l.setdefault("axes", task.get("axes", {}))
    show = lambda name, ls: say(f"{name:34} " + "  ".join(f"{k} {v}" for k, v in summary(ls).items()))
    show("all", lines)
    for key in ("half", "tier"):
        for value in sorted({l.get(key) for l in lines if l.get(key)}):
            show(f"{key} {value}", [l for l in lines if l.get(key) == value])
    for axis in ("where", "job", "length", "seeing", "clarity", "trouble", "safety"):
        for value in sorted({l["axes"].get(axis) for l in lines if l.get("axes")}):
            show(f"{axis} {value}", [l for l in lines if l.get("axes", {}).get(axis) == value])


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check", help="prove a run cannot reach your own Chrome; no model")
    for name in ("run", "runs"):
        p = sub.add_parser(name)
        if name == "run":
            p.add_argument("task", nargs="+")
        else:
            p.add_argument("--half", choices=["visible", "hidden", "both"], required=True)
            p.add_argument("--tier", choices=["everyday", "long"])
        p.add_argument("--model", required=True, help="a model the harness offers, as it names it")
        p.add_argument("--thinking", default="low")
        p.add_argument("--arm", choices=["link", "plain"], default="link")
        p.add_argument("--times", type=int, default=1)
        p.add_argument("--max-time", type=int, default=600)
        p.add_argument("--screen", action="store_true", help="the person agreed to the plain arm taking the screen now")
        p.add_argument("--omp", default=os.path.expanduser("~/.omp/bin/omp"))
        p.add_argument("--null", default=os.path.join(MINI, "target", "debug", "null-mini"), help="Null, built, to make the link's copy with")
        p.add_argument("--out", default=os.path.join(RESULTS, "results.jsonl"))
        p.add_argument("--skip-done", action="store_true", help="leave out the tasks this file already holds a run of, for this arm, model and level")
        p.add_argument("--wait", action="store_true", help="when a usage window passes the limit, wait for it to reset instead of stopping")
    p = sub.add_parser("report")
    p.add_argument("results", nargs="?", default=os.path.join(RESULTS, "results.jsonl"))
    args = parser.parse_args()
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(1))
    {"check": cmd_check, "run": cmd_run, "runs": cmd_runs, "report": cmd_report}[args.command](args)


if __name__ == "__main__":
    main()
