# Agent Voice — Known Limits and How to Address Them

> Recorded 2026-10-03. Companion to `Context/Plans/RealtimeAgentNarration.md` (the implemented
> narration lane) and `Context/Research/Research.md`.
>
> Scope: the "agent speaks to you in your cloned voice while you work" loop, as it stands today
> on this machine — LuxTTS clone of the user's voice, narration lane, dictation in, four
> harnesses wired (omp, Claude Code, opencode, codex).

## Status snapshot

Working today: clone ("Me"), voice resolution per harness, dictation (chord → Whisper Small →
paste → auto-submit), agent speech (queued `speak`, persisted), narration (`narrate=true`,
streamed, ephemeral), barge-in, four harnesses registered and instructed to narrate.

Everything below is a limit of that loop, not a bug: each entry says what it costs the user, why
it exists, and what addressing it would take.

| # | Limit | Impact | Addressable? |
|---|---|---|---|
| 1 | ~~Narration waits for the generation queue~~ | **Addressed** — see below | done |
| 2 | ~~No mid-turn interruption~~ | **Corrected** — the harnesses do support it | done |
| 3 | A normal line renders whole before audio starts (~2–3 s) | Commentary lags the action it describes | Partly — inherent to LuxTTS/MPS; mitigated by warming |
| 4 | ~~Audio only plays while the desktop app runs~~ | **Addressed** — see below | done |
| 5 | MCP tools appear only in a new session | After registering a harness, nothing works until restart | Inherent to MCP clients |
| 6 | Dictation needs the app + two macOS permissions | Chord silently does nothing without them | Inherent to macOS TCC |
| 7 | Voice prompt uses the first 5 s of the reference | A long recording is mostly ignored | Inherent to LuxTTS `encode_prompt(duration=5)` |

---

## 1. Narration waits for the generation queue — ADDRESSED

**Addressed 2026-10-03** by a dedicated narration worker process (`backend/narration_main.py`,
supervised by `backend/services/narration_worker.py`, relayed by `routes/speak.py`). The main
backend spawns it at startup on port 17494 (next free if taken), waits for its health, and hands
narration to it; if it is unavailable, narration falls back to the in-process path below and
merely waits as before.

Measured, with a long generation still `generating` on the main instance's engine:

| | before | after (worker warm) |
|---|---|---|
| `waiting` events | 10 (~10 s) | **0** |
| first chunk | after the generation finished | **2.03 s** |

Two processes have independent MPS contexts, which was verified directly before building this
(a second process synthesized three chunks while the main instance was mid-generation; both
survived). Two *threads* sharing one model abort the process, which is why the fix is a process,
not a second instance in-process.

**Remaining cost.** The worker loads its own LuxTTS copy on first use (~7 s, ~600 MB GPU): the
first narration after startup measured 8.7 s, every one after that ~2 s. Optional follow-up:
warm the worker's model in the background right after it becomes healthy, trading idle GPU memory
for a fast first line.

**Original analysis follows.**

**Symptom.** With a long generation running, a narration request sits in `waiting` events for the
duration of that generation (measured: 10 waiting events / ~10 s for a 2-sentence narration
behind one generation; the cap is `NARRATION_QUEUE_WAIT_SECONDS = 180`).

**Cause.** The lane deliberately shares the work engine
(`backend/routes/speak.py`: `get_tts_backend_for_engine(session.engine)` +
`task_queue.wait_until_idle()` before synthesis). Sharing one instance is what makes narration
crash-proof: two threads inside one torch model abort the process
(`failed assertion _status < MTLCommandBufferStatusCommitted`, fixed in `b789689` by holding the
engine lock across a cancelled stream). Running narration *concurrently* therefore requires a
second model instance — on this hardware, in another process.

**Why not the obvious fixes.**
- CPU-pinned LuxTTS (the original design): LuxTTS's CPU path is a different implementation
  (ONNX `generate_cpu`) that under-generates short text — 0.11 s vs 1.24 s for a one-liner, and
  it crashes its own vocoder on the shortest inputs.
- Kokoro on CPU: instant and safe, but preset-only, so it cannot speak in the user's clone.

**Options.**
- **(a) Second backend process for narration.** A dedicated process holding its own LuxTTS
  instance on MPS, with the main backend proxying narration to it. Two processes each with their
  own command queues are fine on macOS — the abort we hit was threads sharing one model, not two
  processes. Cost: ~600 MB extra GPU memory, one extra model load at first use, plus a
  supervision story (spawn, health, port). Effort: moderate — new process entry point, a
  narration client in `routes/speak.py`, and fallback to the in-process path when it is absent.
- **(b) Preempt instead of queue: pause the generation, narrate, resume.** No second model, but
  the work stops mid-generation, which is worse than waiting for anything but a very long job.
  Requires cooperative checkpointing in the generation loop — invasive.
- **(c) Accept it, but make the wait visible.** Today the pill just sits in `speaking`; it could
  show "waiting for the current generation" and offer to cancel the generation instead. Cheap.
- **Recommendation:** (a) if instant commentary matters; (c) as a stopgap either way.

## 2. No mid-turn interruption — CORRECTED (the harnesses support it)

**Corrected 2026-10-03.** The original entry claimed a dictated question is only read at the
next turn boundary. That was wrong for the harnesses in use: mid-turn steering is a harness
capability, and both of the daily drivers have it. Voicebox needs no change — dictation already
delivers into that path, because auto-submit is what lands the transcript in the running turn.

**Verified directly against omp** (`omp --mode rpc`, protocol in `omp://rpc.md`): a prompt asking
for three sequential `sleep 5` bash calls, then a `{"type":"steer","message":…}` sent seven
seconds in:

```
[steer response] success=True
agent_end                      ← after the FIRST tool call, not the third
reply: "Stopped per your instruction. Job bg_1 (sleep 5) finished on its own: exit 0,
        no output. Not running the remaining sleeps unless you say go."
```

The steer was accepted mid-run and the agent abandoned its remaining tool calls. omp's
`set_interrupt_mode` decides how far that goes: `"immediate"` (the default) checks steering
between tool calls and can abort the rest of the turn; `"wait"` defers the message to turn end.

**Claude Code** does the same in its CLI/TUI — "a message typed while Claude is working is
picked up between tool calls ('steering') … injected into the agent loop mid-task"
(anthropics/claude-code#71726).

**So the loop is complete for these harnesses:** hold the chord, speak, release → the transcript
is pasted *and submitted* into the running turn → the agent steers mid-task and answers in your
voice. Without auto-submit you would still be typing the Enter yourself, which is the one part
that mattered.

| Harness | Mid-turn steering | Status |
|---|---|---|
| omp | yes — verified over RPC; the TUI uses the same prompt path | works today |
| Claude Code | yes — between tool calls, per upstream | works today |
| opencode / codex | not checked | unknown |

**Original entry, for the record.** The claim was that MCP is client→server and the only
server→client push is the speak SSE, so a dictated message could only land in the composer. The
mechanism it missed is that the *harness* is the one that injects queued input into a live run —
Voicebox only has to get the text into that queue, which paste + submit does.

## 3. Latency: ~1–3 s per sentence

**Symptom.** First audio ~1.2 s after the request; each further sentence arrives as it is
synthesized. Commentary therefore trails the action slightly.

**Cause.** LuxTTS on MPS: one diffusion pass before audio can start. Narration used to force one
chunk per sentence to start playing sooner, but rendering sentences independently cost
cross-sentence prosody — the line sounded like separate takes. A line that fits the chunk budget
is now one continuous read (natural), and only text past `max_chunk_chars` streams in
sentence-aligned chunks. The trade is explicit: natural delivery over earliest-possible audio.

**Options.**
- **(a) Warmth.** The model is already kept loaded; the first-use load (~5 s) is the only spike.
  Already done.
- **(b) Shorter sentences.** Already the default; `max_chunk_chars` could be lowered for
  narration specifically.
- **(c) Intra-sentence streaming.** LuxTTS has no incremental synthesis API — not available.
- **(d) Faster engine for narration.** Kokoro on CPU is near-instant but preset-only.
- **Recommendation:** accept; keep the model warm.

## 4. Audio only plays while the desktop app runs — ADDRESSED

**Addressed 2026-10-03** by `services/playback.py` + a `headless_playback` setting (off by
default, toggle in **Settings → MCP**). When it is on, no pill client is subscribed to
`/events/speak`, and the generation was agent-initiated, the backend plays the saved WAV through
the OS (`afplay` on macOS, `paplay`/`aplay` on Linux, `powershell` on Windows), serialized so two
speaks cannot overlap. Verified with a second backend instance that had no pill: it logged
`Headless playback: no pill attached, playing <id>.wav on this machine` and `afplay` was caught
running on that file; the main instance with the pill attached logged nothing and played nothing.

Fixing it exposed a second bug: **agent speech was recorded as `source="manual"`**, because
`POST /speak` and the MCP tool both built a `GenerationRequest` and let `generate_speech` default
the field. That also meant the frontend's `AGENT_SOURCES = {mcp, rest}` check — which exists so
the main window does not autoplay agent speech — never matched. `generate_speech` now takes a
`source` argument and the speak paths declare `rest`/`mcp`.

**Original analysis follows.**

**Symptom.** With the app closed, `voicebox.speak` still generates audio and persists it, but
nothing is heard — the pill window is the only playback surface, and it is driven by Rust.

**Cause.** By design: "no silent background TTS" — the pill is the contract. The backend never
plays audio itself.

**Options.**
- **(a) Headless playback mode.** Have the backend play the synthesized WAV through the OS
  (`afplay` / `sounddevice`) when no pill client is attached. Breaks the always-visible pill
  contract, so it should be opt-in.
- **(b) Remote mode.** The app already supports pointing at a remote backend; playback still
  requires a local client.
- **Recommendation:** (a) behind a setting, for server/remote use.

## 4b. An attached pill can drop its cycle — ADDRESSED

**Symptom.** The agent's speech is synthesized and saved, the pill is running, and nothing is
audible. Intermittent: it depends on how long the generation takes.

**Cause.** Two independent holes. (a) The pill capped its wait for audio at 60 s and cleared that
timer only on `completed`, so any speak slower than a minute — three chunks, or a queue wait, or a
cold model load — was dismissed while it was still being made. (b) The completion path skipped
headless playback whenever the pill was *attached*, so a dropped cycle meant silence rather than a
fallback; a pill that is inert looks exactly like a pill that is playing.

**Fix.** The watchdog re-arms on every status event (it now only guards a backend that has gone
quiet). Completion waits ~4 s for the pill to fetch `/audio/{id}` — that fetch is the ack — and
plays locally if it does not, publishing `speak-end`/`cancelled` so the pill cannot double-play.

**Residual.** If the pill takes the audio and then fails to produce sound (output device, system
volume), nothing detects it: the ack proves the app took the file, not that the user heard it.

## 5. MCP tools appear only in a new session

**Symptom.** After registering voicebox in a harness, that harness has no `voicebox.*` tools
until it restarts. This is why the session that did the registration cannot use it.

**Cause.** MCP clients load tools at session start. Nothing to fix in Voicebox; worth remembering
when wiring a new harness.

## 6. Dictation needs the app and two macOS permissions

**Symptom.** Pressing the chord does nothing at all when Input Monitoring is missing — macOS
drops the key events silently.

**Cause.** macOS TCC. Two deadlocks were fixed to make this reachable at all: the hotkey was
armed only when every gate (including Input Monitoring) was green, so the app never called
`enable_hotkey`, which is what fires the prompt; and the LLM model gated recording even with
refinement off. Both are fixed (`078ca55`). The permission itself still needs the user.

**Options.** None technical. The app now asks for the permission itself, which is the best
available path. Accessibility is required separately for the paste.

## 7. The voice prompt uses the first 5 s of the reference

**Symptom.** A 30 s reference recording is mostly ignored; only the first 5 s shape the voice.

**Cause.** `LuxTTS.encode_prompt(prompt_audio, duration=5)` in
`backend/backends/luxtts_backend.py`. Nothing to fix without changing engines; it is why the
recording guidance is "speak immediately, 10–12 s is plenty".

**Options.** Use Qwen3-TTS for the persisted path (different reference handling). Narration
follows the profile's engine, so switching the profile to Qwen switches narration too.

**Adding a second sample does not blend the two.** `base.combine_voice_prompts` concatenates the
samples end to end (`np.concatenate`) and the encoder then reads the first 5 s of that
concatenation, so every sample after the head is dead weight. Two takes cannot be averaged into
one style — the blend has to be performed inside the 5 s window. Replacing the single sample is
the correct move.

### The reference sets the persona, not just the timbre

Zero-shot cloning copies delivery as well as voice, so the reference is the *style control* for
narration. Measured on this machine, same text and settings, only the reference changed:

| reference | resulting rate |
|---|---|
| a setup announcement ("This will be the voice that all of my agents will use…"), truncated at 5 s | 142 wpm, flat — read like a script |
| take A, deliberately measured | 150 wpm |
| take B, energetic | 174 wpm — the energy carried straight through |
| take C, natural/unhurried | 150 wpm |

So a reference recorded in a testing tone produces a testing-tone agent. Record the delivery you
want the agent to have, and keep the `speed` knob for trimming pace afterwards (`LUXTTS_SPEED`,
0.75 = ~150 wpm).

---

## Suggested order of work

1. ~~**#1 — narration process**~~ — **done 2026-10-03** (see §1).
2. ~~**#4 — headless playback**~~ — **done 2026-10-03** (see §4).
3. ~~**#2 — mid-turn interruption**~~ — **corrected 2026-10-03: the harnesses already do this** (see §2).
4. #3, #5, #6, #7 — accept and document.

Optional, from §1: warm the worker's model at startup so the first narration is not slow.
