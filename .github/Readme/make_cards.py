#!/usr/bin/env python3
"""Build the README's section cards: one terminal window per section.

    python3 .github/Readme/make_cards.py

Each card is the same window as the profile's cards, carrying that section's essence in terminal
form: a shell session where the section shows commands, key/value rows or bullets where it does not.
Edit a card's data below and rerun; nothing else in the repository is read.

Every command shown is one a reader can run. Output lines are only those the tools really print
(`Mini/Scripts/install` says the install line, `ls` prints the names), so a card never invents a
result.
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

    def __init__(self):
        self.svg, self.css, self.y, self.t, self.n = [], [], TOP, 0.0, 0
        self.rows = 0
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
        self._text(VALUE_X, value)
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
            self.svg.append(f'  <g class="rise" {delay(self.t)}>{text_line(PAD, self.y, wrap(line, cols(PAD))[0], color=MUTED if muted else TEXT).strip()}</g>\n')

    def done(self):
        self.y += LH + 10
        self.svg.append(idle_prompt(self.y, self.t + 0.1))

    def render(self, title, aria):
        # A card with a command is a session, so it ends with a fresh prompt like a real terminal;
        # a card of rows or bullets is a panel and simply ends, as the profile's cards do.
        if self.n:
            self.done()
        label = f"{title}. " + " ".join(self.label)
        return title, aria or label, window(round(self.y + 26), title, "".join(self.svg), "".join(self.css), label, heading=True)


CARDS = {}


def build():
    c, title, aria = Card(), "Small surface. Real agent.", None
    c.sub("Press Control+Space from the app you are in. Type, and your agent answers in the box.")
    c.bullets([
        "**Your tools, not just chat.** Null drives your installed Oh-my-pi harness: its tools, its "
        "configured MCP servers, its approval mode.",
        "**Your providers, one conversation.** Switch models, read usage, keep an ordered fallback "
        "list. OMP owns the sign-ins and the retries.",
        "**Readable without getting bigger.** Streamed Markdown, folded tool steps, and code, tables "
        "and diagrams that scroll sideways instead of wrapping.",
        "**Today:** a standalone macOS text app, built from source. Voice, background tasks, "
        "workspaces and the pet are planned, not built.",
    ])
    CARDS["surface"] = c.render(title, aria)

    c = Card()
    c.sub("macOS 13+ · Rust · Xcode Command Line Tools · Tauri CLI 2. No Node, Bun or Python is "
          "needed for the box.")
    c.command("git clone https://github.com/dominionism/Null.git && cd Null")
    c.output("Cloning into 'Null'...")
    c.command("Mini/Scripts/install")
    c.output("Installed /Applications/Null.app and started it. Control+Space opens the box.")
    CARDS["get-started"] = c.render("Get started", None)

    c = Card()
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
    CARDS["use-null"] = c.render("Use Null", None)

    c = Card()
    for key, value in [
        ("/model [filter]", "Choose a model from OMP's catalogue, across providers, without starting a new conversation"),
        ("/login", "Follow the harness's own sign-in steps inside the box, browser sign-in or key entry"),
        ("/usage", "See the harness's usage reports and reset times; providers without a report are marked"),
        ("/backup [filter]", "Set an ordered fallback list: Enter adds or removes a model, Esc saves it"),
        ("/new", "Start a fresh conversation on the last selected model"),
        ("/quit", "Quit and clear the saved conversation pointer"),
    ]:
        c.row(key, value)
    c.note("A lone unknown /word is refused rather than passed to the agent; other messages go "
           "through normally. A listed model is not a guarantee your account can use it.")
    CARDS["commands"] = c.render("A few commands. No settings window.", None)

    c = Card()
    for key, value in [
        ("No listener", "Null has no HTTP port and no database: it starts OMP as a child process and talks over stdio"),
        ("Credentials", "Your /login answers go to the local omp login process, never to Null's settings or log"),
        ("Shortcut", "Control+Space is registered by macOS with no Accessibility or Input Monitoring permission"),
        ("Full Disk Access", "A separate choice: macOS attributes the agent's file access to Null, and the grant is broad file access, not a sandbox"),
        ("Signing", "Permission grants follow the code signature, so an unsigned rebuild can ask again"),
    ]:
        c.row(key, value)
    CARDS["privacy"] = c.render("Privacy and permissions", None)

    c = Card()
    c.sub("One native Tauri app plus the OMP process it launches. The page is a static HTML file: no "
          "web server, bundler or Node runtime.")
    c.command("ls Mini/src")
    c.output("access.rs      backups.rs    harness.rs    log.rs        main.rs       markdown.rs", muted=True)
    c.output("panel.rs       providers.rs  settings.rs   shortcut.rs   signin.rs     translate.rs", muted=True)
    c.note("Null carries OMP's approval mode and MCP definitions into the session; the intent is your "
           "terminal agent in a smaller surface. A Content Security Policy is not configured yet.")
    CARDS["under-the-hood"] = c.render("Under the hood", None)

    c = Card()
    c.command("cd Mini && cargo test        # offline unit tests; live ones are ignored")
    c.command("cargo build                  # local debug binary")
    c.command("cargo tauri build            # app bundle, no installation")
    c.note("Use cargo tauri, not bunx tauri: the latter names a different npm package. Repository CI "
           "checks the Voice frontend and web build, not Mini/.")
    CARDS["development"] = c.render("Development", None)

    c = Card()
    c.sub("Voicebox is a separate tool the box can use for voice, not part of its text path.")
    for key, value in [
        ("Voicebox", "Local voice cloning and synthesis, Whisper transcription, dictation, effects, stories and MCP speech tools"),
        ("Wiring", "OMP can call a voicebox MCP server while that service runs; there is no integrated voice-conversation mode yet"),
        ("API", "127.0.0.1:17493 for REST and /mcp; narration on 17494; voicebox-mcp is the stdio-to-HTTP shim"),
        ("Warning", "Most endpoints have no authentication, and loopback binding is not an authorization layer"),
    ]:
        c.row(key, value)
    c.command("just setup && just dev")
    CARDS["voice"] = c.render("Optional voice tools", None)

    c = Card()
    c.head("Available in the box")
    c.bullets([
        "Text conversation and agent tools",
        "Model selection, sign-in, usage and backups",
        "Structured Markdown in a compact transcript",
        "An independent macOS app",
    ])
    c.head("Planned or incomplete", MUTED)
    c.bullets([
        "Background tasks, activity tray and workspace selection",
        "First-run guidance, /logout and a sign-in timeout",
        "A resizable transcript and repeatable visual checks",
        "Cloned-voice conversation, the optional pet and other platforms",
    ], muted=True)
    c.note("The latest display and some provider flows still need hands-on validation, and no real "
           "provider quota exhaustion has been verified.")
    CARDS["status"] = c.render("Status and direction", None)

    c = Card()
    for key, value in [
        ("MiniApp.md", "The app design"),
        ("Providers.md", "Sign-in, usage and fallbacks"),
        ("ConversationDisplay.md", "How a reply is laid out"),
        ("NullMini.md", "The later phases"),
        ("ADR/", "The decisions that constrain changes"),
        ("Research.md", "The codebase map, including what was never traced"),
    ]:
        c.row(key, value)
    CARDS["reasoning"] = c.render("Read the reasoning", None)

    c = Card()
    c.row("MIT", "See LICENSE")
    c.row("Voices", "Clone only voices you own or have permission to use")
    c.row("Also", "RESPONSIBLE_USE.md · SECURITY.md · CONTRIBUTING.md")
    c.note("Built by dominionism. A small box, with your agent behind it.")
    CARDS["license"] = c.render("License", None)


build()
for name, (title, aria, svg) in CARDS.items():
    ET.fromstring(svg)   # a card that is not well-formed XML never reaches the README
    path = os.path.join(OUT, f"{name}.svg")
    with open(path, "w") as handle:
        handle.write(svg)
    h = re.search(r'height="(\d+)"', svg)
    print(f"wrote {name}.svg  ({W}x{h[1] if h else '?'}, {len(svg) // 1024} KB)")
