# Research: Null

> Last updated: 2026-10-09 (comprehensive). Current-source map of `null-mini` at `abb114d`;
> supersedes the 2026-10-07 Voice-only baseline. The unmerged `capital-folders` cleanup branch at
> `4793f61` uses different paths. Paths below describe the main checkout, not that future layout.

> Second-session addition, 2026-10-09: the `## Null Mini — deep dive (Mini/)` section below was
> written by a different session in parallel with this map. It adds module-level detail to the Mini
> statements above and in `## Architecture`; it does not replace them, and the rest of this file is
> unchanged.

> Third addition, 2026-10-09: `Context/Plans/OwnHarness.md` items 1 to 4 landed on `main`
> (`128f778`) and item 3 on a branch. The statements about Null that this made untrue were corrected
> in place by the session that built them: where the harness comes from, the settings file handed to
> it, the source files and the commands. Nothing else was traced again.

**Evidence boundary.** Source, manifests, relevant tests, research, ADRs and plans were read in this
pass. Runtime evidence is limited to the installed Null startup/page-ready probe, Voice status and
health, profile inventory, and unauthenticated API probes listed below. No test suite, build, model,
provider prompt, sign-in, microphone, audio playback or live conversation was run. Prior hands-on
and synthesis evidence is attributed to its plan, not claimed as newly verified.

## What is this?

Null is a compact desktop interface to the user's own agent harness: ask a question and get an
answer, ask for work and have the agent do it, without tying the interface to one provider's quota.

The intended companion resembles the interaction described in `ChatGPTPetsAndMini.md`, not merely
a voice studio or decorative pet. Text interaction is the current product. Background work,
workspace selection, hands-free conversation in the user's cloned voice, and an optional pet are
later phases. Personal use comes first; making it usable by other people with their own provider
sign-in or API key is the distribution goal.

### Product direction and authoritative plans

- `MiniApp.md` defines the independent app, now called **Null**, opened by **Control+Space**.
  Older “Null Mini” and “fn+Space” wording is historical. The folder/executable/bundle identifier
  retain `Mini`, `null-mini`, and `io.github.dominionism.null-mini`.
- `Providers.md` supersedes Phase 2 of `NullMini.md`: **OMP is the current harness; providers are
  reached through OMP.** Supporting several providers does not require several harness adapters.
  Other harnesses are deferred until there is a need.
- `OwnHarness.md` reverses one decision of `Providers.md`: **Null carries Oh-my-pi inside the app, at
  one version named in `Mini/Engine.toml`, and starts that one.** The user's own copy is used only
  when chosen with `/harness`. A newer version is let in by the harness check (`Mini/src/check.rs`,
  `Mini/Scripts/engine --to`). Its items 5 to 10 (first opening, one-command install, `/update`) are
  not built.
- `ConversationDisplay.md` defines the current presentation: compact black-and-white box, SF Mono,
  one reading edge, null sign beside user messages, no left-side rules, no boxes around code or
  diagrams, quiet/folded tool activity. The latest look is built but not yet owner-accepted.
- `NullMini.md` remains the umbrella for later background tasks, workspaces, voice and pet behavior.
  Its old Voice-server/React implementation sketches must be re-planned against the standalone app
  when those phases begin, not implemented literally.
- `VoicePairing.md` concerns talking into an **already-running terminal agent**. It is not the
  transport used by Null's own conversations. `VoiceLoopLatency.md` and the narration plans contain
  measured speech constraints, not evidence that Null already has continuous voice conversation.
- `ChatGPTPetsAndMini.md` remains dated external reference-product research. It was checked for
  overlap and left unchanged: this pass did not revalidate its external sources or supersede them.

### User-visible guarantees to preserve

- The box opens quickly, hides completely, and takes typing without activating Null over the app
  the user was using. Idle UI is the text field and arrow, not a dashboard of status and hints.
- Null exposes real harness tools and work, not only chat. It mirrors harness approval semantics
  rather than adding a second permission system.
- Provider sign-ins and retry behavior belong to OMP. Null supplies choices and displays outcomes;
  a model being listed does not prove the account may use it.
- The current credential decision permits transient answers passed to the harness's own local
  sign-in process. Null must not retain or log them. This supersedes older absolute “never
  forwards a provider token” wording. Provider-policy statements in plans are dated research,
  not a fresh legal review.
- Text Null must not require the Voice desktop/server. Voice cloning and speech remain separate
  for now. Ordinary dictation/paste and herdr delivery must not be broken by future integration.
- Local speech is the intended voice path. Do not equate that with “nothing leaves the machine”:
  OMP can call remote providers and tools, and the inherited Voice app has optional cloud features.

## Architecture

Null's static page sends commands to its own Rust host, which lazily starts the Oh-my-pi it carries
(or the user's own, when chosen with `/harness`) and exchanges ACP messages over stdio; OMP executes tools and streams replies back. The page decides presentation,
not provider behavior; the Rust host decides window/process/session coordination, not credentials
or inference. Separately, Voice's React application calls a Python FastAPI service that owns SQLite,
audio and local inference; its native host decides OS integration, not domain state. Voice has two
agent routes of its own: a retained Python ACP API and herdr delivery into terminal sessions.

### Roles and responsibility exclusions

| Role | Owns | Does not decide |
| --- | --- | --- |
| `Mini/Page/index.html` | Input, command pickers, transcript, structured reply DOM, scrolling/dragging requests | Process launch, durable transcript, provider catalogue, retry policy |
| `Mini/src/` | Non-activating panel, shortcut, settings, OMP lifecycle, ACP client, event buffer, local sign-in presentation | Provider entitlement/authentication internals, tool execution, model inference |
| OMP: the copy inside Null.app, or the user's own | Credentials, provider/model catalogue, tools, transcript, approvals, actual retries/fallback | Null window and visual layout |
| `app/` | Shared Voice screens, API orchestration, recording state, React Query/zustand state, playback | OS shortcuts, native clipboard/focus transactions, model execution |
| `tauri/` | Voice desktop, sidecar supervision, global chords, focus/paste, pill window, speak subscription | SQLite, profiles, transcription, speech engine selection |
| `web/` | Browser host for shared Voice UI, browser download/playback adapters | Native capability emulation or server supervision |
| `backend/` | Voice HTTP/MCP, domain persistence, audio files, engines, generation queue, narration, Python ACP, herdr delivery | Null's independent UI/session settings, native window visibility |
| macOS / launchd | Shortcut registration, permissions, process launch at login | Harness approval policy or provider access |
| `docs/`, `landing/` | Inherited Voicebox documentation and marketing sites | Application runtime; removed only on the unmerged cleanup branch |

### One complete Null text turn

1. `Mini/src/main.rs::main` initializes settings, harness command handling, sign-in/cache state and
   the panel. OMP is **not** started merely by opening the app.
2. `shortcut.rs::start` registers Control+Space; `panel.rs::toggle/show` brings up a non-activating
   NSPanel. The page receives `mini:shown`, focuses its input and requests missed events.
3. Enter in `Mini/Page/index.html` handles `/model`, `/backup`, `/usage`, `/login`, `/new`, `/quit`
   locally. Ordinary text invokes `harness::send`; blank input and a second concurrent turn are
   rejected. A lone unknown slash-word is rejected, not sent to the agent.
4. `engine.rs` picks the program: the user's own when the `harness` setting names one that can be
   run, else the one beside Null's executable (`Contents/MacOS/omp`), else, with a line in the log,
   one found on PATH or in the install directories. `harness.rs` starts it as
   `omp [--profile ...] [--approval-mode ...] --config harness.yml acp`. It reads OMP's configured approval
   mode explicitly and supplies supported MCP definitions from `~/.omp/agent/mcp.json`, because
   ACP did not inherit these terminal settings in the recorded spike.
5. ACP protocol v1 is initialized. `session/load` resumes a saved conversation when possible;
   otherwise `session/new` creates one in the app's `Workspace` directory. The selected model is
   restored if offered. Null persists the session ID; OMP persists its transcript.
6. `session/prompt` produces text/tool/status notifications. `translate.rs` maps them to numbered
   `mini:event` messages; a 5,000-event in-memory buffer supports `events_since` catch-up. Hiding
   the panel does not stop the turn.
7. The page immediately displays text, then calls Rust `layout` (`markdown.rs`, `pulldown-cmark`)
   to obtain a structural tree. DOM elements and `textContent` render it, never model-authored HTML.
   Code/tables/ASCII diagrams preserve width, shrink where appropriate, then scroll horizontally.
   Tool steps fold; failed steps remain visible. Thinking text and raw tool output are not shown.
8. OMP permission requests retain their protocol responder in Rust until a page button/number-key
   answer or cancellation. Ctrl+C during work sends session cancel; while idle it quits and clears
   the saved conversation. `/new` starts a fresh conversation; `/quit` also clears its pointer.
   An involuntary restart instead attempts to resume it.

### Provider behavior in the current app

- `/model` uses OMP's session config options, not a Null catalogue. Changing model/provider retains
  the session and is refused while a turn runs. Opening `/model` or `/usage` can initialize a
  harness session even before the first prompt.
- `signin.rs` runs **OMP's own `login` command** on pipes, reads its provider list and incremental
  questions, and forwards answers only to its stdin. The page masks sign-in input. Success causes
  a harness restart/cache invalidation and a before/after model-provider comparison. Esc/Ctrl+C
  cancels. No `/logout` or active-sign-in timeout is built.
- `providers.rs` runs `omp usage --json --redact` with a 15-second deadline and 60-second cache;
  `/usage` forces freshness. It stores provider/limit data, not account names. Missing/key-only/local
  reports mean “no usage report,” not no usable model. Tier limits do not exhaust an entire provider.
- `backups.rs` stores the user's ordered model list and puts it in the form of OMP's settings.
  `harness.rs` writes that, with `startup.checkUpdate: false`, to `harness.yml` at every start. Null's order
  goes before existing model/provider-specific fallback entries for that run; original OMP settings
  remain unchanged. **OMP retries and resends; Null does not implement a second retry loop.**
- Fallback is not exclusive to quota exhaustion: refusal and authentication failures can trigger it.
  Null announces model changes. Since OMP can omit a change notification, it also queries config
  after a turn by setting an existing non-model option to its current value.
- Provider failures can arrive as ordinary reply text with `end_turn`, not protocol errors.
  `translate.rs` uses absent token usage to flag a failed reply, enabled by ACP's
  `unstable_end_turn_token_usage` feature. This is version-sensitive, not a universal protocol rule.
  So a reply without a count is taken for a failure only once that harness version has been seen
  to count one (`translate::judge_reply`): the carried version is trusted, and what was seen of
  another is remembered in `settings.json` (`counts_tokens`).
- After 20 quiet seconds without tools/approval, the box explains it is still waiting. Failed reply
  text stays visible in red; manual model selection does not automatically resend the failed prompt.

### Voice surfaces and execution paths

The shared React routes expose Generate/history, Stories/timeline, Captures, Voices, Effects, Models,
and General/Generation/Captures/MCP/GPU/Logs/Changelog/About settings. The browser host shares the
screens but lacks native shortcuts, paste/focus, pill, system audio, server supervision and updater.

**Dictation and agent delivery:**

1. `useChordSync` sends saved chord sets to `hotkey_monitor.rs` after readiness checks. Rust captures
   foreground application identity, shows the hidden `?view=dictate` webview without focusing it,
   and emits `dictate:start`; release emits `dictate:stop`.
2. `DictateWindow` stops its current speech and main-window playback, then
   `useCaptureRecordingSession` records via `useAudioRecording`. MediaRecorder audio is normally
   converted to WAV and uploaded to `POST /captures`.
3. `backend/services/captures.py` decodes/transcodes, runs Whisper, stores audio/transcript and
   returns capture-time paste/refine flags. Optional refinement is a separate local-LLM request.
4. Normal dictation invokes Rust `paste_final_text`: reactivate the captured PID, snapshot the
   clipboard, write text, paste, conditionally restore the clipboard only if no one else changed
   it, and optionally press Return. It restores the app, not an exact previously focused element.
5. Agent dictation instead calls `/voice-targets/message`. The backend validates a live herdr target,
   adds `[voice turn] reply aloud, two sentences max.`, and invokes `herdr agent prompt`. Delivery
   does not read the reply. Failure produces an error, not the old plan's proposed paste fallback.
6. The agent must separately request speech through MCP/REST. Null's independent text app does not
   currently synthesize each text reply or own a hands-free voice loop.

**Persistent generation:** `POST /generate` validates profile/engine, inserts a generation, then
`services/task_queue.py` serializes `run_generation`. The worker lazily loads the engine, prepares
voice prompts, chunks/normalizes/applies effects, writes WAV/version state, updates status and emits
completion. Status SSE polls SQLite. Startup fails stale in-progress rows. Retry reuses a generation;
regeneration creates a take. `POST /generate/stream` bypasses this queue, so “all synthesis is
serialized” is not a current whole-system invariant.

**Agent speech:** plain MCP `voicebox.speak` and REST `/speak` use the persistent generation path.
Voice resolves explicit profile → per-client binding → global default. `agent_voice_enabled` gates
new speech, not user delivery or stop. Rust `speak_monitor.rs` forwards backend `/events/speak` into
hidden webviews; the pill claims completed audio and shows only when playback starts. Optional
headless OS playback covers queued agent speech, including an attached pill that fails to fetch audio.

**Narration:** `narrate=true` / `/speak/narrate` creates an ephemeral session. The first GET stream
claim triggers synthesis and yields base64 WAV chunks. The normal dev path uses a separate narration
process on 17494, with its own model instance; fallback waits for the main queue and holds an engine
lock across synthesis/cancellation. Narration sessions are capped at 16 with an unclaimed TTL of
120 seconds; no generation/history/audio file is persisted for the utterance. WebAudio waits for
buffered playback and pending decodes to drain. **There is no narration headless player:** an
unclaimed stream makes no sound, even with a healthy worker.

### Three distinct agent interfaces

| | Standalone `Mini/` | Python `/mini` | herdr voice target |
| --- | --- | --- | --- |
| Current caller | Null's static page | No current app/web/native consumer found | Voice dictation pill |
| Transport | Tauri IPC + Rust ACP stdio | HTTP/SSE + Python ACP stdio | `herdr agent list/prompt` |
| Session owner | OMP; Null keeps saved ID | OMP; Python keeps memory-only index | Existing terminal harness |
| Current harness | OMP | OMP | Agent already present in selected pane |
| Provider UI | Login, models, usage, backups | None | None |
| Local security boundary | No HTTP listener | Loopback **and** bearer token | General Voice API, no general auth |

The Python API stays by ADR 0002, not as an alternate implementation to route the current Null UI
through. Its session metadata/events disappear on backend restart; no collection listing, resume
ID in create, close/delete route or persisted index is built. Its fallback/provider reporting has
not gained the standalone Rust app's newer behavior.

### Backend lifecycle, storage and security

- `backend/app.py::create_app` chooses main or narration role. Main mounts REST and FastMCP; worker
  mounts health, warm and synthesize endpoints. Main startup initializes DB/queue/auth token,
  repairs interrupted generations and schedules worker/cached-model warm-up and binary checks.
- `main.py` is the development entry; `server.py` is the frozen sidecar with parent watchdog;
  `narration_main.py` selects worker role before importing the app. Direct/frozen entry points
  also initialize DB before lifespan, unlike `uvicorn backend.main:app`.
- `database/` uses synchronous SQLAlchemy/SQLite, hand-written idempotent migrations before
  `create_all`, seeding and version backfill. Media paths are generally relative to the data dir.
  Neither desktop host owns Voice SQLite. Worker synthesis is non-persistent, but worker startup
  still runs initialization/migrations: “worker never writes the database” is too broad.
- Most Voice APIs have no authentication. Loopback default and CORS are not authorization when
  network access is enabled. Python `/mini/*` and `/harnesses` require loopback plus a mode-0600
  install token; MCP absolute-path transcription separately requires loopback.
- MCP tools are `speak`, `transcribe`, `list_captures`, `list_profiles`; the stdio shim proxies the
  HTTP MCP service. `X-Voicebox-Client-Id` selects defaults/tracks clients, not authentication.
- `scripts/voicebox` manages the repo-local Python server and launchd KeepAlive; the Voice desktop
  is separate. Packaged Voice can spawn/reuse a sidecar. Its speak monitor remains fixed to local
  port 17493 even when React is configured for a remote backend.

## Null Mini — deep dive (Mini/)

Added 2026-10-09 by a second session (see the note at the top). This section zooms into `Mini/`; the
Map's own Mini statements in `## Architecture` and `## Domain Model` stand. `Mini/src/*.rs`, the
page's command/event/key sites, `tauri.conf.json`, `Info.plist`, `build.rs`, `capabilities/`, and
both `Mini/Scripts/*` were read. Facts marked *(observed)* come from the app's own
`~/Library/Logs/Null/mini.log` (856 lines), `settings.json` and `backups.yml`, read on 2026-10-09 —
not produced by a run made for this section.

### The files in `Mini/src/`, and what each decides

Twelve when this section was written; `engine.rs` and `check.rs` came with `OwnHarness.md`.

| Module | Owns | Decides / does not decide |
|---|---|---|
| `main.rs` | Wiring: two plugins, state init, 23 `invoke_handler` commands, five dev switches | Nothing else |
| `panel.rs` | The window: 616 px wide, 76-314 px tall, transparent, always on top, non-activating NSPanel, label `mini`; placement, show/hide, `resize` | Where and whether the box is visible, not what it shows |
| `shortcut.rs` | Control+Space, registered with macOS as an ordinary system-wide shortcut (no permission to ask) | That the chord fired, not what it means |
| `harness.rs` | The ACP connection: one thread, one `omp … acp` process, one conversation, a 5,000-event ring, pending approval responders | What the harness reported; never what a provider is or whether a model is good |
| `translate.rs` | ACP JSON → box events; the model list out of session config options; the owner's `mcp.json` → `session/new`; finding the `omp` binary | Pure functions over JSON; no process, no network |
| `backups.rs` | The fallback order, in the form of the harness's settings | The order, not the switching — the harness moves on and re-sends by itself |
| `engine.rs` | Which Oh-my-pi runs: the one beside Null's executable, or the user's own by the `harness` setting; the commands `harnesses` and `set_harness` | Which program, never what it does |
| `check.rs` | Test-only: the harness check, with a stand-in provider and a throwaway folder (`cargo test -- --ignored`) | Whether a version answers as Null needs; nothing at run time |
| `providers.rs` | `omp usage --json --redact` (15 s limit, 60 s cache) set beside the model list | What is left; never an account name |
| `signin.rs` | `omp login` on ordinary pipes: its list, its lines, its questions in, answers out | Nothing about provider semantics and nothing about steps having run |
| `access.rs` | Full Disk Access: a file only it unlocks, one ask, then the user's decision | That macOS asks folder by folder, not whether the user agrees |
| `settings.rs` | `settings.json` in the app config dir; `NULL_MINI_PROFILE` suffixes the app's own file names | Only where the box was and what was last open |
| `markdown.rs` | Markdown → the JSON tree the `layout` command returns | Part kinds, not drawing |
| `log.rs` | One line per event, to stderr and `~/Library/Logs/Null/mini.log` | Nothing |

Exclusions the code states: Null decides that a message was typed, not how the agent works; it
decides the fallback order, not when to fall back; it decides to show the harness's sign-in, not what
a provider requires; it adds no approval layer — the approval mode read from
`omp config get tools.approvalMode` is passed straight through as `--approval-mode`.

### The same text turn, at the call sites

1. Control+Space → `shortcut.rs` (`ShortcutState::Pressed`) → `panel::toggle` → `show` → `mini:shown`.
2. The page focuses the field. Enter → `invoke('send', { text })`.
3. `harness::send_text`: refuse if busy; set busy; publish `user_message`, then `status_change
   running`; queue `Command::Send`.
4. The harness thread (`serve` → `converse` → `Conversation::handle`) builds
   `omp [--profile P] [--approval-mode M] --config …/harness.yml acp`, `initialize`s (the protocol
   version must come back 1), then opens a session: `session/load` on the saved id when the agent
   advertises `loadSession`, else `session/new`; MCP servers come from `~/.omp/agent/mcp.json` in
   either case. *(observed, installed app, 2026-10-09)* the real start line is "starting the
   built-in harness: /Applications/Null.app/Contents/MacOS/omp --approval-mode yolo --config
   /…/io.github.dominionism.null-mini/harness.yml acp", followed by "harness ready: omp 18.4.3".
5. `session/prompt` → `agent_message_chunk` / `agent_thought_chunk` → `on_update` →
   `translate::event_from_update` → `text_delta` → `publish` → `mini:event` → the page appends.
6. Reply ends → `stop_reason`, `reply_failed`, `status_change ready`, `message_done`, `finish`, then
   `Command::CheckModel` re-asks a non-model session config option so a harness-side move to a backup
   is seen and published as `model_switched`.
7. The page re-reads the reply through the coalesced `invoke('layout', { text })`, draws it, measures
   the box and calls `invoke('resize', { height })`.

Approvals: `RequestPermissionRequest` → `approval_request` with its options → the user's answer goes
back as `invoke('respond', { requestId, answer })` → `respond` resolves the stored `Responder` and
returns the session to `running`.

### The page contract, in full

Commands the page invokes (the `invoke_handler` list in `main.rs`): `page_ready`, `quit`, `layout`,
`hide_box`, `resize`, `send`, `interrupt`, `respond`, `models`, `set_model`, `new_conversation`,
`events_since`, `report`, `providers`, `backups`, `set_backups`, `harnesses`, `set_harness`,
`signin_providers`, `signin_start`,
`signin_answer`, `signin_cancel`, `open_url`.

Events: `mini:event` carries `user_message`, `text_delta` (`{text, thinking}`), `tool_activity`,
`status_change` (`running` / `needs_input` / `ready` / `blocked`), `approval_request`,
`message_done`, `model_changed`, `model_switched`, `reply_failed`, `error`, each with a running `seq`
so a reloaded page catches up through `events_since`. `mini:signin` carries the sign-in's lines,
questions and `done`; `mini:shown` means focus and catch up; `mini:notice` carries the
shortcut-unavailable line or the Full Disk Access line, sent from `page_ready` because the page loads
after the app has already tried both.

Sizing contract (`panel.rs` comments, mirrored in the page's CSS): window 616 px wide; 76 px idle
(44 px box + 8 above + 24 below); at most 314 (2 border + 42 row + 212 transcript + 26 notice + 32
margin); `resize` clamps to 76-314 and keeps the top-left corner; a saved position is kept only while
80 px of its top edge is still on a screen.

### On-disk state (observed)

- `…/io.github.dominionism.null-mini/settings.json`: `position [958,322]`,
  `model "opencode-go/deepseek-v4.1-flash"`, `session 01a11fb9-…`, `asked_full_disk true`,
  `backups ["anthropic/claude-opus-5-5","opencode-go/deepseek-v4.1-flash"]`.
- `…/harness.yml` (it was `backups.yml` until 2026-10-09):
  `{"startup":{"checkUpdate":false},"retry":{"fallbackChains":{"default":[…],"openai-codex/*":[…,"openai-codex/gpt-5.6-sol"]}}}`
  — the update check off, then Null's order first and the owner's own list for that provider.
  `settings.json` also holds `harness` (the path of the user's own Oh-my-pi when chosen) and
  `counts_tokens`.
- `…/Workspace`: the working directory of every conversation (`harness::workspace`).
- `mini.log`: one `[mini]` line per event plus a float timestamp; the app's only trace.

### Findings this deep-dive adds

- **A planned event does not exist.** `Providers.md` names a `provider_state` event as a deviation;
  there is none — the page calls `invoke('providers')`.
- **`ConversationDisplay.md` items 1, 8 and 9 are not started:** the look-check harness
  (`Mini/Scripts/Look/`), the note that tells the agent about the box, and more room (which would
  move `MAX_HEIGHT` and the page's height sums).
- **Mini has no task-runner entry.** No `justfile` recipe, and it is outside the bun workspaces
  (Rust crate only); `cargo test` in `Mini/` is manual and was run in neither pass.
- **No other repo code depends on `Mini/`.** Outside it the only references are the plans, the
  retained prototype `Scripts/prototype-omp-spike.py`, and `Memories/`.
- **The rename does not touch it.** `Mini/` already follows ADR 0001; `Mini/src` and
  `Mini/capabilities` are among the stated exceptions in `Context/Plans/CapitalFolders.md`.

## Domain Model

- **Harness versus provider:** OMP supplies agent execution/session/tools; providers supply models
  and account access. Null does not need a new harness for each model vendor.
- **Null conversation:** OMP transcript + saved session ID; one active turn, bounded in-memory UI
  events. Not a Voice `Capture`, not a narration session, not a herdr pane.
- **Null settings:** position, chosen model, session ID, one-time Full Disk Access ask, backup order,
  the user's own harness when chosen, and the harness version last seen to count tokens.
  Stored under the bundle's application-support directory; profile-specific settings/overlay names
  isolate development state. OMP remains transcript/credential owner.
- **VoiceProfile / ProfileSample:** cloned/preset/designed voice configuration and reference audio,
  optional default engine, personality and effects. Designed profiles lack a ready synthesis path.
- **Generation / GenerationVersion:** persistent speech request plus original/effects/take lineage;
  source distinguishes manual, MCP, REST and personality speech. Separate from capture input.
- **Capture:** dictation/recording/upload audio with raw/refined text and model/refinement metadata.
- **Story / StoryItem:** timeline arrangement of generation/version clips; services own edits/export.
- **Channels / mappings / effect presets:** output-routing configuration and reusable DSP chains.
- **Singleton settings:** captures, generation, cloud. MCP bindings supply per-client voice defaults.
  Voice's cloud bearer key is separate from OMP provider credentials; do not conflate the boundaries.
- **Ephemeral runtime state:** generation queue/task registry, speak subscribers, narration sessions,
  Python Mini sessions, Rust Mini event buffer and approval responders. None is a durable task tray.
- `Context/Glossary.md` is still an unfilled template. These distinctions are a factual research
  vocabulary, not newly settled glossary/ADR decisions. `Project` ORM remains without a service/API.

## Patterns

### Stack and development

- **Null:** standalone Rust 2021/Tauri 2 crate in `Mini/`; ACP v3, `pulldown-cmark`, global-shortcut
  plugin, pinned `tauri-nspanel`; macOS 13+, compile-time macOS-only. Static `Page/`, no Node/Vite build.
  Oh-my-pi goes into the app as a Tauri external binary from `Mini/Engine/`, which
  `Mini/Scripts/engine` fills from the release `Mini/Engine.toml` names, checksum checked. A build
  without it stops in `build.rs` and says so. Apple Silicon only. The app is 204 MB with Oh-my-pi 18.8.7 inside (217 MB with 18.4.3).
- **Voice:** Bun workspace (`app`, `tauri`, `web`, `landing`), React 18/TypeScript/Vite/Tailwind,
  TanStack Router/Query, zustand, WaveSurfer, i18next. `docs` is its own Next/Fumadocs project;
  `landing` is an independent Next marketing site.
- **Backend:** Python >=3.12 per `backend/pyproject.toml`, FastAPI/Pydantic/SQLAlchemy, torch/optional
  MLX, Whisper STT, Qwen local refinement/personality LLM, Pedalboard effects. Lazy singleton engine
  registry exposes seven TTS names: qwen, qwen_custom_voice, luxtts, chatterbox, chatterbox_turbo,
  tada, kokoro. Engine choice is distinct from conversation-provider choice.
- Backend pattern: thin routes → services/domain/session → engines behind Protocols. Migrations
  are hand-run, not Alembic. Ruff targets py312, 120 columns, double quotes. Heavy engine imports
  are mostly lazy; platform setup must precede torch import.
- Frontend live API is hand-written `app/src/lib/api/client.ts` + `types.ts`; the generated
  OpenAPI service tree is not used by current screens. Host-injected `Platform` provides filesystem,
  audio, lifecycle, updater and metadata. Direct Tauri imports bypass browser portability.
- React Query holds backend state; zustand holds UI/playback/connection concerns. Separate webviews
  communicate via events, not shared stores. Each active host currently creates its own QueryClient.
- Mini JSON/ACP translation is in `translate.rs`; process/session orchestration in `harness.rs`;
  pure parsers/merges have inline Rust tests. Layout requests are serialized, **not frame-throttled**
  as the display plan proposed. HTML is never produced from model content.

### Available commands, not run in this research

| Surface | Commands |
| --- | --- |
| Null | `Mini/Scripts/engine` once, then `cargo build`, `cargo test`, `cargo tauri build` from `Mini/` |
| Null harness check | `cargo test -- --ignored` (15 live tests); `NULL_MINI_ENGINE=<path>` for another program; `Mini/Scripts/engine --to <version>` |
| Null install | `Mini/Scripts/install` builds/signs/replaces the app and login agent; mutating, not a check |
| Voice desktop dev | `just dev` starts backend if needed; `bun run dev` expects it separately |
| Voice server/web | `bun run dev:server`, `bun run dev:web`, `just dev-web` |
| Voice build | `bun run build`, `bun run build:web` |
| Frontend checks | `bun run typecheck`, `bun run check`, `bun run ci` |
| Backend checks | `just test`, `just check-python` or venv pytest/Ruff |

CI runs the frontend typecheck and web build, and since 2026-10-09 Null's unit tests and harness check on a Mac runner whenever `Mini/` changes (`.github/workflows/null.yml`). Python behavior is not run. Voice has no
frontend behavioral suite and only a manual system-audio Rust integration test. Python tests mix
isolated tests with live-server/model/download checks. Mini's ignored tests are the harness check:
they execute OMP in throwaway folders against a stand-in provider, and one reaches DeepSeek with a
dummy key;
its smoke/self-test switches can send real prompts. `NULL_MINI_PROFILE` isolates settings and OMP
profile args, **but MCP config still comes from the ordinary `~/.omp/agent/mcp.json`**.

### Operational constraints

- Installed Null uses a separate LaunchAgent and optional local signing identity. The install script
  preserves keychain search state but proceeds unsigned if the local signing keychain is absent.
  Full Disk Access is a macOS grant to the app/harness, not a replacement for harness approvals.
  Current personal-machine signing is not a distributable notarized release flow.
- Voice startup warm-up never downloads uncached models. Ordinary inference/capture paths are not
  universally cache-gated; direct captures can initiate loading/downloading.
- Preserve the measured speech findings in `VoiceLoopLatency.md`: PyTorch Whisper small was the
  working baseline; MLX hallucinated/ran much slower on this Mac, same-size CTranslate2 gave no gain,
  and CPU LuxTTS under-generated. Do not retry these directions just because old plan sections
  still recommend them. Those are prior measurements, not a fresh benchmark here.
- Voice turn traces exist, but capture/delivery and speech use separate IDs; a complete joined
  end-to-end trace must not be inferred from older “one record per turn” plan wording.

## Relevant ADRs

- **0001, capitalized folders:** PascalCase throughout, except externally fixed tool names, public
  identifiers/URL components and hidden folders. Applies immediately to new folders. Existing main
  paths remain lowercase until the separate cleanup branch lands.
- **0002, Null Mini is its own app:** independent window, shortcut and harness connection. Voice's
  Python harness and `/mini` routes remain for prospective voice use. Its fn+Space wording predates
  the later Control+Space choice.
- Provider credential ownership and provider-agnostic presentation are settled in `Providers.md`,
  but their proposed ADRs/glossary entries are not written. Research does not create new decisions.

## Open Questions and Current Gaps

### Product completion versus built code

- Core Null typing/tools/model switching/hiding/dragging have owner-reported proof in `MiniApp.md`.
  Actual logout/login behavior and some restart checks remain open. Older per-item/Validation
  paragraphs lag the later hands-on record; do not treat them as evidence that those interactions
  were never used. Do not discard the user's reported login issue based on machine-history inference.
- `/login` has owner-reported success. `/backup`, `/usage`, latest layout/selection/sideways-scroll
  feel and real account-limit behavior are not fully owner-proven. Existing fallback proof used
  stand-in providers; no real quota exhaustion is established.
- No first-run installation/sign-in guidance, `/logout`, active-sign-in timeout, content policy,
  checked-in visual harness or public Null/provider setup documentation is complete. The input is
  already masked; `Providers.md` item 9's remaining secret-residue proof must not be mistaken for
  absence of the mask.
- Full terminal parity is an objective, not a theorem: the original ACP spike reported a skills
  count difference, and lone harness slash commands are blocked by Null's unknown-command rule.
- No background task tray, workspace picker, continuous voice conversation, automatic speaking of
  every Null reply, cross-harness handoff or pet renderer is built. Voice requires a profile first;
  today's inventory is empty. Later voice transport/echo cancellation/engine choices remain open.

### Concrete implementation boundaries to consider for future work

- Both Tauri products have `csp: null`. Mini constructs safe text DOM today; this is not the planned
  second-layer browser policy. Full Disk Access and broad harness tools make that trust boundary
  consequential. No exploit was exercised in this research.
- Packaged narration supervisor invokes `--role narration`, but `backend/server.py` accepts no
  `--role`. Dev worker health does not prove packaged worker startup.
- Stop/mute/restart chord events (`voice:stop`, `voice:toggle`, `dictate:restart`) are emitted but
  have no frontend listeners. `apiClient.stopSpeaking` has no caller. Settings mute itself works.
- Narration without a stream claimant is silent; `scripts/voicebox demo` does not claim its stream.
  MCP/route descriptions still promise the abandoned CPU-specific lane and overstate no-wait behavior.
- `/generate/stream` bypasses the persistent queue. `generation_settings` is not the source of
  `/generate` chunk/crossfade/normalization values; request fields/defaults are used.
- `serverStore` invalidates `app/src/lib/queryClient.ts`, but actual desktop/browser hosts use other
  QueryClient instances. Read from code: switching server can retain the displayed server cache;
  this was not exercised live.
- Voice's remote-server setting does not move Rust's local-only speak subscription. Browser Captures
  export directly imports Tauri plugins ([INFERENCE]: those buttons fail outside Tauri).
  Same-origin hard navigation to `/settings/captures` or `/settings/generation` hits API JSON.
- Mini's bounded event history has no gap marker; process restarts rely on OMP replay. Python Mini
  lacks a durable resume index entirely. Do not use either as proof of background-task persistence.
- Upstream Voicebox updater/branding remains in the Voice host. Root README describes Voicebox, not
  current Null; its preserved-history statement is not evidence of imported upstream history.
- Cleanup remains unmerged; `CapitalFolders.md` holds its state as of 2026-10-09. No
  cleanup/rebase/switch/service mutation was done during this research.

### Observed runtime, 2026-10-09

- Installed `/Applications/Null.app/Contents/MacOS/null-mini` run with
  `NULL_MINI_EXIT_WHEN_READY=1 NULL_MINI_NO_SHORTCUT=1 NULL_MINI_PROFILE=null-research-20261009`
  exited successfully in 0.41 s: `started`, `panel ready`, shortcut disabled, `shown; frontmost app:
  Ghostty`, `page ready`. This proves installed startup and page IPC, not live conversation, visual
  layout acceptance, global shortcut behavior or independence with every configured MCP server down.
  It appended normal operational log lines; no isolated settings/profile artifacts were found.
- `bash scripts/voicebox status`: server and narration worker up, Voice development app down,
  server login agent installed and loaded. This is the script's process view, not a GUI audit.
- `/health` on 17493 and 17494: healthy, model not loaded, PyTorch, MPS available.
- `GET /profiles`: `[]`; no existing profile is available for cloned-voice speech in this data dir.
- `GET /harnesses` without bearer token: 401. `GET /mini/sessions`: 405, POST allowed, consistent
  with no collection-list route. No authenticated session or conversation content was fetched.

### Not traced or not proved

Engine internals, all profile/import/export/DSP/timeline/cloud edge cases, GPU packaging variants,
updater/signature security, every Windows/Linux native path and all inherited release workflows were
surveyed only by role or entry point, not exhaustively audited. No real approval interaction, provider
fallback, audio quality/latency, echo cancellation, remote deployment or login/logout was exercised.
The reference-product research and provider legal/policy sources were not revalidated externally.
This map is preparation for the next task, not a claim of exhaustive correctness or user walkthrough.
