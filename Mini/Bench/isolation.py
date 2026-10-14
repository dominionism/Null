"""Keeping a bench run away from the person's own Chrome, and proving it with no model.

Every run starts the harness inside a macOS sandbox profile (`sandbox-exec`)
that every program it starts inherits. The profile shuts each way into the
person's own Chrome that is open to a program on this Mac:

- the relay their Chrome may be linked to (port 9224), and every other port on
  this Mac but the bench's pages and the bench's own relay;
- Apple Events, by which a script asks Chrome for its tabs;
- LaunchServices, by which `open` starts an app or hands an address to the
  person's browser;
- Chrome's own program, which hands an address to the Chrome already running,
  its profile folder and its sockets;
- the person's own harness services, which run outside any sandbox.

It also keeps pictures of the screen from the shell, and keeps the bench's own
answers and results out of the agent's reach. The providers are reached over
HTTPS (port 443), as the harness needs.

`check()` asks the sandbox about each of these from inside it, with probes
that cannot reach the person's Chrome even if the sandbox let them through,
and says what held. The runner runs it before any run.
"""

import os
import socket
import subprocess
import sys
import tempfile
import time

HOME = os.path.expanduser("~")
HERE = os.path.dirname(os.path.abspath(__file__))
PERSONS_RELAY = 9224
CHROME_APPS = ("/Applications/Google Chrome.app", os.path.join(HOME, "Applications/Google Chrome.app"))
CHROME_PROFILE = os.path.join(HOME, "Library/Application Support/Google/Chrome")
HARNESS_RUN = os.path.join(HOME, ".omp/run")


def profile(pages_port, relay_port, hidden=(), readable=(), extra_ports=()):
    """The sandbox profile of a run: what the harness and all it starts may not do."""
    q = lambda path: '"' + path.replace("\\", "\\\\").replace('"', '\\"') + '"'
    lines = [
        "(version 1)",
        "(allow default)",
        ";; No Apple Events, and nothing opened through LaunchServices: no app, no address in the person's browser.",
        "(deny appleevent-send)",
        "(deny lsopen)",
        ";; The person's own Chrome: its program, its profile and its sockets.",
        *[f"(deny process-exec (subpath {q(app)}))" for app in CHROME_APPS],
        f"(deny file-read* file-write* (subpath {q(CHROME_PROFILE)}))",
        '(deny network-outbound (remote unix-socket (regex #"/\\.com\\.google\\.Chrome\\.")))',
        ";; The person's own harness services, which run outside any sandbox.",
        f"(deny file-read* file-write* (subpath {q(HARNESS_RUN)}))",
        f"(deny network-outbound (remote unix-socket (subpath {q(HARNESS_RUN)})))",
        ";; No port on this Mac but the bench's own; the providers over HTTPS.",
        '(deny network-outbound (remote ip "*:*"))',
        '(allow network-outbound (remote ip "*:443"))',
        *[f'(allow network-outbound (remote ip "localhost:{p}"))' for p in (pages_port, relay_port, *extra_ports) if p],
        ";; No picture of the screen from the shell.",
        '(deny process-exec (literal "/usr/sbin/screencapture"))',
        ";; The bench's answers and results are not the agent's to read. That they are there is",
        ";; not hidden, so that a path through them can still be followed.",
        *[f"(deny file-read-data (subpath {q(path)}))" for path in hidden],
        ";; The run's own folder, inside one of those, is its to read.",
        *[f"(allow file-read-data (subpath {q(path)}))" for path in readable],
    ]
    # A port allowed above must never be the relay the person's Chrome may hold.
    assert PERSONS_RELAY not in (pages_port, relay_port, *extra_ports)
    return "\n".join(lines) + "\n"


def wrap(profile_path, argv):
    return ["/usr/bin/sandbox-exec", "-f", profile_path, *argv]


# ── The check ───────────────────────────────────────────────────────────────

AE_PROBE = r"""
#include <stdio.h>
#include <unistd.h>
int sandbox_check(pid_t pid, const char *operation, int type, ...);
int main(int argc, char **argv) {
    /* 4: an Apple Event's destination, by bundle id; 0x40000000: do not log it. */
    for (int i = 1; i < argc; i++) printf("%s %s\n", argv[i], sandbox_check(getpid(), "appleevent-send", 4 | 0x40000000, argv[i]) ? "denied" : "allowed");
    return 0;
}
"""

LS_PROBE = r"""
#include <stdio.h>
int main(void) { FILE *f = fopen(MARKER, "w"); if (f) fclose(f); return 0; }
"""

LS_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>io.github.dominionism.null-bench-probe</string>
<key>CFBundleName</key><string>NullBenchProbe</string>
<key>CFBundleExecutable</key><string>probe</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSBackgroundOnly</key><true/>
</dict></plist>
"""


def _free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _listening(port):
    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


def _inside(profile_path, argv, timeout=20):
    done = subprocess.run(wrap(profile_path, argv), capture_output=True, text=True, timeout=timeout, cwd=tempfile.gettempdir())
    return done.returncode, (done.stdout + done.stderr).strip()


def _connects_inside(profile_path, port):
    """Whether a program inside the sandbox can open a connection to a port on this Mac. It only connects: nothing is sent."""
    code = f"import socket,sys\ns=socket.socket()\ns.settimeout(3)\nsys.exit(0 if s.connect_ex(('127.0.0.1',{port}))==0 else 1)"
    status, _ = _inside(profile_path, [sys.executable, "-I", "-c", code])
    return status == 0


def check(pages_port, relay_port, hidden=(), say=print):
    """Ask the sandbox, from inside it, about each way to the person's Chrome. True when all of them are shut."""
    results = []
    note = lambda ok, what, how: (results.append(ok), say(f"{'held  ' if ok else 'FAILED'}  {what}: {how}"))
    work = tempfile.mkdtemp(prefix="null-bench-check-")
    path = os.path.join(work, "run.sb")
    with open(path, "w") as f:
        f.write(profile(pages_port, relay_port, hidden))

    # Ports: the person's relay and any other port are shut; the bench's two are open. Listeners
    # of the check's own stand in for the bench's when they are not up, so the open ones are proven too.
    held = []
    for port in (pages_port, relay_port):
        if not _listening(port):
            s = socket.socket()
            s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            s.bind(("127.0.0.1", port))
            s.listen()
            held.append(s)
    other = socket.socket()
    other.bind(("127.0.0.1", 0))
    other.listen()
    try:
        note(not _connects_inside(path, PERSONS_RELAY), f"the relay a person's Chrome links to, port {PERSONS_RELAY}",
             "no connection from inside" + (" (and a relay is listening there now)" if _listening(PERSONS_RELAY) else ""))
        note(not _connects_inside(path, other.getsockname()[1]), "any other port on this Mac", f"port {other.getsockname()[1]}, listening, refused from inside")
        note(_connects_inside(path, pages_port) and _connects_inside(path, relay_port), "the bench's own pages and relay", f"ports {pages_port} and {relay_port} reached from inside")
    finally:
        for s in held + [other]:
            s.close()

    # Apple Events: the sandbox says, for Chrome's own bundle id, whether an event may go. Nothing is sent.
    probe = os.path.join(work, "aecheck")
    subprocess.run(["/usr/bin/cc", "-x", "c", "-o", probe, "-"], input=AE_PROBE, text=True, check=True, capture_output=True)
    outside = subprocess.run([probe, "com.google.Chrome"], capture_output=True, text=True).stdout.strip()
    _, inside = _inside(path, [probe, "com.google.Chrome", "com.google.chrome.for.testing", "com.apple.finder"])
    note(outside.endswith("allowed") and inside.count("denied") == 3, "Apple Events to Chrome and every other app", f"outside: {outside}; inside: {inside.replace(chr(10), ', ')}")

    # LaunchServices: a probe app that does nothing but leave a mark, with no window and no Dock icon,
    # opened with `open` from outside (it leaves its mark) and from inside (it must not).
    app = os.path.join(work, "NullBenchProbe.app")
    os.makedirs(os.path.join(app, "Contents/MacOS"))
    with open(os.path.join(app, "Contents/Info.plist"), "w") as f:
        f.write(LS_PLIST)
    marker = os.path.join(work, "opened")
    subprocess.run(["/usr/bin/cc", "-x", "c", f'-DMARKER="{marker}"', "-o", os.path.join(app, "Contents/MacOS/probe"), "-"], input=LS_PROBE, text=True, check=True, capture_output=True)
    subprocess.run(["/usr/bin/open", "-g", "-a", app], capture_output=True)
    for _ in range(50):
        if os.path.exists(marker):
            break
        time.sleep(0.1)
    worked_outside = os.path.exists(marker)
    if worked_outside:
        os.remove(marker)
    status, said = _inside(path, ["/usr/bin/open", "-g", "-a", app])
    time.sleep(3)
    note(worked_outside and status != 0 and not os.path.exists(marker), "opening an app or an address with open (LaunchServices)",
         f"the probe app {'opened' if worked_outside else 'did NOT open'} from outside; from inside: {said or status}")

    # Chrome's own program, profile and sockets, and the person's harness services.
    chrome = next((a for a in CHROME_APPS if os.path.isdir(a)), None)
    if chrome:
        status, said = _inside(path, [os.path.join(chrome, "Contents/MacOS/Google Chrome"), "--version"])
        note(status != 0 and "not permitted" in said.lower(), "Chrome's own program, which hands an address to the Chrome running", said[:80])
    else:
        note(True, "Chrome's own program", "Google Chrome is not on this Mac")
    if os.path.isdir(CHROME_PROFILE):
        status, said = _inside(path, ["/bin/ls", CHROME_PROFILE])
        note(status != 0 and "not permitted" in said.lower(), "Chrome's profile folder, its sign-ins and history", said.splitlines()[-1][:90] if said else str(status))
    else:
        note(True, "Chrome's profile folder", "not on this Mac")
    if os.path.isdir(HARNESS_RUN):
        status, said = _inside(path, ["/bin/ls", HARNESS_RUN])
        note(status != 0 and "not permitted" in said.lower(), "the person's own harness services", said.splitlines()[-1][:90] if said else str(status))
    else:
        note(True, "the person's own harness services", "none on this Mac")
    status, said = _inside(path, ["/usr/sbin/screencapture", "-h"])
    note(status != 0 and "not permitted" in said.lower(), "pictures of the screen from the shell", said.splitlines()[-1][:80] if said else str(status))
    for secret in hidden:
        status, said = _inside(path, ["/bin/ls", secret])
        note(status != 0, "the bench's own answers and results", f"{os.path.basename(secret)}: {said.splitlines()[-1][:70] if said else status}")
    return all(results)
