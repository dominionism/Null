# Null Mini as its own app

> Blueprint: 2026-10-08. Builds on `Context/Plans/NullMini.md` (the constraints, the spike evidence
> and what the prototype proved) and `Context/ADR/0002-NullMiniIsItsOwnApp.md`.
>
> Status: **Items 1 to 6 are built. The whole path works in the real app without the Voice server;
> nothing has been tried by hand yet. Item 7 (bundle, install, start at login) is next.**

## Goal

Build Null Mini as a small macOS app in `Mini/` that opens on fn+Space, takes typing, and drives the
user's own agent harness (Oh-my-pi first) directly, with no dependence on the Voice desktop or its
server.

## Constraints

- Everything in `Context/Plans/NullMini.md` → "Constraints" and "Decisions made" still holds. The ones
  that shape this build:
  - **Verbatim (user):** "fn + space shouldn't be dependent on null desktop." "These two should be
    separate for now."
  - **Verbatim (user):** "All I really need to see is the text box and the arrow." No placeholder, no
    status line, no hints.
  - **Verbatim (user):** "the primary colors are black and white." White border and arrow, dark fill.
  - Null never reads, stores or forwards a provider token. It starts the user's installed harness.
  - Approvals mirror the harness's own setting (`yolo` on this machine). Null adds no approval layer.
  - Same agent as in the terminal: instruction file, skills, MCP servers, default model.
  - The window never takes app activation.
- Folder names follow `Context/ADR/0001-CapitalizedFolderNames.md`.
- The Voice project is not touched by this build, except removing the prototype at the end (item 9).
- Machine: Apple M1, 16 GB. A login-launched app gets a minimal `PATH`; the harness is found by
  absolute path.

## Shape

1. **One process, no server.** The app is a Tauri 2 app whose Rust side owns the window, the shortcut
   and the connection to the harness. The page talks to it through Tauri commands and events, not
   HTTP. There is no network endpoint, so the lock and its secret are not needed.
2. **No web build.** The box is one static page (`Mini/Page/index.html`, inline CSS and JS), served
   by Tauri directly. No Node, no bundler, no dev server.
3. **The page keeps its event vocabulary.** The Rust side emits the same events the backend's
   `/mini` stream sends today (`user_message`, `text_delta`, `tool_activity`, `approval_request`,
   `status_change`, `model_changed`, `message_done`, `error`), so the page's `show()` and all of its
   look carry over unchanged. Only its transport changes.
4. **The harness connection is rebuilt in Rust**, with the behaviour proven in
   `backend/harness/acp.py`: approval mode read from `omp config get tools.approvalMode` and passed as
   `--approval-mode`; the servers in `~/.omp/agent/mcp.json` passed in `session/new`; model list and
   switching through the session's `model` config option; a session loaded again after the agent
   process restarts.
5. **The harness keeps the transcript.** The app stores only a small settings file: last session id,
   chosen model, window position.
6. **A background app.** No Dock icon. It starts at login.

Proposed layout (to be confirmed by item 1):

```text
Mini/
├── Cargo.toml, build.rs, tauri.conf.json
├── capabilities/        tool-fixed name
├── src/                 tool-fixed name (Rust)
│   ├── main.rs
│   ├── panel.rs         window as a non-activating panel, sizing, position
│   ├── shortcut.rs      the consuming fn+Space key tap
│   ├── harness.rs       ACP connection to the harness
│   └── settings.rs
├── Page/index.html      the box
├── Icons/
└── Scripts/             build, install
```

## Work items

1. **Scaffold `Mini/` and prove the layout**
   - What: a minimal Tauri 2 app in `Mini/` with the crate and `tauri.conf.json` at the folder's top
     level, `frontendDist` pointing at `Page/`, a window that shows a static page. Confirm the Tauri
     CLI finds the config without a `src-tauri` folder and that `Page/`, `Icons/`, `Scripts/` work
     capitalized.
   - Why: ADR 0001; everything else builds on this.
   - Depends on: none.
   - Risk: low. The Tauri CLI finds a project by its `tauri.conf.json`, not by a folder named
     `src-tauri` (established during the folder-rename groundwork). Know a problem by: `tauri dev`
     failing to find the app.
   - Source: researched (Tauri CLI behaviour) + inferred.
   - Status: Complete (2026-10-08)
   - **Built:** `Mini/Cargo.toml`, `build.rs`, `tauri.conf.json`, `capabilities/default.json`,
     `src/main.rs`, `src/log.rs`, `Page/index.html` (a placeholder until item 6), `Icons/`.
   - **Proven:** the app builds and, run on its own with no dev server and no Node tooling, loads
     its page from `Page/` and the page calls back into it. `tauri info` run in `Mini/` finds the
     project with the config at the folder's top level. `cargo tauri` (the CLI, 2.x) is installed
     for bundling. The app resolved Tauri 2.12.1; the prototype was on 2.9.5.
   - **For checks without a person:** `NULL_MINI_EXIT_WHEN_READY=1` makes the app quit once its
     page has loaded. It logs to `~/Library/Logs/Null Mini/mini.log`.

2. **Window and shortcut**
   - What: move the proven code from `tauri/src-tauri/src/prototype_mini_spike.rs` into
     `src/panel.rs` and `src/shortcut.rs`: non-activating panel (`tauri-nspanel` v2), shown over
     full-screen apps and on every Space, hidden on Esc, fn+Space again or click-away; the active key
     tap that swallows Space while fn is held and re-enables itself; resize keeping the top-left
     corner; `accept_first_mouse`. Set the app's activation policy to accessory (no Dock icon).
     Remember the window position in settings.
   - Why: acceptance 1 and 5 of `NullMini.md`; "hard to move around" should stay fixed across restarts.
   - Depends on: 1.
   - Risk: behaviour differs in a built app from `tauri dev`. Know it by: running the checklist in
     item 8 on the installed build.
   - Source: codebase (prototype, proven on screen by the user).
   - Status: In progress — built and starting correctly; not yet tried by hand
   - **Built (2026-10-08):** `src/panel.rs`, `src/shortcut.rs`, `src/settings.rs`. `tauri-nspanel`
     is pinned to the revision the prototype was proven with and compiles against Tauri 2.12.1.
     The page asks through commands (`hide_box`, `resize`) and hears `mini:shown`.
   - **Proven by start-up runs:** the window becomes a panel, the key tap is created, the page
     loads. Six unit tests pass (where a saved position is still reachable; settings round trip).
   - **Not proven:** everything a person has to do — fn+Space, typing, hiding, dragging, position
     remembered. That is item 8's checklist. `NULL_MINI_NO_SHORTCUT=1` runs the app without the
     key tap, so it can be tried while the prototype still owns fn+Space.

3. **Accessibility permission**
   - What: on start, check `AXIsProcessTrusted`. If not trusted, ask macOS to show its prompt once,
     show one line in the box saying what to do, and create the key tap when the grant arrives.
   - Why: a standalone app needs its own grant; today the terminal's grant is borrowed.
   - Depends on: 2.
   - Risk: an unsigned build loses its grant on every rebuild. See item 7.
   - Source: inferred.
   - Status: In progress — written in `src/shortcut.rs`; the missing-permission path is untested
   - **Note:** run from a terminal the app borrows the terminal's grant, so the prompt, the wait
     and the notice in the box (`mini:permission`, to be shown by the page in item 6) can only be
     tried on an installed build.

4. **Harness connection in Rust**
   - What: `src/harness.rs`: find the harness binary (user-set path, then `PATH`, then
     `~/.omp/bin`, `~/.opencode/bin`, `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`); spawn
     `omp [--approval-mode <mode>] acp`; `initialize`; `session/new` or `session/load` with the MCP
     servers; `session/prompt` streaming updates; answer `session/request_permission`;
     `session/cancel`; `session/set_config_option` for the model; drain stderr; reload a session
     after a restart. Translate updates into the events in "Shape" 3.
   - Why: the brain, without the Voice server.
   - Depends on: 1.
   - How: the official `agent-client-protocol` crate, version 3, with its `process` feature. It
     spawns the agent (`AcpAgent::from_args`), and a client is built with handlers for session
     updates and permission requests, then given one long-lived task that sends the requests
     (`InitializeRequest`, `NewSessionRequest`, `LoadSessionRequest`, `PromptRequest`,
     `SetSessionConfigOptionRequest`). That task takes its orders from a channel fed by the app's
     commands. A permission request's responder is held until the user answers.
   - Risk: sending a cancel or a model change while a prompt is awaiting its reply needs the
     crate's non-blocking request form, which was not read in detail. Know it by: Ctrl+C not
     stopping a reply in the live test. Fallback: speak JSON-RPC over stdio directly (the app uses
     seven methods, and `scripts/prototype-omp-spike.py` shows each message).
   - Source: codebase (`backend/harness/acp.py`, proven live) + researched (the crate's published
     source, 3.2.0: README quick start, `examples/yolo_one_shot_client.rs`, `src/session.rs`).
   - Status: Complete (2026-10-08), apart from the hands-on checks in item 8
   - **Built:** `src/harness.rs` (the connection) and `src/translate.rs` (protocol JSON to the
     box's events, model list, MCP entries, finding the CLI; 14 unit tests). Requests are built
     from the protocol's JSON and replies read as JSON, so the code follows the wire format, not
     the crate's type names.
   - **Proven live with `NULL_MINI_SMOKE`**, Voice server not involved:

     | Check | Result |
     |---|---|
     | Start | `omp --approval-mode yolo acp`, found at `~/.omp/bin/omp`; harness ready |
     | A question | Answered in 1.9 s |
     | A shell command | Ran with tool events and no approval request |
     | MCP servers | OMP reported `voicebox`, as in a terminal |
     | Stop after 3 s | Reply ended `cancelled` at 3.1 s. The open risk (cancel while a prompt is in flight) did not materialize |
     | App restarted | The saved conversation was loaded back and recalled a word from its first turn |
     | Another provider | 50 models listed; moved to `openai-codex/gpt-5.6-luna` and answered |

   - **Not established:** what a usage limit looks like (still never hit); the approval path with
     a real permission request (OMP runs `yolo` here; covered only by the Python tests' logic).

5. **Commands and events between page and app**
   - What: commands `send`, `interrupt`, `respond`, `models`, `set_model`, `new_conversation`,
     `events_since`, `hide`, `resize`; one event channel carrying the events in "Shape" 3 with a
     running number, kept in a bounded buffer so the page can catch up after a reload. One reply at a
     time per conversation; a model change is refused while a reply is in flight.
   - Why: replaces HTTP and the lock.
   - Depends on: 4.
   - Risk: an event missed while the window is hidden. Know it by: hiding mid-reply and reopening.
   - Source: codebase (`backend/services/mini.py` semantics).
   - Status: Complete (2026-10-08), apart from the hands-on checks in item 8
   - **Built:** commands `send`, `interrupt`, `respond`, `models`, `set_model`,
     `new_conversation`, `events_since`, `hide_box`, `resize`, `quit`; one event channel
     (`mini:event`) carrying numbered events, kept in a buffer of 5,000. The app also emits
     `mini:shown` and `mini:permission`.
   - **Deviation:** the chosen model and the open conversation are saved by the app
     (`settings.json`), so a restart of the app comes back to the same conversation.

6. **The page**
   - What: copy `tauri/prototype-mini-spike.html` to `Mini/Page/index.html` and replace only its
     transport (`call`, `openSession`, `tail`) with commands and the event listener. Keep the look,
     the keys, the model list, the drag code and the sizing contract. Remember the chosen model in
     the app's settings instead of the page's local storage.
   - Why: the look and behaviour are already approved by the user.
   - Depends on: 5.
   - Risk: a regression in behaviour. Know it by: the interaction checks used during the polish work,
     re-pointed at the new page.
   - Source: codebase.
   - Status: Complete (2026-10-08), apart from the hands-on checks in item 8
   - **Built:** `Mini/Page/index.html`, the approved page with only its transport replaced, plus
     `/quit` and a notice for the Accessibility permission.
   - **Proven with `NULL_MINI_SELFTEST`:** a message typed into the real page went to OMP and the
     page showed the prompt line, the reply and a completed tool line, with the arrow back to
     idle. This ran with the screen locked, so the window growing with the reply was not seen.

7. **Build, sign, install, start at login**
   - What: `Mini/Scripts/` to build the app bundle and copy it to `/Applications`; a stable local
     signing identity so the Accessibility grant survives rebuilds; start at login
     (`tauri-plugin-autostart`).
   - Why: "fn + space shouldn't be dependent on" a terminal command or another app.
   - Depends on: 2, 3.
   - Risk: creating the signing identity needs the user at the Keychain prompt.
   - Source: inferred.
   - Status: In progress — the bundle builds and works; nothing is installed yet
   - **Built (2026-10-08):** `cargo tauri build` produces `Null Mini.app` (12 MB), a background
     app (`LSUIElement`, from `Mini/Info.plist`). Run from the bundle, the whole path works.
     `Mini/Scripts/install` builds, copies the app to `/Applications`, writes a login item
     (`~/Library/LaunchAgents/io.github.dominionism.null-mini.plist`) and starts it;
     `Mini/Scripts/uninstall` reverses that. Neither script has been run.
   - **Deviation:** start at login is a launchd agent written by the install script, not
     `tauri-plugin-autostart`: one less dependency, and the app need not manage it.
   - **Remaining:** a stable signing identity. The bundle is signed ad hoc, so every new build
     must be switched on again under Accessibility.

8. **Prove it on the installed app**
   - What: with the Voice desktop and its server stopped: fn+Space shows the box over a normal and a
     full-screen app without taking activation and without typing a space; a question streams an
     answer; a task shows its tool lines and is carried out; Ctrl+C stops; `/model` switches provider
     and the conversation carries on; `/new` starts over; the box drags; position and model survive
     a restart of the app; the app is running after logging out and in.
   - Why: acceptance.
   - Depends on: 1–7.
   - Status: Not started

9. **Remove the prototype from the Voice desktop**
   - What: delete the two prototype files, the marked edits in `main.rs`, the `tauri-nspanel`
     dependency and the `mini-spike` capability entry. The backend's harness layer, `/mini` routes
     and lock stay (ADR 0002).
   - Why: one mini, not two listening for fn+Space.
   - Depends on: 8.
   - Status: Not started

## Verification

- Rust unit tests for the pure parts: update → event, config options → model list, MCP entry
  conversion, binary discovery.
- One opt-in live test against the installed OMP.
- Item 8's checklist, by hand, on the installed build.

## Validation

- No ADR is contradicted. ADR 0002 is what this plan carries out; folder names follow ADR 0001, with
  `src/` and `capabilities/` kept as tool-fixed names.
- The order holds: items 2 and 4 depend only on item 1 and can be built in either order.
- One "how" is still open inside item 4: sending while a prompt is in flight (see its risk).
- Nothing here has been run. The window and shortcut code is proven in the prototype; the harness
  behaviour is proven in Python; neither is proven in this app yet. Item 8 is where that happens.

## Decisions made (2026-10-08)

| Question | Decision |
|---|---|
| Quitting and settings | Typed commands in the box only (`/quit`), like `/model` and `/new`. No menu-bar icon |
| Where a conversation works | A folder of its own under the app's support directory |
| Name and identifier | "Null Mini", `io.github.dominionism.null-mini` |

## Out of scope

- Voice, the pet, background tasks and other harnesses: later phases of `NullMini.md`, to be
  re-planned against this app when reached.
- Windows and Linux.
