# Research: Voicebox

> Last updated: 2026-10-05 — voice loop end-to-end deep-dive (architecture baseline: 2026-10-04 comprehensive)

## What is this?

A local-first AI voice studio: clone voices and generate speech (7 TTS engines), dictate into any
app from a global hotkey, and give MCP-aware agents a voice — all inference on the user's machine.

## Owner orientation

**Product behavior (what the user observes).**
- Clone a voice from a few seconds of reference audio, then generate speech in it; results land in
  History as versions (original / effects / takes).
- Hold a chord anywhere, speak, release → transcript is transcribed (Whisper), optionally refined
  by a local LLM, and either pasted into the focused field or delivered into a running agent pane.
- Agents speak back in a cloned voice. Two surfaces: the queued `speak` (persisted, plays via the
  pill) and `narrate` (ephemeral, streamed sentence chunks, plays via the pill's WebAudio player).
- One always-on-top pill window shows every state: `recording`, `transcribing`, `refining`,
  `speaking`. Agent speech is *never* silent by design (the pill is the contract).

**Must never happen.**
- Agent speech silently dropping (no audio, no visible pill) — hard-won; see the `speak` watchdog /
  headless-fallback work.
- Narration or agent chatter writing to History / the data dir — narration is ephemeral by design.
- Two GPU generations running concurrently → one serial queue owns generation.
- Dictation pasting into the wrong field, or clobbering the user's clipboard (conditional restore).

**System roles.**

| Role | Owns | Explicitly does not decide |
|---|---|---|
| `app/` — React UI source (shared, aliased as `@`) | All screens, routing, React Query server state, zustand UI state | Platform specifics (each host injects `Platform`); the HTTP contract (`lib/api/client.ts` is hand-written) |
| `tauri/` — desktop host (Rust) | Window/sidecar supervision, global hotkey chords, focus capture, clipboard save/restore, synthetic paste, pill window + speak SSE, system-audio capture | Business logic; never touches SQLite or models — it supervises the sidecar and emits UI events |
| `web/` — browser host | Same UI with a no-op/browser `Platform` (download vs save, updater disabled, no system audio) | Native features — deliberately unsupported |
| `backend/` — FastAPI sidecar | Domain + persistence + engine execution + MCP + the narration lane | UI presentation; per-platform native key handling |
| `backends/` — engine adapters | One TTS/STT/LLM engine each behind a `runtime_checkable` Protocol; `ModelConfig` metadata in the registry | Engine selection (registry decides), chunking, effects, persistence |
| `landing/` + `docs/` | Next.js marketing site and docs site, independent | Everything app-related |

**One execution path (REST generation).** `POST /generate` → validate profile + resolve engine →
insert `generations` row (`status=generating`) → enqueue on the single global asyncio queue →
worker `run_generation` → lazily `load_engine_model` → build cached voice prompt → `generate_chunked`
(sentence split + crossfade) → normalize → atomic WAV under `data/generations/` → `create_version`
(`original`, default) → terminal status → publish `speak-end`. Clients observe via SSE:
`GET /generate/{id}/status` (DB polled at 1 Hz) or `GET /events/speak` (in-memory pub/sub → pill).

## Architecture

React UI in a Tauri webview talks HTTP to a bundled Python FastAPI server on `127.0.0.1:17493`;
that server owns SQLite, model weights, audio files, TTS/STT/LLM inference, and the MCP endpoint at
`/mcp`. A **second process** — the narration worker (`VOICEBOX_ROLE=narration`, port 17494) — runs
the same `create_app()` with only `/health` + `POST /narration/synthesize`, so agent commentary can
synthesize at the same time as a long generation. The desktop host only supervises the sidecar and
injects native input; it holds no domain state.

Two hard boundaries:
1. **Persistent GPU generation goes through one serial queue** (`services/task_queue.py`:
   one `asyncio.Queue`, one `_generation_worker`) — routes must never call the engine directly.
   *Caveats:* `POST /generate/stream` and `generate_audio_sync` bypass the queue; the in-process
   narration path does not enqueue either (it takes a per-engine `asyncio.Lock` and only
   `wait_until_idle`s); and the queue is per-process, so the narration worker is outside it.
2. **Engine choice is by engine-name string**; MLX-vs-PyTorch is decided only for
   `qwen`/`qwen_custom_voice`/`qwen_llm`/Whisper via `utils/platform_detect.get_backend_type()`
   (MLX iff Darwin+arm64 and `mlx.core` imports). Everything else is PyTorch-only.

## The voice loop, stage by stage (deep-dive 2026-10-05)

One dictate → agent → hear-answer turn crosses every tier. Measured baseline lives in
`Context/Plans/VoiceLoopLatency.md`; the numbers below are that plan's, taken on this machine
(MPS, Whisper `small`, LuxTTS clone).

| # | Stage | Owner | Warm cost |
|---|---|---|---|
| 1 | chord → focus snapshot | `tauri/hotkey_monitor.rs` (`keytap` chords → `ChordAction` → `Effect`, `focus_capture`) | — |
| 2 | record → blob → **WAV** | `app/useAudioRecording` (MediaRecorder `audio/webm;codecs=opus`, no timeslice) → `convertToWav` (WebAudio `decodeAudioData` → WAV) | B6, unmeasured |
| 3 | upload + decode + STT | `POST /captures` → `services/captures.create_capture` → `get_whisper_model()` singleton | 1.05–1.93 s |
| 4 | refine (only if `auto_refine`) | client chains `POST /captures/{id}/refine` → `refinement.refine_transcript` | ~1.3 s warm / 9.6 s cold (off here) |
| 5 | route the transcript | `app/DictateWindow.onFinalText` | — |
| 6 | deliver to the agent | `POST /voice-targets/message` → `herdr agent prompt` | 0.02–0.13 s |
| 7 | answer synthesis | MCP `voicebox.speak`/`narrate` → narration lane (worker 17494) | 2.08–2.46 s to first chunk |
| 8 | playback | pill `useNarrationStream` + `narrationPlayer.ts` (WebAudio) | 0.15 s `START_LEAD_SECONDS` |

Control flow, and the boundaries that matter:

- **Capture decides transcription, not playback.** `POST /captures` returns `transcript_raw`,
  `auto_refine`, `allow_auto_paste`, `submit_after_paste` in **one JSON body — no SSE anywhere in
  dictation**. The client decides whether to refine (a second request, `POST /captures/{id}/refine`)
  and whether to paste. `transcript_refined ?? transcript_raw` is the final text.
- **`DictateWindow.onFinalText` is the fork:** `action === 'agent'` → `sendVoiceMessage` (no
  clipboard, no focus, no paste); anything else → Rust `paste_final_text` (clipboard snapshot →
  `CGEventPost` Cmd+V → restore **only if `changeCount` is unchanged**). `action` rides the
  `dictate:start` payload from Rust.
- **Delivery is input, not output.** `agent_voice_enabled` gates speech (`/speak*` → 409, MCP speak
  → `ValueError`, headless playback skipped) but **not** `/voice-targets/message`, and not
  `/speak/stop` (silence must always work).
- **The agent's reply re-enters through the API, never through the loop:** it calls
  `voicebox.speak`, and the pill is the only playback surface — driven by Rust `speak_monitor`
  (subscribes `GET /events/speak`, re-emits `dictate:speak-start`) plus the two frontend
  EventSources (`/generate/{id}/status` for plain speak, `/speak/{id}/stream` for narration).
- **Format correction:** dictation uploads **WAV** (browser-side `convertToWav`); WebM/Opus is only
  the fallback when that conversion throws (WebKit), plus the file-import path and the bench. So the
  ffmpeg bug below is real but does not sit on the default dictation path — the plan's bench row
  "WebM/Opus — what the app sends" is imprecise.

### Shipped 2026-10-04/05 (uncommitted, in the working tree)

- **Startup warm-up — the measured dominator was cold start, not any warm stage.**
  `app.py::_warm_startup_models()` (background asyncio task, non-blocking) preloads Whisper and, only
  when `auto_refine`, the refinement LLM — each only if already in the HF cache (**never
  downloads**; `is_model_cached` gate). The narration worker warms separately via
  `POST /narration/warm`, called by `services/narration_worker.py` once `/health` is up, under the
  same `engine_stream_lock` the synth path takes. Gated by
  `capture_settings.warm_models_on_startup` (default on; Settings → Captures → Transcription).
  Effect: first-turn STT 4.55 → 2.50 s; narration first chunk 4.11 → 2.71 s.
- **ffmpeg discovery.** `utils/audio.py::_ensure_ffmpeg_available()` prepends the first existing dir
  of `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin` when `shutil.which("ffmpeg")` fails, before
  librosa's audioread fallback. Reason: a server started by launchd (or the packaged app) inherits a
  minimal `PATH`, so ffmpeg-decoded inputs failed as "the recording may be empty or corrupt". No env
  var, no `imageio-ffmpeg` fallback.
- **LuxTTS diffusion steps 8 → 6** (`luxtts_backend.LUXTTS_STEPS`): the measured knee for
  time-to-first-audio. Applies to **every** LuxTTS generation, not only narration.

New symbols: `capture_settings.warm_models_on_startup` (ORM + `_migrate_capture_settings` +
`CaptureSettings{Response,Update}` + TS `CaptureSettings`), `_warm_startup_models` (`app.py`),
`POST /narration/warm` (`routes/narration_worker.py`), `_ensure_ffmpeg_available` (`utils/audio.py`).
Tests: `backend/tests/test_warm_startup.py` (warm gating truth table), `test_ffmpeg_discovery.py`
(PATH discovery). Instrument: `scripts/loop-bench` — HTTP-only per-stage bench, needs a live healthy
server, deletes every capture row it creates. **The in-process turn trace and the frontend marks
(plan items 1–2) are not built**; the bench is the only instrument.

## Domain Model

- **VoiceProfile** — `voice_type ∈ {cloned, preset, designed}`. `cloned` uses `profile_samples`
  (audio + reference text). `preset` is locked to `preset_engine`+`preset_voice_id` (Kokoro/Qwen
  CustomVoice). `designed` carries `design_prompt` (roadmap). Optional `personality` (free-form
  persona), `effects_chain` (JSON), `default_engine`. `profiles.name` is UNIQUE.
- **Generation** — one request: text, language, engine, model_size, seed, instruct, status,
  `audio_path` (mirrors the **default version**), `is_favorited`, `source`
  (`manual` | `personality_speak` | `mcp` | `rest`).
- **GenerationVersion** — lineage: `original` (clean), effects versions, `take-N` regenerations,
  `clean` (backfill); self-FK `source_version_id`; one `is_default`; deleting the last is refused.
- **Story / StoryItem** — multi-track timeline; items reference generation + optional pinned version.
- **EffectPreset** — `is_builtin` rows immutable, seeded from `utils/effects.BUILTIN_PRESETS`.
- **Capture** — dictation/recording/file audio + `transcript_raw`/`transcript_refined` + flags.
- **AudioChannel / ChannelDeviceMapping / ProfileChannelMapping** — output routing.
- **MCPClientBinding** — per-`X-Voicebox-Client-Id` voice/engine/personality defaults + `last_seen_at`.
- **Singleton rows id=1** — `capture_settings`, `generation_settings`, `cloud_settings`.
- **`capture_settings`** also holds the dictation chords (`chord_push_to_talk_keys`,
  `chord_toggle_to_talk_keys`, `chord_voice_stop_keys`, `chord_voice_toggle_keys`,
  `chord_agent_keys`), `hotkey_enabled`, `headless_playback`, `default_playback_voice_id`,
  `agent_voice_enabled` (mute gate), `agent_target` (herdr pane id for voice pairing), and
  `warm_models_on_startup` (startup model preload, default on).
- **`generation_settings`** holds `max_chunk_chars`, `crossfade_ms`, `normalize_audio`,
  `autoplay_on_generate`, `agent_narration`.

Voice resolution precedence (shared by MCP `voicebox.speak` **and** REST `POST /speak`, code in
`mcp_server/resolve.py`): explicit `profile` → per-client binding → `capture_settings.default_playback_voice_id` → error.

Chords are **sets of keys** (W3C `KeyboardEvent.code` names, canonicalized to keytap variants), not
sequences. Defaults live in `utils/capture_chords.py` and mirror `app/src/lib/utils/keyCodes.ts`:
macOS right Cmd+Option (+`KeyA` agent, `KeyX` stop, `KeyM` mute, `Space` toggle), Windows right
Ctrl+Shift equivalents.

## Patterns

- **Layering.** `routes/` thin (validate → delegate → `HTTPException`) → `services/` business logic
  + ORM sessions → `backends/` engines behind Protocols. Adding an engine touches the registry
  (`backends/__init__.py`) and a new backend module only. `ModelConfig` is built by registry factory
  functions, *not* per-class `MODEL_CONFIGS` (the Protocol docstring implies otherwise; no class sets it).
- **Heavy imports are lazy.** `torch`/`mlx`/`transformers`/engine libs are imported inside factories
  or load/generate functions, so the server starts fast. Exception: `app.py` imports torch at module
  scope deliberately, *after* ROCm env setup.
- **Model load is lazy but warmed at startup.** Backend instances are process-global singletons
  (`_stt_backend`, `_llm_backends` keyed by engine), size-guarded: a model stays in RAM until a
  different size is requested. Startup preloads what the next dictation needs
  (`warm_models_on_startup`), never downloads (`is_model_cached` gate before `load_model`).
- **Audio input is decoded by librosa and canonicalized to WAV.** `load_audio` (sr=24000, mono)
  reaches `ffmpeg` through audioread for non-native formats, so the process `PATH` is a real
  dependency — hence `_ensure_ffmpeg_available()`. Dictation itself sends WAV; the browser converts.
- **Paths in the DB are relative to the data dir** (`config.to_storage_path` /
  `resolve_storage_path`); every write goes through those. Empty path must resolve to `None`
  (404 guards depend on it).
- **Migrations**: hand-rolled, idempotent, run before `Base.metadata.create_all`
  (`database/migrations.py`; no Alembic — single-user SQLite). Add a `_migrate_*` helper and call it
  from `run_migrations()`.
- **Naming**: ORM models aliased `DB`-prefixed (`VoiceProfile as DBVoiceProfile`); Pydantic models
  suffixed `...Create/...Response/...Request`; backend classes engine-prefixed.
- **Python style** (`backend/STYLE_GUIDE.md` + `pyproject.toml` win over `CONTRIBUTING.md`):
  Python 3.12+, Ruff (not Black), 120 cols, double quotes, `list[str]`/`X | Y`, Google docstrings,
  `%s` lazy logging, `# noqa` must carry a reason, no bare except. Ruff adoption is incremental.
- **TypeScript**: React 18 + `@tanstack/react-router` (code-defined routes, no filesystem routes),
  React Query for server data, one zustand store per concern, Biome for lint/format, Tailwind 4,
  WaveSurfer for audio, i18next (9 locales).
- **API client** — the live client is the **hand-written** `app/src/lib/api/client.ts` (plain
  `fetch`, ~130 methods) + `types.ts`. The entire generated tree
  (`lib/api/{index,models,schemas,services,core}`, produced by `scripts/generate-api.sh` from
  `app/openapi.json`) is **imported nowhere** and already drifts from the hand client.
- **Tests**: `backend/tests/` mixes real pytest tests with manual scripts needing a live server,
  network/HF downloads, or a specific GPU. A large fraction are not GPU-free. CI runs only
  frontend typecheck + web build (`ci.yml`) — no Rust, no Python, no Biome, no Docker in CI.
- **Release flow**: `bumpversion` (`.bumpversion.cfg`, 8 tracked version files) driven by the
  `.agents/skills/{draft-release-notes,release-bump,triage-prs}` skills; tag push → `release.yml`
  *reads* (does not compile) the CHANGELOG section. The CHANGELOG header claiming automatic compile
  is misleading. `justfile` is the canonical task runner.
- **Dev CLI**: `scripts/voicebox start|stop|restart|status|logs|install|uninstall` starts the
  backend detached (no `bun dev`), waits for the 17494 worker, and `install` writes a macOS
  launchd LaunchAgent (`~/Library/LaunchAgents/dev.voicebox.server.plist`).

## Agent narration lane (the recent center of gravity)

- `POST /speak/narrate` and MCP `voicebox.speak(narrate=true)` → `routes/speak.py::narrate_speech`.
  `services/narration.resolve_narration_lane(db, profile, requested_engine)` returns `(engine, reason)`
  only when `generation_settings.agent_narration` is on, the engine validates for the profile, and
  the profile is `cloned` or `preset`. It resolves the engine exactly like `POST /generate`
  (requested → `default_engine` → `preset_engine` → `qwen`) — an earlier CPU-lane design
  (`cloned`→`luxtts`, `preset`→`kokoro`) was abandoned because LuxTTS's CPU path under-generates
  short text (`Context/Plans/RealtimeAgentNarration.md` "Implementation notes"). Otherwise it
  degrades to the queued `mode="generation"` speak — "narration-chatter falls back to normal speak".
- Lane present → in-memory `NarrationSession` (cap 16, TTL 120 s) → `GET /speak/{id}/stream` (SSE:
  `ready`, `loading`, `waiting`, `chunk` = base64 WAV, `done`, `error`). `claim_session` makes a
  second GET a 409. Worker relay via `POST {17494}/narration/synthesize`; if the worker is absent,
  the same route synthesizes in-process under `engine_stream_lock` + `wait_until_idle`.
- Nothing is persisted. Disconnect cancels at the next chunk boundary and publishes
  `speak-end "cancelled"`.
- `POST /speak/stop` → `narration.cancel_all()` + `playback.stop_playback()` + `speak-end cancelled`
  (the user's stop chord).
- **Headless playback** (`services/playback.py`): when no pill is subscribed to `/events/speak`,
  `headless_playback` is on, and the generation is agent-initiated, the backend plays the WAV via
  `afplay`/`paplay`/PowerShell. Completion waits ~4 s for the pill to fetch `/audio/{id}` (that fetch
  *is* the ack) and falls back locally if it does not. **Wired only to the queued generation path**
  (`services/generation.py`) — narration never triggers OS playback and has no `/audio/{id}`, so with
  no pill attached a narration synthesizes to nobody (the plan's open residual).
- `agent_voice_enabled` (capture_settings) is the mute gate: `/speak*` return 409 and the MCP tool
  raises a "do not retry" error when off. `/speak/stop` is deliberately not gated.
- The `narration` role mounts exactly `/health` + `POST /narration/{warm,synthesize}`; the main role
  mounts 21 routers + `/mcp` + the SPA. `GET /events/speak` is consumed by the Rust `speak_monitor`,
  not by the frontend.

## Voice pairing (new, herdr)

`services/voice_targets.py` is plain functions (no `Transport` protocol class) shelling out to the
local `herdr` CLI — only `agent list` (10 s) and `agent prompt` (30 s); `agent read` is **not**
implemented. `routes/voice_targets.py` exposes `GET /agents` (parses `result.agents` → pane/agent/
status; `ready = status ∈ {idle, done}`) and `POST /voice-targets/message` → re-list agents, validate
the target is live, then `herdr agent prompt <pane> "<VOICE_TURN_INSTRUCTION> <text>"`. Failure codes:
409 (no target / target gone / agent blocked), 503 (herdr missing), 502 (other).
`VOICE_TURN_MARKER = "[voice turn]"`, `VOICE_TURN_INSTRUCTION = "[voice turn] reply aloud, two
sentences max."` The marker is **read by the four harness instruction files** (`~/.omp`, `~/.claude`,
`~/.config/opencode`, `~/.codex`), not by code — nothing forces the spoken reply if the agent ignores
it. Target is `capture_settings.agent_target`; the agent chord is `chord_agent_keys`; delivery is
*not* gated by the mute switch (input, not output). The paste path into the focused field is the
universal fallback and must keep working unchanged. See `Context/Plans/VoicePairing.md`; two wiring
gaps are live (see Open Questions).

## Relevant ADRs

No `Context/ADR/` and no `Context/Glossary.md` exist; decisions are recorded in-repo. Binding ones:

- No Alembic (rationale in `database/migrations.py`).
- GPU work serialized via one queue (`services/task_queue.py`).
- Narration must run in a **separate process** — two threads sharing one torch/MPS model abort the
  process; two processes are safe (`Context/Plans/AgentVoiceLimits.md` §1).
- HF offline forcing is deliberately *not* applied in inference paths (`utils/hf_offline_patch.py`).
- Ruff/`pyproject.toml` is authoritative; `CONTRIBUTING.md`'s Black/PEP 8 text is stale.
- `Context/Plans/{RealtimeAgentNarration,AgentVoiceLimits,VoicePairing,VoiceLoopLatency}.md` are the
  durable design records for the narration lane, its known limits, agent voice pairing, and the
  voice-loop latency baseline.

## Open Questions / unverified this session

- `generation_settings` (max_chunk_chars, crossfade_ms, normalize_audio, autoplay_on_generate) is
  persisted and served by `GET/PUT /settings/generation` but **not consulted** by `POST /generate`,
  which uses request-body defaults (`routes/generations.py`). Real divergence.
- `Project` ORM model (`projects` table) is declared and exported with zero routes/services — dead.
- `tada` maps to `hume_backend.py` (`HumeTadaBackend`). `TADA_MODEL_REPOS.get(model_size, TADA_1B_REPO)`
  means any unrecognized `model_size` string (e.g. `"1.7B"`) silently loads the 1B repo rather than
  erroring — a real, verified fallback path.
- Whether the `original` version is intentionally deletable while another version exists (only the
  last version is protected) — not re-verified.
- `docs/PROJECT_STATUS.md` (dated 2026-07-02, "402 open issues / 88 open PRs") is stale: its plan
  table references deleted files and it predates the narration lane; treat as a roadmap snapshot only.
- Web build works with no declared `@tauri-apps/*` deps in `web/package.json` only because bun
  workspace hoisting supplies them — fragile, unverified against a clean install.
- **Voice loop (2026-10-05 deep-dive):** no in-process turn tracing and no frontend marks (plan items
  1–2); `scripts/loop-bench` is the only instrument, so upload/decode and first-chunk→audible are
  unmeasured.
- **The mute/stop chords are half-wired.** Rust emits `voice:stop` / `voice:toggle`, but **no
  frontend listener exists**, so `apiClient.stopSpeaking()` (`POST /speak/stop`) is never called from
  the chord and the mute chord cannot flip `agent_voice_enabled`. Backend + Settings toggle work; the
  chord→endpoint link does not.
- **Agent delivery has no paste fallback.** `DictateWindow.onFinalText`'s agent branch shows a
  destructive toast and returns; it does not fall back to `paste_final_text` (plan item 8 wanted it).
- `create_capture` has **no model-cache pre-check**, so an uncached `stt_model` would download inline
  on a dictation — only the UI readiness gate (`GET /capture/readiness`) prevents it.
- MCP `voicebox.speak`'s description lacks the `[voice turn]` / two-sentence brevity convention (plan
  item 2); only the harness instruction files carry it.
- `Context/Plans/VoiceLoopLatency.md` labels its bench row "WebM/Opus — what the app sends"; the app
  sends WAV (WebM only on the `convertToWav` fallback or the import path). Measurement unaffected.

## Stale / dead code (verified 2026-10-04, extended 2026-10-05)

- `app/src/hooks/useAutoUpdater.tsx` (rich, toast UI) is **unreachable**: every caller imports the
  bare `@/hooks/useAutoUpdater` and Vite's default extension order resolves `.ts` first, so the
  toast-less `useAutoUpdater.ts` wins and App's `showToast:true` is silently ignored.
  *(Correction to the 2026-10-02 note, which had these backwards.)*
- Entire generated API tree unused (above).
- `app/src/components/AudioTab/` orphaned (superseded by `/captures`, still labelled prototype);
  `components/AudioStudio/` is an empty `.gitkeep`.
- `components/ServerSettings/` mostly superseded by `components/ServerTab/` — only
  `ModelManagement.tsx` is imported; `ConnectionForm`, `GpuAcceleration`, `ServerStatus`,
  `ModelProgress` are ~30 KB of near-duplicate dead code.
- `app/src/main.tsx` + `app/index.html` + `app/vite.config.ts` are a stale shell that renders
  `<App/>` without `PlatformProvider`; root scripts build only `tauri/` and `web/`.
- Root `requirements.txt` is a stale duplicate; `backend/requirements*.txt` are the real ones.
- `apiClient.stopSpeaking()` (`app/src/lib/api/client.ts`) is defined with no caller, and the Rust
  `voice:stop` / `voice:toggle` events have no frontend listener (see Open Questions).
