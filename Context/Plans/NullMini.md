# Null Mini — a floating pet you type or talk to, on any provider

> Blueprint: 2026-10-08, revised the same day after the user's answers. Builds on
> `Context/Research/Research.md` (comprehensive, 2026-10-07) and
> `Context/Research/ChatGPTPetsAndMini.md` (the reference product). Sibling plans: `VoicePairing`
> (talking into an already-running terminal agent) and `VoiceLoopLatency` (the measured baseline).
>
> Status: **A working path exists end to end (2026-10-08): fn+Space → type → OMP answers, through
> the prototype box.** Proven through the live server; the on-screen part awaits the user's hands-on
> check. Items 4–6 are partly built; items 7–9 are superseded by `MiniApp.md`. Voice is deferred by
> the user until the text mini is proven.

> **Direction change (2026-10-08):** Null Mini becomes its own app in `Mini/`, separate from the Voice
> desktop and its server (`Context/ADR/0002-NullMiniIsItsOwnApp.md`). Architecture decision 4 and
> items 5–9 were written for a mini inside the desktop app. That build was re-planned in
> `Context/Plans/MiniApp.md`, the active plan for Phase 1; the statuses under items 5–9 say what
> became of each. Folder names follow `Context/ADR/0001-CapitalizedFolderNames.md`.
>
> **Name and shortcut (2026-10-08):** the user named the app **Null** ("Just call it Null") and moved
> its shortcut to Control+Space, which needs no permission. "Null Mini", "the mini" and "fn+Space"
> below are this plan's original words for that app and its shortcut.

## Goal

Give Null a floating mini that opens on fn+Space as a small, compact text box — a mini CLI — whose
brain is the user's own agent harness (Oh-my-pi first), so it answers questions and does real work on
whichever provider the user chooses. Voice conversation and the pet follow once that is proven.

## Constraints

- **Verbatim (user):** "That is the same goal for Null. It should be exactly the same."
- **Verbatim (user):** "when my usage limit is used up … my mini pet is not usable at all." / "I don't
  rely only on ChatGPT, or a singular AI provider. I can switch whenever I want to manage usage."
- **Verbatim (user):** "This is going to be a personal tool, but I plan on open-sourcing this so that
  other people can use Null via BYOK (Bring Your Own Key), or authentication with an AI provider
  (Claude, Codex, or OpenCode)."
- **Verbatim (user):** "it should be able to have its own harness around the agent (whichever one I am
  using) … the agentic harness that I currently use is Oh-my-pi (OMP) … have my null mini be able to
  use omp to do all of the heavy work I want it to do."
- **Verbatim (user):** "let's not focus on the voice-to-voice for now. What I was thinking was more
  clicking on fn + space to activate a mini textbox that I can type to (like a mini cli just like
  right now, but very small and compact). We will work on the voice stuff later. I need to prove that
  the Null mini will work first."
- **Verbatim (user, on the mini's look, 2026-10-08):** "All I really need to see is the text box and
  the arrow." No placeholder text, no status line, no hints: "there's just a lot of filler that …
  makes everything look really messy."
- **Verbatim (user):** "my cloned voice will [be] the voice used during the voice-to-voice mode."
- **Verbatim (user):** "If I ask Null a question, then it answers. If I ask Null to do something, and
  then it does it." / "the harness's normal permission mode works as well, and spoken approval only
  after the null mini reads the request."
- **Provider policy (researched, see Evidence):**
  - Anthropic: third-party products may not *offer* claude.ai login or subscription rate limits
    without approval; API keys are the sanctioned route.
  - OpenAI: Codex app-server sign-in may be used by local or open-source apps, never by commercial or
    hosted services.
  - OpenCode: signs in to ChatGPT plans and takes API keys for most providers itself.
  - Consequence: **Null never reads, stores or forwards a provider token.** It only starts the
    user's own installed harness, which holds its own sign-in.
- **Local-first** (project premise): speech stays on the machine. Only conversation text goes to the
  chosen provider, through the harness.
- **The existing dictation path must keep working unchanged**: the push-to-talk chord still pastes
  into the focused field, and the agent chord still delivers through `herdr`.
- **Generation stays serialized on the single queue** (`services/task_queue.py`).
- **The overlay never takes app activation.** The app the user was in stays frontmost.
- **Machine budget:** Apple M1, 16 GB.
- **Server PATH is minimal under launchd** (the reason `_ensure_ffmpeg_available` and herdr's
  fallback paths exist). Harness binaries must be found explicitly, not assumed on `PATH`.

## Decisions made (2026-10-08)

| Question | Decision |
|---|---|
| Personal or distributed | Personal first, then open source for others to use with their own key or sign-in |
| First harness | Oh-my-pi (OMP). Its own tools, providers and logins do the work |
| What the mini can do | Whatever the harness can do. No Null-side capability list for Phase 1 |
| First surface | Text only, fn+Space. Voice later |
| Conversation voice (later) | The user's cloned voice |
| Approvals | Null adds no approval layer and mirrors the harness's own setting. On this machine OMP is set to no prompts (`yolo`), so the mini never asks. Condition (user): "as long as the experience is the same with me talking to my agent in OMP". Spoken approval, later, only after the mini reads the request back |

## Acceptance

**Phase 1 — done when, on this machine, with Null running and no terminal open:**

1. fn+Space shows the mini's text box. The previously active app is still the active app, and no
   space character lands in it.
2. A typed question streams an answer from OMP on the selected model.
3. A typed request to do something is carried out by OMP with its own tools and its normal
   permission mode. What it is doing is visible as it works; if OMP asks, the mini shows the request.
4. The model can be changed in the mini — including to another provider OMP is logged in to — and
   the conversation continues.
5. Esc or fn+Space again hides the mini completely.
6. Dictation-and-paste and the `herdr` agent chord behave exactly as before.
7. **Same agent as in the terminal.** The agent behind the mini has what OMP has in the terminal:
   the same instruction file, skills, MCP servers, default model and thinking level, and the same
   approval behaviour.

**Later phases** add: any of Claude Code, Codex or OpenCode as the brain with the user's own key or
sign-in; continuing a conversation on another harness after a limit; background tasks with Running /
Needs input / Ready / Blocked; a hands-free spoken conversation in the user's cloned voice; the pet.

## Architecture decisions

1. **The brain is a harness, not a model API.** Null drives the user's installed harness headlessly.
   The harness brings its own sign-in, tools, MCP servers, instruction files and skills. This covers
   provider sign-in, switching and heavy work in one move, and keeps Null out of token handling.
2. **One internal `Harness` protocol; the first adapter is a generic ACP client.**
   - OMP speaks the Agent Client Protocol natively (`omp acp`). Verified on this machine: it answers
     an `initialize` in 0.17 s, reports protocol version 1, session list / resume / fork / close,
     image and embedded-context prompts, MCP servers over HTTP, and one auth method — "Use existing
     local credentials".
   - OpenCode speaks it natively too (`opencode acp`), as do dozens of other agents. One adapter, a
     new row per agent.
   - Claude Code and Codex get native adapters later (Claude Agent SDK; `codex app-server`), because
     those report usage-limit windows and reset times, which ACP does not carry.
3. **Provider switching comes from the harness.** OMP has its own provider logins (`omp login`) and
   model catalogue (`omp models`). The mini's model picker lists what OMP can reach, so changing
   provider works in Phase 1 without any second adapter.
4. **The mini is a new window, not a change to the dictation pill.** Label `mini`, a non-activating
   panel on macOS so it can take typing without activating Null. The `dictate` pill and its
   never-focus rule stay as they are.
5. **fn+Space needs a consuming key tap.** The chord engine can see the fn key (`Key::Function`
   exists in keytap and in Null's key tables) but its tap is listen-only, so it cannot swallow the
   Space — every press would type a space into the app underneath. The standard global-shortcut
   plugin is not expected to accept fn as a modifier (item 2 confirms). A small dedicated tap that
   consumes the keystroke is the planned route; it uses the Accessibility permission Null already
   asks for.
6. **Null shows work, it does not gate it.** Tool activity from the harness is rendered as it
   happens. Approval prompts appear only when the harness itself asks.
7. **Two tiers of work (Phase 3).** Conversation on a fast model; heavy work delegated to background
   sessions on any harness, model and effort, tracked on the mini.
8. **Voice is a local cascade (Phase 4)** — turn detection, speech-to-text, the harness,
   text-to-speech — built on Pipecat rather than hand-rolled, speaking in the user's cloned voice.
9. **The pet uses the Codex sprite contract (Phase 5)**, so existing custom pets work unchanged.
   OpenAI's built-in pet images are not bundled.

## Language

`Context/Glossary.md` is empty. Terms this plan introduces, to be settled by `/grill`:

- **Mini** — the floating companion window. **Pet** — the optional sprite shown in it.
- **Harness** — an installed agent CLI Null can drive (OMP, OpenCode, Claude Code, Codex, any ACP
  agent).
- **Mini session** — one conversation between the user and a harness, owned by Null.
- **Task** — a mini session running in the background on delegated work.
- Collisions to resolve: the code already uses "agent" for a `herdr`-visible terminal session,
  "session" for a narration session, and "turn" for a traced voice turn.

## Work items

### Phase 1 — Prove the mini (text, fn+Space, OMP)

Items 1 and 2 are spikes: throwaway code, each ending in a recorded decision. They are independent.

**Order changed (user, 2026-10-08):** "I want to make sure that things are functional before moving
on." After item 3, a working path was built first — item 4, and the core of items 5, 6 and 8 wired to
the prototype box from item 2 — instead of finishing each item in turn. The rest of each item
follows; what is done and what remains is recorded under each.

1. **Spike: drive OMP from another program**
   - What: a script outside the app that opens `omp acp`, creates a session in a scratch directory,
     and records: time to first text for a five-word prompt on OMP's fast model; how finely text
     arrives; how tool use is reported; how model and thinking level are chosen for a session;
     cancelling mid-reply; what an approval request looks like under each `--approval-mode`;
     attaching Null's MCP server to the session; and how a provider error or usage limit is
     reported. Repeat the same checks against `omp --mode rpc`. Run once from a terminal and once
     from a process started by launchd.
   - Why: decides whether the generic ACP route is good enough for OMP or whether its RPC mode is
     needed, and whether OMP answers fast enough to feel like a CLI.
   - Depends on: none.
   - Risk: a launchd-started process cannot find `~/.omp/bin/omp` or reach OMP's stored logins.
     Know it by: the launchd run failing where the terminal run passes.
   - Decision rule: use ACP unless RPC mode offers something Phase 1 needs that ACP lacks (model
     switching mid-session, tool activity, cancel). Record first-text latency either way.
   - Source: verified locally (`omp --help`, `omp acp` handshake) + researched (ACP).
   - Status: Complete (2026-10-08)
   - **Decision: use ACP for OMP.** It covers everything Phase 1 needs. RPC mode was not examined,
     because the decision rule did not require it.
   - **Results** (`scripts/prototype-omp-spike.py`, OMP 18.4.3, default model
     `opencode-go/deepseek-v4.1-flash`):

     | Check | Result |
     |---|---|
     | Start-up | `initialize` 0.16–0.22 s; `session/new` 0.10–0.15 s |
     | First text, new session | 1.4–2.0 s |
     | First text, later turns | 1.2–1.7 s |
     | First text on `openai-codex/gpt-5.6-luna` | 3.5 s (one sample, first call to that provider) |
     | Thinking level `off` vs `high` | No faster on one-word prompts (1.6–2.0 s) |
     | Streaming | ~10–13 characters per chunk; thinking streams separately |
     | Cancel | Prompt returned `cancelled` 0.01 s after `session/cancel`; session usable afterwards |
     | Context per turn | ~7,800 input tokens of OMP's own prompt, cached after the first turn |

   - **What OMP exposes over ACP:**
     - Session config options: `model` (50 choices across the providers OMP is logged in to),
       `thinking` (off, auto, low, high, max), `mode` (default, plan). Changed with
       `session/set_config_option`.
     - Changing the model mid-session to another provider kept the conversation: turn 1 on
       opencode-go, turn 2 on openai-codex recalled a word from turn 1.
     - Tools: `tool_call` (title, kind, status, raw input) then `tool_call_update` (in progress,
       completed or failed, raw output).
     - Permission, tested over seven actions per mode (three shell commands, write, read and edit
       a file, a plain question):
       - Started with no flag: asked before every shell command, and for nothing else. Answering
         "Always allow" once stopped the asking for the rest of that session, including for
         different commands. Whether that carries to a new session was not tested.
       - `--approval-mode yolo`: never asked.
       - `--approval-mode always-ask`: also asked only for shell commands in these tests.
       - Options offered: allow once, always allow, reject, always reject. A rejected call is
         reported as failed and the model says so.
       - This machine's own OMP setting is `tools.approvalMode = yolo`, yet `omp acp` with no flag
         still asked. ACP does not follow the terminal setting unless the flag is passed.
     - `usage_update` after each turn: context used, context size, and cost in USD.
     - Null's MCP server attached to a single session (HTTP, with a custom client-id header) works;
       the model called `voicebox.list_profiles` through it.
     - Sessions persist in OMP. `session/load` in a new process replays the transcript;
       `session/list` filters by working directory. Null needs to keep only an index.
     - Errors arrive as JSON-RPC `-32603` with `data.details`.
   - **Same agent as the terminal?** One question put to OMP both ways — over ACP and through its
     own `omp -p` — asking what it had loaded (the model's own report, not an inspection):

     | | Over ACP | `omp -p` |
     |---|---|---|
     | Global instruction file | Loaded | Loaded |
     | Default model and thinking | Same | Same |
     | Skills | 17 | 19 |
     | MCP servers | None | `voicebox` |

     Over ACP, OMP does not load its own MCP servers; the client must pass them. The two-skill
     difference is unexplained. ACP also offers 66 slash commands; passing them through the text
     box is untested.
   - **launchd:** the same checks passed under the exact environment launchd gives the Null server
     (`PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `HOME`), with OMP started by absolute path. OMP is a
     native executable and needs nothing else on `PATH`. This reproduced the environment; it was not
     a process spawned by launchd itself.
   - **Not established:** what a provider usage limit looks like (none was hit).
   - **On this machine OMP is logged in to** openai-codex, opencode-go and ollama. Anthropic is not
     among them, so Claude is not reachable through OMP today.

2. **Spike: a window that takes typing without taking the app, on fn+Space**
   - What: a scratch `mini` window converted to a non-activating panel with `tauri-nspanel` (v2
     branch). Verify: it accepts keyboard input while the previous app stays active; it shows over
     full-screen apps and on every Space; Esc and click-away hide it. Add an active key tap that
     consumes Space while fn is held and verify: no space reaches the focused app; the existing
     chords still fire; the tap recovers if the system disables it; nothing fires while a password
     field has secure input. Check what macOS's own Globe-key action (input source, emoji, dictation)
     does on fn release for each setting of "Press 🌐 key to".
   - Why: Phase 1 acceptance 1 and 5; architecture decisions 4–5.
   - Depends on: none.
   - Risk: `tauri-nspanel` is a git dependency that swaps the window class and can break on a Tauri
     or macOS update. The Globe-key action may still fire on release. Know it by: the menu bar
     switching to Null on show, or the emoji picker appearing after fn+Space.
   - Decision rule: adopt if all checks pass. If the panel fails, fall back to an always-on-top
     window that restores the previous app's activation on hide. If the Globe action cannot be
     suppressed, document setting it to "Do Nothing" and offer a second default binding.
   - Source: researched (tauri-nspanel) + codebase (`build_dictate_window`, keytap is listen-only).
   - Status: Complete (2026-10-08)
   - **Prototype:** `tauri/src-tauri/src/prototype_mini_spike.rs` and
     `tauri/prototype-mini-spike.html`, active only when the app was started with
     `NULL_MINI_SPIKE=1` (`NULL_MINI_SPIKE=1 bun run dev` from the repo root). It was kept in the
     repo until something replaced it (user's decision, 2026-10-08), and removed the same day once
     the app in `Mini/` had (`MiniApp.md` item 9).
   - **Established so far (2026-10-08):** `tauri-nspanel` v2 compiles and links against this repo's
     Tauri 2.9.5. On launch the window converts to a non-activating panel without crashing, and the
     consuming key tap is created.
   - **Decision: adopt the non-activating panel (`tauri-nspanel` v2) and a dedicated consuming key
     tap for fn+Space.** Observed by the user and in the prototype's log (2026-10-08):

     | Check | Result |
     |---|---|
     | fn+Space shows the box | Yes, every time (seven shows logged) |
     | Space reaches the app underneath | No — swallowed; the user saw no space in Notes |
     | Frontmost app after the box appears | Unchanged: Notes before, Notes 0.4 s after |
     | Typing lands in the box | Yes — five characters received, no leading space |
     | Esc / fn+Space again / click-away hide it | All three work |
     | Key tap disabled by the system | Did not happen during the test |

   - **Globe key:** this Mac has "Press 🌐 key to" set to Do Nothing, so nothing fires on fn
     release. Behaviour under the other settings (input source, emoji, dictation) is untested and
     matters for other people's machines.
   - **Second run (2026-10-08, after the handoff):** the user confirmed fn+Space shows the box over
     the app in use, including over a full-screen app, and Esc, fn+Space again and click-away each
     hide it. The log shows it over Ghostty, Safari and Google Chrome with the frontmost app
     unchanged each time.
   - **Not yet observed:** on another Space; the dictation chord while the tap is running (deferred
     by the user, 2026-10-08); a password field with secure input. Carried into item 7's checklist.
   - **Found on the way:** the Rust build cache still pointed at the project's old folder
     (`~/Developer/Voice`), so any rebuild failed. The affected packages were cleaned and the app
     builds again.

3. **`Harness` protocol and registry**
   - What: `backend/harness/base.py` with a `runtime_checkable` `Harness` Protocol — `info()`,
     `start_session()`, `send()` (async iterator of events), `respond()` (approvals and questions),
     `interrupt()`, `list_models()`, `close()` — and normalized events: `TextDelta`, `MessageDone`,
     `ToolActivity`, `ApprovalRequest`, `StatusChange`, `LimitReached`, `HarnessError`.
     `backend/harness/__init__.py` holds the registry and lazy factories, mirroring
     `backends/__init__.py`. `backend/harness/discovery.py` finds binaries by `shutil.which`, then
     known install paths (`~/.omp/bin`, `~/.opencode/bin`, `~/.local/bin`, `/opt/homebrew/bin`), then
     a user-set path.
   - Why: the mini, and later the voice loop and tray, must never depend on a harness's transport.
   - Depends on: item 1.
   - Risk: the event set is too thin for one harness's tool reporting. Know it by: an adapter
     needing a side channel.
   - Source: inferred from codebase (engine Protocol pattern) + researched interfaces.
   - Status: Complete (2026-10-08)
   - **Built:** `backend/harness/base.py`, `__init__.py`, `discovery.py` and
     `backend/tests/test_harness.py`.
   - **Deviations:** `set_model()` added to the protocol, because acceptance 4 needs a
     mid-conversation model change and the listed methods had none. A user-set path is tried
     first, not last, so it can override a wrong auto-detected binary. `/usr/local/bin` added to
     the install paths. `MessageDone` carries a stop reason (completed, cancelled, truncated,
     refused). `HarnessUnavailableError` added for a harness that cannot be started.

4. **ACP adapter, with OMP as its first row**
   - What: `backend/harness/acp.py` on the official `agent-client-protocol` Python SDK: spawn the
     agent, `initialize`, authenticate with the agent's "existing local credentials" method, create
     or load a session with a working directory, send prompts, map session updates to the events in
     item 3, answer permission requests, cancel. A launch table whose first row is `omp acp`.
     `info()` reports installed, version and whether credentials are present, with the exact fix
     command (`omp login`) when they are not.
     To match the terminal: start OMP with `--approval-mode` set to its own configured
     `tools.approvalMode`, and pass the servers from `~/.omp/agent/mcp.json` in `session/new` —
     over ACP, OMP follows neither on its own.
   - Why: Phase 1 acceptance 2–4 with one adapter that also covers OpenCode later.
   - Depends on: item 3.
   - Risk: the SDK must be added to `backend/requirements.txt` and the PyInstaller bundle; ACP has
     two protocol versions in the wild. Know it by: negotiating the version at `initialize` and a
     test against the installed OMP.
   - Source: verified locally + researched (ACP Python library).
   - Status: In progress — adapter built and proven against the installed OMP (2026-10-08)
   - **Built:** `backend/harness/acp.py` on `agent-client-protocol` 0.12.1 (added to
     `backend/requirements.txt`; installing it changed no other package), the `omp` row in the
     registry, and `backend/tests/test_harness_acp.py` (a fake connection, no agent started).
   - **Proven live**, under the environment launchd gives the server:

     | Check | Result |
     |---|---|
     | Plain question | Answered; first text 0.9–1.3 s |
     | Shell command | Ran with no approval request — the approval mode carried over |
     | MCP servers | OMP reported `voicebox`, the same server it has in a terminal |
     | Interrupt | Reply ended `cancelled`; the next message was answered |
     | Agent restarted | The same session was loaded again and recalled an earlier turn |

   - **How terminal parity is reached:** the approval mode is read with
     `omp config get tools.approvalMode` (it is not in `~/.omp/agent/config.yml` on this machine;
     the answer is `yolo`) and passed as `--approval-mode` before `acp`. The servers in
     `~/.omp/agent/mcp.json` are passed in `session/new`, limited to the transports the agent
     says it supports.
   - **Learned about the SDK:** it starts the agent with a trimmed environment (`HOME`, `PATH` and
     a few more), which is enough for OMP. The agent's stderr must be drained or it can block. The
     default 64 KiB line limit is raised, because one message is one line.
   - **Remaining:** `info()` cannot yet say whether OMP is signed in, so it gives no fix command;
     the live check was a scratch script, not the opt-in test the plan calls for; the PyInstaller
     bundle is unchecked; the shape of a usage limit is still unknown, so `LimitReached` is never
     emitted.

5. **Mini sessions: service, storage, API**
   - What: `backend/services/mini.py` owning `MiniSession` (harness, harness session id, workspace,
     model, status, title). Table `mini_sessions` and singleton `mini_settings`, added by a
     `_migrate_mini_tables` helper in `database/migrations.py`. `backend/routes/mini.py`, registered
     in `routes/__init__.py`: `GET /harnesses`, `GET|PUT /mini/settings`, `POST|GET /mini/sessions`,
     `POST /mini/sessions/{id}/messages`, `GET /mini/sessions/{id}/events` (SSE),
     `POST /mini/sessions/{id}/respond`, `POST /mini/sessions/{id}/interrupt`,
     `PATCH /mini/sessions/{id}`. The harness keeps the transcript; Null keeps only the index.
     Default workspace: a scratch directory under the data dir.
   - Why: one brain path that text, and later voice and tasks, all use.
   - Depends on: items 3, 4.
   - Risk: path collisions with app routes — the web build already has one (`/settings/captures`).
     Know it by: keeping every backend path under `/mini/` and `/harnesses`, and the app's settings
     page at `/settings/mini`.
   - Source: inferred from codebase (routes → services layering, migration pattern, singleton rows).
   - Status: In progress — the conversation path is built; storage and settings are not
   - **Built (2026-10-08):** `backend/services/mini.py`, `backend/routes/mini.py`, request models
     in `backend/models.py`, `backend/tests/test_mini.py`. Routes: `GET /harnesses`,
     `POST /mini/sessions`, `GET /mini/sessions/{id}`, `PATCH /mini/sessions/{id}` (model),
     `GET …/models`, `POST …/messages`, `GET …/events`, `POST …/respond`, `POST …/interrupt`.
     Harnesses are stopped when the server shuts down.
   - **Model switching (2026-10-08, acceptance 4):** `PATCH` moves an open conversation to any
     model OMP lists, on any provider it is signed in to; `POST /mini/sessions` accepts a model
     to start on. Proven through the running server: a conversation started on opencode-go,
     moved to openai-codex and back, and recalled a word from its first turn each time. The
     choice belongs to the session: OMP's own default is untouched and a new session starts on
     it unless a model is asked for. A change is refused while a reply is in flight, and an
     unknown model is a 400. A session asked to start on a model that is no longer offered
     opens on the default instead of failing.
   - **Proven through the running server:** asked to create a file, OMP created
     `data/mini/workspace/hello.txt` and replied.
   - **How events work:** a reply runs as a background task, so hiding the mini does not stop it.
     `POST …/messages` returns a cursor; `GET …/events?after=N` streams everything after it and
     ends when the session is idle and the reader has caught up. A reader that comes back later
     resumes from its cursor. A heartbeat goes out every 15 s of silence.
   - **Deviations:** sessions are held in memory, keyed by the harness's own session id. A server
     restart forgets them (OMP keeps the transcripts). The events stream ends with each reply; it
     is not one long-lived stream.
   - **Remaining:** the `mini_sessions` and `mini_settings` tables and their migration,
     `GET|PUT /mini/settings` (including the default model, which today only the prototype box
     remembers), `GET /mini/sessions`, a session title.
   - **Noticed:** in development the data directory is inside this repo, so the default
     workspace is too, and OMP may read this repo's project instructions from there.
   - **After ADR 0002:** the Null app in `Mini/` does not use these endpoints; it keeps its own
     settings file. What remains here is not needed by that app. Whether the Voice server needs it
     is a question for the voice-mode re-plan.

6. **Lock the new endpoints down**
   - What: a per-install secret generated on first run (data dir, mode 0600), required as a bearer
     token on `/mini/*` and `/harnesses`, handed to the webviews by the Tauri host. Refuse these
     routes for non-loopback callers even when the server is started in remote mode.
   - Why: these endpoints run commands on the machine through a harness. The server has no auth
     today, and any local process can already reach it. It must be in before the repo is offered to
     others.
   - Depends on: item 5.
   - Risk: the token breaks the dev flow or the web build. Know it by: a dev bypass that is off by
     default and a test that an unauthenticated call is rejected.
   - Source: inferred from codebase (no auth; `startServer(remote?)` exists).
   - Status: In progress — the lock is on; handing the secret to the real windows is not built
   - **Built (2026-10-08):** `backend/services/mini_auth.py`. A secret is created at server start
     in `<data dir>/mini-token` (mode 0600). Every `/mini/*` route and `/harnesses` requires it as
     a bearer token and refuses callers that are not on this machine. Checked on the running
     server: no token and a wrong token both get 401; a browser preflight from an unlisted
     origin is refused.
   - **Remaining:** the Tauri host handing the secret to the webviews. The prototype reads the
     development data directory at a path fixed when it was compiled, which will not work in a
     packaged build. No dev bypass was built; none has been needed.
   - **After ADR 0002:** the Null app has no network endpoint and needs no secret. The only window
     that used this lock was the prototype, which `MiniApp.md` item 9 removes. Nothing else needs
     the secret handed to a window today.

7. **The `mini` window and its shortcut (Rust)**
   - What: `build_mini_window` beside `build_dictate_window` in `main.rs` (label `mini`, URL
     `?view=mini`, transparent, undecorated, all workspaces, hidden at setup), converted to a
     non-activating panel on macOS. Add `mini` to `capabilities/default.json`. A `mini_shortcut.rs`
     module holding the consuming key tap from item 2; the binding comes from `mini_settings`, is
     rebindable and can be turned off. Commands `show_mini`, `hide_mini`, `update_mini_shortcut`.
     Emits `mini:open`. Persist the window position.
   - Why: Phase 1 acceptance 1 and 5.
   - Depends on: item 2.
   - Risk: focus and Spaces behaviour differ between dev and a packaged build. Know it by: running
     item 2's checklist again on a built app.
   - Source: inferred from codebase + spike 2.
   - Status: Superseded by `MiniApp.md` item 2: the window and the shortcut live in `Mini/`, not in
     the desktop app.

8. **The mini's text UI — a compact CLI**
   - What: `app/src/components/Mini/` — `MiniWindow` (mounted from `App.tsx` on `?view=mini`, the
     same branch pattern as `?view=dictate`), `MiniComposer` (Enter sends, Shift+Enter newline, Esc
     hides, up-arrow recalls the last prompt), `MiniTranscript` (streamed reply, rendered markdown,
     one line per tool action as it happens), `MiniApproval` (shown only when the harness asks), a
     harness-and-model chip that opens the model list, and `useMiniSession`. New client methods and
     types in the hand-written `lib/api/client.ts` and `types.ts`. Strings in all nine locales.
   - Why: Phase 1 acceptance 2–4; "very small and compact".
   - Depends on: items 5, 7.
   - Risk: the window must grow with the reply without covering the user's work. Know it by: a
     height cap with scroll.
   - Source: specified from user + codebase (`DictateWindow`, `CapturePill`).
   - Status: Superseded by `MiniApp.md` item 6: one static page in `Mini/Page/`, not React
     components in `app/`. The look and behaviour recorded below carried over.
   - **Prototype (2026-10-08):** `tauri/prototype-mini-spike.html` now sends on Enter and shows
     the streamed reply, one line per tool action, approval buttons, and errors. Ctrl+C stops a
     reply, `/new` starts a new conversation, up-arrow recalls the last prompt, and the window
     grows with the reply. Its script was run against the live server with a stand-in page
     (question, task, interrupt) and behaved. The user tried it on screen and confirmed it works
     ("This is perfect", 2026-10-08). It renders plain text, not markdown.
   - **Model picker in the prototype (2026-10-08):** `/model`, `/model <filter>` or a click on
     the status line lists the models by provider with the current one marked. Typing filters,
     the arrows move, Enter picks, Esc cancels. The pick is remembered in the box's local
     storage and new conversations start on it. Run against the live server with a stand-in
     page; on screen it is unverified until the user tries it.
   - **Look (2026-10-08, at the user's request):** idle, the box shows only the arrow and the
     text field. The arrow is the status: it pulses while the agent works and turns amber while
     it waits for an answer. Words appear only for something the user must know (for example a
     message sent while a reply is still running) and disappear after four seconds. The real UI
     should keep this.
   - **Palette (user, 2026-10-08):** "the primary colors are black and white": a dark fill with a
     white border, arrow and highlights, no purple. Amber (the agent is asking) and red (errors)
     remain as the only other colours. The box can be dragged by the arrow, the border, the empty
     part of the field or any empty space, and has a clear border.

9. **Mini settings page**
   - What: `components/ServerTab/MiniPage.tsx` at `/settings/mini` in `router.tsx`: each harness with
     installed / signed-in state and the command to fix it, default harness and model, default
     workspace, the shortcut.
   - Why: the user must see why a brain is unavailable and change it without a terminal.
   - Depends on: item 5.
   - Risk: an already dense settings area. Know it by: one row per harness, three facts each.
   - Source: inferred from codebase (`ServerTab/MCPPage.tsx`).
   - Status: Superseded for now. `MiniApp.md` decided on typed commands in the box and no settings
     page. The box reports a harness it cannot find as an error when a message is sent; whether the
     harness is signed in is shown nowhere.

### Phase 2 — Other people's harnesses, keys and sign-in

10. **OpenCode on the ACP adapter**
    - What: a second launch-table row, `opencode acp`, with a live test against the installed
      OpenCode (1.18.33). OpenCode's own provider sign-in and keys apply.
    - Why: the open-source audience; most providers by key, ChatGPT plans by sign-in.
    - Depends on: item 4.
    - Risk: differences in how OpenCode reports tools or permissions over ACP. Know it by: the test.
    - Source: researched (OpenCode ACP docs).
    - Status: Not started

11. **Codex adapter**
    - What: `backend/harness/codex.py` — spawn `codex app-server`, `initialize`, `thread/start`,
      `turn/start`, `item/agentMessage/delta`, the approval requests, `turn/interrupt`, `model/list`,
      `account/read`, `account/rateLimits/*`. Sign-in through Codex's own flow
      (`account/login/start`, ChatGPT or API key).
    - Why: the open-source audience; Codex reports percent of limit used and reset time.
    - Depends on: item 3.
    - Risk: OpenAI permits this sign-in for local and open-source apps only. Know it by: stating
      that limit in the README; a commercial fork would need Sign in with ChatGPT instead.
    - Source: researched (app-server docs, including the permitted-use statement).
    - Status: Not started

12. **Claude adapter**
    - What: `backend/harness/claude.py` on `claude-agent-sdk` (`ClaudeSDKClient`, partial messages,
      `can_use_tool` bridged to `ApprovalRequest`, `RateLimitEvent` bridged to `LimitReached`).
    - Why: the open-source audience.
    - Depends on: item 3.
    - Risk: **policy.** For other people, Null may offer Claude by API key only; offering Claude
      subscription sign-in needs Anthropic's approval. Know it by: the README and the settings page
      naming API key as the supported route. See Open decisions.
    - Source: researched (Agent SDK overview note).
    - Status: Not started

13. **Limits as a first-class state**
    - What: every adapter maps its limit signal to `LimitReached` with the window and reset time
      when known. The mini shows Blocked with the reason and a "continue on …" action.
    - Why: the problem this project exists to solve.
    - Depends on: items 4, 11, 12.
    - Risk: a limit arriving as a generic error on some route. Know it by: item 1's capture of
      OMP's real failure shape, repeated per adapter.
    - Source: researched (`RateLimitEvent`, `account/rateLimits/updated`).
    - Status: Not started

14. **Carry a conversation across harnesses**
    - What: add `mini_messages` (a text mirror of each turn). `POST /mini/sessions/{id}/handoff`
      starts a session on the target harness in the same workspace, seeded with recent turns
      verbatim and older turns summarized. OMP can also import Claude Code and Codex sessions
      directly (`--from-claude`, `--from-codex`); use that where it applies.
    - Why: harness sessions are not portable; Null's own transcript is the bridge.
    - Depends on: items 5, 10–12.
    - Risk: the new harness lacks context the old one built through tool use. Know it by: the
      handoff note stating what was carried and what was not.
    - Source: inferred + verified locally (`omp --help`).
    - Status: Not started

### Phase 3 — Heavy work in the background

15. **Tasks and delegation**
    - What: a task is a mini session in background mode. Add MCP tools to `mcp_server/tools.py` —
      delegate, list, status, reply, stop — so a conversation can start work on any harness, model,
      effort and workspace and check on it. The user can start a task directly too.
    - Why: long work should not hold the text box hostage.
    - Depends on: item 5.
    - Risk: tasks spawning tasks. Know it by: a depth limit of one and a cap on concurrent tasks.
    - Source: inferred + reference product.
    - Status: Not started

16. **Activity tray and status**
    - What: Running / Needs input / Ready / Blocked with the reference product's priority (Needs
      input, Blocked, Ready, Running), shown on the mini and in a tray of sessions. A `GET
      /events/mini` stream and a `mini_monitor` modelled on `speak_monitor.rs`, because a hidden
      WebKit window drops long-lived connections. "Open full conversation" opens `/mini/$id` in the
      main window.
    - Why: following work while doing something else.
    - Depends on: items 8, 15.
    - Risk: status drifting from the harness's truth after a crash. Know it by: reconciling on
      reconnect and marking orphaned sessions Blocked at startup, as generations are marked failed.
    - Source: researched (`ChatGPTPetsAndMini.md`) + codebase (stale-generation cleanup).
    - Status: Not started

17. **Workspaces**
    - What: a `mini_workspaces` list (path, name, last used). `@` in the text box selects one. The
      session's working directory is what gives a harness its project instructions and tools.
    - Why: the reference product's quick chat has no project context — a reported limitation.
    - Depends on: item 5.
    - Risk: starting a harness in the wrong directory. Know it by: the mini always showing the
      workspace beside the model chip.
    - Source: researched (reported limitation) + inferred.
    - Status: Not started

### Phase 4 — Voice (deferred until Phase 1 is proven)

**Prerequisite:** this install has no voice profile — 0 profiles and 0 samples in
`data/voicebox.db` as of 2026-10-08. The cloning system is present; the user's cloned voice must be
re-created or restored before this phase.

**Honest limit:** ChatGPT's voice uses a speech-native, full-duplex model. Nothing equivalent exists
for Claude or through a harness. Null's voice mode is a local cascade: natural pauses and
interruption, yes; sub-second replies and spoken "mhmm", not in the first version. The voice layer
costs no provider usage.

18. **Spike: hearing the user over the mini's own voice**
    - What: in a scratch Tauri window, open the mic with `echoCancellation: true` while playing
      speech at laptop-speaker volume through Web Audio, an `<audio>` element and a WebRTC track.
      Measure residual echo with the user silent, and whether the user's speech survives talk-over.
    - Why: interruption on speakers. Decides the voice transport.
    - Depends on: none.
    - Risk: WKWebView does not cancel its own page audio (one third-party report says so).
    - Decision rule, in order: WebRTC to the backend; Web Audio plus a WebSocket; native capture and
      playback with macOS voice processing; half-duplex as the floor.
    - Source: researched (reports conflict; must be measured).
    - Status: Not started

19. **Spike: conversation speech engines on this machine**
    - What: speech-to-text — Moonshine, Parakeet, today's Whisper `small` — scored on delay and
      accuracy over 10–20 recorded utterances. Text-to-speech in the user's cloned voice — Pocket TTS
      against LuxTTS — scored on time to first audio and a listening check of likeness.
    - Why: the cloned voice is required; LuxTTS takes 2.1–2.5 s to first audio, Pocket TTS claims
      ~200 ms on CPU.
    - Depends on: a restored voice profile.
    - Risk: MLX is known-bad here; Pocket TTS on an M1 is slower than its published M4 figures.
    - Decision rule: the fastest pair that transcribes the test set correctly and whose clone the
      user accepts by ear.
    - Source: researched + measured facts from `VoiceLoopLatency.md`.
    - Status: Not started

20. **Make worker roles launchable in packaged builds**
    - What: add `--role` to `server.py`'s argument parser and set `VOICEBOX_ROLE` from it before the
      app is imported.
    - Why: the packaged narration worker cannot start today; the voice loop lives in that worker.
    - Depends on: none.
    - Source: inferred from codebase.
    - Status: Not started

21. **The conversation loop**
    - What: `backend/voice/pipeline.py`, a Pipecat pipeline in the narration worker process:
      transport from item 18 → Silero VAD → Smart Turn v3 (local) → speech-to-text from item 19 →
      `MiniBrainProcessor` (posts the utterance to the mini session API, streams the reply, calls
      `/interrupt` on barge-in) → speakable-text filter → text-to-speech from item 19 → transport.
      A voice-mode instruction block keeps replies short and unread code off the speaker.
    - Why: a hands-free conversation with pauses and interruption, on the same brain path as text.
    - Depends on: items 5, 18, 19, 20.
    - Risk: Pipecat's API moves between releases and must coexist with the backend's pins
      (`numpy<2`, torch 2.14). Know it by: a dry install in a copy of the venv before any code.
    - Source: researched (Pipecat docs, Smart Turn README).
    - Status: Not started

22. **Pocket TTS as an engine** (only if item 19 selects it)
    - What: a `pocket` engine added by the repo's `add-tts-engine` skill, with a per-profile voice
      state cached under the data dir. Available to the narration lane as well.
    - Depends on: item 19.
    - Source: researched + codebase (`.agents/skills/add-tts-engine`).
    - Status: Not started

23. **Voice layout, shortcut and safeguards**
    - What: a second shortcut opens the mini with a pill whose bars follow the real microphone
      level, with listening / thinking / speaking states and live captions. A readiness gate blocks
      voice mode until models are downloaded. Harness sessions started by the mini identify as
      client `null-mini`, and `voicebox.speak` from that client is a no-op, so nothing is said twice.
      A spoken approval is accepted only after the mini has read the request aloud.
    - Depends on: items 7, 8, 21.
    - Source: specified from user + codebase (`CapturePill`, `ClientIdMiddleware`).
    - Status: Not started

24. **Trace the conversation**
    - What: stages in `utils/timing.py` for voice turns — `speech_end`, `turn_confirmed`,
      `stt_final`, `brain_first_token`, `tts_first_audio`, `audio_start` — under one id per turn.
    - Why: the latency target must be measured, not felt.
    - Depends on: item 21.
    - Source: inferred from codebase (`timing.py`, `turnTrace.ts`).
    - Status: Not started

### Phase 5 — The pet (any time after item 8)

25. **Sprite renderer**
    - What: `components/Mini/PetSprite.tsx` implementing the atlas contract with the documented row
      timings. Mapping: idle → `idle`, Running → `running`, Needs input → `waiting`, Ready →
      `review`, Blocked → `failed`, dragging → `running-left` / `running-right`. A still first frame
      under reduced motion.
    - Depends on: item 8.
    - Source: researched (`ChatGPTPetsAndMini.md`, sprite contract).
    - Status: Not started

26. **Pet loading and the Mini option**
    - What: `GET /pets` lists pets from `<data>/pets/` and, read-only, `~/.codex/pets/`, validating
      `pet.json` and the sheet's dimensions. Settings offer a pet or "Mini" (no sprite), a size, and
      visibility: always, only while active, only on shortcut.
    - Depends on: item 25.
    - Source: researched (pet package format; complaints about a non-hideable overlay).
    - Status: Not started

## Verification

- **Per spike:** the decision rule's measurements, written into this plan under the item.
- **Backend:** pytest for `Harness` event normalization with a fake harness, the mini session state
  machine, the auth gate (item 6), and the migration. None need a GPU or a live provider.
- **Adapters:** one opt-in live test per harness, skipped when its CLI is absent.
- **Phase 1 end to end:** the seven acceptance checks, run on a packaged build, not only `bun dev`.
- **Same agent as the terminal:** the same question to the mini's session and to `omp -p` must
  report the same instruction file, MCP servers and skills.
- **Regression:** dictation-and-paste and the `herdr` chord, by hand, before and after.

## Validation flags

- **Amends a standing decision (Phase 4).** "The pill is the only playback surface for agent speech"
  becomes "…for speech requested through `voicebox.speak`". Needs an ADR before voice work starts.
- **Both spike-dependent choices are settled:** the adapter transport is ACP (item 4), and the
  shortcut is a dedicated consuming key tap on a non-activating panel (item 7).
- **Measured:** OMP returns first text in 1.2–2.0 s on its default fast model, and 3.5 s in one
  sample on a ChatGPT model. That delay is the provider's, not Null's; the mini will feel like a
  CLI with a short pause, not like instant typing.
- **New dependency in the backend:** `agent-client-protocol` 0.12.1, installed into the pinned
  environment without changing any other package. Not yet checked in the PyInstaller bundle.
- **No ADR or glossary exists** to hold this plan to; the terms in "Language" are proposals.

## Open decisions (user-owned)

1. **Claude for other people.** By API key it is straightforward. Offering Claude subscription
   sign-in to others needs Anthropic's approval. Recommended: document API key as the supported
   route and ask Anthropic if subscription sign-in matters to you.
2. **The fn key.** If macOS's Globe-key action cannot be suppressed, are you willing to set it to
   "Do Nothing", or would you rather have a different default shortcut?
3. **Voice (deferred):** the shortcut; whether local-only is a hard rule; the latency target.
4. **The pet (deferred):** Mini with no sprite first, a Codex-format pet you already have, or new art.
5. **Screen context (deferred):** "look at this" → a screenshot of the front window attached to the
   prompt, or left to whatever the harness offers.

## Out of scope

- Renaming Voicebox to Null in the bundle id, binaries and updater endpoint. The updater still
  points at upstream releases and must be dealt with before the app is distributed.
- Windows and Linux. The window and shortcut work is macOS-first; the backend is portable.
- Attaching the mini to a terminal session that is already running (the `herdr` path keeps that).
- Changing the dictation pill, the narration lane's contract, or the generation queue.

## Evidence

- Verified on this machine, 2026-10-08: `omp` 18.4.3 at `~/.omp/bin/omp` with `acp`, `--mode rpc`,
  `login`, `models`, `--approval-mode`, `--from-claude`, `--from-codex`; the ACP handshake result in
  Architecture decision 2; keytap 0.4.0 defines `Key::Function` and taps listen-only; the running
  server reports 0 voice profiles.
- Claude Agent SDK: [overview and policy note](https://code.claude.com/docs/en/agent-sdk/overview),
  [Python reference](https://code.claude.com/docs/en/agent-sdk/python),
  [headless mode](https://code.claude.com/docs/en/headless).
- Codex: [app-server](https://learn.chatgpt.com/docs/app-server) (protocol, sign-in modes, the
  statement on permitted use, rate limits),
  [Sign in with ChatGPT](https://learn.chatgpt.com/docs/sign-in-with-chatgpt).
- OpenCode: [server](https://opencode.ai/docs/server), [ACP](https://opencode.ai/docs/acp),
  [providers](https://opencode.ai/docs/providers).
- Agent Client Protocol: [agents](https://agentclientprotocol.com/get-started/agents),
  [Python library](https://agentclientprotocol.com/libraries/python).
- Voice loop: [Pipecat Smart Turn](https://docs.pipecat.ai/server/utilities/smart-turn),
  [Smart Turn v3 model](https://github.com/pipecat-ai/smart-turn),
  [Pocket TTS](https://github.com/kyutai-labs/pocket-tts).
- Overlay: [tauri-nspanel](https://github.com/ahkohd/tauri-nspanel).
- Echo cancellation in a Tauri webview: one
  [third-party report](https://claudeskills.info/skills/aiskillstore/marketplace/voice-system-expert/)
  of failure; unverified, hence item 18.
- Measured on this machine: `Context/Plans/VoiceLoopLatency.md`.
