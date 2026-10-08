# Realtime Agent Narration

> Status: **implemented** (2026-10-02). All 12 work items landed; see "Implementation notes" at the
> end for the three deviations from this plan and how the whole thing was verified.

## Goal

Make agent speech in Voicebox usable as live pair-programmer commentary: narration audio starts
playing while the agent is still working, and the user cutting in with the dictation chord stops
it instantly.

## Constraints

- Agent narration must be audible **while a persistent generation is running**; today both share
  one serial queue (`services/task_queue.py`), so a speak request waits behind the whole job.
- **GPU work stays serialized on the single queue** (`services/task_queue.py`) — a route must never
  call `backend.generate()` directly against a GPU engine. Narration therefore runs on **CPU**
  instances of CPU-capable engines only.
- Narration must never write to `generations`, `generation_versions`, or the data directory: agent
  chatter must not pollute History or leave failed rows when interrupted.
- The queued `POST /speak` contract and the existing pill behaviour for non-narration speech must
  stay byte-identical (no shims, no behaviour change for callers that don't opt in).
- Only engines that can run on CPU qualify: `kokoro` (preset, 82M) and `luxtts` (cloned, English).
  All other engines fall back to the existing queued path.
- New narration code must not break `generate_chunked`'s existing behaviour — `run_generation` and
  `POST /generate/stream` depend on it exactly as it is.
- Audio starts with **no user gesture** in the pill window. Established: wry/Tauri enable autoplay by
  default, which sets WKWebView `mediaTypesRequiringUserActionForPlayback = None` and passes
  Chromium `--autoplay-policy=no-user-gesture-required`. No config change required — but do not add
  `with_autoplay(false)` or a Windows `additionalBrowserArgs` that drops that flag.
- Follow repo conventions: Ruff/py312 backend, generated API client left alone, hand-rolled
  idempotent migrations, `sse_starlette.EventSourceResponse` for SSE, `EventSource` (GET-only) as the
  only frontend streaming consumer.

## Work items

1. **Narration lane: CPU-pinned engine instances**
   - What: Add a device override and a second, narration-only backend registry.
     - `backend/backends/kokoro_backend.py::KokoroTTSBackend`: `__init__(self, device: str | None = None)`,
       store `self._device_override`; the `device` property returns
       `self._device_override or get_torch_device(allow_mps=False)`.
     - `backend/backends/luxtts_backend.py::LuxTTSBackend`: same pattern, default
       `get_torch_device(allow_mps=True, allow_xpu=True)`; its existing `_load_model_sync` CPU branch
       (`device="cpu", threads=min(threads, 8)`) is the load path for `"cpu"`.
     - `backend/backends/__init__.py`: add `NARRATION_ENGINES = frozenset({"kokoro", "luxtts"})`,
       `_narration_backends: dict[str, TTSBackend] = {}` + `_narration_backends_lock = threading.Lock()`,
       and `get_narration_backend_for_engine(engine) -> TTSBackend` (double-checked locking, constructs
       `KokoroTTSBackend(device="cpu")` / `LuxTTSBackend(device="cpu")`, else `ValueError`).
       Add `async def load_narration_engine_model(engine) -> None` mirroring `load_engine_model`
       (`:528`) — same `is_model_cached` check and `model_load_progress` wrapping, calling
       `backend.load_model()` (no size arg; kokoro/luxtts take none).
       Add `unload_narration_backends() -> None` (unload each, clear the dict).
     - `backend/app.py::_run_shutdown`: call `unload_narration_backends()` next to the existing three
       unloads, in its own `try/except` so one failure can't block the others.
     - Leave `_tts_backends`, `get_tts_backend_for_engine`, and `load_engine_model` untouched.
   - Why: the registry is keyed by engine name only (`_tts_backends[engine]`), so a CPU-pinned Kokoro
     cannot coexist with the GPU one, and no device override mechanism exists anywhere.
   - Depends on: none.
   - Risk: two Kokoro instances double RAM/VRAM; forgetting the shutdown hook leaves the model
     resident for the process lifetime. Detect: assert `get_narration_backend_for_engine("kokoro") is
     not get_tts_backend_for_engine("kokoro")`, and that the dict is empty after
     `unload_narration_backends()`.
   - Source: inferred from codebase (`backends/__init__.py:202-207,670-720,528-535`;
     `kokoro_backend.py:127-144`; `luxtts_backend.py:34-40,74-87`; `services/tts.py:23-25`).

2. **`agent_narration` setting (backend)**
   - What: `GenerationSettings.agent_narration = Column(Boolean, nullable=False, default=True)` in
     `backend/database/models.py`; add `_migrate_generation_settings(engine, inspector, tables)` to
     `backend/database/migrations.py` modelled on `_migrate_capture_settings` (guard
     `if "generation_settings" not in tables: return`, then
     `if "agent_narration" not in columns: _add_column(engine, "generation_settings", "agent_narration BOOLEAN NOT NULL DEFAULT 1", "agent_narration")`)
     and register it in `run_migrations()` after `_migrate_capture_settings` (there is no
     generation_settings helper today).
     `backend/models.py`: add `agent_narration: bool = True` to `GenerationSettingsResponse` and
     `agent_narration: Optional[bool] = None` to `GenerationSettingsUpdate`.
     `GET/PUT /settings/generation` and `services/settings.py::_apply_patch` need no change.
   - Why: kill switch for the whole feature; `_apply_patch` is already generic and drops `None` for
     non-nullable columns.
   - Depends on: none.
   - Risk: an existing `generation_settings` row must gain the column with `DEFAULT 1` or the app
     reads NULL into a non-nullable ORM column. Detect: `GET /settings/generation` on a database
     created before this change returns `agent_narration: true`.
   - Source: inferred from codebase (`migrations.py` `_migrate_capture_settings` + `_add_column`;
     `models.py` settings models; `services/settings.py::_apply_patch`; routes `PUT` uses
     `model_dump(exclude_unset=True)`).

3. **Narration lane resolution (hybrid rule)**
   - What: New `backend/services/narration.py` with
     `resolve_narration_lane(db, profile, requested_engine: str | None, language: str) -> tuple[str, str] | None`
     returning `(engine, reason)` or `None`:
     - `None` when `agent_narration` is off (`services.settings.get_generation_settings(db)`);
     - `None` when `requested_engine` is set and not in `NARRATION_ENGINES` (never silently override
       an explicit engine choice);
     - `voice_type == "preset"` → `(profile.preset_engine, "preset")` if it is in `NARRATION_ENGINES`,
       else `None` (Qwen CustomVoice presets need the GPU);
     - `voice_type == "cloned"` → `("luxtts", "cloned")` when `language == "en"`, else `None`
       (LuxTTS is English-only);
     - `"designed"` → `None`.
     Defensively run `profiles.validate_profile_engine(profile, engine)` before returning.
   - Why: the two CPU-capable engines map onto the two profile kinds that can use them — a Kokoro
     preset profile keeps its exact preset voice, an English cloned profile reuses its samples under
     LuxTTS. Anything else degrades to the queued path rather than silently changing the voice.
   - Depends on: item 1 (`NARRATION_ENGINES`), item 2 (the setting).
   - Risk: a cloned profile that has no samples or an invalid sample path will fail at
     `create_voice_prompt_for_profile` mid-stream. Detect: `backend/tests/test_narration_lane.py`
     covers the truth table; the stream endpoint surfaces the error as an `error` event.
   - Source: inferred from codebase (`services/profiles.py:516-559` + `CLONING_ENGINES`;
     `models.VoiceProfile*` `voice_type` semantics; README engine table for CPU capability).

4. **Chunk streaming synthesis primitive**
   - What: In `backend/utils/chunked_tts.py`:
     - Extract the inner `generate_one` closure (`:221-260`) to a module-level
       `async def _generate_one(backend, chunk_text, chunk_seed, *, voice_prompt, language, instruct, trim_fn, runaway_detector, crossfade_ms, retry_depth=0)`
       and have `generate_chunked` call it. Pure refactor — no behaviour change; the runaway-retry
       tree, trim ordering (runaway check on raw output, trim after), and per-chunk seed
       (`seed + i`) must be preserved exactly.
     - Add `def apply_edge_fades(audio: np.ndarray, sample_rate: int, fade_ms: int) -> np.ndarray`:
       `n = min(int(sample_rate * fade_ms / 1000), len(audio) // 2)`; `audio[:n] *= np.linspace(0, 1, n)`,
       `audio[-n:] *= np.linspace(1, 0, n)` — the same linear windows `concatenate_audio_chunks` uses.
     - Add `async def generate_chunked_stream(...) -> AsyncIterator[tuple[np.ndarray, int, str]]`
       (same parameters as `generate_chunked`), yielding `(apply_edge_fades(chunk, sr, crossfade_ms), sr, chunk_text)`
       per chunk; the single-chunk fast path yields the original `text`'s result once, faded.
       The yielded text is the chunk as produced by `split_text_into_chunks` — even when a runaway
       retry internally synthesized several sub-chunks, they are concatenated into that one yield.
       Docstring must state: chunks are **faded and butt-joined, not overlapped**, so streamed audio
       is not sample-identical to `generate_chunked`; streaming output is for live playback only and
       is never persisted.
   - Why: seq synthesis is the only way audio can reach the user before the utterance finishes, and
     the routing/trim/runaway logic must stay single-sourced.
   - Depends on: none.
   - Risk: refactoring `_generate_one` out of `generate_chunked` silently changes trim/retry
     behaviour for every existing generation. Detect: run the same text+seed through the old and new
     `generate_chunked` on one engine and compare durations/sample counts; existing tests
     (`test_refinement_*`, `test_qwen_runaway_retry`, `test_all_models_e2e`) must pass.
   - Source: inferred from codebase (`utils/chunked_tts.py:63-260`, quoted crossfade math at
     `:174-203`).

5. **Narration sessions + `narrate` opt-in (`POST /speak/narrate`, `voicebox.speak`)**
   - What: In `backend/services/narration.py` add the session registry:
     `@dataclass NarrationSession(id, text, profile_id, profile_name, engine, language, created_at, claimed_at)`,
     module state `_sessions: dict[str, NarrationSession]`, `_MAX_SESSIONS = 16`, `_TTL_SECONDS = 120`,
     `create_session(...)`, `get_session(id)`, `claim_session(id) -> NarrationSession | None`
     (returns `None` if already claimed), `discard_session(id)`, `_sweep(now)`, and
     `engine_stream_lock(engine) -> asyncio.Lock` (one lock per engine instance — two windows must not
     call one torch model concurrently).
     No change to `SpeakRequest` — the narration opt-in is the route, not a body field.
     `backend/routes/speak.py`:
     - extract the shared prologue (client id → `resolve_profile` → binding → personality flag →
       engine default) into `_resolve_speak_context(data, request, db)`, used by both routes;
     - keep `POST /speak` exactly as it is today (same `response_model=GenerationResponse`, same
       `speak-start` payload with no `narration` key);
     - add `POST /speak/narrate` (same resolution/bindings) which, when
       `resolve_narration_lane(...)` returns an engine: resolves the personality flag the same way,
       and when enabled + `profile.personality` calls `services/personality.rewrite_as_profile`
       **before** storing the text (keeps the LLM off the streaming path), then
       `create_session(...)`, publishes
       `speak-start {"generation_id": session.id, "narration": true, "profile_name": ..., "source": "rest", "client_id": ...}`,
       and returns `{narration_id, mode: "narration", profile, engine, stream_url: "/speak/{id}/stream"}`.
       If the lane is `None`, it delegates to the existing queued path and returns the
       `GenerationResponse` (graceful degradation — narration-chatter falls back to normal speak).
     `backend/mcp_server/tools.py`: `voicebox_speak` gains `narrate: bool = False`; when true it
     takes the narration branch and returns
     `{"narration_id", "mode": "narration", "profile", "source": "mcp", "stream_url", "poll_url": None}`
     and publishes `speak-start` with `client_id=current_client_id.get()`; `_speak`/`_speak_response`
     remain the non-narration path. Update the tool description to state that narration is streamed,
     ephemeral, and not saved to History.
   - Why: narration sessions are the identity the pill subscribes to; without a `generations` row
     there is nothing to poll, and the ephemerality is the point.
   - Depends on: items 1-4.
   - Risk: sessions leak if a client never connects, and a second `GET` on a claimed session would
     double-synthesize. Detect: TTL/cap sweep bounded by `_MAX_SESSIONS`; `claim_session` returning
     `None` → `409`; a unit test asserts eviction.
   - Source: inferred from codebase (`routes/speak.py:26-93`; `mcp_server/tools.py:46-60,231-286`;
     `mcp_server/resolve.py`; `services/personality.py` via the existing `personality=true` path).

6. **`GET /speak/{narration_id}/stream` (SSE)**
   - What: New route in `backend/routes/speak.py`, `sse_starlette.EventSourceResponse` (same import as
     `routes/events.py:12`). `session = narration.get_session(id)` → `404` if missing;
     `narration.claim_session(id)` → `409` if already claimed. The generator:
     1. `event: ready` → `{"narration_id", "profile_name", "engine", "text"}`;
     2. `event: loading` → `{}` (emitted *before* the model load so the pill can show progress-less
        "speaking" while a first-use download runs);
     3. `async with narration.engine_stream_lock(session.engine):` → `await load_narration_engine_model(session.engine)`,
        then `voice_prompt = await profiles.create_voice_prompt_for_profile(session.profile_id, db, engine=session.engine)`
        and `async for index, (audio, sr, chunk_text) in enumerate(generate_chunked_stream(backend, ...)):`
        → `event: chunk` → `{"index", "sample_rate", "text": chunk_text, "wav": base64(wav_bytes)}`
        via `services.tts.audio_to_wav_bytes`;
     4. `event: done` → `{"chunks": n}`;
     5. on `Exception` → `logger.exception` + `event: error` → `{"message"}`;
     6. `finally:` `narration.discard_session(id)` and publish `speak-end`
        `{"generation_id": id, "status": "completed"|"failed"|"cancelled"}`.
     Own the DB session inside the generator (`next(get_db())`, closed in `finally`) — mirror
     `run_generation`, which does not borrow the request session. Never call `enqueue_generation`.
     `trim_fn`/`runaway_detector` come from `engine_needs_trim`/`engine_retries_runaway` for the
     narration engine, exactly as `/generate/stream` does.
   - Why: the pill needs per-chunk audio over a GET (EventSource cannot POST) and the stream's
     lifetime is the narration's lifetime — closing it is the cancel path.
   - Depends on: items 1, 4, 5.
   - Risk: SSE writes are the only sync point, so a slow client back-pressures synthesis — acceptable
     (that is the intent: no listener, no work). Cancelling mid-chunk cannot abort a torch call
     already inside `asyncio.to_thread`, so cancel latency is one chunk (one sentence). Detect: close
     the EventSource during a multi-sentence narration and confirm the server logs a single
     `speak-end` with `status: "cancelled"` and no orphan session.
   - Source: inferred from codebase (`routes/events.py:23-46`; `routes/generations.py` stream route;
     `utils/progress.py`/`utils/tasks.py` not needed here).

7. **Frontend: WebAudio narration player**
   - What: New `app/src/lib/utils/narrationPlayer.ts` (plain module, no React):
     `stopNarration()` (bump a module-level `epoch`, stop+disconnect every scheduled
     `AudioBufferSourceNode`, clear `nextStartAt`), `scheduleNarrationChunk(epoch, wavBase64, { onStart })`
     (lazily create one `AudioContext`, `resume()` if suspended, base64 → `ArrayBuffer` →
     `decodeAudioData`, discard if `epoch` is stale, `startAt = max(ctx.currentTime + 0.15, nextStartAt)`,
     `source.start(startAt)`, `nextStartAt = startAt + buffer.duration`, fire `onStart` for the first
     scheduled source), `currentNarrationEpoch()`.
     New `app/src/lib/hooks/useNarrationStream.ts` exposing
     `{ start(narrationId, { onStart, onActivity }), stop() }`: closes any previous `EventSource`,
     captures the current epoch, opens `apiClient.getNarrationStreamUrl(id)`, and uses **named-event
     listeners** (`addEventListener('chunk' | 'done' | 'error')`) — ignore `ready`/`loading` except to
     call `onActivity`. Close the source on `done`, `error`, and `onerror`: this endpoint is not
     resumable, so EventSource's automatic reconnect would re-synthesize the whole narration.
   - Why: sentence chunks must butt-join without an audible seam and with a lead that survives
     jitter; `decodeAudioData` per chunk handles the engine's varying sample rates (24 kHz/48 kHz).
   - Depends on: item 6 (URL shape).
   - Risk: an underrun (chunk arrives after its scheduled slot) becomes a gap rather than drift;
     CPU synthesis of a sentence is normally shorter than its playback, so this should not bite
     unless the CPU is saturated. Detect: narrate a long multi-sentence line while a GPU generation
     and a dictation transcription run, and listen for gaps.
   - Source: inferred from codebase (`lib/api/client.ts` URL-builder idiom; `useGenerationProgress.ts`
     EventSource pattern; `DictateWindow.tsx` speaks via `HTMLAudioElement` today) + researched
     finding that wry/Tauri autoplay defaults remove the AudioContext gesture requirement on macOS
     and Windows, so no unlock fallback is needed.

8. **DictateWindow: narration branch**
   - What: In `app/src/components/DictateWindow/DictateWindow.tsx`, parse `narration` from the
     `dictate:speak-start` payload. When truthy: `dismissSpeak()`;
     `setSpeaking({ generationId: id, startedAt: null })`; `setSpeakElapsed(0)`; call
     `narrationStart(id, { onStart, onActivity })` where `onStart` mirrors the current
     `audio.onplaying` block (`emit('dictate:show')`, set `startedAt = Date.now()`, reset elapsed) and
     `onActivity` resets the existing 60 s `statusTimeoutRef` watchdog (turning it into an inactivity
     watchdog). Do **not** open the `/generate/{id}/status` `EventSource` for narration.
     `dismissSpeak()` must additionally call `narrationStop()`. The non-narration path
     (`HTMLAudioElement` + status EventSource) stays exactly as it is.
   - Why: the pill is already the single playback surface and the only place the "always visible,
     never silent" contract can be honoured; branching on the payload keeps one pill state machine.
   - Depends on: item 7.
   - Risk: `dismissSpeak` runs in effect cleanup as well, so `narrationStop()` must be idempotent and
     safe when nothing is playing. Detect: mount/unmount the pill window repeatedly and confirm no
     stray `AudioBufferSourceNode` keeps playing.
   - Source: inferred from codebase (`DictateWindow.tsx:117-155` teardown/playback, `:166-230`
     speak-start/end handlers, `:268` effective state; `CapturePill` is purely presentational).

9. **Barge-in**
   - What: In the `dictate:start` listener call `dismissSpeak()` **before**
     `sessionRef.current.startRecording()`, and emit a new Tauri event `speak:interrupt`. In
     `app/src/App.tsx` add a listener alongside the existing Tauri subscriptions:
     `usePlayerStore.getState().setIsPlaying(false)` (`AudioPlayer.tsx:326` already pauses WaveSurfer
     when `isPlaying` goes false). No resume after interruption — barge-in is terminal for that
     utterance, consistent with "last speak wins".
   - Why: today nothing in the capture path touches playback, and the pill's audio and the main
     window's player are separate webviews, so the main window needs an explicit signal.
   - Depends on: item 8.
   - Risk: mic bleed — `useAudioRecording.ts` already requests
     `echoCancellation/noiseSuppression/autoGainControl`, but echo cancellation is not tuned for a
     loud TTS source through the same device, so the first ~200 ms of a dictation may still contain
     narration. Detect: play a narration, press the chord mid-sentence, and read the resulting
     capture's `transcript_raw` — it must not contain the narration's words. If it does, that is a
     follow-up (duck/gate the capture's first frames), not a silent pass.
   - Source: inferred from codebase (`DictateWindow.tsx:88-92`; `useAudioRecording.ts:86-92`;
     `stores/playerStore.ts` — `setIsPlaying`, no dedicated stop; `AudioPlayer.tsx:320-329`).

10. **Settings UI + i18n**
    - What: `app/src/lib/api/types.ts` → `GenerationSettings` gains `agent_narration: boolean;`
      (`GenerationSettingsUpdate` is `Partial<>`, so no other change). `app/src/components/ServerTab/GenerationPage.tsx`
      → new `SettingRow` after the `autoplayOnGenerate` row: `htmlFor="agentNarration"`,
      `checked={settings?.agent_narration ?? true}`,
      `onCheckedChange={(v) => update({ agent_narration: v })}`, `Toggle` from `@/components/ui/toggle`,
      labels via `t('settings.generation.agentNarration.title' | '.description')`.
      Add those two keys to `app/src/i18n/locales/en/translation.json` (other locales fall back; the
      9-locale gap is tracked upstream).
    - Why: the feature must be switchable off without an env var, and `useGenerationSettings` already
      provides an optimistic `update({ field })` mutation.
    - Depends on: item 2.
    - Risk: the toggle reads `true` while the backend row is still `NULL` on an unmigrated DB —
      item 2's migration must land first. Detect: toggle off, `GET /settings/generation` returns
      `agent_narration: false`, and the next `voicebox.speak(narrate=true)` takes the queued path.
    - Source: inferred from codebase (`GenerationPage.tsx` `SettingRow`/`Toggle` usage;
      `useSettings.ts` full optimistic pattern; `lib/api/client.ts` settings methods;
      `types.ts:244-251`).

11. **Backend tests**
    - What: `backend/tests/test_narration_lane.py` — the `resolve_narration_lane` truth table
      (preset-kokoro → `("kokoro", "preset")`, cloned+`en` → `("luxtts", "cloned")`, cloned+`fr` → `None`,
      explicit `engine="qwen"` → `None`, `agent_narration=False` → `None`); narration instance identity
      and `device == "cpu"` (`pytest.importorskip("torch")`); `unload_narration_backends()` clears the
      registry. `backend/tests/test_narration_stream.py` — `generate_chunked_stream` against a stub
      backend (object with `async generate(text, voice_prompt, language, seed, instruct) -> (np.ndarray, sr)`):
      chunks arrive in order with heads/tails faded to 0 (assert `chunk[0] == approx(0.0)` and the last
      sample `== approx(0.0)` with `fade_ms=50`), chunk texts equal `split_text_into_chunks(text)`, and
      the total sample count equals the non-streaming path minus the crossfade overlaps (assert the
      documented relationship, not equality); session registry create/get/claim/discard, TTL sweep, and
      cap eviction. Both files: `@pytest.mark.asyncio` (`asyncio_mode=auto` is configured).
    - Why: `_generate_one` extraction and the fade/crossfade asymmetry are the two silent-breakage
      risks, and the lane's availability rule is the whole feature's gate.
    - Depends on: items 3, 4, 5.
    - Risk: tests that instantiate real engines pull torch/network. Detect: run
      `pytest backend/tests/test_narration_lane.py backend/tests/test_narration_stream.py -v` — must
      pass with no network; note CI has no backend gate, so this is a local `just test` obligation.
    - Source: inferred from codebase (`backend/pyproject.toml` testpaths/`asyncio_mode=auto`;
      `tests/README.md` conventions; `utils/chunked_tts.py` call contract).

12. **Docs + CHANGELOG**
    - What: `CHANGELOG.md` `[Unreleased]` entry for agent narration (streamed, CPU lane, barge-in,
      opt-in `narrate`). `docs/content/docs/overview/mcp-server.mdx` — document `narrate: true` on
      `voicebox.speak`, the ephemeral/no-History semantics, the streaming pill contract, and the lane
      availability rule (Kokoro preset or English cloned voice; otherwise the call degrades to the
      normal queued speak). `docs/content/docs/developer/tts-generation.mdx` — a "Narration lane"
      section: CPU-pinned instances, the `GET /speak/{id}/stream` event schema (`ready`/`loading`/`chunk`/
      `done`/`error`), cancellation semantics, and why narration never touches the serial queue.
      `backend/README.md` — add the two new routes to its route list if it enumerates routes.
    - Why: the lane changes what a `speak` call means, and the MCP tool is the user-facing contract.
    - Depends on: items 5, 6.
    - Risk: docs claiming narration is saved (the current tool description says audio "is saved to the
      Captures / History tab") become actively wrong. Detect: grep the docs for the old claim.
    - Source: inferred from codebase (`mcp_server/tools.py` tool description; `docs/content/docs/overview/`
      and `developer/` tree; CHANGELOG Keep-a-Changelog shape).

## Verification

End-to-end smoke (no test substitutes for this):

1. `uvicorn backend.main:app --port 17493` (or `bun run dev:server`).
2. Start a long generation on a GPU engine (a few thousand characters) and confirm it is still
   `generating`.
3. `curl -N http://127.0.0.1:17493/speak/{narration_id}/stream` for a session created via
   `POST /speak/narrate` — first `chunk` event must arrive **before** the generation completes, and
   `ps`/GPU memory must show the narration model loaded alongside the work model.
4. In the app: `voicebox.speak(text, narrate=true)` from an agent → pill appears and audio starts
   without waiting for the queue; `GET /history` gains no row.
5. Press the dictation chord mid-narration → audio stops immediately, `speak-end {status: "cancelled"}`
   appears on `/events/speak`, no orphan session, and the resulting capture's `transcript_raw`
   contains none of the narration words.
6. Toggle `agent_narration` off → the same call takes the queued path and lands in History.

## Implementation notes (deviations from the plan above)

0. **The CPU narration lane was abandoned for the shared-engine, queue-gated design (found
   2026-10-02, real engine).** LuxTTS's CPU path is a different implementation (ONNX
   `generate_cpu` vs the torch path) and it systematically under-generates: the same reference
   and text produced **0.11 s on CPU vs 1.24 s on MPS** for a 32-char sentence, 1.75 s vs
   2.86 s for 73 chars, and the shortest inputs crashed its own vocoder (`Kernel size can't be
   greater than actual input size`). Kokoro (also CPU) is unaffected but preset-only, so it
   cannot narrate a cloned voice. Narration now resolves the profile's engine exactly like
   `POST /generate`, runs it on the **same backend instance**, and waits for the serial queue
   to drain (`task_queue.wait_until_idle`) before synthesizing, emitting a `waiting` event per
   second. The CPU-lane machinery (narration backend registry, `device=` overrides,
   `NARRATION_ENGINES`, the `backend=` parameter on `create_voice_prompt_for_profile`, the
   device-scoped LuxTTS prompt cache) was removed with it.
   One fix from that investigation was kept, because it repairs a real crash that predates this
   feature: **`model_load_progress` now holds the tqdm patch exclusively**, so two overlapping
   model loads can no longer restore each other's `tqdm.tqdm` class (observed as
   `type object 'tqdm' has no attribute '_lock'`).

1. **Sentence-level chunking was added (item 4/6).** As planned, `generate_chunked_stream`
   reused `split_text_into_chunks`, which *packs* whole sentences into `max_chunk_chars`
   windows — so a three-sentence narration (the normal case) became a single chunk and
   streamed nothing. Added `split_text_into_sentence_chunks()` (one sentence per chunk,
   over-long sentences capped by the existing splitter) and a `split_sentences=True` flag
   on the generator; the narration route passes it. `generate_chunked` is unaffected.
2. **The endpoint has real integration tests (item 11).** The plan said not to test the SSE
   route because it would need a model. It does not: `test_narration_stream_endpoint.py`
   runs the real app under **uvicorn on a loopback socket** with the engine stubbed, and
   covers the event schema, WAV decoding, `409` on a second listener, `404` on an unknown
   session, and cancellation. `ASGITransport` was tried first and abandoned — it completes
   the app before returning, so it can neither observe incremental delivery nor a
   disconnect.
3. **`onNarrationDrained` waits for pending decodes, not just scheduled sources (item 7).**
   Found by the browser smoke test: the stream finishes while chunks are still decoding, so
   draining on `sources.length === 0` fired immediately and would have cut narration off
   the moment it finished synthesizing. Player state is now per-narration
   (`pendingDecodes` + `sources` + callbacks), and `beginNarration` discards the previous
   state so stale decodes cannot decrement the live counters.

## Verification performed

- **Real engine, real clone (2026-10-02).** With LuxTTS installed in `backend/venv` and a
  profile cloned from a real recording: the queued path produced 2.47 s of audio and persisted
  it; the narration lane ran **queue-gated** — 10 `waiting` events (~10 s) while a long
  generation held the queue, first `chunk` with **1.24 s of audio** (the correct MPS-path
  duration) once the generation reported `completed`, 2 chunks / 2.86 s total. `waiting` and
  `chunk` payloads, `speak-end`, and the no-history guarantee all confirmed on the wire.
- **Narration loudness fixed (2026-10-02).** The stream never normalized, so narration came out
  **-40.3 dBFS** next to the queued path's -20 dBFS — effectively inaudible against agent
  results. Chunks are now normalized per chunk (`normalize_audio`) and measured at **-20.0 dBFS**
  with peaks -1.4 to -4.6 dBFS on a real cloned voice. Pinned by a loudness assertion in
  `test_narration_stream_endpoint.py`.
- `pytest backend/tests` — **186 passed, 4 skipped**; 5 failures all pre-existing and
  environment-only: 4 × `test_mlx_smoke.py` (MLX not installed in this venv) and
  `test_progress.py::test_hf_progress_tracker` (a manual debug script that imports
  `utils.progress` and needs a live HF download).
- New: `test_narration_lane.py` (20), `test_narration_stream.py` (14),
  `test_narration_stream_endpoint.py` (6) — all passing.
- The `generate_chunked` refactor is pinned by an independent reimplementation of the
  pre-refactor algorithm (`test_generate_chunked_matches_pre_refactor_reference`).
- `bun run typecheck` clean; `biome check` clean on every new file.
- Browser smoke (real Chromium against the running backend, engine stubbed): 3-sentence
  narration produced 3 streamed chunks and **1678 ms of actual playback for 1650 ms of
  scheduled audio**, with `AudioContext` `running`; barge-in dropped the in-flight chunk
  and drained in 0 ms.

## Open decision (resolved: option B)

`resolve_narration_lane` originally offered the lane only for CPU-capable engines. On Apple
Silicon that left Kokoro (preset only) as the sole working option, because LuxTTS's CPU path
under-generates. **Resolved 2026-10-02: option B — the clone, queue-gated.** Narration now
resolves the profile's engine exactly like `POST /generate` and runs it on the same backend
instance, waiting for the serial queue to drain (`task_queue.wait_until_idle`) before it
starts and emitting a `waiting` event per second so the pill stays alive. The CPU-lane
machinery (narration backend registry, `device=` overrides, `NARRATION_ENGINES`,
`create_voice_prompt_for_profile(backend=...)`, the device-scoped LuxTTS prompt cache) was
removed with it; the exclusive tqdm progress patch was kept, because it fixes a real
concurrent-model-load crash that predates this feature.

| Option | Narration voice | Starts while working? |
|---|---|---|
| A. Kokoro preset on CPU | a preset, not your clone | yes (not chosen) |
| **B. Your clone, queue-gated** | **your clone** | **when the queue is idle; waits during a long generation** |
| C. Separate narration process | your clone | blocked by the same LuxTTS CPU limitation |

## Out of scope (harness-side, cannot be built here)

The user's larger goal — asking a mid-task question and having the agent answer from its in-flight
state — needs an inbound channel into the agent harness. Voicebox exposes only client→server tool
calls plus the one-way `/events/speak` push, and dictation delivers text by clipboard paste into the
focused field. A harness that supports mid-flight messages could wire dictation → its own message API
→ agent answer → `voicebox.speak(narrate=true)`; that integration is not specified here because it is
harness-specific and unverified for any named agent.
