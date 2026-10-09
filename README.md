<p align="center">
  <img src=".github/Readme/hero.svg" width="100%" alt="Null — your agent, a shortcut away. A monochrome illustration of the floating input and Control+Space shortcut." />
</p>

<p align="center">
  <strong>A compact macOS interface to your own agent.</strong><br/>
  Ask a question. Put it to work. Keep your choice of models and providers.
</p>

<p align="center">
  <code>macOS 13+</code> &nbsp; <code>Oh-my-pi</code> &nbsp; <code>Rust + Tauri</code> &nbsp; <a href="LICENSE">MIT</a>
</p>

<p align="center">
  <a href="#get-started">Get started</a> &nbsp; / &nbsp;
  <a href="#use-null">Use Null</a> &nbsp; / &nbsp;
  <a href="#privacy-and-permissions">Privacy</a> &nbsp; / &nbsp;
  <a href="#under-the-hood">Architecture</a> &nbsp; / &nbsp;
  <a href="#development">Development</a>
</p>

---

## Small surface. Real agent.

Press <kbd>Control</kbd> + <kbd>Space</kbd> from the app you are in. Type a question or an instruction.
Your agent answers and works in a floating box, without activating Null over your current app.
No Dock icon, no menu-bar icon, no extra window chrome. Put it away when you are done.

- **Your tools, not just chat.** Null drives your installed [Oh-my-pi](https://github.com/can1357/oh-my-pi)
  harness, including its tools and configured MCP servers. Approvals follow the harness's own mode.
- **Your providers, one conversation.** Switch models, check usage, and choose an ordered list of
  backups. OMP owns the sign-ins and carries out retries; Null gives them a compact interface.
- **Readable without getting bigger.** Streamed Markdown, folded tool steps, and code, tables and
  text diagrams that scroll sideways instead of breaking their shape. Black, white, and very little else.

**Today:** a standalone text app for macOS, built from source. Voice conversation, background tasks,
workspace selection and the optional pet are planned, not part of the box yet.
The banner above is an illustration, not a recording of an agent session.

## Get started

### 1. Bring your harness

Install **[Oh-my-pi](https://github.com/can1357/oh-my-pi)** (`omp`). You can use existing sign-ins or
run `/login` inside Null after installation. Providers, subscriptions and API keys are managed by
OMP; their availability and terms still apply.

Null looks for `omp` on `PATH`, then in known install locations, including `~/.omp/bin`,
`~/.opencode/bin`, `~/.local/bin`, `/opt/homebrew/bin` and `/usr/local/bin`. It does not install OMP.

### 2. Build and install

You need **macOS 13+**, the **[Rust toolchain](https://rustup.rs/)**, **Xcode Command Line Tools**, and
**Tauri CLI 2**. See [Tauri's macOS prerequisites](https://v2.tauri.app/start/prerequisites/#macos).
No Node, Bun, Python or Voice server is needed to build the Null box.

```sh
# Once, if the Apple developer tools are not already installed
xcode-select --install

cargo install tauri-cli --version "^2" --locked

git clone https://github.com/dominionism/Null.git
cd Null
Mini/Scripts/install
```

The script builds `Null.app`, installs it in `/Applications`, registers a login item and starts it.
It replaces any previously installed Null app. This is a source-build workflow with local signing
support, not a notarized release installer. See [permissions and signing](#privacy-and-permissions).

### 3. Open the box

Press <kbd>Control</kbd> + <kbd>Space</kbd>. Use `/login` if you need a provider, `/model` to choose
one of its models, then type your request. You do not need to leave a terminal running.

<details>
<summary><strong>Uninstall</strong></summary>

```sh
Mini/Scripts/uninstall
```

This stops Null and removes `/Applications/Null.app` and its login item at
`~/Library/LaunchAgents/io.github.dominionism.null-mini.plist`.
Settings, workspace files and logs remain in these folders; remove them separately only if you no
longer need their contents:

- `~/Library/Application Support/io.github.dominionism.null-mini/`
- `~/Library/Logs/Null/`

OMP's own sign-ins and conversation history are not removed.

</details>

## Use Null

| Action | Key |
| --- | --- |
| Show or hide the box | <kbd>Control</kbd> + <kbd>Space</kbd> |
| Send a message or choose a list entry | <kbd>Enter</kbd> |
| Close a picker, cancel sign-in, or hide the box | <kbd>Esc</kbd> |
| Stop the current reply | <kbd>Ctrl</kbd> + <kbd>C</kbd> |
| Quit and close the conversation when idle | <kbd>Ctrl</kbd> + <kbd>C</kbd> |
| Recall the last prompt in an empty field | <kbd>↑</kbd> |
| Hide / move the box | Click away / drag outside text |

The arrow pulses while the agent works and turns amber when it asks for an answer. The transcript
grows to about ten lines, then scrolls. Hiding the box does not stop a reply.

### A few commands. No settings window.

| Command | What it does |
| --- | --- |
| `/model [filter]` | Choose a model from OMP's catalogue, across providers, without starting a new conversation. |
| `/login` | Follow the harness's own sign-in steps inside the box, including browser sign-in or key entry. |
| `/usage` | See the harness's usage reports and reset times. Providers without a report are marked as such. |
| `/backup [filter]` | Set an ordered fallback list. Enter adds or removes a model; Esc saves the order. |
| `/new` | Start a fresh conversation on the last selected model. |
| `/quit` | Quit and clear the saved conversation pointer. The next launch starts a fresh conversation. |

A listed model is not a guarantee that your account can use it. Providers with reported limits
exhausted are dimmed, but remain selectable. Model changes wait until the current reply is stopped
or finished. A lone unknown `/word` is refused rather than passed to the agent; other messages go
through normally.

<details>
<summary><strong>When a provider stops answering</strong></summary>

Null passes your backup order to OMP through a separate configuration overlay. That order goes
before the existing model/provider-specific fallback entries for the run; your original OMP
settings are not modified. OMP decides when to retry or fall back and resends the message itself.
Fallback can follow a refusal or sign-in failure as well as a usage limit.

The box reports detected model switches and highlights failed replies. Failure detection currently
uses OMP's end-of-turn token-usage signal, an unstable protocol extension. The fallback path has
been exercised with simulated failures; real account-limit behavior is still awaiting verification.
After a failed reply, choosing another model does **not** automatically resend the message: recall
it with the up arrow or type it again.

</details>

## Privacy and permissions

**Local interface does not mean offline agent.** Null has no HTTP listener or database of its own.
It starts OMP as a child process and talks over stdio. OMP may send prompts and tool-provided context
to the provider you select; tools and MCP servers may make their own network requests.

**Credentials belong to the harness.** During `/login`, Null temporarily receives your answers and
forwards them to the local `omp login` process; password fields are masked in the UI. It does not
write those answers to its settings or log. OMP handles authentication and credential storage.
No separate Null account is required.

**The shortcut needs no Accessibility or Input Monitoring permission.** macOS registers
Control+Space directly, so other apps cannot use that shortcut while Null owns it. If registration
fails, Null opens the box with a notice.

**Full Disk Access is a separate choice.** macOS attributes the child agent's file access to Null.
The app checks once and, when needed, opens the Full Disk Access settings with an explanation.
Granting it allows the agent broader access to protected files; it is not a sandbox and does not
replace OMP's approval rules. The box itself does not require that grant just to open.

<details>
<summary><strong>Signing and permission persistence</strong></summary>

macOS ties permission grants to the application's code signature. An unsigned rebuild can require
permission again. When `~/Library/Application Support/Null Signing/signing.keychain-db` exists, the
installer attempts to sign with the `Null Local Signing` identity, using the adjacent `password`
file. When that keychain is absent, it installs the build unsigned with a notice.

That local identity is a personal-machine setup, not a distributed Developer ID. Protect its
keychain and password: a process with access to them can sign as Null. The installer does not create
a signing identity for a new machine.

</details>

<details>
<summary><strong>What Null keeps</strong></summary>

Under `~/Library/Application Support/io.github.dominionism.null-mini/`:

| File or folder | Purpose |
| --- | --- |
| `settings.json` | Window position, chosen model, saved session ID, Full Disk Access prompt state and backup order |
| `backups.yml` | Model-only OMP fallback overlay, rewritten when starting the harness with a nonempty backup order |
| `Workspace/` | Default working directory for conversations; not a filesystem access boundary |

When no backup order is set, Null does not pass an overlay; an older file may remain on disk.
OMP owns the transcript and credentials. An app restart can resume the saved session; `/quit` and
idle Ctrl+C clear its pointer, not OMP's stored transcript.

Operational logs are in `~/Library/Logs/Null/mini.log`. Development smoke/self-test modes can also
log reply text, so do not use sensitive prompts in those checks.

</details>

## Under the hood

The box is one native Tauri app plus the OMP process it launches. The page is a static HTML file:
no web server, bundler or Node runtime. Rust owns the non-activating panel, the shortcut and the ACP
connection; OMP owns agent execution, providers and conversation history.

```text
Control+Space → Null's page → Tauri commands → OMP over ACP
                    ↑                            │
                    └── text, tools, approvals ──┘
```

Null explicitly carries OMP's configured approval mode and supported MCP definitions into the ACP
session. The intent is your terminal agent in a smaller surface, not an independent set of agent
permissions. Terminal parity still has edges: the box does not currently pass through OMP's lone
slash commands or display its thinking and full tool output.

Replies are parsed into a Markdown tree in Rust, then rendered with DOM elements and `textContent`.
Model-authored HTML is not executed. **A Content Security Policy is not configured yet**; careful
rendering is one boundary, not a substitute for that missing defense.

<details>
<summary><strong>Source map</strong></summary>

| Source | Responsibility |
| --- | --- |
| [`Mini/Page/index.html`](Mini/Page/index.html) | Input, commands, transcript, layout, scrolling and selection |
| [`Mini/src/main.rs`](Mini/src/main.rs) | Startup, plugins, state and command registration |
| [`panel.rs`](Mini/src/panel.rs), [`shortcut.rs`](Mini/src/shortcut.rs) | Window, placement, sizing, visibility and Control+Space |
| [`harness.rs`](Mini/src/harness.rs), [`translate.rs`](Mini/src/translate.rs) | ACP process/session lifecycle, approvals, events and protocol translation |
| [`providers.rs`](Mini/src/providers.rs), [`signin.rs`](Mini/src/signin.rs), [`backups.rs`](Mini/src/backups.rs) | Usage reports, harness-owned sign-in and fallback configuration |
| [`markdown.rs`](Mini/src/markdown.rs) | Reply text into structured parts, including partial streamed input |
| [`settings.rs`](Mini/src/settings.rs), [`access.rs`](Mini/src/access.rs), [`log.rs`](Mini/src/log.rs) | Persistence, Full Disk Access check and logging |

</details>

## Development

```sh
cd Mini
cargo test          # offline unit tests; live integration tests are ignored
cargo build         # local debug binary
cargo tauri build   # app bundle; no installation
```

Use `cargo tauri`, not `bunx tauri`. The latter names a different npm package.
`cargo test -- --ignored` runs live harness/provider checks; it is not an offline test command.
Current repository CI checks the Voice frontend and web build, **not `Mini/`**.

<details>
<summary><strong>Development switches and smoke checks</strong></summary>

All switches are environment variables:

| Switch | Behavior |
| --- | --- |
| `NULL_MINI_EXIT_WHEN_READY=1` | Exit after the page loads; no prompt is sent. |
| `NULL_MINI_NO_SHORTCUT=1` | Show at startup without claiming Control+Space. |
| `NULL_MINI_SMOKE="<text>"` | Send a real harness prompt, log the reply, and exit without a window. Supports `NULL_MINI_SMOKE_MODEL` and `NULL_MINI_SMOKE_STOP_AFTER` (seconds). |
| `NULL_MINI_SELFTEST="<text>"` | Send through the real page, log its output, and exit. |
| `NULL_MINI_PROFILE=<name>` | Pass an isolated profile to OMP and use profile-specific Null settings/backup files. |
| `NULL_MINI_BACKUPS="m/one,m/two"` | Override the backup order for this run without changing the saved order. |

After `cargo build`, a startup-only check that leaves the installed shortcut alone:

```sh
NULL_MINI_EXIT_WHEN_READY=1 NULL_MINI_NO_SHORTCUT=1 \
  NULL_MINI_PROFILE=null-probe ./target/debug/null-mini
```

**Profile isolation does not isolate MCP.** Definitions still come from the normal
`~/.omp/agent/mcp.json`. Smoke/self-test prompts can call those servers and execute real tools,
even under a separate profile. The startup-only command above sends no prompt.

</details>

## Optional voice tools

Null is the main application. [Voicebox](https://github.com/jamiepine/voicebox) is a separate tool
it can use for voice capabilities: local voice cloning and synthesis, Whisper transcription,
dictation, effects, stories and MCP speech tools. The separation is documented in
[ADR 0002](Context/ADR/0002-NullMiniIsItsOwnApp.md).

**Null's text path does not call the Voice API.** A configured `voicebox` MCP server can still let
OMP request speech while that service is running; that is not an integrated voice-conversation mode.
Voice inference runs locally, but model downloads, optional cloud features and connected agents
can use the network.

<details>
<summary><strong>Run or develop the Voice stack</strong></summary>

In addition to Rust and the platform build tools, this needs Python 3.12+, [Bun](https://bun.sh),
and [just](https://github.com/casey/just).

```sh
just setup                     # Python environment + JavaScript dependencies
just dev                       # Voice backend + desktop app
# Or, once the Python environment is active:
bun run dev:server             # API only
just test                      # backend pytest suite, including environment-dependent checks
bun run typecheck && bun run check
```

The FastAPI service on `127.0.0.1:17493` owns SQLite, models, audio files and `/mcp`. Development
narration uses a separate process on 17494. Packaged narration-worker startup has a known argument
mismatch; see the [research map](Context/Research/Research.md). Headless playback covers queued
speech, not unclaimed narration streams.

MCP exposes `voicebox.speak`, `voicebox.transcribe`, `voicebox.list_captures` and
`voicebox.list_profiles`; `voicebox-mcp` is the stdio-to-HTTP shim. Repository development data lives
in `data/`. Internals and the updater still use upstream Voicebox naming and releases.

**Do not expose the API as if it were authenticated.** Most endpoints have no authentication;
loopback binding and CORS are not a general authorization layer. The retained Python `/mini/*`
and `/harnesses` routes require both loopback and an install token. They do not power the Null box.

</details>

## Status and direction

| Available in the box | Planned or incomplete |
| --- | --- |
| Text conversation and agent tools | Background tasks, activity tray and workspace selection |
| Model selection, sign-in, usage and backups | First-run guidance, `/logout` and sign-in timeout |
| Structured Markdown and a compact transcript | Resizable transcript and repeatable visual checks |
| Independent macOS app | Cloned-voice conversation, optional pet and other platforms |

The latest display and some provider flows still need hands-on validation. No real provider quota
exhaustion has been verified. See the plans for evidence and open decisions rather than assuming
that every implemented path is finished.

<details>
<summary><strong>Repository layout and ongoing cleanup</strong></summary>

```text
Mini/                  Null app: Rust host, static Page/, install scripts
app/                   Shared Voice React UI
tauri/                 Voice desktop host
web/                   Voice browser host
backend/               Voice API, engines, MCP, SQLite and Python harness
scripts/               Voice lifecycle, packaging and development helpers
data/                  Voice development data
docs/ · landing/       Inherited documentation and marketing sites
Context/               Plans, decisions and research
CHANGELOG.md           Voice changelog, also read by the web build
```

The unmerged `capital-folders` branch capitalizes the existing directories and removes the inherited
sites and other upstream material. Paths here describe the current checkout. See
[the cleanup plan](Context/Plans/CapitalFolders.md) and [ADR 0001](Context/ADR/0001-CapitalizedFolderNames.md)
before making that switch. `Memories/` holds local handoffs and is not tracked in Git.

</details>

### Read the reasoning

[App design](Context/Plans/MiniApp.md) ·
[Providers](Context/Plans/Providers.md) ·
[Conversation display](Context/Plans/ConversationDisplay.md) ·
[Future phases](Context/Plans/NullMini.md) ·
[Decisions](Context/ADR/) ·
[Codebase research](Context/Research/Research.md)

## License

[MIT](LICENSE).

Clone only voices you own or have permission to use. The Voice tooling's
[responsible-use guidance](RESPONSIBLE_USE.md), [security policy](SECURITY.md) and
[contribution guide](CONTRIBUTING.md) remain available; Null-specific development is described above.

<p align="center"><sub>Built by <a href="https://github.com/dominionism">dominionism</a>. A small box, with your agent behind it.</sub></p>
