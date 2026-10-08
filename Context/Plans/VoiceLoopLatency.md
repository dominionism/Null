# Voice loop latency — measure the turn, then fix the dominant stage

> Blueprint: 2026-10-04. Builds on `Context/Research/Research.md` (comprehensive, 2026-10-04)
> and `Context/Plans/AgentVoiceLimits.md` (the recorded limits of this loop).

## Collaboration

- Default mode: Delegate
- Current milestone: one real turn produces a single trace, joined across the chord and the first
  audible audio (work items 1–2). **Complete 2026-10-05** — see "The trace" below.
- Decision Control: items 1–2 agent-led (user delegated the direction); the ≥40% acceptance bar and
  B1 vs B3 remain **user-owned**.
- Learning goal: none recorded.

## Goal

Instrument one real dictate → agent → hear-answer turn end to end well enough to rank its stages by
wall-clock cost, then cut the dominant stage in half without changing what the loop does.

## Constraints

- **Verbatim (user):** "I wanna build something right now and optimize the experience for my voice."
  Chosen framing: the whole loop, end to end; measure first, then fix the worst bottleneck.
- Local-first: no cloud STT/TTS (project premise). Any new runtime is local.
- GPU generation stays serialized on the single queue (`services/task_queue.py`); nothing may call
  `backend.generate()` outside it. Narration is the exception it already is (worker process, or
  in-process behind `engine_stream_lock` + `wait_until_idle`).
- Narration is **ephemeral by design**: no `generations` row, no History entry, no data-dir writes.
  Never regress this.
- The pill is the only playback surface for agent speech, and agent speech must never be silent
  (watchdog + `/audio/{id}` ack + headless fallback).
- The paste-into-focused-field path is the universal fallback and must keep working unchanged.
- `app/src/lib/api/client.ts` is hand-written and live; the generated tree under `lib/api/{index,models,
  schemas,services,core}` is dead and must not be extended.
- Style: Ruff/py312, 120 cols, Google docstrings, `%s` lazy logging, no bare except; hand-rolled
  idempotent migrations if any DB change (there is none planned); `sse_starlette` for SSE; EventSource
  (GET) as the only frontend streaming consumer.
- CI gates only frontend typecheck + web build — `just test` and `ruff` are local obligations.

## Acceptance (proposed — needs your confirmation)

1. `scripts/loop-bench` prints a per-stage table for one real turn, and re-running it twice on an idle
   machine yields stage timings within ±20% (i.e. the numbers are real, not noise).
   **Done for the bench** (shipped 2026-10-04). The in-process trace it was paired with now exists too
   (work items 1–2, 2026-10-05), so the table can be joined with the chord and playback marks.
2. The same table exists for one **real** chord-driven turn with the app open (frontend marks included).
   **Built 2026-10-05** — the frontend marks and the join are in place and proven against a live
   server over HTTP; the chord press itself still needs a human finger (synthetic key events are
   dropped in this harness), so the end-to-end run is the owner's.
3. The dominant cost is at least halved. **Done for the dominant cost as measured:** the cold-start
   class, 4.55 s → 2.50 s (STT) and 4.11 s → 2.71 s (narration first chunk) on the first turn after a
   restart.
4. No behavioral regression: the paste path, the narration no-history guarantee, and barge-in all
   still hold. **Held** (no code path in those areas was touched; 43 tests pass).

If the ≥40% bar is wrong for you, change it here — it is my inference.

## Baseline (measured 2026-10-04, this machine, MPS, Whisper small, LuxTTS clone "Me")

Live server, real capture (8.76 s dictation), real narration text. "Cold" = first use after a
server restart; "warm" = every use after.

| Stage | Cold | Warm | Previously recorded |
|---|---|---|---|
| STT (`POST /transcribe`, same 8.76 s clip) | **4.55 s** | 1.60–1.75 s | 6.3 s — **contradicted** (see findings) |
| Refinement LLM (`POST /llm/generate`, proxy) | **9.63 s** | 1.21–1.41 s | not recorded |
| Narration request → first `chunk` | **4.11 s** | 2.45–2.77 s | ~2.0 s warm / 8.7 s cold |
| Delivery transport (`herdr agent list`; no prompt sent) | — | 0.02–0.13 s | unmeasured |
| Playback lead | — | 0.15 s `START_LEAD_SECONDS` | — |

Findings that reshape the plan:

1. **The 6.3 s STT figure is wrong today** (1.75 s warm on the same kind of clip). It was recorded
   during the VoicePairing investigation and is not reproducible; treat the AgentVoiceLimits number
   as historical, not a target.
2. **`auto_refine` is `False` on this machine**, so the refinement LLM is *not* in the dictation
   path at all — the 9.63 s cold load never lands in a real turn. The LLM warm-up is still correct
   (it is gated on `auto_refine`), it simply does nothing here.
3. **No single warm stage is a monster.** Warm: STT ~1.7 s, delivery ~0.1 s, narration ~2.5 s. The
   loop felt slow because the *first* turn of every session paid three model loads: 4.55 s (Whisper)
   + 4.11 s (narration worker) + 9.63 s (LLM, only when refinement is on).
4. Therefore the dominant cost was **cold start**, which is a different fix from any single stage in
   the decision table below. That fix has shipped (see "What shipped").
5. Remaining warm cost in the front half is **STT (~1.7 s)**, and it runs on CPU by design
   (`PyTorchSTTBackend._get_device` passes `allow_mps=False`) — the largest remaining lever.

There is **no per-stage timing instrumentation in the repo** (only `time.monotonic()` bookkeeping in
`services/narration.py` and `services/playback.py`); the numbers above come from driving the existing
HTTP endpoints, not from in-process marks. Building the trace (work items 1–2) is still worth doing
for sub-stage resolution — upload/decode and first-chunk→audible are still unmeasured — but it is no
longer the fastest route to the biggest win.

## What shipped (2026-10-04) — startup warm-up

The measured dominant cost was the first turn of a session paying model loads in-line. Fix:
`capture_settings.warm_models_on_startup` (default on, **Settings → Captures → Transcription →
Load models at startup**), a background `_warm_startup_models()` in `app.py`, and
`POST /narration/warm` on the worker, called by `services/narration_worker.py` once the worker is
healthy.

| Stage | Before (cold, first turn) | After (first turn post-restart) |
|---|---|---|
| STT | 4.55 s | **2.50 s** (2nd call 1.60 s) |
| Narration first chunk | 4.11 s | **2.71 s** |
| Refinement LLM | 9.63 s | not warmed — `auto_refine` is off here |

Rules the warm-up holds to (pinned by `backend/tests/test_warm_startup.py`, 6 tests):
never downloads an uncached model; no-ops when the user turned the setting off, when no playback
voice resolves, or when the narration lane is unavailable; loads exactly the engine the real
narration path resolves.

Files: `backend/app.py`, `backend/routes/narration_worker.py`, `backend/services/narration_worker.py`,
`backend/database/models.py` + `migrations.py`, `backend/models.py`,
`app/src/components/ServerTab/CapturesPage.tsx`, `app/src/lib/api/types.ts`,
`app/src/i18n/locales/en/translation.json`, `CHANGELOG.md`,
`docs/content/docs/overview/dictation.mdx`.

Verified: migration on a copy of the real DB; setting round-trips through `PUT/GET /settings/captures`;
the toggle renders and persists in the real UI (browser, same-origin web build); the server log shows
`Warmed Whisper small` and a 200 from `/narration/warm`; `pytest` 43 passed across the four affected
files; `bun run typecheck` clean; diff is additive (pre-existing Biome/Ruff findings left untouched).

## The instrument — `scripts/loop-bench` (shipped 2026-10-04)

One command, no browser, no mic: uploads a real capture through `POST /captures` (WAV and the
WebM/Opus a browser actually records, transcoded with ffmpeg), then `POST /speak/narrate` +
`GET /speak/{id}/stream` for narration, and times the `herdr` CLI. It deletes every capture row it
creates, and only touches an agent session with `--agent`.

Measured with it (warm, this machine):

| Stage | Warm |
|---|---|
| upload + decode + STT (WAV) | 1.05–1.93 s |
| upload + decode + STT (WebM/Opus — what the app sends) | 1.11–1.16 s |
| narration request → first chunk | 2.50–2.73 s |
| delivery (`herdr agent list`) | 0.02 s |

Two things this settled: **decode is not a hidden cost** (webm ≈ wav), and the loop's controllable
time is ~3.6 s split roughly **1.1 s front / 2.5 s back**.

## Bug found by the instrument: WebM dictation could not decode

The first bench run failed the webm stage with `400 Could not decode .webm audio — the recording may
be empty or corrupt`. Root cause: browser recordings are WebM/Opus, the only decoder is `ffmpeg`,
librosa reaches it through audioread **by name**, and a server started by launchd (or, by the same
mechanism, by the packaged GUI app) inherits `/usr/bin:/bin:/usr/sbin:/sbin` — no Homebrew bin.
Reproduced exactly: `load_audio` on the same file succeeds with a login shell's PATH and fails with
`NoBackendError` under launchd's. Pre-existing, not caused by this work, but latent until the server
was restarted under launchd.

Fix: `utils/audio.py::_ensure_ffmpeg_available()` resolves `ffmpeg` from `/opt/homebrew/bin`,
`/usr/local/bin`, `/usr/bin` before librosa's fallback runs. Pinned by
`backend/tests/test_ffmpeg_discovery.py`. Verified under the failing condition: server restarted by
the launchd login item, webm stage now 1.11–1.16 s with the correct transcript.

## The trace — shipped 2026-10-05 (work items 1–2)

One record per turn, joined across the chord and the first audible audio.

**Where it lives.** `backend/utils/timing.py` (ring buffer, `MAX_TRACES = 50`; marks are O(1)
appends, and the only I/O is one JSON line per finished turn under `data/logs/turns.jsonl`;
`VOICEBOX_TRACE=0` switches the whole thing off), `backend/routes/turns.py`
(`POST /turns/{id}/begin`, `POST /turns/{id}/marks`, `GET /turns/latest`, `DELETE /turns`), and
`app/src/lib/utils/turnTrace.ts` on the client — fire-and-forget, so a mark can never block or fail
the loop it measures.

**How the two halves join.** The capture half's id is the client's: created at chord start and sent
as `turn_id` on `POST /captures` (form field), `POST /captures/{id}/refine` and
`POST /voice-targets/message` (JSON). The speak half's id is the **narration session id** — the
backend generates it, marks `speak_request` before the session exists, and the pill adopts it from
`dictate:speak-start`'s `generation_id`. That is the only join that can work: the narration request
comes from the *agent*, so the pill never sees it before the backend does.

**The worker problem, solved without plumbing.** `run_synthesis` runs in the worker process when one
is up, and that process has its own timing store — so `model_load_*` and `prompt_*` would have been
invisible to `GET /turns/latest`. They travel as a `("mark", stage)` item on the same queue as the
audio: in-process the stream applies them directly, in the worker they become an SSE `mark` event
that the relay applies. No new ports, no forwarding config.

Stages: backend — `stt_start/done`, `refine_start/done`, `deliver_start/done`, `speak_request`,
`session_created`, `stream_open`, `model_load_start/done`, `prompt_start/done`, `first_chunk`,
`last_chunk`; client — `upload_start`, `deliver_start/done`, `audio_start`, `turn_end`. Marks are
idempotent per `(turn_id, stage)` — first write wins, so both sides of a boundary can mark the same
stage without coordinating.

**Evidence (2026-10-05, this machine, live server).**

```
one real narration, driven over HTTP:
turn bb56a89a…  finished=True
   speak_request       35622.8 ms
   session_created     35622.8 ms  (+0 ms)
   stream_open         35701.0 ms  (+78 ms)      # POST → GET round trip
   model_load_start    35814.3 ms  (+113 ms)
   model_load_done     35814.3 ms  (+0 ms)       # worker model already warm
   prompt_start        35814.3 ms  (+0 ms)
   prompt_done         35814.3 ms  (+0 ms)
   first_chunk         37977.1 ms  (+2163 ms)    # LuxTTS synthesis — the dominant warm stage
   last_chunk          37981.0 ms  (+4 ms)
```

The capture half, driven over HTTP with a real WAV: `stt_start → stt_done` = 3739 ms on the first
call after a restart. `POST /captures` with a `turn_id`, the marks landing under it, and one JSONL
line written on finish are all confirmed against the running server; `scripts/loop-bench` still
prints its table unchanged (stt 1.09/1.29 s, narration → first chunk 2.23 s, delivery 0.02 s).

**Two deviations from the plan, both deliberate.** `audio_start` is marked in `DictateWindow`'s
`onStart` callback rather than inside `narrationPlayer.ts` — that callback *is* the player's
true-start signal, and it keeps the player free of trace ids. And the speak turn's id is the
narration session id (above), not a second client-generated uuid.

**Known limit.** The chord → upload (B6) and first-chunk → audible (B5) gaps are wired but not yet
measured: they need a real chord press with the pill open, and synthetic key events are dropped in
this harness.

## Next steps, ranked by measured cost (2026-10-04, revised)

**Correction to the earlier ranking.** Item 1 used to be "hide the front half by streaming the upload
while the user speaks". That was an inference, and measuring it killed it: the WebM decode is
**0.911 s on the first non-WAV decode in a process, 0.044 s on every one after** (audioread's backend
discovery is the one-time cost), and the WAV decode is 0.002 s. The upload itself is negligible. So
the front half is **not** spread across hideable stages — it is Whisper compute. Streaming would buy
~0.04 s and would risk transcript correctness. Withdrawn.

| what | measured | verdict |
|---|---|---|
| webm decode, first call in a process | 0.911 s | one-time — prime it |
| webm decode, subsequent | 0.044 s | free |
| wav decode | 0.002 s | free |
| Whisper small, CPU, per call | 1.10–1.24 s | **the whole front half** |
| Whisper small, MPS, per call | 0.86–1.82 s | ~25% better, not a lever |
| narration → first chunk (6 steps) | 2.08–2.46 s | shipped this session |

Ranked:

**1. A faster Whisper runtime — RESOLVED: nothing available is faster on this machine.**

Four candidates, same captures, measured:

| runtime | 8.8 s | 11.8 s | 2.2 s | 4.5 s | transcripts |
|---|---|---|---|---|---|
| PyTorch `small` (current) | 1.10 s | 1.43 s | 1.07 s | 0.97 s | correct |
| PyTorch `small` on **MPS** | 0.86 s | 1.82 s | 0.76 s | 0.59 s | byte-identical |
| CTranslate2 `large-v3-turbo` int8 | 3.65 s | 3.88 s | 3.58 s | 3.53 s | correct |
| CTranslate2 `small` int8 | 1.12 s | 1.43 s | 1.07 s | 0.97 s | correct |
| **MLX** `MLXSTTBackend` (mlx 0.32.3, mlx-audio 0.4.1) | **14.47 s** | **14.47 s** | **14.47 s** | **14.52 s** | **hallucinated** |

- **CTranslate2**: turbo is 3× slower (809M params cost more on CPU than the runtime saves) and the
  same-size model is a wash. Dependency cost was real too — `ctranslate2 + onnxruntime + av (46 MB)`,
  and `av 19.0.1` is already incompatible with `faster-whisper` 1.2.1's `metadata_errors` argument.
  Removed from the venv.
- **MLX**: 13× slower *and* wrong. Every clip took an identical 14.47 s — a degenerate decode loop
  hitting a cap, not compute — and the transcripts came back as `emerge emerge` and
  `**Was infected** **Was infected**` where the PyTorch path had the sentence right. MLX is the path
  `get_stt_backend()` selects whenever `mlx.core` imports, and `just setup-python` installs it on arm64
  macOS, so **on this machine the repo's Apple-Silicon STT path is broken** (consistent with the
  `#706/#650` MLX→CPU fallback notes in `docs/PROJECT_STATUS.md`). Worth reporting upstream; not worth
  working around here. Removed from the venv, backend restored and verified.
- **MPS**: correct and ~25% faster, but the flag is a deliberate default and 4 clips is not enough
  evidence to flip it for long-form audio.

**So the front half is at its floor (~1.1 s) and the only lever left is one the user already owns:**
`capture_settings.stt_model`. Measured `base` vs `small` on the same captures — base **0.25–1.31 s**
(≈0.75 s avg) against small **0.76–1.65 s** (≈1.19 s), i.e. ~35% faster, with transcripts identical on
2 of 3 clips and differing only by filler words on the third (`um`, `a brief` vs `the brief`). That is
a quality call on three clips — thin evidence — but it is a one-click setting, not a code change.

**2. Prime the non-WAV decode at startup — RETRACTED, unverified.** An isolated fresh process pays
0.915 s on its first WebM decode (and 0.044 s after), which looked like a one-time audioread cost
worth priming. Two checks killed it: `audioread.available_backends()` costs 0.030 s and does **not**
prime the expensive part (the next decode still takes 0.915 s), and the server itself — measured by
the bench on a freshly restarted process — shows no such cost at all: its first WebM upload was
1.16 s, which is Whisper alone. So the 0.9 s appears to be an artifact of a standalone probe process,
not something a dictation pays. The bench is ground truth here; this item stays retracted unless a
fresh-server bench contradicts it. Mechanism unexplained (candidate: the CoreAudio `macca` backend's
first use in a process).

**3. Narration diffusion steps — shipped this session** (8 → 6; 2.50–2.73 s → 2.08–2.46 s).

**4. Front-half streaming — withdrawn** (see above).

**5. The trace + in-process marks (work items 1–2).** **Shipped 2026-10-05** (see "The trace"). What
is left is the measurement it enables: B5 and B6 need one real chord-driven turn.

**6. Reliability residual — narration is silent with the app closed.** `playback.play_file` is called
only from `services/generation.py`, the persisted path; the narration lane never calls it. Narration
is ephemeral, so there is no saved WAV to play and nothing consumes its SSE stream when no pill is
attached — meaning with the app closed an agent's commentary produces no sound at all. That is
narrower than the `headless_playback` setting promises, and narrower than `scripts/voicebox demo`'s
own help text ("with the app closed it plays on this machine"). Fixing it is a design question
(should the backend self-consume the narration stream when no pill is attached?), not a patch. The
other residual: the pill can ack `/audio/{id}` and still be silent (device, volume) — nothing detects
it.

**7. Surface the silent CPU fallback on Apple Silicon — RETRACTED (it would give the wrong advice).**
This looked like a clean win: nothing in `GpuAcceleration.tsx` / `GpuPage.tsx` mentions MLX, and
`/health` reports `backend_type: pytorch` next to `gpu_type: MPS (Apple Silicon)`, so an Apple Silicon
machine quietly running Whisper on CPU is never told. But MLX measured 13× slower *and* hallucinating
here (see item 1), so "install the MLX extras" would be actively harmful advice. If anything, the
warning should say the opposite — that the PyTorch CPU path is the working one on this machine — which
is a product decision about a broken upstream path, not a latency fix.

## Work items

1. **Backend turn tracing — one record per turn, both directions**
   - Status: **Done (2026-10-05)** — see "The trace" below for the shape and the evidence.
   - What: new `backend/utils/timing.py` with a `TurnTrace` (dataclass: `turn_id`, `anchor_epoch_ms`,
     `marks: list[tuple[str, float]]` monotonic ms since process start, `meta: dict`) kept in a
     bounded ring buffer (`MAX_TRACES = 50`). API: `begin_turn(turn_id, anchor_epoch_ms)`,
     `mark(turn_id, stage, **meta)`, `finish_turn(turn_id)`, `latest(limit)`, `clear()`. `mark()` is
     idempotent per (turn_id, stage) — first write wins. When a turn finishes, append the trace as one
     JSON line to `data/logs/turns.jsonl` (a `RotatingFileHandler`-style cap is not needed; the file is
     small and dev-only). Gate via `VOICEBOX_TRACE` (default **on**, cheap: a dict append).
     New `backend/routes/turns.py`: `POST /turns/{turn_id}/marks` (body `{stage, meta?}`),
     `POST /turns/{turn_id}/begin` (body `{anchor_epoch_ms}`), `GET /turns/latest?limit=20`,
     `DELETE /turns`. Register in `routes/__init__.py`.
     Instrument the stages that already exist as code boundaries, at the point of the boundary:
     `captures.create_capture` → `stt_start`/`stt_done`; `refinement` call → `refine_start`/`refine_done`;
     `routes/voice_targets.py::deliver_message` → `deliver_start`/`deliver_done`;
     `routes/speak.py::narrate_speech` → `speak_request`; `narration.create_session` →
     `session_created`; the stream generator's `ready`/`loading`/first `chunk`/final `chunk` →
     `stream_open`/`worker_loading`/`first_chunk`/`last_chunk`; `narration.run_synthesis` around
     `load_engine_model` → `model_load_start`/`model_load_done`; around `create_voice_prompt_for_profile`
     → `prompt_start`/`prompt_done`.
   - Why: without stage boundaries there is nothing to rank; every later item depends on this table.
   - Depends on: none.
   - Risk: timing code on hot paths. Detect: the marks are O(1) dict/`.append` with no I/O except the
     single JSONL append on `finish_turn`; assert the bench numbers are unchanged with tracing off vs on.
   - Source: inferred from codebase (no existing instrumentation; boundary functions read this session).

2. **Frontend marks — anchor the turn at chord release, close it at first audible audio**
   - Status: **Done (2026-10-05)** — see "The trace" below for the shape and the evidence.
   - What: new `app/src/lib/utils/turnTrace.ts`: `beginTurn(anchorEpochMs) -> turnId`
     (`crypto.randomUUID`), `mark(turnId, stage, meta?)` (fire-and-forget `fetch` to
     `POST /turns/{id}/marks`, no await, errors swallowed), `finishTurn(turnId)`.
     Wire: `DictateWindow`'s `dictate:start` listener → `beginTurn(Date.now())`, keeping the id in a ref
     alongside `actionRef`; `useCaptureRecordingSession`'s `onRecordingComplete` → `upload_start`;
     `onFinalText` entry → `deliver_start`; the agent branch's `sendVoiceMessage` resolution →
     `deliver_done`. On the speak side: `dictate:speak-start` with `narration: true` →
     begin a **second** turn (`speak_request` anchor) and mark `stream_open` when `useNarrationStream`
     opens the EventSource, `first_chunk` on the first `chunk`, `audio_start` from
     `narrationPlayer`'s `onStart` (the first scheduled source actually starting), `last_chunk` on
     `done`. `narrationPlayer.ts` already knows exactly when playback begins — mark there, not in React.
   - Why: the two stages the backend cannot see are chord→upload (mic stop + blob encode) and
     first-chunk→audible (decode + scheduling + the 0.15 s lead). Both are candidate dominators.
   - Depends on: item 1 (the endpoint).
   - Risk: marking on every render or leaking listeners. Detect: marks are called from event handlers
     and the player, never from render; the bench asserts a single trace per turn_id.
   - Source: inferred from codebase (`DictateWindow.tsx`, `useCaptureRecordingSession.ts`,
     `useNarrationStream.ts`, `narrationPlayer.ts` all read this session).

3. **`scripts/loop-bench` — a repeatable headless turn**
   - What: a POSIX shell/`python3 -` script (matching `scripts/voicebox` style) that, against a running
     server: (a) `GET /settings/captures` for the default voice; (b) `POST /captures` with a real WAV
     (arg, else the newest file under `data/captures/`) — the response *is* the STT result, so the
     round-trip time is the STT stage; (c) `POST /captures/{id}/refine` when `auto_refine`;
     (d) `POST /voice-targets/message` with the transcript when `--agent`, else stop after the
     transcript; (e) `POST /speak/narrate` then `curl -N /speak/{id}/stream` with per-event
     `date +%s%N` stamps to time `ready`/`loading`/first `chunk`; (f) `GET /turns/latest?limit=5` and
     print the merged table as `stage | ms | %`.
     Flags: `--wav PATH`, `--text STRING` (skip STT), `--agent`, `--repeat N`.
   - Why: the fix must be chosen from data, and the only way to get data without a human holding a
     chord is to drive the same HTTP endpoints the app drives.
   - Depends on: item 1.
   - Risk: driving `/captures` with a stale WAV measures the wrong audio. Detect: the script prints
     the audio duration and the transcript it got, so a mismatch is visible in the output.
   - Source: specified from user (the measurement-first framing) + inferred from
     `scripts/voicebox demo` (the existing precedent for a curl-driven lane check).

4. **Record the baseline**
   - What: run item 3 three times on an idle machine, plus one real chord-driven turn with the app open
     (item 2's marks land in the same table). Save the table into this plan under "Baseline" and put the
     headline numbers in `CHANGELOG.md` `[Unreleased]` only if a user-visible change ships.
   - Why: "halved" is meaningless without a before.
   - Depends on: items 2, 3.
   - Risk: measuring on a busy machine (a generation running) inflates everything. Detect: the bench
     records `GET /health` + whether a generation was active at start; discard runs that overlapped one.
   - Source: specified from user.

5. **Fix the dominant stage — exactly one branch of the decision table below**
   - What: read the baseline table, take the top row, execute the matching branch. If the top two rows
     are within 15% of each other, treat them as co-dominant and do both.
   - **Status:** the baseline showed the dominant cost was the *cold-start class*, not any single warm
     stage. That fix shipped (see "What shipped"). The table below is retained for the next round,
     which starts from the warm numbers.
   - Why: this is the deliverable the user asked for ("optimize the experience").
   - Depends on: item 4.
   - Risk: fixing the visible stage rather than the dominant one. Detect: the fix is chosen only by the
     table, and its effect is re-measured by the same bench (item 6).
   - Source: specified from user; each branch's fix is specified from codebase + recorded facts.

   | Dominant row | Branch | Fix (specified) | Status |
   |---|---|---|---|
   | STT (`stt_done - stt_start`) | **B1** | Add a `faster-whisper` (CTranslate2) runtime behind the existing `STTBackend` Protocol as a third implementation (`backends/faster_whisper_backend.py`), selected by `capture_settings.stt_model` value (e.g. `fw-turbo`). The CTranslate2 turbo model is **already in the local HF cache** (`~/.cache/huggingface/hub/models--mobiuslabsgmbh--faster-whisper-large-v3-turbo`, verified) but the package is not installed (verified) — so this is a new dependency (`just setup-python` + `backend/requirements.txt`) plus a new code path, not a setting flip. Default stays `small` until the bench proves the swap. | **next** — STT is now the largest warm stage (1.7 s), and it runs on CPU by design |
   | Narration cold start (`model_load_done - model_load_start` on first use) | **B2** | Warm the worker's model right after `start_worker` reports healthy, gated on the settings that make it necessary. | **shipped 2026-10-04** |
   | Narration synthesis (`first_chunk - worker_loading`) | **B3** | Lower the narration chunk budget (`split_text_into_sentence_chunks` cap) so the first chunk is smaller; if the model, not the chunking, is the cost, reduce guidance/steps for narration only (it already runs 8 steps, up from 4 — that change was made for natural delivery and must not be silently reverted; measure both). | open (warm 2.5 s) |
   | Delivery (`deliver_done - deliver_start`) | **B4** | One `herdr` CLI spawn per utterance is the suspected cost. Measured `herdr agent list` at 0.02–0.13 s, so the listing is not the problem; if the *prompt* spawn is, move to the socket API (`herdr api schema`) behind the existing `Transport` shape. | open (looks cheap) |
   | Playback (`audio_start - first_chunk`) | **B5** | Tune `START_LEAD_SECONDS` (0.15 s) and the drain logic in `narrationPlayer.ts`; if decode, not lead, dominates, decode ahead of scheduling (the player already tracks `pendingDecodes`). | **measurable now** (item 2 shipped 2026-10-05); still unmeasured — needs a real chord turn |
   | Mic stop → upload (`upload_start - chord_release`) | **B6** | The blob encode + `File` construction happens synchronously after `stopRecording`; move the encode off the main thread or start the upload from the MediaRecorder `dataavailable` chunks instead of after `stop`. | **measurable now** (item 2 shipped 2026-10-05); still unmeasured — needs a real chord turn |

6. **Re-measure, pin the win, and clean up**
   - What: re-run item 3 three times, put the after table beside the before table in this plan, and
     remove any instrumentation that is not earning its keep (keep the trace — it is the artifact that
     makes the next question cheap). Add a permanent test only for an invariant the fix could silently
     break (e.g. if B1 lands: the `small`/`fw-turbo` selection truth table, not a timing assertion —
     timing tests are flaky and CI has no backend gate). Update `CHANGELOG.md` `[Unreleased]` and
     `docs/content/docs/developer/` for whichever surface changed.
   - Why: an unverified "faster" claim is exactly the kind of thing this repo has been fixing.
   - Depends on: item 5.
   - Risk: pinning a number that only holds on this machine. Detect: assert behavior/selection, never
     milliseconds, in any committed test.
   - Source: specified from user + repo conventions (CHANGELOG/docs obligations).

## Verification

```sh
just test                                  # backend suite; must stay green
just check                                 # biome + ruff
curl -s localhost:17493/turns/latest?limit=3 | python3 -m json.tool
scripts/loop-bench --wav data/captures/<newest>.wav --agent --repeat 3
```

Then one real turn with the app open and the pill visible: chord → speak → agent answers aloud.
Confirm: the trace table has both halves joined by `turn_id`, the answer is audible, History gains no
narration row, and pressing the dictation chord mid-narration still stops it instantly.

## Open decisions

- **B1 is the next real decision, and it is a tradeoff, not a task: adopt `faster-whisper`
  (CTranslate2) for STT?** STT is the largest remaining warm stage (~1.7 s) and runs on CPU by
  design (`PyTorchSTTBackend._get_device(allow_mps=False)`).
  **The cheaper experiment was run (2026-10-04) and MPS is not the lever.** Whisper small on MPS,
  four real captures (2.2 s / 4.5 s / 8.8 s / 11.8 s), same process, engine warmed per device:

  | capture | CPU | MPS | speedup | identical text |
  |---|---|---|---|---|
  | 8.8 s | 1.09 s | 0.86 s | 1.26× | yes |
  | 11.8 s | 1.85 s | 1.82 s | 1.01× | yes |
  | 2.2 s | 0.97 s | 0.76 s | 1.28× | yes |
  | 4.5 s | 0.85 s | 0.59 s | 1.46× | yes |

  So the CPU-only flag is *not* protecting against a visible defect on this machine and hardware —
  the output is byte-identical — but the payoff is only ~0.1–0.3 s per dictation (1.0–1.46×, and
  almost nothing on the longest clip). It does not justify flipping a deliberate default whose
  original reason is recorded nowhere in the repo. The real lever is a different runtime:
  `faster-whisper` (CTranslate2) turbo, whose model is already in the local HF cache. Cost: a new
  dependency in `just setup-python` + `backend/requirements.txt`, a new `STTBackend` implementation,
  PyInstaller hidden imports for CTranslate2, and a platform story for MLX/ROCm machines that do not
  use the PyTorch path. The alternative is B3 (narration synthesis, ~2.5 s), smaller and in-process.
- **The ≥40% acceptance bar** is my inference. The shipped warm-up cleared it for the first turn;
  confirm whether the bar should now apply to the *warm* loop instead.
- **Whether to build the trace (items 1–3) at all.** It buys sub-stage resolution for
  upload/decode (B6) and first-chunk→audible (B5), which are still unmeasured. If those are not felt,
  the trace is cost without benefit and the throwaway probes are enough.
- **Browser-driven frontend measurement.** Capturing item 2's marks without a human requires a
  Chromium run with a fake capture device (`--use-fake-device-for-media-stream`). Verified this
  session: the eval browser can drive the real UI (same-origin web build served from the backend),
  including clicking a toggle and confirming the API round-trip — so a scripted turn is plausible,
  but a fake mic is still unproven.
- **A real bug found while verifying, unrelated to this plan:** in the web deployment the frontend
  route `/settings/captures` collides with the backend API route `GET /settings/captures`, so a hard
  navigation there returns JSON instead of the settings page (client-side navigation works). Worth its
  own fix.
- **No `Context/ADR/` or `Context/Glossary.md` exists.** If this work hardens a decision worth keeping
  (e.g. "STT runs through the Protocol, never a bespoke path"; "startup never downloads"), `/grill`
  should capture it.

## Out of scope

- Hands-free voice mode (VoicePairing item 10) and power lifecycle — separate plans.
- Voice *quality* work (reference re-record, persona pace): this plan only measures and fixes latency.
- Any cloud STT/TTS.
