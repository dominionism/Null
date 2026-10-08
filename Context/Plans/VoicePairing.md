# Voice pairing — talk to your agents, not at your keyboard

## Goal

Let the user hold a key, speak, and have the words land as a **user message in a chosen, already-running agent session** — from any app, without focusing a terminal — with the agent's work and answer coming back as speech.

## Acceptance

Done when the user can, from an app that is not the agent's terminal and with no keystroke of their own:
1. hold the agent chord, speak a request ("blueprint a Rust backend for this"), release;
2. see the pill name the target agent, and watch the message arrive in that session as a user message;
3. hear the agent's progress and its short spoken answer;
4. interrupt by speaking again — the new words reach the running turn, not the next one.

5. **Voice mode** (item 10): toggled on by chord, the user speaks without holding anything and the agent answers; toggled off, the chord behaves as a one-shot again.
6. **Voice off** (item 4): one chord stops the agent mid-sentence — narration, in-app playback and headless playback all at once; a second keeps it quiet until re-enabled.
7. **Power** (item 5): the stack comes up without a terminal (`voicebox start`, or automatically at login) and goes down with one command (`voicebox stop`), with `status` naming exactly one server pid.

And the pre-existing path is untouched: the original chord still pastes into the focused field, and dictation into a plain text field behaves exactly as it does today.

## Constraints

- **Verbatim (user):** "My agents will still have the capacity to everything it already can plus my voice to respond when I talk to it."
- **Verbatim (user):** "I don't need to keep typing."
- **Verbatim (user):** "If I tell it to create this repo and make me a website, or write this backend in rust, or plan with me, or blueprint with me … that is my desired working experience."
- The existing paste-into-focused-field path **must keep working unchanged** — it is the universal fallback and the "dictate into any app" feature.
- **Verbatim (user):** "we continue the cmd + option when I want it to speak and it responds to me one at a time. Or have cmd + option and then shift + v to enter pure voice mode where it's conversational."
- **Verbatim (user):** "could be tighten and provide a full fix for voice off, or kill voice? When I want it to stop talking (meaning completely shut off, then there should be a key bind for that as well). Even more, there should also be a key bind that enables voice mode as well."
- **Verbatim (user):** "I don't want to have to keep doing bun dev all the time. So work power on and power off mechanisms."
- **Verbatim (user):** "I know that voice gets activated when I mention voice, but it would be good if I didn't need to do that. (Doing command + option) should immediately prompt voice from agents (not doing it will indicate no voice communications is needed)."
- **Chord constraint (codebase):** chords are *sets of keys* (`capture_chords.py`: `["MetaRight", "AltGr"]`, `["MetaRight", "AltGr", "Space"]`), matched as a set by the tauri hotkey monitor. There is no sequence support, so "⌘+⌥ then ⇧+V" is implemented as a single dedicated chord rather than a key sequence.
- Local-first: no cloud STT/TTS (project premise, `Research.md`).
- Codebase patterns that bind this work: `routes/` thin → `services/` logic → `backends/` engines; hand-rolled idempotent migrations in `database/migrations.py` (no Alembic); DB paths relative to the data dir via `config.to_storage_path`; `app/src/lib/api/{index,models,schemas,services,core}` is generated from `/openapi.json` by `scripts/generate-api.sh` and must never be hand-edited; `X-Voicebox-Client-Id` already identifies a calling harness (`MCPClientBinding`); style per `backend/STYLE_GUIDE.md` + `pyproject.toml` (Ruff, 120 cols, Google docstrings, lazy `%s` logging, no bare `except`).
- No `Context/ADR/` and no `Context/Glossary.md` exist (gap noted in `Research.md`). Nothing to contradict; no ADR is added by this plan.

## Transport strategy

The user's agents already run inside **`herdr`** — "a terminal workspace manager for AI coding agents" (`herdr server`, Homebrew-installed, daemonized) — and it exposes exactly the surface this feature needs, as JSON, from a socket API with a CLI wrapper:

| need | herdr command | notes |
|---|---|---|
| enumerate agents | `herdr agent list` | per agent: `pane_id`, `agent`, `agent_status`, `cwd`, `terminal_title`, `focused`, `terminal_id` |
| submit a message | `herdr agent prompt <TARGET> <TEXT>` | `<TARGET>` accepts a pane id (`w7:p1`, verified); rejects with `agent_blocked` before sending any input when the agent is blocked |
| read the reply | `herdr agent read <TARGET> --source recent-unwrapped --lines N` | verified working; `recent-unwrapped` exists for exactly this |
| turn state | `herdr agent wait <TARGET> --until idle\|done\|blocked` | states: `idle`, `working`, `blocked`, `done`, `unknown` |
| lower latency later | `herdr api snapshot` / `herdr api schema` | the socket API behind the CLI, with a bundled schema |

Verified live during planning: `herdr agent list` reported a claude agent (`idle`, `~/Desktop/Profiles`, pane `w7:p1`) and this very omp session as `working`; `herdr agent get w7:p1` and `herdr agent read w7:p1 --lines 2` both returned JSON/text as documented.

**Decision: herdr is the primary transport.** It needs no new dependency, matches how the user already works, and gives three things the alternatives do not: agent **discovery** (so no self-registration protocol is needed), **reply reading** (so the loop works even for an agent that ignores the speak conventions), and **turn state** (so the pill can say whether the agent is idle or busy). `agent_blocked` also removes the mis-addressed-keystroke risk that would otherwise make this safety-critical.

Alternatives, in the order they earn their place:

| transport | when it applies | why not first |
|---|---|---|
| **native**: claude-code inbox socket, opencode HTTP server, codex app-server socket, omp RPC | a machine without herdr | each needs its own setup (a setting, a flag, a daemon) and its own protocol; omp's is stdio-only and cannot reach a session Voicebox did not launch |
| **tmux** `send-keys -t <pane> -l <text>` + `Enter` | a machine without herdr and without native support | requires installing tmux and launching agents inside it (not installed on this machine), and needs a hand-rolled "is this pane an agent or a shell" guard that herdr gives for free |

AppleScript terminal paths (Terminal.app `do script`, iTerm2 `async_send_text`, Ghostty `input text`) are **not** used: `do script` executes text as shell input when the agent is not in the foreground, and each adds per-terminal branching for no capability herdr lacks. (The user's terminal is Ghostty, which is why the terminal-level question was asked at all.)

**Implementer note:** herdr ships its own skill — run `herdr --skill` before writing the transport, and consult `https://herdr.dev/llms.txt` for debugging. The CLI wrapper is the recommended first implementation; the socket API (`herdr api schema`) is the latency/streaming upgrade if spawning a process per utterance proves too slow.

## Work items

1. **Dictation STT — stay on `small`; treat accuracy as a separate, optional experiment**
   - What: **change nothing.** `turbo` is already the product default (`models.py:252` and the DB column both default to it) and this machine is on `small` by explicit choice. Measured during planning, on one real capture (`fc1391bf…`, 9 s): `small` loaded in 1.4 s and transcribed in **6.3 s**; `turbo` did **not complete within 600 s on CPU** (and did not complete within 900 s on MPS). Do not switch to `turbo` on this path.
   - Why: the accuracy gap is real — `small` rendered that capture as "…what we were working on for my MIME project…" — but a model that never returns is worse than one that mishears. The accuracy lever that remains is the "ask aloud when unsure" convention in item 2.
   - Depends on: none.
   - Risk: leaving `small` keeps technical-term errors in the loop. Know it by: the agent repeating back a misheard name as a question (item 2) is the mitigation; a better STT would need its own evaluation, not a setting flip.
   - **Measured facts for whoever revisits this:** STT never uses the GPU — `PyTorchSTTBackend._get_device()` calls `get_torch_device(allow_xpu=True, allow_directml=True)` and `allow_mps` defaults to `False` (`backends/base.py`), so Whisper falls back to CPU while LuxTTS gets MPS. Both turbo attempts used transformers' `pipeline` with `language="en"`, which emits the "custom `forced_decoder_ids`" deprecation warning — the timeout may be a generation-config pathology rather than raw model cost, so the honest conclusion is "turbo as invoked here does not complete", not "turbo is slow".
   - **Credible alternative, if accuracy must improve:** `faster-whisper` (CTranslate2) is a different runtime and runs turbo in real time on CPU — and a CTranslate2 turbo model is **already in the local HF cache** (`models--mobiuslabsgmbh--faster-whisper-large-v3-turbo`) even though the package is not installed. That is a new dependency plus a new STT code path, so it is its own work item, not part of this one.
   - Source: measured during planning (both models, both devices) + codebase (`WHISPER_HF_REPOS`, schema defaults, `get_torch_device`).

2. **Spoken-reply conventions — one line, four files**
   - What: the existing convention block in each harness instruction file already covers narration ("One sentence per step") but leaves the **result announcement** unbounded: "Announce results when done. Call `voicebox.speak(text="<the result>")`". Add the missing brevity rule there — results are at most two sentences (a headline and the next step), detail belongs in the text reply, never speak a file, diff, or plan body — and mirror the same sentence in `voicebox.speak`'s description in `backend/mcp_server/tools.py`. Files: `~/.omp/agent/AGENTS.md`, `~/.claude/CLAUDE.md`, `~/.config/opencode/AGENTS.md`, `~/.codex/AGENTS.md`.
   - Why: a 2 248-character speak measured 166 s of monologue; that came through the unconstrained "announce results" line, not the narration line.
   - Depends on: none.
   - Risk: agents ignore prose. Know it by: watching a real session's spoken length afterwards; the tool description is the higher-leverage surface, the instruction files second.
   - Source: inferred from the instruction files and measured (speak durations in `generations`).
3. **Voice turns — the chord *is* the request for voice**
   - What: mark a delivery as a voice turn so the agent answers aloud without being told. When the agent chord delivers a transcript, prefix the delivered text with a stable marker (e.g. `[voice turn: reply aloud, 2 sentences max]`) and teach the marker in the instruction files from item 2 ("a message starting with `[voice turn]` must be answered aloud, briefly"). The marker is applied **only** on the agent path — never on the paste path, where it would pollute whatever field the user is dictating into.
   - Why: today the agent speaks because it was instructed to narrate, or because the user said the word "voice". The chord itself should carry the intent: use it and you get a spoken reply; don't use it and you get text.
   - Depends on: item 4's delivery path for the marker, item 2 for the convention that gives it meaning.
   - Risk: the agent ignores the marker, or the marker leaks into a spoken reply. Know it by: checking the delivered text in the agent's transcript on the first real turn.
   - Source: inferred from the user's requirement + codebase (chords/delivery).
4. **Voice off — stop talking, and stay quiet**
   - What: two chords and one endpoint. `POST /speak/stop` cancels every live narration session, kills any headless player process, and publishes `speak-end`/`cancelled` so the pill dismisses (the same signal the playback fallback already uses). Chord A — **stop talking now** — calls it once. Chord B — **voice mode toggle** (enable/disable) — flips a persisted `voice_enabled` flag on `capture_settings`; when off, agent speech is suppressed at the source rather than played and stopped.
   - Why: "completely shut off" is two different needs — silence this utterance, and silence the machine until I say otherwise.
   - Depends on: none for chord A; chord B needs the capture-settings field + the speak paths honouring it.
   - Risk: a chord that silently does nothing (the pill has audio, the backend has narration, and headless playback is a separate process — stopping one is not stopping all). Know it by: the endpoint addressing all three, and a test that asserts each is cancelled.
   - Source: inferred from the user's requirement + codebase (`speak:interrupt` exists for the pill only; no backend stop endpoint).
5. **Power on / power off — no terminal**
   - What: a `scripts/voicebox` control script (`start` | `stop` | `status` | `restart`) that starts the backend detached (no tty, no `bun dev`), writes a pid file, and stops it cleanly; plus a launchd user agent (`~/Library/LaunchAgents/…plist`, `KeepAlive`) so the backend is up after login without any command. `start` also launches the desktop app when asked, since the pill lives there. The existing `keepServerRunningOnClose` setting and the parent-pid watchdog are respected: the script starts the server *without* a parent that exits.
   - Why: the user should never type `bun dev` again, and today's failures came from the app running without a server.
   - Depends on: none.
   - Risk: two servers fighting over port 17493 (the app reuses whatever it finds at startup, so a stray duplicate breaks the pill) and the watchdog killing a server whose parent went away. Know it by: `status` reporting one pid, and `start` being idempotent.
   - Source: inferred from codebase (`scripts/`, the dev-sidecar placeholder, the parent watchdog) + today's live failure.
6. **Agent discovery and voice binding**
   - What: read `herdr agent list` and expose the live agents to the app (`GET /agents`), then let a voice profile be bound to an agent target. Store the binding on `mcp_client_bindings` (add nullable `agent_target` — the pane id — plus `transport`) via a new `_migrate_*` helper called from `run_migrations()`, with `PUT /voice-targets/{client_id}` to set it and `GET /voice-targets` to read it back joined with live herdr state.
   - Why: Voicebox must know which agent to address, and herdr can *enumerate* them — so the user picks from a live list instead of configuring an address by hand.
   - Depends on: none.
   - Risk: a stale pane id after a pane closes. Know it by: re-listing on every read, and treating a target absent from the list as unavailable rather than sending into the void.
   - **Generated client:** adding these endpoints includes running `scripts/generate-api.sh` against a live backend; item 8 reads the regenerated client.
   - Source: inferred from codebase (`MCPClientBinding`, migration pattern) + verified herdr output.
7. **herdr transport**
   - What: `backend/services/voice_targets.py` with a `Transport` protocol (`available() -> bool`, `send(text) -> None`, `status() -> str`) and a herdr implementation shelling out to the CLI (`herdr agent prompt <pane> <text>`; `herdr agent list`; `herdr agent read`), parsing the JSON envelope (`{"id": ..., "result": ...}`). Handle `agent_blocked` as a distinct, user-visible refusal — never a silent drop.
   - Why: this is the injection path, and the only one that needs no setup on this machine.
   - Depends on: item 3.
   - Risk: herdr absent or its server stopped (the app must degrade to paste, not error), and CLI spawn latency per utterance. Know it by: `herdr status`/`agent list` probe before send, and measuring end-to-end send latency; if it is poor, move to the socket API (`herdr api schema`).
   - Source: researched + verified live (`herdr agent prompt/read/list` help and real output).
8. **Route dictation to the active agent**
   - What: in `app/src/components/DictateWindow/DictateWindow.tsx`, branch inside `onFinalText`: when an agent target is active, `POST /voice-targets/{client_id}/message` (new endpoint, thin route → `voice_targets.send`) instead of `invoke('paste_final_text')`; on any transport failure, fall back to the paste path and surface the reason. The agent path gets its own chord, added the way the existing two are (`capture_settings` chord fields + `utils/capture_chords.py` defaults + the tauri hotkey monitor), so the two meanings never blur; the pill shows the target while recording.
   - Why: this is the feature — the transcript stops being a keystroke and becomes a message.
   - Depends on: items 6, 7.
   - Risk: a silent mis-route (pasted when the user meant the agent, or the reverse) and double submission. Know it by: one chord per meaning, the pill naming the target, and a fallback that reports itself.
   - Source: inferred from codebase — `onFinalText` is the single delivery point for the transcript; `paste_final_text` is invoked from the frontend, so the decision needs no Rust change.
9. **Turn state and target surface**
   - What: the pill shows the bound agent and its `agent_status` while recording (and a one-line hint when it is `working`, so the user knows the words will land mid-turn); Settings → MCP gets one row per harness with target, status, and a picker fed by the live agent list.
   - Why: pairing is turn-taking; the user must be able to see whether the agent is listening, busy, or blocked, and fix a stale binding without reading a log.
   - Depends on: items 6, 7.
   - Risk: UI overreach in an already dense screen. Know it by: one row per client, three facts.
   - Source: inferred from codebase (`ServerTab/MCPPage.tsx` already lists bindings) + verified herdr states.
10. **Voice mode — the hands-free conversational loop**
   - What: a toggled mode (chord B from item 4) in which the capture runs continuously with **end-of-utterance detection** instead of push-to-talk: speech is segmented on trailing silence, each segment is delivered as a voice turn (item 3), and the mic is suspended while the agent is speaking so the agent's own voice cannot be transcribed as the user's. The pill shows the mode, the bound agent, and its state.
   - Why: this is the stated goal — "literally me talking to my agents without having to do anything."
   - Depends on: items 3, 4, 7, 8.
   - Risk: three distinct failure modes, each with its own tell. (a) **Echo** — the mic hears the agent's playback and transcribes it as the user; mitigated by half-duplex (suspend capture while speaking), which is why the agent's replies must be short (item 2). (b) **Segmentation** — a fixed silence timeout cuts the user off mid-thought or fires on a breath; know it by tuning against real speech and showing the captured text in the pill before it is sent. (c) **Latency** — a turn costs ~6 s of transcription (measured) plus synthesis plus the agent's thinking, so the loop is deliberate rather than snappy; know it by measuring end-to-end turn time and keeping replies short.
   - Source: inferred from the user's requirement; the absence of any VAD in the codebase is a verified gap.
11. **Speak the agent's reply without agent cooperation** (planned, not first)
   - What: after a send, `herdr agent wait <pane> --until idle|done` then `herdr agent read --source recent-unwrapped --lines N`, strip the TUI chrome, and speak the tail.
   - Why: closes the loop for an agent that ignores item 2's conventions — the user hears the answer regardless.
   - Depends on: item 7.
   - Risk: **parsing rendered terminal output is inherently fuzzy** (borders, spinners, status lines). Know it by: comparing the extracted text against the agent's real reply on several turns before trusting it; keep agent-cooperative narration (item 2) as the primary path and this as the safety net.
   - Source: verified herdr capability (`agent read --source recent-unwrapped`) — feasibility confirmed, fidelity unproven.
12. **Native transports and tmux fallback** (only if the product must support machines without herdr)
   - What: implement the same `Transport` protocol for claude-code's session inbox socket, opencode's embedded HTTP server, codex's app-server control socket, and tmux `send-keys`; select by availability, preferring herdr.
   - Why: Voicebox ships to users who do not run herdr.
   - Depends on: items 6, 7.
   - Risk: four more protocols, each with its own setup requirement, and none of them provides discovery or reply-reading as cleanly as herdr. Know it by: implementing one at a time, each behind its own availability probe, and never letting a missing transport block the paste path.
   - Source: researched — claude-code (`code.claude.com/docs/en/cross-session-messaging#the-sessions-inbox-socket`), opencode (`opencode.ai/docs/server`, `POST /session/:id/prompt_async`), codex (`developers.openai.com/codex/app-server`, `turn/start` / `turn/steer`), tmux (`man.openbsd.org/tmux#send-keys`).

## Open questions

- **Why `turbo` timed out** — unexplained, and worth one bounded investigation before anyone tries again: transformers' Whisper pipeline, `language="en"`, on both CPU and MPS, for a 9-second clip. The `faster-whisper` route sidesteps it entirely.
- **Does `herdr agent prompt` steer or queue mid-turn?** Its help says submission is accepted unless the agent is blocked, and that it "does not track turns", so the mid-turn semantics belong to the harness (verified for omp; documented for claude-code). The user-visible difference is whether the agent answers immediately or after its current turn — worth confirming per harness before promising item 5's acceptance step 4.
- **Which agent is the user's first target?** herdr currently reports a claude agent in `~/Desktop/Profiles` (idle) and the omp session behind this conversation (working). The first end-to-end demo should use whichever they pair with daily.
- **Is half-duplex voice mode enough, or is real barge-in required?** Half-duplex (mic closed while the agent speaks) is simple and robust but means you cannot interrupt by speaking — you would press the chord. True barge-in needs echo cancellation, which is a much larger change. The user should hear the tradeoff before item 10 is built.
- **Which agent should the first end-to-end turn target?** herdr reports a claude agent idle in `~/Desktop/Profiles` and this omp session working; the first demo should use the one they pair with daily.
- **Does the herdr socket API need authentication for a local client?** The CLI worked unauthenticated here; the socket path and any token requirement are in `herdr api schema` and should be checked before a non-CLI client is written.
