<p align="center">
  <img src="Mini/Icons/128x128@2x.png" alt="Null" width="112" height="112" />
</p>

<h1 align="center">Null</h1>

<p align="center">
  <strong>A small box you open with <kbd>Control</kbd>+<kbd>Space</kbd> and type at your own agent.</strong><br/>
  Your harness, your sign-ins, your models, your tools. Nothing leaves the machine.<br/>
  macOS.
</p>

Null is a floating text box with no window chrome, no menu bar and no Dock icon. You press
Control+Space anywhere, type a question or an instruction, and the agent CLI you already use answers
in the box. Everything that makes the agent *your* agent carries over: the same sign-ins, the same
model list across providers, the same tools, the same MCP servers, the same approval mode. Null holds
none of it.

It runs as a single process. There is no server, no HTTP surface and no database: the box starts
Oh-my-pi as a child process and speaks the Agent Client Protocol to it over stdio.

**Two things live in this repository.**

| | What it is | State |
| --- | --- | --- |
| **Null** (`Mini/`, `null-mini`) | The box described above | The current product |
| **The voice stack** (Voicebox: `app/`, `tauri/`, `web/`, `backend/`) | Local voice cloning, speech generation, dictation, agent speech | Inherited; planned as Null's voice-to-voice mode, not wired to the box yet |

The two are independent by decision (see [ADR 0002](Context/ADR/0002-NullMiniIsItsOwnApp.md)): the box
must keep working when the voice stack is not running. See
[The voice stack](#the-voice-stack-voicebox) below for what it is and how it will connect.

## Requirements

- **macOS 13 or later.** The box is macOS-only on purpose and will not build elsewhere.
- **[Oh-my-pi](https://github.com/can1357/oh-my-pi)** (`omp`) installed and signed in — it is the
  agent the box drives today. Null finds it on `PATH`, in `~/.omp/bin`, `~/.opencode/bin`,
  `~/.local/bin`, `/opt/homebrew/bin` or `/usr/local/bin` (a login-launched app gets a minimal
  `PATH`, so the install folders are checked).
- To build it: the Rust toolchain and the Tauri 2 CLI, `cargo install tauri-cli --version "^2"`.

There is nothing to sign in to in Null itself and no API key to paste: provider credentials belong to
the harness, and Null never reads, stores or forwards them.

## Install

```sh
Mini/Scripts/install
```

That builds the app (`cargo tauri build` inside `Mini/`), copies `Null.app` to `/Applications`,
writes a login item at `~/Library/LaunchAgents/io.github.dominionism.null-mini.plist`, and starts it.
From then on it comes back at login.

```sh
Mini/Scripts/uninstall   # stops it, removes the login item and /Applications/Null.app
```

Settings and logs are left behind by the uninstall script; delete
`~/Library/Application Support/io.github.dominionism.null-mini` and `~/Library/Logs/Null` to be rid
of them.

**Two permission notes, both deliberate.**

- Control+Space is registered with macOS as an ordinary system-wide shortcut, so the box needs **no**
  Accessibility or Input Monitoring permission. The cost is that no other app gets Control+Space
  while Null is running.
- macOS holds Null responsible for whatever the agent it starts reads. Without **Full Disk Access**
  it asks about each protected folder in turn, and an agent searching your home folder sets off a run
  of those questions. Null checks for it once at start and, if it is off, opens the Full Disk Access
  list in System Settings and says why in the box. There is no way for an app to ask for it directly;
  turning it on is your call.

**Signing.** macOS ties the answers to those permission questions to the code signature. If the build
is unsigned, every rebuild counts as a new app and asks again. If
`~/Library/Application Support/Null Signing/signing.keychain-db` exists (with its `password` file
beside it), the install script signs with the local identity `Null Local Signing` so the answers
hold; without it, it installs unsigned and says so.

## Using the box

| Key | What it does |
| --- | --- |
| Control+Space | Opens the box, or puts it away |
| Enter | Sends what you typed |
| Esc | Puts the box away |
| Ctrl+C | Stops the reply in progress; with nothing running, quits — which closes the conversation |
| Click away | Puts the box away |
| Drag anywhere but text | Moves the box; the position is remembered |

The arrow at the left is the whole status display: it pulses while the agent works and turns amber
while it waits for your answer. There is no other indicator, by design — the box is the text field
and the arrow until there is something to say. It grows as a reply arrives, up to about ten lines of
transcript, and scrolls past that.

**Commands.** Anything else you type goes to the agent. A lone unknown `/word` is refused with the
list rather than sent.

| Command | What it does |
| --- | --- |
| `/model [filter]` | Lists every model the harness can reach across all its providers, and switches; the conversation is kept. Providers with nothing left for now are dimmed. |
| `/usage` | What each provider has left, read from the harness's own report (`omp usage --json --redact`). Local models and key-only providers simply show no report. |
| `/backup [filter]` | Opens the same model list to set the order to fall back through when the model in use stops answering: Enter adds or takes out the model under the cursor, Esc is done. Whatever you type filters the list. |
| `/login` | Runs the harness's own sign-in inside the box: its provider list, its questions, your answers. Anything you type goes only to that process on the other end of a pipe — never to Null, its settings or its log. |
| `/new` | Starts a new conversation, on the model last chosen. |
| `/quit` | Quits and closes the conversation, so the next start is a fresh one (as a terminal program would). |

**How the fallback order works.** Null does not watch for a used-up limit, and it does not resend
anything. It hands the order you chose to the harness in a settings file of its own, which the
harness reads at start and applies to every model; your own Oh-my-pi settings are not modified. The
harness moves to the next model and sends the message again by itself. If it does, the box says so —
a model switch is never silent. A refusal from a provider that arrives as an ordinary reply is called
out too, since the harness does not always report one as an error.

## What Null does not do yet

Written down so it is not mistaken for a bug:

- No `/logout`, and no timeout on a sign-in that is waiting for you.
- The agent is not told it is talking through a small box, so it has no idea how wide the box is.
- A failed reply is not re-sent automatically after you pick another model.
- Ten lines of transcript, and no way to grow it yet.
- The page has no content-security policy beyond being local (it never builds HTML out of what a
  model writes, which is the real defence).
- No pet, no background tasks, no workspaces, no voice. See
  [Context/Plans/NullMini.md](Context/Plans/NullMini.md) for those phases.

## What it keeps, and where

The harness owns the transcript, the credentials and the tools. Null keeps only what it must:

| Path | What is in it |
| --- | --- |
| `~/Library/Application Support/io.github.dominionism.null-mini/settings.json` | Window position, the model last chosen, the id of the conversation last open, whether it has already asked about Full Disk Access, the backup order |
| `…/io.github.dominionism.null-mini/backups.yml` | The fallback order in the harness's settings format, rewritten at each start; empty when you have set no order |
| `…/io.github.dominionism.null-mini/Workspace` | The working directory of every conversation — where the agent reads and writes when you do not say otherwise |
| `~/Library/Logs/Null/mini.log` | One line per event, with a timestamp. The app's only trace: `started`, `panel ready`, `shown; frontmost app: …`, `starting the harness: …`, `reply ended: …` |

`NULL_MINI_PROFILE=<name>` runs everything under isolated files of its own
(`settings.<name>.json`, `backups.<name>.yml`) and passes `--profile <name>` to the harness, so
sign-in and first-run behaviour can be tried without touching the real ones.

## How it works

One Tauri 2 process with no server. The window is a non-activating panel: it takes typing without
becoming the active app, shows over full-screen apps and on every Space, and never steals focus
(`panel.rs` logs the frontmost app before and after showing, so this is checkable).
`Mini/Page/index.html` is one static file — no bundler, no Node — that talks to Rust through Tauri
commands and hears from it through events. A reply's Markdown is turned into a tree of parts in
`markdown.rs` and drawn by the page, element by element, with `textContent`; nothing a model writes
ever becomes HTML.

| File | What it decides |
| --- | --- |
| `Mini/src/main.rs` | Wiring: plugins, state, the command list, the development switches |
| `Mini/src/panel.rs` | The window: size, placement, show and hide, resize as the reply grows |
| `Mini/src/shortcut.rs` | Control+Space, and the notice when something else has taken it |
| `Mini/src/harness.rs` | The ACP connection: one thread, one harness process, one conversation, approvals, the event ring the page reads back from |
| `Mini/src/translate.rs` | ACP messages and session options into the box's own event vocabulary; the harness's MCP servers into what `session/new` accepts |
| `Mini/src/providers.rs`, `signin.rs`, `backups.rs` | What is left per provider, the harness's sign-in on pipes, the fallback order |
| `Mini/src/markdown.rs` | Markdown into parts. Half a reply has to read sensibly, so an open code fence is already a code block |
| `Mini/src/settings.rs`, `access.rs`, `log.rs` | The settings file, the Full Disk Access ask, the log |

## Develop the box

```sh
cd Mini
cargo test              # unit tests, inline in each module
cargo tauri build       # the app bundle; Scripts/install wraps this
```

Run `cargo tauri`, never `bunx tauri`, which fetches an unrelated npm package. Two tests are marked
`#[ignore]` because they reach the harness or a provider for real (`cargo test -- --ignored`); the
rest are offline, with the recorded payloads as fixtures.

Six switches exist for development, all read from the environment:

| Switch | What it does |
| --- | --- |
| `NULL_MINI_EXIT_WHEN_READY=1` | Quit as soon as the page has loaded — "start, load, stop" as a check |
| `NULL_MINI_NO_SHORTCUT=1` | Do not register Control+Space; show the box at start instead. For running beside the installed copy |
| `NULL_MINI_SMOKE="<text>"` | Send one message to the harness, print what comes back, quit. No window, no shortcut. Also `NULL_MINI_SMOKE_MODEL` and `NULL_MINI_SMOKE_STOP_AFTER` (seconds) |
| `NULL_MINI_SELFTEST="<text>"` | Type into the real page, wait for the reply, log what the page shows, quit |
| `NULL_MINI_PROFILE=<name>` | Isolated settings and harness profile, as above |
| `NULL_MINI_BACKUPS="m/one,m/two"` | A backup order for one run, without writing settings |

Note that `NULL_MINI_PROFILE` isolates Null's files and the harness profile, **not** the MCP servers:
those still come from the ordinary `~/.omp/agent/mcp.json`.

Nothing in `Mini/` is covered by this repository's CI, which runs the frontend typecheck and the web
build only.

## The voice stack (Voicebox)

`app/`, `tauri/`, `web/` and `backend/` are the local voice stack inherited from
[Voicebox](https://github.com/jamiepine/voicebox): clone a voice from a few seconds of audio, generate
speech in it across seven TTS engines, dictate into any app from a global chord, transcribe with
Whisper, edit multi-voice stories on a timeline, apply audio effects, and give an MCP-aware agent a
voice you own — all inference on your machine, nothing sent anywhere.

It is here because it is the layer Null will use for voice-to-voice mode later. **It is not wired to
the box today**: no code in `Mini/` calls it, and the box works with it stopped. The only link right
now is configuration — Null hands the harness the owner's `~/.omp/agent/mcp.json`, and that file
registers a `voicebox` MCP server pointing at the voice stack's HTTP MCP endpoint, so an agent running
inside the box can already call it (for instance `voicebox.speak`) whenever that server is up.

Running it, unchanged from upstream:

```sh
just setup                    # Python virtualenv + bun install
just dev                      # backend on 127.0.0.1:17493 + the desktop app
bun run dev:server            # the API alone
just test                     # pytest, backend/tests
bun run typecheck && bun run check
```

It needs Python 3.12+ and [Bun](https://bun.sh) (>= 1.0.0; the lockfile is Bun's). The FastAPI server
owns SQLite, the model weights, the audio files and the MCP endpoint at `/mcp` on port 17493;
agent narration runs in a second process on port 17494 so commentary can synthesize while a
generation holds the engine. A stdio shim (`voicebox-mcp`) proxies the HTTP MCP service for clients
that only speak stdio, and the MCP tools are `voicebox.speak`, `voicebox.transcribe`,
`voicebox.list_captures` and `voicebox.list_profiles`. Data lives in `data/` (`voicebox.db`, profiles,
captures, generations) when run from the repository.

It still calls itself Voicebox internally: package `voicebox` 0.5.0, bundle id `sh.voicebox.app`,
`VOICEBOX_*` environment variables, `voicebox.db`, the `voicebox-server` / `voicebox-mcp` binaries,
and an updater pointed at upstream releases. Renaming it is not done; see
[Context/Plans/NullMini.md](Context/Plans/NullMini.md), out of scope.

Its API is unauthenticated on loopback by default, which is worth knowing before exposing the port:
the session endpoints under `/mini/*` and `/harnesses` are the exception and require a per-install
token written to `<data>/mini-token` (mode 0600).

## Repository layout

```
Mini/                 Null — the box. Rust + Tauri 2, one static page, no server
  src/                the twelve modules in the table above
  Page/index.html     the whole UI, inline CSS and script
  Scripts/install     build, sign, install to /Applications, login item
  Scripts/uninstall   stop, remove the login item and the app

app/ tauri/ web/      the voice stack's shared UI, desktop host and browser host
backend/              the voice stack's FastAPI server, engines, MCP server, SQLite
landing/ docs/        the voice stack's marketing site and documentation site
data/                 the voice stack's data directory when run from here
scripts/              helpers for both (voicebox CLI, packaging, prototypes)

Context/              Plans, ADRs and research for this repository
Memories/             handoff notes between work sessions
CHANGELOG.md          the voice stack's changelog; the web build reads it
```

An unmerged branch named `capital-folders` renames every folder to PascalCase (`Backend/`,
`Tauri/`, `Web/`, `App/`, `Scripts/`) and deletes the inherited `docs/` and `landing/` sites. The
paths above describe the current working copy, not that future layout; see
[Context/Plans/CapitalFolders.md](Context/Plans/CapitalFolders.md) and
[ADR 0001](Context/ADR/0001-CapitalizedFolderNames.md).

## Context

This repository keeps its reasoning next to the code, and it is the fastest way to understand why
Null is the way it is:

- [Context/Plans/MiniApp.md](Context/Plans/MiniApp.md) — the plan the box is built from.
- [Context/Plans/Providers.md](Context/Plans/Providers.md) — sign-in, usage and the fallback order.
- [Context/Plans/ConversationDisplay.md](Context/Plans/ConversationDisplay.md) — how a reply is laid out.
- [Context/ADR/](Context/ADR/) — the decisions that constrain changes.
- [Context/Research/Research.md](Context/Research/Research.md) — the codebase map, including what was
  never traced.

## License

MIT — see [LICENSE](LICENSE).

Null is built on [Voicebox](https://github.com/jamiepine/voicebox) by Jamie Pine, used under the MIT
License; the voice stack in this repository is that work. Upstream's commit history is not carried
over — this repository begins at its own initial commit — so upstream authorship is credited here
rather than in `git log`.

Because the voice stack can clone voices and say things in them, the rules in
[RESPONSIBLE_USE.md](RESPONSIBLE_USE.md) apply to it: clone your own voice, or one you have
permission to use. [SECURITY.md](SECURITY.md) is the voice stack's inherited policy.
[CONTRIBUTING.md](CONTRIBUTING.md) covers the voice stack's development; the box's conventions are in
the table above and in each file's header comment.
