#!/usr/bin/env python3
"""Build the README's section cards: one terminal window per section.

    python3 .github/Readme/make_cards.py

The README is a stack of these windows and nothing else: each section, each numbered step and each
panel that used to be a drop-down is one window, titled in its own title bar. Edit a card's data
below and rerun; nothing else in the repository is read.

Every command shown is one a reader can run. Output lines are only those the tools really print
(`Mini/Scripts/install` says the install line, `ls` prints the names, git prints its cloning notice),
so a card never invents a result.
"""
import os
import re
import xml.etree.ElementTree as ET
from html import escape

from terminal import BAR, FG, GREEN, MUTED, PAD, TEXT, W, delay, idle_prompt, prompt, window

OUT = os.path.dirname(os.path.abspath(__file__))

FS, HFS, LH = 12.5, 14, 19       # body and heading font sizes, line height
EM = 0.65                        # widest monospace advance seen (WebKit's SF Mono); Menlo is 0.6
BULLET_X = PAD + 22              # bullet text; the marker sits in the gap before it
KEY_COLS = 18                    # green key column, in characters
VALUE_X = PAD + KEY_COLS * FS * EM
TOP = BAR + 34                   # first baseline under the title bar


def cols(x, size=FS):
    """How many characters fit between x and the window's right padding."""
    return int((W - PAD - x) / (size * EM))


def wrap(text, width):
    """Greedy word wrap that keeps **bold** markup: returns lines of (text, bold) runs."""
    plain, bold = "", []
    for i, part in enumerate(re.split(r"\*\*(.+?)\*\*", text)):
        plain += part
        bold += [i % 2 == 1] * len(part)
    lines, start = [], 0
    while start < len(plain):
        end = len(plain) if len(plain) - start <= width else plain.rfind(" ", start, start + width + 1)
        if end <= start:
            end = start + width
        runs, i = [], start
        while i < end:
            j = i
            while j < end and bold[j] == bold[i]:
                j += 1
            runs.append((plain[i:j], bold[i]))
            i = j
        lines.append(runs)
        start = end + 1 if end < len(plain) and plain[end] == " " else end
    return lines


def text_line(x, y, runs, italic=False, color=TEXT):
    body = "".join(f'<tspan fill="{FG}" font-weight="700">{escape(t)}</tspan>' if b else escape(t) for t, b in runs)
    style = ' font-style="italic"' if italic else ""
    return f'  <text x="{x}" y="{y:.1f}" font-size="{FS}" fill="{color}"{style}>{body}</text>\n'


class Card:
    """Builds one window's body top to bottom, counting the height as it goes."""

    def __init__(self, key_cols=KEY_COLS):
        self.svg, self.css, self.y, self.t, self.n, self.rows = [], [], TOP, 0.0, 0, 0
        self.value_x = PAD + key_cols * FS * EM
        self.label = []

    def _text(self, x, text, color=TEXT, italic=False):
        for k, runs in enumerate(wrap(text, cols(x))):
            self.y += LH if k else 0
            self.svg.append(text_line(x, self.y, runs, italic=italic, color=color))

    def sub(self, text):
        """An italic line under the title bar: the section in one sentence."""
        self.y += 2
        self._text(PAD, text, italic=True)
        self.label.append(text.replace("**", ""))

    def head(self, text, color=GREEN):
        self.y += LH + 14
        self.svg.append(f'  <text x="{PAD}" y="{self.y:.1f}" font-size="{HFS}" font-weight="700" fill="{color}">{escape(text)}</text>\n')
        self.label.append(text)

    def note(self, text):
        self.y += LH + 6
        self._text(PAD, text, color=MUTED)
        self.label.append(text.replace("**", ""))

    def row(self, key, value):
        self.y += LH + (7 if self.rows else 0)
        self.rows += 1
        self.svg.append(f'  <text x="{PAD}" y="{self.y:.1f}" font-size="{FS}" font-weight="700" fill="{GREEN}">{escape(key)}</text>\n')
        self._text(self.value_x, value)
        self.label.append(f"{key}: {value.replace('**', '')}")

    def bullets(self, items, muted=False):
        for n, item in enumerate(items):
            self.y += LH + (4 if n else 8)
            color = MUTED if muted else GREEN
            self.svg.append(f'  <text x="{PAD + 4}" y="{self.y:.1f}" font-size="{FS}" fill="{color}">▸</text>\n')
            for k, runs in enumerate(wrap(item, cols(BULLET_X))):
                self.y += LH if k else 0
                self.svg.append(text_line(BULLET_X, self.y, runs, color=MUTED if muted else TEXT))
            self.label.append(item.replace("**", ""))

    def raw(self, lines, muted=True):
        """Pre-formatted lines: a tree or a diagram, exactly as written."""
        for n, line in enumerate(lines.split("\n")):
            self.y += LH if n else 8
            assert len(line) <= cols(PAD), f"too wide for the window: {line!r}"
            self.svg.append(f'  <text x="{PAD}" y="{self.y:.1f}" font-size="{FS}" fill="{MUTED if muted else TEXT}" xml:space="preserve">{escape(line)}</text>\n')
        self.label.append(lines.replace("\n", " "))

    def command(self, command):
        """A prompt line that types itself out."""
        self.t += 0.35 if self.n == 0 else 0.15
        name = f"c{self.n}"
        self.n += 1
        self.y += LH + 12
        svg, css, self.t = prompt(self.y, command, self.t, name)
        self.svg.append(svg)
        self.css.append(css)
        self.t += 0.35
        self.label.append(f"$ {command}")

    def output(self, text, muted=False):
        for line in text.split("\n"):
            self.y += LH - 3
            body = text_line(PAD, self.y, wrap(line, cols(PAD))[0], color=MUTED if muted else TEXT).strip()
            body = body.replace("<text ", '<text xml:space="preserve" ', 1)
            self.svg.append(f'  <g class="rise" {delay(self.t)}>{body}</g>\n')

    def done(self):
        self.y += LH + 10
        self.svg.append(idle_prompt(self.y, self.t + 0.1))

    def render(self, title):
        # A card with a command is a session, so it ends with a fresh prompt like a real terminal;
        # a card of rows or bullets is a panel and simply ends, as the profile's cards do.
        if self.n:
            self.done()
        label = f"{title}. " + " ".join(self.label)
        return title, window(round(self.y + 26), title, "".join(self.svg), "".join(self.css), label, heading=True)


CARDS = {}


# ── Link chips ────────────────────────────────────────────────────────────
# A link inside a card cannot be clicked: an SVG loaded as an image is inert, and GitHub strips an
# inline <svg> out of markdown. So each link is its own small window-styled image wrapped in <a>:
# clickable, and with no text there is no underline for GitHub to draw.
CHIP_H, CHIP_PAD, CHIP_FS = 30, 14, 12.5
CHIP_CW = CHIP_FS * 0.6
CHIP_FONT = "ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,'Liberation Mono',monospace"

CHIPS = {
    "oh-my-pi": ("Oh-my-pi", "https://github.com/can1357/oh-my-pi", True),
    "rust": ("Rust toolchain", "https://rustup.rs/", True),
    "tauri": ("Tauri macOS prerequisites", "https://v2.tauri.app/start/prerequisites/#macos", True),
    "voicebox": ("Voicebox", "https://github.com/jamiepine/voicebox", True),
    "adr-0002": ("ADR 0002", "Context/ADR/0002-NullMiniIsItsOwnApp.md", False),
    "adr-0001": ("ADR 0001", "Context/ADR/0001-CapitalizedFolderNames.md", False),
    "bun": ("Bun", "https://bun.sh", True),
    "just": ("just", "https://github.com/casey/just", True),
    "research": ("Research map", "Context/Research/Research.md", False),
    "cleanup-plan": ("Cleanup plan", "Context/Plans/CapitalFolders.md", False),
    "page-html": ("Page/index.html", "Mini/Page/index.html", False),
    "harness-rs": ("harness.rs", "Mini/src/harness.rs", False),
    "translate-rs": ("translate.rs", "Mini/src/translate.rs", False),
    "engine-rs": ("engine.rs", "Mini/src/engine.rs", False),
    "check-rs": ("check.rs", "Mini/src/check.rs", False),
    "markdown-rs": ("markdown.rs", "Mini/src/markdown.rs", False),
    "panel-rs": ("panel.rs", "Mini/src/panel.rs", False),
    "settings-rs": ("settings.rs", "Mini/src/settings.rs", False),
    "miniapp": ("App design", "Context/Plans/MiniApp.md", False),
    "providers": ("Providers", "Context/Plans/Providers.md", False),
    "own-harness": ("Own harness", "Context/Plans/OwnHarness.md", False),
    "display": ("Conversation display", "Context/Plans/ConversationDisplay.md", False),
    "phases": ("Future phases", "Context/Plans/NullMini.md", False),
    "adrs": ("Decisions", "Context/ADR/", False),
    "license": ("MIT", "LICENSE", False),
    "responsible": ("Responsible use", "RESPONSIBLE_USE.md", False),
    "security": ("Security", "SECURITY.md", False),
    "contributing": ("Contributing", "CONTRIBUTING.md", False),
    "nav-get-started": ("Get started", "#get-started", False),
    "nav-use-null": ("Use Null", "#use-null", False),
    "nav-privacy": ("Privacy", "#privacy-and-permissions", False),
    "nav-architecture": ("Architecture", "#under-the-hood", False),
    "nav-development": ("Development", "#development", False),
}

# The header's navigation, chipped like everything else so nothing on the page is underlined.
NAV = ["nav-get-started", "nav-use-null", "nav-privacy", "nav-architecture", "nav-development"]

# Which chips sit under which card, in the README's order.
LINK_ROWS = [
    ["oh-my-pi"],
    ["rust", "tauri"],
    ["page-html", "harness-rs", "engine-rs", "translate-rs", "markdown-rs", "panel-rs", "settings-rs", "check-rs"],
    ["voicebox", "adr-0002"],
    ["bun", "just", "research"],
    ["cleanup-plan", "adr-0001"],
    ["miniapp", "providers", "own-harness", "display", "phases", "adrs", "research"],
    ["license", "responsible", "security", "contributing"],
]


def chip(slug):
    """One clickable chip, and the markdown that links it."""
    label, url, external = CHIPS[slug]
    arrow = 16 if external else 0
    w = round(CHIP_PAD * 2 + len(label) * CHIP_CW + arrow)
    mark = (
        f'  <text x="{w - CHIP_PAD}" y="{CHIP_H / 2 + 4}" font-size="11" fill="{MUTED}" text-anchor="end" xml:space="preserve">↗</text>\n'
        if external
        else ""
    )
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{CHIP_H}" viewBox="0 0 {w} {CHIP_H}" '
        f'role="img" aria-label="{escape(label)}">\n'
        f"  <title>{escape(label)}</title>\n"
        f"  <style>text {{ font-family: {CHIP_FONT}; }}</style>\n"
        f'  <rect x="0.5" y="0.5" width="{w - 1}" height="{CHIP_H - 1}" rx="8" fill="#161B22" stroke="#21262D"/>\n'
        f'  <text x="{CHIP_PAD}" y="{CHIP_H / 2 + 4}" font-size="{CHIP_FS}" fill="#58A6FF" xml:space="preserve">{escape(label)}</text>\n'
        f"{mark}"
        f"</svg>\n"
    )
    ET.fromstring(svg)
    with open(os.path.join(OUT, f"link-{slug}.svg"), "w") as handle:
        handle.write(svg)
    alt = f"{label} (opens {url})" if external else label
    return f'  <a href="{url}"><img src=".github/Readme/link-{slug}.svg" height="{CHIP_H}" alt="{alt}" /></a>'


def link_rows():
    """The markdown blocks for the chips, to paste under their cards."""
    blocks = []
    for slugs in LINK_ROWS:
        body = "\n  &nbsp;\n".join(chip(slug) for slug in slugs)
        blocks.append(f'<p align="center">\n{body}\n</p>')
    return blocks


def add(name, title, build, key_cols=KEY_COLS):
    card = Card(key_cols)
    build(card)
    CARDS[name] = card.render(title)


# ── Small surface. Real agent. ────────────────────────────────────────────
def surface(c):
    c.sub("Press Control+Space from the app you are in. Type, and your agent answers in the box.")
    c.bullets([
        "**Your tools, not just chat.** Null drives the Oh-my-pi harness it carries: its tools, your "
        "configured MCP servers, your approval mode.",
        "**Your providers, one conversation.** Switch models, read usage, keep an ordered fallback "
        "list. OMP owns the sign-ins and the retries.",
        "**Readable without getting bigger.** Streamed Markdown, folded tool steps, and code, tables "
        "and diagrams that scroll sideways instead of wrapping.",
        "**Today:** a standalone macOS text app, built from source. Voice, background tasks, "
        "workspaces and the pet are planned, not built.",
    ])
    c.note("The banner above is an illustration, not a recording of an agent session.")


# ── Get started ───────────────────────────────────────────────────────────
def step_harness(c):
    c.sub("Nothing to install first. Null carries one checked version of Oh-my-pi (omp) inside the app.")
    c.row("Your folder", "It works from OMP's usual folder, ~/.omp: existing sign-ins, skills and MCP servers are used as they are. With none, run /login inside Null.")
    c.row("Managed by OMP", "Providers, subscriptions and API keys. Their availability and terms still apply.")
    c.row("Your own copy", "/harness switches to an Oh-my-pi found on PATH, then in ~/.omp/bin, ~/.opencode/bin, ~/.local/bin, /opt/homebrew/bin or /usr/local/bin, and back")
    c.note("The carried version is named in Mini/Engine.toml and moves only after the repository's "
           "harness check has passed it. Apple Silicon Macs only, for now.")


def step_build(c):
    c.sub("An Apple Silicon Mac with macOS 13+, the Rust toolchain, Xcode Command Line Tools and "
          "Tauri CLI 2. No Node, Bun, Python or Voice server is needed for the box.")
    c.command("xcode-select --install           # once, if the Apple tools are not installed")
    c.command('cargo install tauri-cli --version "^2" --locked')
    c.command("git clone https://github.com/dominionism/Null.git && cd Null")
    c.command("Mini/Scripts/install")
    c.output("Installed /Applications/Null.app and started it. Control+Space opens the box.")
    c.note("The script fetches the Oh-my-pi named in Mini/Engine.toml (about 200 MB) and keeps it "
           "only if its checksum matches. Then it builds Null.app, installs it, registers a login "
           "item and starts it, replacing any previously installed Null app. A source-build workflow "
           "with local signing support, not a notarized release installer.")


def step_open(c):
    c.row("First time", "With no provider signed in, the box opens by itself, once, and shows the sign-in list")
    c.row("Control+Space", "Open the box")
    c.row("/login", "Add a provider, if you need one")
    c.row("/model", "Choose one of its models")
    c.row("Then", "Type your request. You do not need to leave a terminal running.")


def uninstall(c):
    c.command("Mini/Scripts/uninstall")
    c.row("Removes", "/Applications/Null.app, and the login item at ~/Library/LaunchAgents/io.github.dominionism.null-mini.plist")
    c.row("Left behind", "Settings, workspace files and logs, in ~/Library/Application Support/io.github.dominionism.null-mini/ and ~/Library/Logs/Null/")
    c.note("OMP's own sign-ins and conversation history are not removed.")


# ── Use Null ──────────────────────────────────────────────────────────────
def use_null(c):
    for key, value in [
        ("Control+Space", "Show or hide the box"),
        ("Enter", "Send a message, or choose a list entry"),
        ("Esc", "Close a picker, cancel sign-in, or hide the box"),
        ("Ctrl+C", "Stop the reply; when idle, quit and close the conversation"),
        ("↑", "Recall the last prompt in an empty field"),
        ("Drag", "Move the box; click away hides it"),
    ]:
        c.row(key, value)
    c.note("The arrow pulses while the agent works and turns amber when it asks. The transcript grows "
           "to about ten lines, then scrolls. Hiding the box does not stop a reply.")


def commands(c):
    for key, value in [
        ("/model [filter]", "Choose a model from OMP's catalogue, across providers, without starting a new conversation"),
        ("/login", "Follow the harness's own sign-in steps inside the box, browser sign-in or key entry"),
        ("/usage", "See the harness's usage reports and reset times; providers without a report are marked"),
        ("/backup [filter]", "Set an ordered fallback list: Enter adds or removes a model, Esc saves it"),
        ("/harness", "See which Oh-my-pi the box runs, the built-in one or your own, and switch between them"),
        ("/update", "Move the built-in Oh-my-pi to the newest version the repository's harness check has passed"),
        ("/new", "Start a fresh conversation on the last selected model"),
        ("/quit", "Quit and clear the saved conversation pointer"),
    ]:
        c.row(key, value)
    c.note("A lone unknown /word is refused rather than passed to the agent; other messages go "
           "through normally. A listed model is not a guarantee that your account can use it: "
           "providers with reported limits exhausted are dimmed but stay selectable, and a model "
           "change waits until the current reply is stopped or finished.")


def provider_stops(c):
    c.bullets([
        "Null passes your backup order to OMP through a separate configuration overlay. The order "
        "goes before the existing model and provider-specific entries for the run, and your own OMP "
        "settings are not modified.",
        "OMP decides when to retry or fall back and resends the message itself. Fallback can follow a "
        "refusal or a sign-in failure as well as a usage limit.",
        "The box reports detected model switches and highlights failed replies, but it never resends "
        "one: recall it with the up arrow or type it again.",
    ])
    c.note("Failure detection uses OMP's end-of-turn token-usage signal, an unstable protocol "
           "extension, which the repository's harness check asks of every version before Null "
           "carries it. The fallback path has been exercised with simulated failures; real "
           "account-limit behavior is still awaiting verification.")


# ── Privacy and permissions ───────────────────────────────────────────────
def privacy(c):
    c.row("Not offline", "Local interface does not mean offline agent: OMP may send prompts and tool context to your chosen provider, and tools and MCP servers may make their own network requests")
    c.row("Model lists", "When it starts, OMP asks several outside services for their public model catalogues and probes local model servers on this Mac. This is OMP's own behaviour, in a terminal as in Null")
    c.row("No listener", "Null has no HTTP listener and no database of its own; it starts the OMP it carries as a child process and talks over stdio")
    c.row("Credentials", "Your /login answers go to the local omp login process, are masked in the UI and are never written to Null's settings or log. OMP handles authentication; no Null account exists")
    c.row("Shortcut", "Control+Space needs no Accessibility or Input Monitoring permission. If registration fails, the box opens with a notice")
    c.row("Approvals", "Null passes on OMP's approval setting and adds none of its own. OMP's own default lets the agent act without asking you first, and the first opening says so")
    c.row("Full Disk Access", "A separate choice, asked once, after the first reply, with an explanation: macOS attributes the agent's file access to Null. It is broader file access, not a sandbox, and it does not replace OMP's approval rules. The box opens without it")


def signing(c):
    c.bullets([
        "macOS ties permission grants to the application's code signature, so an unsigned rebuild "
        "can require permission again.",
        "When ~/Library/Application Support/Null Signing/signing.keychain-db exists, the installer "
        "signs with the Null Local Signing identity, using the password file beside it. Without that "
        "keychain it installs the build unsigned, with a notice.",
        "The Oh-my-pi inside the app is left as published, with its author's Developer ID signature. "
        "The installer signs the outer app only, and that signature covers it.",
    ])
    c.note("That identity is a personal-machine setup, not a distributed Developer ID, and the "
           "installer does not create one on a new machine. Protect both the keychain and its "
           "password: a process with access to them can sign as Null.")


def what_null_keeps(c):
    c.sub("Under ~/Library/Application Support/io.github.dominionism.null-mini/")
    c.row("settings.json", "Window position, chosen model, saved session ID, Full Disk Access prompt state, whether the first opening has happened, backup order, and your own harness if you chose it")
    c.row("harness.yml", "Settings handed to OMP at every start: no update check, and your backup order when there is one. Rewritten each time")
    c.row("Workspace/", "Default working directory for conversations; not a filesystem access boundary")
    c.note("Your own OMP settings are not modified. OMP "
           "owns the transcript and the credentials: a restart resumes the saved session, while "
           "/quit and idle Ctrl+C clear Null's pointer, not OMP's stored transcript. Logs are in "
           "~/Library/Logs/Null/mini.log, and the smoke and self-test modes can log reply text, so "
           "keep sensitive prompts out of those checks.")


# ── Under the hood ────────────────────────────────────────────────────────
def under_the_hood(c):
    c.sub("One native Tauri app plus the OMP process it carries and launches. The page is a static "
          "HTML file: no web server, bundler or Node runtime.")
    c.raw("Control+Space → Null's page → Tauri commands → OMP over ACP\n"
          "                    ↑                            │\n"
          "                    └── text, tools, approvals ──┘")
    c.bullets([
        "Rust owns the non-activating panel, the shortcut and the ACP connection; OMP owns agent "
        "execution, providers and conversation history.",
        "Null starts the Oh-my-pi beside its own executable, at the version in Mini/Engine.toml. "
        "Only the program is Null's: it works from your ~/.omp folder, and /harness can run your "
        "own copy instead.",
        "Null carries OMP's approval mode and supported MCP definitions into the session: your "
        "terminal agent in a smaller surface, not a separate set of permissions.",
        "Terminal parity has edges. The box does not pass through OMP's lone slash commands or show "
        "its thinking and full tool output.",
        "Replies are parsed into a Markdown tree in Rust and rendered with DOM elements and "
        "textContent, so model-authored HTML is never executed. **A Content Security Policy is not "
        "configured yet**, and careful rendering is not a substitute for one.",
    ])


def source_map(c):
    for key, value in [
        ("Page/index.html", "Input, commands, transcript, layout, scrolling and selection"),
        ("main.rs", "Startup, plugins, state and command registration"),
        ("panel.rs, shortcut.rs", "Window, placement, sizing, visibility and Control+Space"),
        ("harness.rs, translate.rs", "ACP process and session lifecycle, approvals, events, protocol translation"),
        ("engine.rs", "Which Oh-my-pi runs: the carried one or your own, and /harness"),
        ("providers.rs, signin.rs", "Usage reports and harness-owned sign-in"),
        ("welcome.rs", "The first opening: with no provider signed in, the box opens once with the sign-in list"),
        ("backups.rs", "The fallback order written for OMP, and the switch reporting it"),
        ("markdown.rs", "Reply text into structured parts, including partial streamed input"),
        ("settings.rs, access.rs, log.rs", "Persistence, the Full Disk Access check, and logging"),
        ("check.rs, standin.rs", "The harness check: live tests of what Null relies on, against the stand-in provider that scripted runs also use"),
    ]:
        c.row(key, value)


# ── Development ───────────────────────────────────────────────────────────
def development(c):
    c.command("Mini/Scripts/engine          # once: fetch the Oh-my-pi the build takes in")
    c.command("cd Mini && cargo test        # offline unit tests; live ones are ignored")
    c.command("cargo build                  # local debug binary")
    c.command("cargo tauri build            # app bundle, no installation")
    c.note("Use cargo tauri, not bunx tauri: the latter names a different npm package. "
           "cargo test -- --ignored is the harness check: live tests on the carried Oh-my-pi with a "
           "stand-in provider, one of which reaches a real provider with a dummy key. With them "
           "runs the key check, which opens the box for a few seconds, signs in with a marked "
           "dummy key and searches Null's log and settings for it. "
           "NULL_MINI_ENGINE=<path> runs them on another Oh-my-pi, and Mini/Scripts/engine --to "
           "<version> moves Null to that version only if they pass. Repository CI runs both on a Mac "
           "runner whenever Mini/ changes, beside the Voice frontend typecheck and web build.")


def dev_switches(c):
    for key, value in [
        ("EXIT_WHEN_READY=1", "Exit after the page loads; no prompt is sent"),
        ("NO_SHORTCUT=1", "Show at startup without claiming Control+Space"),
        ("SMOKE=\"<text>\"", "Send a real harness prompt, log the reply, exit without a window; also SMOKE_MODEL and SMOKE_STOP_AFTER (seconds)"),
        ("SELFTEST=\"<text>\"", "Type into the real page, a line at a time, each once the page has stopped working: a message, a command, a choice from a list, an answer to a sign-in. Then log what the page shows and exit"),
        ("PROFILE=<name>", "Pass an isolated profile to OMP and use profile-specific Null settings and harness-settings files"),
        ("BACKUPS=\"m/one,m/two\"", "Override the backup order for this run without changing the saved order"),
        ("UPDATES=<address>", "Where /update reads which Oh-my-pi is checked, in place of the repository: a made-up file as a file:// address"),
        ("FOLDER=<path>", "Run on that folder in place of yours, made if missing: OMP's folder, its MCP definitions and Null's own files are all under it, so a new one is a Mac with nothing signed in"),
        ("STANDIN=1", "Offer a stand-in provider in that folder, so a message is answered or refused with no real sign-in: standin-anthropic/ok, /limit, /auth, /noaccess, and the same for openai and codex; needs FOLDER"),
    ]:
        c.row(f"NULL_MINI_{key}", value)
    c.command("NULL_MINI_EXIT_WHEN_READY=1 NULL_MINI_NO_SHORTCUT=1 \\")
    c.command("  NULL_MINI_PROFILE=null-probe ./target/debug/null-mini")
    c.command("NULL_MINI_FOLDER=/tmp/null-run NULL_MINI_STANDIN=1 NULL_MINI_SMOKE=ping \\")
    c.command("  NULL_MINI_SMOKE_MODEL=standin-anthropic/ok ./target/debug/null-mini")
    c.note("Profile isolation does not isolate MCP: definitions still come from the normal "
           "~/.omp/agent/mcp.json, and smoke or self-test prompts can call those servers and run "
           "real tools. The startup-only check above sends no prompt. A run on a folder of its "
           "own reads none of that and leaves nothing behind but the folder; a local model server "
           "on this Mac, such as Ollama, is still found.")


# ── Optional voice tools ──────────────────────────────────────────────────
def voice(c):
    c.sub("Voicebox is a separate tool the box can use for voice, not part of its text path.")
    c.row("Voicebox", "Local voice cloning and synthesis, Whisper transcription, dictation, effects, stories and MCP speech tools")
    c.row("Wiring", "Your text path never calls the Voice API. A configured voicebox MCP server can still let OMP request speech while that service runs, which is not an integrated voice-conversation mode")
    c.row("Inference", "Runs locally, while model downloads, optional cloud features and connected agents can use the network")


def voice_run(c):
    c.sub("Needs Python 3.12+, Bun and just, in addition to Rust and the platform build tools.")
    c.command("just setup                     # Python environment + JavaScript dependencies")
    c.command("just dev                       # Voice backend + desktop app")
    c.command("bun run dev:server             # API only, with the Python environment active")
    c.command("just test                      # backend pytest suite")
    c.command("bun run typecheck && bun run check")
    c.row("API", "127.0.0.1:17493 owns SQLite, the models, the audio files and /mcp; development narration runs on 17494")
    c.row("MCP", "voicebox.speak, voicebox.transcribe, voicebox.list_captures and voicebox.list_profiles; voicebox-mcp is the stdio-to-HTTP shim")
    c.note("Packaged narration-worker startup has a known argument mismatch, and headless playback "
           "covers queued speech, not unclaimed narration streams; see the research map. "
           "Development data lives in data/, and the internals and updater still use upstream "
           "Voicebox naming and releases. Do not expose the API as if it were authenticated: most "
           "endpoints have none, and loopback binding with CORS is not an authorization layer. The "
           "retained Python /mini/* and /harnesses routes need loopback plus an install token, and "
           "they do not power the box.")


# ── Status and direction ──────────────────────────────────────────────────
def status(c):
    c.head("Available in the box")
    c.bullets([
        "Text conversation and agent tools",
        "Model selection, sign-in, usage and backups",
        "Its own Oh-my-pi inside the app, or yours with /harness; /update for the newest checked one",
        "Structured Markdown in a compact transcript",
        "An independent macOS app",
    ])
    c.head("Planned or incomplete", MUTED)
    c.bullets([
        "Background tasks, activity tray and workspace selection",
        "First-run guidance, /logout and a sign-in timeout",
        "A one-command install",
        "A resizable transcript and repeatable visual checks",
        "Cloned-voice conversation, the optional pet and other platforms",
    ], muted=True)
    c.note("The latest display and some provider flows still need hands-on validation, and no real "
           "provider quota exhaustion has been verified. See the plans for evidence and open "
           "decisions rather than assuming every implemented path is finished.")


def layout(c):
    c.raw("Mini/                  Null app: Rust host, static Page/, install scripts\n"
          "app/                   Shared Voice React UI\n"
          "tauri/                 Voice desktop host\n"
          "web/                   Voice browser host\n"
          "backend/               Voice API, engines, MCP, SQLite and Python harness\n"
          "scripts/               Voice lifecycle, packaging and development helpers\n"
          "data/                  Voice development data\n"
          "docs/ · landing/       Inherited documentation and marketing sites\n"
          "Context/               Plans, decisions and research\n"
          "CHANGELOG.md           Voice changelog, also read by the web build")
    c.note("The unmerged capital-folders branch capitalizes the existing directories and removes the "
           "inherited sites and other upstream material; the paths above describe the current "
           "checkout. Memories/ holds local handoffs and is not tracked in Git.")


def reasoning(c):
    for key, value in [
        ("MiniApp.md", "The app design"),
        ("Providers.md", "Sign-in, usage and fallbacks"),
        ("OwnHarness.md", "Why Null carries its harness, and the check that guards it"),
        ("ConversationDisplay.md", "How a reply is laid out"),
        ("NullMini.md", "The later phases"),
        ("ADR/", "The decisions that constrain changes"),
        ("Research.md", "The codebase map, including what was never traced"),
    ]:
        c.row(key, value)


def license_card(c):
    c.row("MIT", "See LICENSE")
    c.row("Voices", "Clone only voices you own or have permission to use")
    c.row("Also", "RESPONSIBLE_USE.md · SECURITY.md · CONTRIBUTING.md")
    c.note("Built by dominionism. A small box, with your agent behind it.")


add("surface", "Small surface. Real agent.", surface)
add("step-harness", "1. The harness comes with Null", step_harness)
add("step-build", "2. Build and install", step_build)
add("step-open", "3. Open the box", step_open)
add("uninstall", "Uninstall", uninstall)
add("use-null", "Use Null", use_null)
add("commands", "A few commands. No settings window.", commands)
add("provider-stops", "When a provider stops answering", provider_stops)
add("privacy", "Privacy and permissions", privacy)
add("signing", "Signing and permission persistence", signing)
add("what-null-keeps", "What Null keeps", what_null_keeps)
add("under-the-hood", "Under the hood", under_the_hood)
add("source-map", "Source map", source_map, key_cols=31)
add("development", "Development", development)
add("dev-switches", "Development switches and smoke checks", dev_switches, key_cols=30)
add("voice", "Optional voice tools", voice)
add("voice-run", "Run or develop the Voice stack", voice_run)
add("status", "Status and direction", status)
add("layout", "Repository layout and ongoing cleanup", layout)
add("reasoning", "Read the reasoning", reasoning)
add("license", "License", license_card)

for name, (title, svg) in CARDS.items():
    ET.fromstring(svg)   # a card that is not well-formed XML never reaches the README
    with open(os.path.join(OUT, f"{name}.svg"), "w") as handle:
        handle.write(svg)
    h = re.search(r'height="(\d+)"', svg)
    print(f"wrote {name}.svg  ({W}x{h[1] if h else '?'}, {len(svg) // 1024} KB)")

nav = "\n  &nbsp;\n".join(chip(slug) for slug in NAV)
print(f'<p align="center">\n{nav}\n</p>')

for block in link_rows():
    print()
    print(block)
