# Research: Null

> Last updated: 2026-10-07 (comprehensive). Supersedes the 2026-10-04 baseline and the 2026-10-05
> voice-loop deep-dive; their findings are kept where still true.

**How to read this.** Unmarked statements were read in the code on 2026-10-07. Statements marked
*(carried)* come from the 2026-10-04/05 passes and were not re-read. Nothing was executed in this
pass — no tests, no server, no build. Measured numbers are quoted from
`Context/Plans/VoiceLoopLatency.md`.

## What is this?

Null is a derivative of [Voicebox](https://github.com/jamiepine/voicebox) (MIT): a local-first voice
I/O stack that clones voices and generates speech (7 TTS engines), dictates into any app from a
global chord, and gives MCP-aware agents a voice — all inference on the user's machine.

**Naming.** Only the repo name and the README's Provenance section say "Null". Everything else still
says Voicebox: root package `voicebox` 0.5.0, product name, bundle id `sh.voicebox.app`, MCP server
name `voicebox`, `VOICEBOX_*` env vars, `voicebox.db`, the `voicebox-server` / `voicebox-mcp`
binaries, and the updater endpoint. The git history is one commit (`90a885c`, 2026-10-07).

## Owner orientation

**Product behavior (what the user observes).**
- Clone a voice from a few seconds of reference audio, then generate speech in it; results land in
  History as versions (original / effects / takes).
- Hold a chord anywhere, speak, release → the audio is transcribed (Whisper), optionally refined by a
  local LLM, and either pasted into the focused field or delivered into a running agent session.
- Agents speak back in a cloned voice. Two surfaces: the queued `speak` (persisted, plays via the
  pill) and `narrate` (ephemeral, streamed sentence chunks, plays via the pill's WebAudio player).
- One always-on-top pill window shows every state: `recording`, `agent`, `transcribing`, `refining`,
  `speaking`, `completed`, `error`. Agent speech is never silent by design — the pill is the contract.

**Must never happen.**
- Agent speech silently dropping (no audio, no visible pill).
- Narration or agent chatter writing to History or the data dir — narration is ephemeral by design.
- Two generations running concurrently on one engine instance → one serial queue owns generation.
- Dictation pasting into the wrong field, or clobbering the user's clipboard (conditional restore).
- The pill taking keyboard focus from the app the user is typing in.

**System roles.**

| Role | Owns | Explicitly does not decide |
|---|---|---|
| `app/` — React UI source (shared, aliased `@`) | All screens, routing, React Query server state, zustand UI state, the pill's state machine and audio playback | Platform specifics (each host injects `Platform`); when the pill window is shown, hidden or positioned (Rust does) |
| `tauri/` — desktop host (Rust) | Sidecar supervision, global chords, focus capture, clipboard save/restore, synthetic paste, pill window lifecycle, the `/events/speak` subscription, system-audio capture | Business logic; never touches SQLite or models. It decides *that* a chord fired, not what the transcript is for |
| `web/` — browser host | Same UI with a browser `Platform` (download vs save, updater disabled, no system audio) | Native features — deliberately unsupported |
| `backend/` — FastAPI sidecar | Domain, persistence, engine execution, MCP, the narration lane, agent delivery | UI presentation; native key handling; whether audio is actually audible |
| `backend/backends/` — engine adapters | One TTS/STT/LLM engine each behind a `runtime_checkable` Protocol | Engine selection (registry decides), chunking, effects, persistence |
| `landing/` + `docs/` | Next.js marketing site and docs site, independent | Everything app-related |

**One execution path — a voice turn into an agent.**

1. User holds the agent chord. `keytap` → `hotkey_monitor.rs` resolves `ChordAction::AgentPushToTalk`
   → snapshots focus → positions and shows the pill (no `set_focus`) → emits `dictate:start`
   `{focus, action: "agent"}`.
2. `DictateWindow` begins a turn trace, cancels any agent speech (barge-in), emits `speak:interrupt`
   to the main window, and starts recording.
3. Chord release → `dictate:stop` → recording stops → WAV upload to `POST /captures` → Whisper →
   one JSON body with `transcript_raw` and the paste flags.
4. If `auto_refine`, the client makes a second request, `POST /captures/{id}/refine`.
5. `onFinalText` forks on `action`: `"agent"` → `POST /voice-targets/message` → `herdr agent prompt`;
   anything else → Rust `paste_final_text`.
6. The agent replies by calling MCP `voicebox.speak` — a new request, not part of the loop. The
   backend publishes `speak-start` on `/events/speak`; Rust `speak_monitor` re-emits
   `dictate:speak-start`; the pill plays the audio and shows itself when sound actually starts.

*(carried)* **REST generation path.** `POST /generate` → validate profile + resolve engine → insert
`generations` row (`status=generating`) → enqueue on the single asyncio queue → `run_generation` →
lazy `load_engine_model` → cached voice prompt → `generate_chunked` (sentence split + crossfade) →
normalize → atomic WAV under `data/generations/` → `create_version` (`original`) → terminal status →
publish `speak-end`. Clients observe via SSE `GET /generate/{id}/status` (DB polled at 1 Hz).

## Architecture

React UI in a Tauri webview talks HTTP to a bundled Python FastAPI server on `127.0.0.1:17493`; that
server owns SQLite, model weights, audio files, TTS/STT/LLM inference, and the MCP endpoint at `/mcp`.
A second process — the narration worker (`VOICEBOX_ROLE=narration`, port 17494) — runs the same
`create_app()` with only `/health` and `POST /narration/{warm,synthesize}`, so agent commentary can
synthesize while a generation holds the main process's engine. The desktop host supervises the
sidecar and injects native input; it holds no domain state.

**Entry points.**

| Entry | What starts |
|---|---|
| `backend/main.py` | Dev server (`python -m backend.main`, or `uvicorn backend.main:app`) |
| `backend/server.py` | PyInstaller sidecar: `--host/--port/--data-dir/--parent-pid/--version`; picks CPU/CUDA/ROCm variant from the binary name; parent-pid watchdog |
| `backend/narration_main.py` | Narration worker; sets `VOICEBOX_ROLE` before importing the app |
| `tauri/src-tauri/src/main.rs::run` | Desktop host: plugins, state, the hidden pill window, `speak_monitor` |
| `tauri/src/main.tsx`, `web/src/main.tsx` | Mount shared `<App/>` inside `PlatformProvider` |
| `backend/mcp_shim/__main__.py` | `voicebox-mcp`: stdio ↔ Streamable-HTTP proxy for stdio-only MCP clients |
| `scripts/voicebox` *(carried)* | `start/stop/restart/status/logs/install/uninstall`; `install` writes a launchd LaunchAgent |

**Main-role startup order** (`app.py::_run_startup`): `init_db` → `init_queue` → mark stale
`generating`/`loading_model` rows failed → GPU compatibility log → CUDA/ROCm binary update checks
(background) → HF cache dir → start narration worker (background) → warm models (background).

**Hard boundaries.**
1. **Persistent generation goes through one serial queue** (`services/task_queue.py`: one
   `asyncio.Queue`, one `_generation_worker`). Routes must not call the engine directly.
   *Caveats:* `POST /generate/stream` and `generate_audio_sync` bypass it *(carried)*; in-process
   narration does not enqueue — it takes a per-engine `asyncio.Lock` and `wait_until_idle`s (up to
   180 s, then drops); the queue is per-process, so the narration worker is outside it.
2. **Engine choice is by engine-name string.** MLX-vs-PyTorch is decided only for `qwen`,
   `qwen_custom_voice`, `qwen_llm` and Whisper via `utils/platform_detect.get_backend_type()`
   *(carried)*.

**Sidecar lifecycle** (outlined from `main.rs`, `start_server` body not read in full): the host
reuses a healthy server already on 17493, kills orphans on legacy port 8000, and spawns the sidecar
otherwise. On exit the server self-terminates through the parent-pid watchdog unless "keep server
running" is set (a `.keep-running` sentinel plus `POST /watchdog/disable`).

**Cross-cutting.**
- **Auth:** none. Loopback by default; CORS allowlist plus `VOICEBOX_CORS_ORIGINS`. One explicit
  guard seen: MCP `transcribe(audio_path=…)` is loopback-only. Routes were not audited for others.
- **Caller identity:** `ClientIdMiddleware` copies `X-Voicebox-Client-Id` into a ContextVar. `.mcp.json`
  registers this repo's server for Claude Code as client `claude-code`.
- **Config:** `config.py` data dir (`--data-dir`; dev default `./data`), `VOICEBOX_MODELS_DIR` →
  `HF_HUB_CACHE`, `VOICEBOX_ROLE`, `VOICEBOX_TRACE`, `VOICEBOX_NARRATION_WORKER`, cloud URLs.
- **Errors:** services raise, routes translate to `HTTPException`. MCP tools raise `ValueError` with
  agent-directed wording ("Do not retry").
- **Logging:** stderr, uvicorn-style colours; the host forwards sidecar output as `server-log` events.

**HTTP surface** — 22 routers on the main role: health/shutdown/watchdog, profiles (+samples, avatar,
export, channels, effects), channels, generate (+retry, cancel, status SSE, stream, import), history,
transcribe, llm, captures (+refine, retranscribe, readiness), stories, effects (+versions), audio,
models, settings (`/settings/captures`, `/settings/generation`), tasks/cache, cuda, rocm, speak,
mcp bindings, `/events/speak`, cloud, agents + voice-targets, turns.

**MCP tools:** `voicebox.speak`, `voicebox.transcribe`, `voicebox.list_captures`,
`voicebox.list_profiles`.

**Tauri commands (23):** server (`start_server`, `stop_server`, `restart_server`,
`set_keep_server_running`, `set_backend_override`), audio (`start/stop_system_audio_capture`,
`is_system_audio_supported`, `list_audio_output_devices`, `play_audio_to_devices`,
`stop_audio_playback`), permissions (`check_/open_accessibility…`, `check_/open_input_monitoring…`),
dictation (`paste_final_text`, `enable_hotkey`, `disable_hotkey`, `update_chord_bindings`), and four
`debug_*` commands.

## The pill window

The pill is a second Tauri webview, not a component of the main window.

**Window** (`main.rs::build_dictate_window`): label `dictate`, URL `?view=dictate`, 420×64, no
decorations, transparent, always on top, visible on all workspaces, skipped from the taskbar, not
resizable, no shadow, created hidden at app setup. Both windows share one capability set.

**Placement:** horizontally centred, 4% down from the top of the current monitor (primary monitor as
fallback), recomputed on every show. It is not draggable and its position is not persisted.

**Show / hide are Rust-owned.**
- Show: chord start (`hotkey_monitor.rs`) or the `dictate:show` event. It never calls `set_focus`.
- Hide: on `dictate:hide`, Rust sets click-through, parks the window at (−10000, −10000), then hides
  it. `hide()` alone was unreliable for transparent always-on-top windows on macOS.
- Click-through toggling is skipped on Linux (it aborts if the window was never realized).

**Content** (`app/src/App.tsx` → `DictateWindow` → `CapturePill`): a 40 px rounded pill with a dot,
a label, five animated bars and an elapsed timer. `CapturePill` is also rendered in the main window
by the Captures tab and the Captures settings page. States: `hidden` plus `recording`, `agent`, `transcribing`, `refining`,
`speaking`, `completed`, `rest`, `error`. The error pill is a button that copies its message.

**Event contract.**

| Direction | Event | Payload / effect |
|---|---|---|
| Rust → pill | `dictate:start` | `{focus, action}`; `action` ∈ `push`, `toggle`, `agent` |
| Rust → pill | `dictate:stop` | Stop recording |
| Rust → pill | `dictate:restart` | Push-to-talk upgraded to toggle mid-hold. **No listener** |
| Rust → pill | `voice:stop`, `voice:toggle` | Stop / mute chords. **No listener** |
| Rust → pill | `dictate:speak-start` | Backend SSE payload as a JSON *string*: `generation_id`, `narration?`, `profile_name`, `source`, `client_id` |
| Rust → pill | `dictate:speak-end` | JSON string: `generation_id`, `status` |
| Pill → Rust | `dictate:show`, `dictate:hide` | Surface or tuck away the window |
| Pill → main window | `speak:interrupt` | Stop in-app WaveSurfer playback |
| Pill → main window | `capture:created`, `capture:updated` | Seed / refresh the captures list |
| Pill → main window | `system:accessibility-missing` | Prompt for the permission |

**Agent-speech cycle in the pill.**
- `speak-start` only *primes* the pill; the window is shown when audio actually starts, so the user
  never sees a silent pill during synthesis.
- Plain speak: `EventSource` on `/generate/{id}/status` → on `completed`, `new Audio(/audio/{id})`.
  Fetching `/audio/{id}` is the ack the backend's headless-playback fallback waits for.
- Narration: `useNarrationStream` → `EventSource` on `/speak/{id}/stream` → `narrationPlayer.ts`
  (WebAudio, 0.15 s start lead). Dismissal waits for buffered audio to drain, not for stream end.
- Watchdogs: 60 s of silence from either stream dismisses; a `completed` speak-end with no audio
  dismisses after 15 s.
- Last speak wins: a new `speak-start` tears down the previous cycle.
- Barge-in: `dictate:start` calls `dismissSpeak()` before the mic opens. Closing the narration stream
  is what cancels synthesis server-side; `/speak/stop` is not called.

**Why Rust owns the speak subscription:** hidden WebKit windows on macOS throttle long-lived network
connections, so an `EventSource` in the hidden pill missed events. Tauri's event bus reaches hidden
webviews reliably. `speak_monitor.rs` reconnects with backoff (0.5 s → 30 s cap) and treats 45 s
without a frame as a dead stream (the backend pings every 15 s).

## The voice loop, stage by stage

Measured baseline (this machine, MPS, Whisper `small`, LuxTTS clone) from the latency plan.

| # | Stage | Owner | Warm cost |
|---|---|---|---|
| 1 | chord → focus snapshot | `tauri/hotkey_monitor.rs` (`keytap` chords → `ChordAction` → `Effect`, `focus_capture`) | — |
| 2 | record → blob → WAV | `app/useAudioRecording` → `convertToWav` *(carried)* | unmeasured |
| 3 | upload + decode + STT | `POST /captures` → `services/captures.create_capture` → Whisper singleton | 1.05–1.93 s |
| 4 | refine (only if `auto_refine`) | client chains `POST /captures/{id}/refine` | ~1.3 s warm / 9.6 s cold |
| 5 | route the transcript | `DictateWindow.onFinalText` | — |
| 6 | deliver to the agent | `POST /voice-targets/message` → `herdr agent prompt` | 0.02–0.13 s |
| 7 | answer synthesis | MCP `voicebox.speak` / narrate → narration lane | 2.08–2.46 s to first chunk |
| 8 | playback | pill `useNarrationStream` + `narrationPlayer.ts` | 0.15 s start lead |

Boundaries that matter:

- **Capture decides transcription, not delivery.** `POST /captures` returns `transcript_raw`,
  `auto_refine`, `allow_auto_paste`, `submit_after_paste` in one JSON body. There is no SSE in
  dictation. The client decides whether to refine and where the text goes.
- **Delivery is input, not output.** `agent_voice_enabled` gates speech (`/speak` and
  `/speak/narrate` → 409, MCP speak → `ValueError`) but not `/voice-targets/message` and not
  `/speak/stop` — muting agents must never mute the user, and silence must always work.
- **The reply re-enters through the API.** The backend does not wait for or read the agent's answer.
- **Audio format:** dictation uploads WAV; WebM/Opus only when browser-side conversion throws
  *(carried)*. Non-WAV input is decoded by librosa (ffmpeg via audioread) and transcoded to WAV.

**Turn trace** (dev instrumentation, on by default, `VOICEBOX_TRACE=0` disables):
- `backend/utils/timing.py`: in-memory ring of 50 traces; first write wins per `(turn_id, stage)`;
  one JSON line per finished turn to `<data>/logs/turns.jsonl`.
- `routes/turns.py`: `POST /turns/{id}/begin`, `POST /turns/{id}/marks`, `GET /turns/latest`,
  `DELETE /turns`. Client side: `app/src/lib/utils/turnTrace.ts`, fire-and-forget.
- **A dictate → hear-answer turn produces two records, not one.** The capture half uses a
  client-generated id (sent as `turn_id` on captures, refine and deliver) and ends at delivery. The
  speak half uses the narration session id, generated by the backend. Nothing links the two ids.
- Worker-side stages (`model_load_*`, `prompt_*`) travel as `mark` items on the audio queue and as an
  SSE `mark` event from the worker, so they land in the main process's store.
- Only narration is traced on the speak side; a plain queued `speak` records nothing.

**Startup warm-up.** `app.py::_warm_startup_models` preloads Whisper and, only when `auto_refine`,
the refinement LLM — each only if already in the HF cache (never downloads). The narration worker
warms via `POST /narration/warm` once healthy. Gated by `capture_settings.warm_models_on_startup`.

## Agent narration lane

- `POST /speak/narrate` and MCP `voicebox.speak(narrate=true)` → `routes/speak.py::narrate_speech`.
- `services/narration.resolve_narration_lane` returns `(engine, reason)` only when
  `generation_settings.agent_narration` is on, the engine validates for the profile, and the profile
  is `cloned` or `preset`. Engine resolution matches `POST /generate`: requested → `default_engine` →
  `preset_engine` → `qwen`. Otherwise the call degrades to the queued speak (`mode="generation"`).
- Personality rewrite, if requested, runs *before* the session is created so the LLM stays off the
  streaming path.
- Lane present → in-memory `NarrationSession` (cap 16, unclaimed TTL 120 s) → `GET /speak/{id}/stream`
  (SSE: `ready`, `loading`, `waiting`, `chunk` = base64 WAV, `done`, `error`). `claim_session` makes a
  second GET a 409.
- With a worker, the stream relays `POST {worker}/narration/synthesize`. Without one, the same route
  synthesizes in-process under `engine_stream_lock` after `wait_until_idle`.
- Loudness: gain is computed once from the first chunk and reused, so levels hold across sentences.
- In-process synthesis runs in a detached task: a client disconnect must not release the engine lock
  while a worker thread is still inside the model, or the next narration aborts the process on MPS.
- Nothing is persisted. Disconnect cancels at the next chunk boundary and publishes
  `speak-end "cancelled"`.
- `POST /speak/stop` → `narration.cancel_all()` + `playback.stop_playback()` + `speak-end cancelled`
  with no generation id.
- **Headless playback** (`services/playback.py`): wired only to the queued generation path in
  `services/generation.py`. When no pill is subscribed to `/events/speak`, the setting is on and the
  generation is agent-initiated, the backend plays the WAV through the OS. Narration never triggers
  it, so with no pill attached a narration synthesizes to nobody.
- **Worker process** (`services/narration_worker.py`): adopts a worker already on 17494, else spawns
  one (`python -m backend.narration_main` in dev), waits up to 120 s for `/health`, then warms it.
  `VOICEBOX_NARRATION_WORKER=0` disables it. Any failure leaves narration in-process.

## Voice pairing (herdr)

- `services/voice_targets.py`: plain functions shelling out to the local `herdr` CLI — `agent list`
  (10 s timeout) and `agent prompt` (30 s). Looks in `PATH`, then `/opt/homebrew/bin`,
  `/usr/local/bin`, `~/.local/bin`.
- `GET /agents`: live agents with `target`, `agent`, `status`, `ready`, `cwd`, `title`, `focused`.
  A missing herdr is a normal response (`available: false`), not an error.
- `POST /voice-targets/message`: resolve target (request → `capture_settings.agent_target`), re-list
  agents, verify the target is live, then prompt with the voice-turn prefix. Codes: 409 (no target,
  target gone, or agent blocked on the user), 503 (herdr missing or unreachable), 502 (other).
- `VOICE_TURN_INSTRUCTION = "[voice turn] reply aloud, two sentences max."` The marker is read by the
  harness instruction files (`~/.omp`, `~/.claude`, `~/.config/opencode`, `~/.codex`), not by code
  *(carried)* — nothing forces a spoken reply if the agent ignores it.

## Domain Model

Tables (16): `profiles`, `profile_samples`, `generations`, `generation_versions`, `stories`,
`story_items`, `projects`, `effect_presets`, `audio_channels`, `channel_device_mappings`,
`profile_channel_mappings`, `capture_settings`, `generation_settings`, `cloud_settings`,
`mcp_client_bindings`, `captures`. Field-level detail below is *(carried)*.

- **VoiceProfile** — `voice_type ∈ {cloned, preset, designed}`. `cloned` uses `profile_samples`
  (audio + reference text). `preset` is locked to `preset_engine` + `preset_voice_id`. `designed`
  carries `design_prompt` and has no synthesizable voice yet. Optional `personality`,
  `effects_chain`, `default_engine`. `profiles.name` is UNIQUE.
- **Generation** — text, language, engine, model_size, seed, instruct, status, `audio_path` (mirrors
  the default version), `is_favorited`, `source` (`manual` | `personality_speak` | `mcp` | `rest`).
- **GenerationVersion** — lineage: `original`, effects versions, `take-N`, `clean`; one `is_default`;
  deleting the last is refused.
- **Story / StoryItem** — multi-track timeline; items reference a generation and optional version.
- **Capture** — dictation / recording / file audio + `transcript_raw` / `transcript_refined` + flags.
- **MCPClientBinding** — per-client voice, engine and personality defaults + `last_seen_at`.
- **Singleton rows id=1** — `capture_settings`, `generation_settings`, `cloud_settings`.
- **`capture_settings`** holds the chords (`chord_push_to_talk_keys`, `chord_toggle_to_talk_keys`,
  `chord_voice_stop_keys`, `chord_voice_toggle_keys`, `chord_agent_keys`), `hotkey_enabled`,
  `headless_playback`, `default_playback_voice_id`, `agent_voice_enabled`, `agent_target`,
  `warm_models_on_startup`, plus `stt_model`, `llm_model`, `auto_refine`, paste flags.
- **`generation_settings`** holds `max_chunk_chars`, `crossfade_ms`, `normalize_audio`,
  `autoplay_on_generate`, `agent_narration`.

**Voice resolution** (`mcp_server/resolve.py`, shared by MCP and REST speak): explicit `profile` →
per-client binding → `capture_settings.default_playback_voice_id` → error.

**Chords** are sets of keys, not sequences, with left/right modifier fidelity. Five actions:
push-to-talk, toggle-to-talk, agent push-to-talk, voice stop, voice toggle. Defaults *(carried)*:
macOS right Cmd+Option (+`KeyA` agent, `KeyX` stop, `KeyM` mute, `Space` toggle); Windows right
Ctrl+Shift equivalents.

**Engines:** TTS `qwen`, `qwen_custom_voice`, `luxtts`, `chatterbox`, `chatterbox_turbo`, `tada`,
`kokoro`; STT `whisper` (5 sizes); LLM `qwen_llm` (3 sizes).

## Patterns

- **Layering.** `routes/` thin → `services/` logic + ORM sessions → `backends/` engines behind
  Protocols. Adding an engine touches the registry (`backends/__init__.py`) and one new module.
  `ModelConfig` is built by registry factory functions.
- **MCP tools adapt, routes orchestrate.** `mcp_server/tools.py` lazily imports `generate_speech` and
  `narrate_speech` from `routes/` rather than re-implementing them.
- **Heavy imports are lazy** (inside factories and load functions). Exception: `app.py` imports torch
  at module scope, deliberately after the ROCm env setup.
- **Models are process-global singletons**, lazily loaded, warmed at startup, never downloaded
  implicitly at startup.
- **Paths in the DB are relative to the data dir** (`config.to_storage_path` /
  `resolve_storage_path`). An empty path must resolve to `None`; 404 guards depend on it.
- **Migrations** *(carried)*: hand-rolled, idempotent, run before `create_all`
  (`database/migrations.py`; no Alembic). Add a `_migrate_*` helper and call it from
  `run_migrations()`.
- **Naming** *(carried)*: ORM models aliased `DB`-prefixed; Pydantic models suffixed
  `…Create/…Response/…Request`; backend classes engine-prefixed.
- **Python style:** `backend/pyproject.toml` is authoritative — Python 3.12+, Ruff, 120 cols, double
  quotes. `CONTRIBUTING.md`'s Black text is stale *(carried)*.
- **TypeScript:** React 18, `@tanstack/react-router` with code-defined routes, React Query, one
  zustand store per concern (8 stores), Biome, Tailwind 4, i18next (9 locales).
- **Platform abstraction:** `app/src/platform/types.ts` defines `Platform` = `filesystem`, `updater`,
  `audio`, `lifecycle`, `metadata`. `tauri/src/platform/` and `web/src/platform/` implement it.
  Direct `@tauri-apps/api` imports in `app/` (as in `DictateWindow`) bypass it.
- **One SPA, two windows.** `App.tsx` branches on `?view=dictate` before any main-app hooks run.
- **Cross-window state travels by Tauri events**, not shared stores — the pill and main window are
  separate webviews.
- **API client:** the live client is the hand-written `app/src/lib/api/client.ts` + `types.ts`. The
  generated tree under `lib/api/` is not imported *(carried)*.
- **Tests:** 38 `test_*.py` files in `backend/tests/`, mixing real pytest tests with scripts needing
  a live server, downloads or a GPU *(carried)*. One Rust test file. No frontend test files.
- **CI** (`ci.yml`): frontend typecheck + web build only. No Python, Rust or Biome in CI.
- **Tasks:** `justfile` is the task runner; root `package.json` scripts wrap the same things.
- **Release** *(carried)*: `bumpversion` across 8 version files, driven by `.agents/skills/`.

## Relevant ADRs

`Context/ADR/` exists and is empty; `Context/Glossary.md` is an unfilled template. Decisions live in
code comments and the four plans:

- No Alembic (`database/migrations.py`).
- Generation serialized on one queue (`services/task_queue.py`).
- Narration runs in a separate process — two threads sharing one torch/MPS model abort the process;
  two processes are safe (`Context/Plans/AgentVoiceLimits.md` §1).
- Narration shares the work engine; the CPU-lane design was abandoned
  (`Context/Plans/RealtimeAgentNarration.md`, "Open decision").
- The pill is the only playback surface for agent speech; Rust owns its visibility.
- Startup never downloads a model.
- Whisper stays on PyTorch CPU `small` on this machine: MLX measured 13× slower and hallucinated,
  CTranslate2 was no faster (`Context/Plans/VoiceLoopLatency.md`).
- Plans: `RealtimeAgentNarration` (implemented), `AgentVoiceLimits` (limits register),
  `VoicePairing` (partly built), `VoiceLoopLatency` (trace shipped; B1-vs-B3 and the acceptance bar
  are user-owned decisions).

## Open Questions

**New this pass**

- **Packaged builds cannot start the narration worker.** `narration_worker._spawn_command` re-runs
  the frozen executable with `--role narration`, but `server.py` defines no `--role` argument and
  uses `parse_args()`, and never sets `VOICEBOX_ROLE`. Read from code, not run: the child should exit
  on the unknown flag and narration should fall back to in-process, waiting behind the queue.
- **`dictate:restart` has no listener.** Rust coalesces a push-to-talk → toggle upgrade into one
  restart event; nothing in `app/` handles it.
- **README Provenance says the full commit history is preserved; the repo has one commit.**
- **The updater still points upstream** (`jamiepine/voicebox` releases, upstream public key). A
  packaged Null build would offer upstream Voicebox updates. Not tested.
- **MCP `voicebox.speak` description is stale:** it promises a "CPU narration lane" needing "a Kokoro
  preset profile, or an English cloned profile", and says speech is saved to "Captures / History".
  The implementation shares the work engine and accepts any cloned or preset profile.
- **A voice turn is two unlinked trace records** (see Turn trace). An end-to-end number still needs a
  join by wall clock.

**Still true (re-verified)**

- **The stop and mute chords are half-wired.** Rust emits `voice:stop` / `voice:toggle`; no frontend
  listener exists, and `apiClient.stopSpeaking()` has no caller. The backend endpoint and the
  Settings toggle work; the chord → endpoint link does not.
- **Agent delivery has no paste fallback.** The agent branch shows a destructive toast and returns.
- **`generation_settings` is not consulted by `POST /generate`**, which uses request-body values.
- **`create_capture` has no model-cache pre-check**; only the UI readiness gate prevents an inline
  download during a dictation.
- **Narration is silent with no pill attached** (headless playback covers only the queued path).
- **`Project` ORM model** is declared and exported with no routes or services.
- **Web deployment route collision:** backend `GET /settings/captures` and `/settings/generation`
  shadow the SPA routes of the same path on a hard navigation (behaviour reported in the latency
  plan; both routes confirmed to exist).
- `VoicePairing` items not built: voice mode / end-of-utterance detection (item 10), reading the
  agent's reply without its cooperation (item 11), non-herdr transports (item 12). `herdr agent read`
  is not implemented.

**Carried, not re-verified**

- `tada`: an unrecognized `model_size` silently loads the 1B repo.
- Whether the `original` version is intentionally deletable while another exists.
- `docs/PROJECT_STATUS.md` (2026-07-02) is a stale roadmap snapshot.
- Web build depends on bun workspace hoisting for `@tauri-apps/*`.
- STT never uses MPS (`allow_mps` defaults to false in `get_torch_device`).

## Stale / dead code

Re-verified this pass:
- `apiClient.stopSpeaking()` has no caller.
- Two `useAutoUpdater` files exist (`.ts` and `.tsx`).
- `components/AudioTab/`, `components/AudioStudio/` and `components/ServerSettings/` still exist
  alongside their replacements.
- `mcp_server/server.py::mount_into` is exported but never called; `create_app` wires the MCP app
  itself.
- Root `requirements.txt` sits beside the real `backend/requirements*.txt`.

*(carried)*:
- `useAutoUpdater.tsx` (toast UI) is unreachable because Vite resolves `.ts` first.
- The generated API tree is imported nowhere and drifts from the hand client.
- `ServerSettings/`: only `ModelManagement.tsx` is imported.
- `app/src/main.tsx` + `app/index.html` + `app/vite.config.ts` are a stale shell without
  `PlatformProvider`.

## Not traced

Read only as file names, route lists or outlines:

- **Backend:** stories/timeline, effects, export/import, history, profiles internals, model
  management and download progress, CUDA/ROCm binary services, cloud sync (`voicebox.sh`), refinement
  and personality prompts, each engine backend's internals, `utils/chunked_tts.py`, PyInstaller build.
- **Rust:** `clipboard.rs`, `focus_capture.rs`, `synthetic_keys.rs`, `keyboard_layout.rs`,
  `audio_capture/*`, `audio_output.rs`, and the body of `start_server`.
- **Frontend:** every main-window screen (Generate, Stories, Voices, Captures, Effects, Models,
  Settings pages), the stores, `useCaptureRecordingSession` beyond its state transitions,
  `useAudioRecording`, `narrationPlayer.ts` beyond its constants.
- **Other:** `landing/`, `docs/` (66 content files), Docker files, release workflows,
  `scripts/loop-bench`, `scripts/voicebox`.
