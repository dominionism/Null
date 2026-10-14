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
        "**Today:** a standalone macOS text app with a small pet beside it, installed with one "
        "command. Several conversations can work at once, each in a folder you choose. Voice is "
        "planned, not built.",
    ])
    c.note("The pet above is the one beside the box, moving as it does there: at rest, at work while a "
           "reply is under way, and hopping when the reply ends.")


# ── Get started ───────────────────────────────────────────────────────────
def step_harness(c):
    c.sub("Nothing to install first. Null carries one checked version of Oh-my-pi (omp) inside the app.")
    c.row("Your folder", "It works from OMP's usual folder, ~/.omp: existing sign-ins, skills and MCP servers are used as they are. With none, run /login inside Null.")
    c.row("Managed by OMP", "Providers, subscriptions and API keys. Their availability and terms still apply.")
    c.row("Your own copy", "/harness switches to an Oh-my-pi found on PATH, then in ~/.omp/bin, ~/.opencode/bin, ~/.local/bin, /opt/homebrew/bin or /usr/local/bin, and back")
    c.note("The carried version is named in Mini/Engine.toml and moves only after the repository's "
           "harness check has passed it. Apple Silicon Macs only, for now.")


def step_build(c):
    c.sub("An Apple Silicon Mac with macOS 13 or later. Nothing else: no Rust, no developer tools, "
          "no Node.")
    c.command("curl -fsSL \\")
    c.command("  https://raw.githubusercontent.com/dominionism/Null/main/Mini/Scripts/null \\")
    c.command("  | sh -s install")
    c.output("Installed Null 0.1.0 in /Applications and started it. Control+Space opens the box.")
    c.note("The command fetches the newest released Null (about 100 MB) and keeps it only if it has "
           "the published checksum, its seal holds, it is Null, and the Oh-my-pi inside carries its "
           "author's signature. Then it signs the app on your Mac, puts it in /Applications, "
           "registers a login item and starts it, replacing a Null that is there. The script is "
           "Mini/Scripts/null: read it first if you like. Null has no Apple certificate, so the "
           "same file downloaded with a browser would be stopped by macOS.")
    c.head("Or build it yourself")
    c.command("xcode-select --install           # once, if the Apple tools are not installed")
    c.command('cargo install tauri-cli --version "^2" --locked')
    c.command("git clone https://github.com/dominionism/Null.git && cd Null")
    c.command("Mini/Scripts/install")
    c.note("That needs the Rust toolchain, Xcode Command Line Tools and Tauri CLI 2. The script "
           "fetches the Oh-my-pi named in Mini/Engine.toml (about 200 MB), builds Null.app, and "
           "signs and installs it the same way.")


def step_open(c):
    c.row("First time", "With no provider signed in, the box opens by itself, once, and shows the sign-in list")
    c.row("Control+Space", "Open the box")
    c.row("/login", "Add a provider, if you need one")
    c.row("/model", "Choose one of its models")
    c.row("Then", "Type your request. You do not need to leave a terminal running.")


def uninstall(c):
    c.command("curl -fsSL \\")
    c.command("  https://raw.githubusercontent.com/dominionism/Null/main/Mini/Scripts/null \\")
    c.command("  | sh -s uninstall")
    c.output("Null is removed. Settings stay in ~/Library/Application Support/io.github.dominionism.null-mini.")
    c.row("From a clone", "Mini/Scripts/uninstall does the same")
    c.row("Removes", "/Applications/Null.app, and the login item at ~/Library/LaunchAgents/io.github.dominionism.null-mini.plist")
    c.row("Left behind", "Settings, workspace files and logs, in ~/Library/Application Support/io.github.dominionism.null-mini/ and ~/Library/Logs/Null/, and Null's signing identity, in ~/Library/Application Support/Null Signing/, so that a Null installed later is the same app to macOS")
    c.note("OMP's own sign-ins and conversation history are not removed.")


# ── Use Null ──────────────────────────────────────────────────────────────
def use_null(c):
    for key, value in [
        ("Control+Space", "Show or hide the box"),
        ("Enter", "Send a message, or choose a list entry"),
        ("Esc", "Close a picker, cancel sign-in, or hide the box"),
        ("Ctrl+C", "Stop the reply; when idle, quit and close the conversation, after one warning if others are still working"),
        ("↑", "Recall the last prompt in an empty field"),
        ("/ then Enter", "List the box's commands and the harness's, each with a few words"),
        ("Drag", "Move the box; click away hides it"),
    ]:
        c.row(key, value)
    c.note("The arrow pulses while the agent works and turns amber when it asks. The pet beside the "
           "box shows the same, hops once when a reply ends and sinks when one fails. The transcript "
           "grows to about ten lines, then scrolls. Hiding the box does not stop a reply, and neither "
           "does /new or opening another conversation: it goes on working, the pet speaks for it, "
           "and /history lists it first.")


def commands(c):
    for key, value in [
        ("/model [filter]", "Choose a model from OMP's catalogue, across providers, without starting a new conversation"),
        ("/login", "Follow the harness's own sign-in steps inside the box, browser sign-in or key entry"),
        ("/usage", "See the harness's usage reports and reset times; providers without a report are marked"),
        ("/backup [filter]", "Set an ordered fallback list: Enter adds or removes a model, Esc saves it"),
        ("/verify [filter]", "Name a second model that checks each answer once it is given, or nobody. The answer is shown first. Then the box says who verifies it, and that model says that the answer stands or what to correct. Each one costs a reply from that model"),
        ("/harness", "See which Oh-my-pi the box runs, the built-in one or your own, and switch between them"),
        ("/update", "Move the built-in Oh-my-pi to the newest version the repository's harness check has passed, and say when a newer Null is out and what gets it"),
        ("/attach [path]", "Hand the agent a file with the next message: choose it from a list of your folders inside the box, or give its path. The plus at the end of the row does the same"),
        ("/history [words]", "List the earlier conversations, the box's own and the terminal's, and open one again. Typing narrows the list by title and folder, then adds the conversations in which those words were said. Tab sorts it by folder. On a row, the pencil renames and the x moves the conversation to the Trash. Conversations that are working, asking or not yet looked at come first, under \"now\". The button after the plus opens the same list"),
        ("/rename <title>", "Give the open conversation a title of your own. In /history, Command+R does it for the chosen row, and Command+Delete moves that row to the Trash"),
        ("/pet", "Show the pet beside the box, or put it away"),
        ("/computer", "See whether the agent may see the screen and work the apps on this Mac, and switch it. While it is on, the list also says how the two permissions macOS wants for it stand, and Enter on a missing one asks macOS for it"),
        ("/browser", "Let the agent work your own Chrome, with your sign-ins, through OMP's browser link, shown in Chrome as Null. The list sets Chrome up: Null makes the extension, opens Chrome's extensions page for you to load it once, and says when Chrome is linked"),
        ("/folder [path]", "Start a new conversation in a folder of your choosing, where the agent has that project's files and instructions: pick one that conversations already work in, walk to another, or give its path. The box says where each time such a conversation comes to the front"),
        ("/new", "Start a fresh conversation on the last selected model, in the box's own folder. A conversation that is working goes on; at most four work at once"),
        ("/quit", "Quit and clear the saved conversation pointer. With conversations still working it says how many first, and quits when asked again"),
        ("/<name>", "OMP's own commands and the skills it finds, your Playbook among them, are listed under the box's own, and one is carried out there when you type it by the name the list shows, arguments and all"),
    ]:
        c.row(key, value)
    c.note("A lone /word is not sent to the agent as a message: it opens the list, which holds both "
           "the box's own commands and the ones the harness says it can carry out, each with a few "
           "words. A name only the harness knows is sent as the harness spells it, so a skill of "
           "yours is typed by the name you know it by. A name neither has is not sent at all. Other "
           "messages go through normally. A listed model is not a guarantee that your account can use "
           "it: providers with reported limits exhausted are dimmed but stay selectable, and a model "
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
    c.row("Attached files", "Null reads no file's content. A file you attach goes to the harness as its full path, which travels to your provider with the message, your user name in it. The content follows only when the agent reads the file")
    c.row("Conversations", "OMP keeps your conversations, one file each. To find one by something said in it, Null reads those files on this Mac as you type in /history, keeps nothing of them and logs no word. Deleting moves a conversation's file to the Mac's Trash, where it can be put back, and changes nothing in it")
    c.row("Shortcut", "Control+Space needs no Accessibility or Input Monitoring permission. If registration fails, the box opens with a notice")
    c.row("Approvals", "Null passes on OMP's approval setting and adds none of its own. OMP's own default lets the agent act without asking you first, and the first opening says so")
    c.row("Computer use", "Off until you switch it on with /computer. Then the agent may read what any window holds and press and type in any app, and, with Screen Recording, take pictures of the screen. OMP does this, not Null: Null neither looks at the screen nor keeps what the agent saw. While it is on, Null also tells the agent its own rules for working a Mac: the ways in, few turns, and leaving your things alone. They are the same for everyone and say nothing about you. It follows OMP's approval setting like any other tool, so with OMP's default the agent uses your Mac without asking you first. Set to ask, this OMP cannot put its question through the box and refuses computer use")
    c.row("Browser link", "Off until you set Chrome up and switch it on with /browser. Then the agent may work your Chrome as you, on any site you are signed in to, in tabs of its own, gathered in a tab group named Null. OMP does this through its own extension, which Null copies under its name; your sign-ins stay in Chrome. While the link's relay runs, it listens on this Mac only, refuses web pages, and takes no password: another program on this Mac could drive Chrome through it, as with OMP's own")
    c.row("Verifying", "Off until you name a model with /verify. Then, when a reply that took steps has ended, Null moves that conversation to the model you named, sends it one message of Null's own that asks it to check the answer against what was seen, and puts the conversation back. While a backup is answering, the conversation is not moved: the model that answered is asked instead. It is the only message Null sends that you did not type, and the box says so each time. Conversations started with /folder are left alone")
    c.row("Its permissions", "Two: Accessibility and Screen Recording, which macOS charges to Null because Null starts the agent. Null asks macOS for them only when you switch computer use on or choose one in the /computer list, never at a start. Screen Recording may be left off: the agent then reads and presses, and has no pictures")
    c.row("Full Disk Access", "A separate choice, asked once, after the first reply, with an explanation: macOS attributes the agent's file access to Null. It is broader file access, not a sandbox, and it does not replace OMP's approval rules. The box opens without it")


def signing(c):
    c.bullets([
        "macOS ties permission grants to the application's code signature, so an unsigned rebuild "
        "can require permission again.",
        "The installer signs with a Null Local Signing identity kept on your Mac, in "
        "~/Library/Application Support/Null Signing/: a keychain of its own and the password file "
        "beside it. On a Mac that has none it makes one, without asking you anything: macOS is never "
        "told to trust the certificate, it only has to be the same one each time. If signing fails, "
        "it installs the build unsigned, with a notice.",
        "The Oh-my-pi inside the app is left as published, with its author's Developer ID signature. "
        "The installer signs the outer app only, and that signature covers it.",
    ])
    c.note("That identity belongs to one Mac and is not a distributed Developer ID. Protect both "
           "the keychain and its password: a process with access to them can sign as Null, and so "
           "inherit what macOS has allowed Null. Whether macOS keeps its answers across an update "
           "signed this way is still to be seen on a second Mac.")


def what_null_keeps(c):
    c.sub("Under ~/Library/Application Support/io.github.dominionism.null-mini/")
    c.row("settings.json", "Window position, chosen model, saved session ID, Full Disk Access prompt state, whether the first opening has happened, backup order, your own harness if you chose it, whether the pet shows, whether computer use is switched on, the thinking level you last set with /effort, the model you named to verify answers, and whether the browser link is on and Chrome was seen to load it")
    c.row("history.json", "A title for each conversation made in the box: the first line of its first message, at most 80 characters, since OMP gives none for these. Any title you gave with /rename. And, while there are any, which conversations are working or not yet looked at: an id and one word each, so that work cut off by a restart is found again")
    c.row("routing.json", "What each of the last 500 replies cost, in numbers: how many steps it took, how many of them failed, how long it ran, when it ended and how, the model and its thinking level, whether the conversation was in the box's own folder, and whether the reply was a second model verifying. No word that was typed, shown or read, and no conversation's id or title. Nothing is done with the numbers yet, and they do not leave this Mac")
    c.row("harness.yml", "Settings handed to OMP at every start: no update check, your backup order when there is one, and computer use on or off once you have used its switch. Rewritten each time")
    c.row("rules.md", "While computer use or the browser link is on: Null's own rules for each, which OMP adds to what it tells the model. The same words for everyone, carried in the app. Rewritten at every start, and removed when both are off")
    c.row("BrowserLink/", "Null's copy of OMP's browser extension, under Null's name, which Chrome loads from here. Made when you set Chrome up")
    c.row("Workspace/", "Default working directory for conversations; not a filesystem access boundary")
    c.note("Your own OMP settings are not modified. OMP "
           "owns the transcript and the credentials: a restart resumes the saved session, while "
           "/quit and idle Ctrl+C clear Null's pointer, not OMP's stored transcript, which /history "
           "finds again. A conversation you delete goes to the Mac's Trash. Logs are in "
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
        "A command the harness itself offers, its skills among them, is carried out there when you "
        "type it: the list shows both the box's commands and the harness's, and a name only the "
        "harness knows is passed through as the harness spells it. Its thinking and full tool "
        "output are still not shown.",
        "Replies are parsed into a Markdown tree in Rust and rendered with DOM elements and "
        "textContent, so model-authored HTML is never executed. **A Content Security Policy is not "
        "configured yet**, and careful rendering is not a substitute for one.",
    ])


def source_map(c):
    for key, value in [
        ("Page/index.html", "Input, commands, transcript, layout, scrolling and selection"),
        ("main.rs", "Startup, plugins, state and command registration"),
        ("panel.rs, shortcut.rs", "Window, placement, sizing, the pet's room beside the box, visibility and Control+Space"),
        ("harness.rs, translate.rs", "ACP process and session lifecycle, approvals, events, protocol translation"),
        ("attach.rs", "The files that go with the next message, and the list of folders they are chosen from"),
        ("history.rs", "Earlier conversations as the harness lists them, their titles and renaming, opening one again, and moving one to the Trash"),
        ("search.rs", "Finding a conversation by something said in it: OMP's files are read as you type, and nothing is kept"),
        ("engine.rs", "Which Oh-my-pi runs: the carried one or your own, and /harness"),
        ("providers.rs, signin.rs", "Usage reports and harness-owned sign-in"),
        ("welcome.rs", "The first opening: with no provider signed in, the box opens once with the sign-in list"),
        ("backups.rs", "The fallback order written for OMP, and the switch reporting it"),
        ("computer.rs", "The switch for computer use: what is handed to OMP for it, and how it stands"),
        ("rules.rs", "Null's own rules (Mini/Rules/), told to the agent while computer use or the browser link is on"),
        ("browser.rs", "The browser link: Null's copy of the extension, setting Chrome up and waiting for it, and the switch"),
        ("verify.rs", "The second model that checks an answer: when it is asked, and the words it is asked with"),
        ("routing.rs", "What each reply cost, kept in numbers: its steps, the ones that failed, how long it ran, the model, and how it ended"),
        ("markdown.rs", "Reply text into structured parts, including partial streamed input"),
        ("settings.rs, access.rs, log.rs", "Persistence, what macOS allows Null (Full Disk Access, and the two permissions of computer use), and logging"),
        ("check.rs, standin.rs", "The harness check: live tests of what Null relies on, against the stand-in provider that scripted runs also use"),
    ]:
        c.row(key, value)


# ── Development ───────────────────────────────────────────────────────────
def development(c):
    c.command("Mini/Scripts/engine          # once: fetch the Oh-my-pi the build takes in")
    c.command("cd Mini && cargo test        # offline unit tests; live ones are ignored")
    c.command("cargo build                  # local debug binary")
    c.command("cargo tauri build            # app bundle, no installation")
    c.command("python3 -m unittest discover -s Mini/Bench   # the browser link's bench, its checkers")
    c.note("Use cargo tauri, not bunx tauri: the latter names a different npm package. "
           "cargo test -- --ignored is the harness check: live tests on the carried Oh-my-pi with a "
           "stand-in provider, one of which reaches a real provider with a dummy key. With them "
           "runs the key check, which opens the box for a few seconds, signs in with a marked "
           "dummy key and searches Null's log and settings for it, and the box check: about thirty "
           "runs of the real app, each one thing a person does in the box, on folders of their own "
           "with the stand-in provider. Those take the keyboard for some minutes, so leave the Mac "
           "alone and unlocked meanwhile. "
           "NULL_MINI_ENGINE=<path> runs the harness check on another Oh-my-pi, and "
           "Mini/Scripts/engine --to <version> moves Null to that version only if it passes. "
           "NULL_MINI_CHROME=<Chrome for Testing> lets one question load Null's copy of the browser "
           "link's extension in a Chrome of its own. "
           "Repository CI runs all three on a Mac runner whenever Mini/ changes. Mini/Bench/ measures "
           "the browser link on made-up signed-in sites; python3 Mini/Bench/bench.py check proves, "
           "with no model, that its runs cannot reach your own Chrome.")


def dev_switches(c):
    for key, value in [
        ("EXIT_WHEN_READY=1", "Exit after the page loads; no prompt is sent"),
        ("NO_SHORTCUT=1", "Show at startup without claiming Control+Space"),
        ("SMOKE=\"<text>\"", "Send a real harness prompt, log the reply, exit without a window; also SMOKE_MODEL and SMOKE_STOP_AFTER (seconds)"),
        ("SELFTEST=\"<text>\"", "Type into the real page, a line at a time, each once the page has stopped working: a message, a command, a choice from a list, an answer to a sign-in. A line that ends in \" &\" does not wait for its reply. Then log what the page shows and exit"),
        ("PROFILE=<name>", "Pass an isolated profile to OMP and use profile-specific Null settings and harness-settings files"),
        ("BACKUPS=\"m/one,m/two\"", "Override the backup order for this run without changing the saved order"),
        ("UPDATES=<address>", "Where /update reads which Oh-my-pi is checked, in place of the repository: a made-up file as a file:// address"),
        ("RELEASE=<address>", "Where /update learns which Null is the newest, in place of GitHub: a made-up address whose last part is a tag, null-v0.2.0"),
        ("FOLDER=<path>", "Run on that folder in place of yours, made if missing: OMP's folder, its MCP definitions and Null's own files are all under it, so a new one is a Mac with nothing signed in"),
        ("LINK_COPY=<folder>", "Make Null's copy of the browser link's extension in that folder, as /browser makes it, and exit without a window; LINK_ENGINE names the Oh-my-pi to make it from. For a Chrome of its own, such as the bench's"),
        ("STANDIN=1", "Offer a stand-in provider in that folder, so a message is answered or refused with no real sign-in: standin-anthropic/ok, /limit, /auth, /noaccess, and /slow, which says the message back a word at a time; the same for openai and codex; needs FOLDER. Told to by a message's last line (ask the shell: <command>), standin-anthropic/ok asks the harness for that tool, as a model would"),
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


# ── Status and direction ──────────────────────────────────────────────────
def status(c):
    c.head("Available in the box")
    c.bullets([
        "Text conversation and agent tools",
        "Model selection, sign-in, usage and backups",
        "Its own Oh-my-pi inside the app, or yours with /harness; /update for the newest checked one",
        "Structured Markdown in a compact transcript",
        "An independent macOS app, installed with one command",
        "A first opening that leads a newcomer to sign in",
        "Files attached to a message, and every earlier conversation found again with /history",
        "Conversations renamed, moved to the Trash, and found by something said in them",
        "A small pet beside the box that shows what the agent is doing",
        "Work in the background: several conversations at once, each seen at a glance, in a folder you choose",
        "Computer use, switched on with /computer: Oh-my-pi reads windows and works the Mac's apps",
        "The browser link, set up with /browser: the agent works your own Chrome as you, in tabs of its own, under Null's name",
        "A second model that checks each answer, named with /verify",
    ])
    c.head("Planned or incomplete", MUTED)
    c.bullets([
        "/logout and a sign-in timeout",
        "An npm package around the install command",
        "A resizable transcript and repeatable visual checks",
        "Cloned-voice conversation and other platforms",
    ], muted=True)
    c.note("The latest display, the pet, attached files, history with its renaming, deleting and "
           "searching, work in the background, computer use with its two permissions, the browser link, and some "
           "provider flows still need hands-on validation, and no real "
           "provider quota exhaustion has been verified. See the plans for evidence and open "
           "decisions rather than assuming every implemented path is finished.")


def layout(c):
    c.raw("Mini/                  Null: the Rust host in src/, the page in Page/, tests/, Scripts/\n"
          "Mini/Bench/            Made-up signed-in sites that measure the browser link, and their runner\n"
          ".github/workflows/     Null's tests, its releases, and the weekly Oh-my-pi check\n"
          ".github/Readme/        The banner and the cards of this README, and the scripts that draw them")
    c.note("Context/ and Memories/ hold the owner's own plans, decisions, research and handoffs, and "
           "are not tracked in Git.")


def license_card(c):
    c.row("MIT", "See LICENSE")
    c.note("Built by dominionism. A small box, with your agent behind it.")


add("surface", "Small surface. Real agent.", surface)
add("step-harness", "1. The harness comes with Null", step_harness)
add("step-build", "2. Install", step_build)
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
add("status", "Status and direction", status)
add("layout", "Repository layout", layout)
add("license", "License", license_card)

for name, (title, svg) in CARDS.items():
    ET.fromstring(svg)   # a card that is not well-formed XML never reaches the README
    with open(os.path.join(OUT, f"{name}.svg"), "w") as handle:
        handle.write(svg)
    h = re.search(r'height="(\d+)"', svg)
    print(f"wrote {name}.svg  ({W}x{h[1] if h else '?'}, {len(svg) // 1024} KB)")
