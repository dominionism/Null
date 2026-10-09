# Null as its own app

> Blueprint: 2026-10-08. Builds on `Context/Plans/NullMini.md` (the constraints, the spike evidence
> and what the prototype proved) and `Context/ADR/0002-NullMiniIsItsOwnApp.md`.
>
> Naming: the app is called **Null** (user, 2026-10-08). Earlier documents and commits call it Null
> Mini or the mini. The names nobody sees keep the old form: the `Mini/` folder, the `null-mini`
> executable, the identifier `io.github.dominionism.null-mini` and the `NULL_MINI_*` switches.
>
> Status: **Built and installed. Null is in `/Applications`, starts at login and opens on
> Control+Space with no permission. Most of item 8's hands-on checks have passed, and the app is now
> signed so that macOS remembers what it has been allowed, has Full Disk Access, and the prototype
> is gone from the Voice desktop (item 9). Still open in item 8: the log-out check.**

## Goal

Build Null as a small macOS app in `Mini/` that opens on Control+Space, takes typing, and drives the
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
- **Verbatim (user, 2026-10-08, on seeing the app installed):** "I don't like how it's called Null
  Mini (Just call it Null). I do like the icon you chose."
- **Verbatim (user, 2026-10-08):** "Would it be possible if we changed the hot key to be control +
  space?" Offered two ways to do it, the user chose the one that needs no permission.
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

Layout as built:

```text
Mini/
├── Cargo.toml, Cargo.lock, build.rs, tauri.conf.json, Info.plist
├── capabilities/        tool-fixed name
├── src/                 tool-fixed name (Rust)
│   ├── main.rs          start-up, commands, the development switches
│   ├── panel.rs         window as a non-activating panel, sizing, position
│   ├── shortcut.rs      the Control+Space shortcut
│   ├── harness.rs       ACP connection to the harness
│   ├── translate.rs     protocol JSON to the box's events, model list, finding the CLI
│   ├── settings.rs
│   └── log.rs
├── Page/index.html      the box
├── Icons/
└── Scripts/             install, uninstall
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
     page has loaded. It logs to `~/Library/Logs/Null/mini.log`.

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
   - Status: In progress — the shortcut and hiding are proven by hand on the installed app; dragging
     and the remembered position are not yet reported
   - **Built (2026-10-08):** `src/panel.rs`, `src/shortcut.rs`, `src/settings.rs`. `tauri-nspanel`
     is pinned to the revision the prototype was proven with and compiles against Tauri 2.12.1.
     The page asks through commands (`hide_box`, `resize`) and hears `mini:shown`.
   - **Proven by start-up runs:** the window becomes a panel, the shortcut is registered, the page
     loads. Six unit tests pass (where a saved position is still reachable; settings round trip).
   - **Deviation (2026-10-08, the user's choice):** the shortcut is Control+Space, registered with
     macOS as an ordinary shortcut through `tauri-plugin-global-shortcut` 2.4.0 (for an ordinary
     key it calls `RegisterEventHotKey`). The system keeps the keystroke, so the Space never
     reaches the app in front, and no permission is needed. The fn+Space key tap was built first,
     proven on the installed app, and removed in `1a1d838`. If the shortcut cannot be registered,
     the box opens by itself and says so (`mini:notice`).
   - **Cost of that choice:** other apps no longer receive Control+Space while Null is running.
     macOS's own shortcuts on that key (input sources) are switched off on this Mac.
   - **Proven by hand on the installed app (2026-10-08):** with fn+Space, the box opened and closed
     over Safari and Ghostty and a click elsewhere hid it. With Control+Space, the user confirmed
     "Control+Space works"; the log shows it opening and closing over Ghostty. The log never
     recorded the app in front changing.
   - **Not yet reported:** dragging, and the box reopening where it was left. The app does save a
     position when the box hides (`settings.json` held one after the first click-away).
     `NULL_MINI_NO_SHORTCUT=1` runs the app without the shortcut and shows the box at start.

3. **Accessibility permission**
   - What: on start, check `AXIsProcessTrusted`. If not trusted, ask macOS to show its prompt once,
     show one line in the box saying what to do, and create the key tap when the grant arrives.
   - Why: a standalone app needs its own grant; today the terminal's grant is borrowed.
   - Depends on: 2.
   - Risk: an unsigned build loses its grant on every rebuild. See item 7.
   - Source: inferred.
   - Status: Removed (2026-10-08) — built, proven on the installed app, then dropped with the key tap
   - **Proven before removal:** on the installed app macOS showed its prompt, the user switched the
     app on, and the key tap started in the running app with no restart.
   - **Why removed:** only the fn+Space key tap needed the permission. Control+Space is registered
     with macOS directly (item 2), so Null asks for no permission. Its Accessibility entry was
     cleared with `tccutil reset Accessibility io.github.dominionism.null-mini`.

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
   - **Deviation:** no setting holds a user-chosen path to the harness. The lookup supports one
     and is tested, but the app always calls it with none, so the order in use is `PATH`, then
     the install folders.
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
     `new_conversation`, `events_since`, `hide_box`, `resize`, `quit`, plus `page_ready` and the
     self-test's `report`; one event channel (`mini:event`) carrying numbered events, kept in a
     buffer of 5,000. The app also emits `mini:shown` and `mini:notice` (words the box must show;
     it replaced `mini:permission`).
   - **Deviation:** the chosen model and the open conversation are saved by the app
     (`settings.json`), so a restart of the app comes back to the same conversation. Since
     2026-10-08 that holds only for a restart the user did not ask for (login, an update):
     `/quit` and Ctrl+C clear the saved conversation, at the user's request.

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
     `/quit` and a notice the app can fill (`mini:notice`), used when the shortcut cannot be
     registered.
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
   - Status: Complete (2026-10-08), apart from the log-out-and-in check in item 8
   - **Built (2026-10-08):** `cargo tauri build` produces `Null.app` (12 MB), a background
     app (`LSUIElement`, from `Mini/Info.plist`). Run from the bundle, the whole path works.
     `Mini/Scripts/install` builds, copies the app to `/Applications`, writes a login item
     (`~/Library/LaunchAgents/io.github.dominionism.null-mini.plist`) and starts it;
     `Mini/Scripts/uninstall` reverses that.
   - **Run (2026-10-08):** both scripts. The app is installed at `/Applications/Null.app` and
     running under its login item. It logs to `~/Library/Logs/Null/mini.log`.
   - **Deviation:** start at login is a launchd agent written by the install script, not
     `tauri-plugin-autostart`: one less dependency, and the app need not manage it.
   - **Signing (2026-10-08, the user's decision):** the app is signed with a self-signed
     certificate, "Null Local Signing", so macOS knows it by
     `identifier "io.github.dominionism.null-mini" and certificate leaf = H"7261d9c7…"` and no
     longer by the exact build. Signing had been dropped when the Accessibility permission went
     away, and came back because macOS also ties the file-access answers it records to who signed
     the app (item 8). No hardened runtime: Null hosts an agent, as a terminal does.
   - **Where the certificate lives:** `~/Library/Application Support/Null Signing/`, a keychain file
     of its own (`signing.keychain-db`) beside the password that opens it. It is not in the login
     keychain and not on the keychain search list. `Mini/Scripts/install` puts it on the list
     while it signs and takes it off again; without the keychain it installs unsigned.
   - **How it was made:** a 2048-bit key and a ten-year certificate from `/usr/bin/openssl` (key
     usage `digitalSignature`, extended key usage `codeSigning`), brought in with
     `security import`, then trusted for code signing with
     `security add-trusted-cert -r trustRoot -p codeSign`, which the user approved at macOS's
     prompt. `codesign` refuses the certificate without that trust, and does not find it unless
     its keychain is on the search list. Claude Code's safety check stopped the first attempt
     ("unauthorized persistence"); it went ahead only after the user authorised it.
   - **What this costs:** anything running as the user can read that password, sign as Null, and
     so inherit whatever macOS has granted Null. Worth remembering with Full Disk Access on.
   - **For other people:** a self-signed certificate works on one Mac. For macOS to keep what it
     has granted across updates on anyone else's, every release has to be signed by the same
     identity, which for a distributed app means an Apple Developer ID.

8. **Prove it on the installed app**
   - What: with the Voice desktop and its server stopped: Control+Space shows the box over a normal
     and a full-screen app without taking activation and without typing a space; a question streams
     an answer; a task shows its tool lines and is carried out; Ctrl+C stops; `/model` switches
     provider and the conversation carries on; `/new` starts over; the box drags; position and model
     survive a restart of the app; the app is running after logging out and in.
   - Why: acceptance.
   - Depends on: 1–7.
   - Status: In progress
   - **Passed by hand (2026-10-08, the user's report):** Control+Space opens and closes the box; it
     opens over a full-screen app and no space is typed underneath; the box drags and reopens
     where it was left; it grows as a reply comes in; a question is answered and complex tasks are
     carried out; Ctrl+C stops a reply; `/model` changes provider and model; `/new` starts over.
   - **Found in the trial:**
     - macOS asked seven permission questions in 17 seconds during one task (Documents, Desktop,
       Downloads, Photos, Media Library, iCloud Drive, data from other apps). macOS holds Null
       responsible for what the harness touches, and Null was a new app to it. The terminal never
       asks because it has Full Disk Access. The user does not want these prompts.
     - While the app was unsigned macOS tied each answer to the exact build (`cdhash`), so every
       rebuild would have asked again. Fixed by signing (item 7). The answers recorded for the
       unsigned build were cleared (`tccutil reset All io.github.dominionism.null-mini`).
     - The user chose to give Null Full Disk Access, as the terminal has, so that macOS stops
       asking about folders, and asked that Null prompt for it straight away so that other people
       are not left approving permissions one at a time. macOS has no prompt for Full Disk Access,
       so Null asks in its own way, once (`src/access.rs`): at start it tries to open a file only
       Full Disk Access unlocks, which also puts Null in the list, switched off. If that is
       refused it opens the list in System Settings and keeps the box up with one line saying
       what to switch on, until the user puts the box away. Proven on the installed app: the list
       opened and macOS recorded Null in it. The user switched it on; Null had to be restarted for
       it to take effect, and its own check then reported Full Disk Access on. The user then ran
       a task and reported: "It works. No permission prompts."
     - The user asked for the box's own commands in another colour and for a lower height limit.
       Both are installed and pass the scripted checks; the user has not yet seen them: `/model`,
       `/new` and `/quit` turn blue as they are typed, and the transcript and the model list stop
       at 212 px (ten lines of transcript), down from 344.
     - The user then asked for a terminal typeface, a cleaner arrow ("looks like it's two
       different lines connected") and text that is centred. Installed, not yet seen by the user:
       SF Mono throughout (13.5 px in the field); the arrow drawn as one stroke with round ends
       and a round corner; the typed text raised 1 px. Measured in the app's web engine, small
       letters had been centred 1.5 px below the row's middle and capitals 0.25 px below, while
       the arrow was dead centre; now they sit 0.5 px below and 0.75 px above it. The user's
       words were "centered horizontally"; this was read as the text's height in the row, since
       the text is left-aligned by design.
     - Signing proven: the build installed after Full Disk Access was switched on still has it.
   - **How the look was checked:** the page was drawn in an off-screen WKWebView, the app's own
     web engine, with a stand-in for `window.__TAURI__`, and the pictures were read and measured.
     Those scripts were session scratch and are not in the repo.
   - **Not yet tried:** position and model surviving a restart of the app; running after
     logging out and in. The user reported that it "doesn't run when I log out", but the Mac shows
     no log-out since 2026-10-05 and the app has run without a break since it was installed, so
     this check is still open.
   - **Never yet seen in this app:** a real approval request (OMP runs `yolo` here) and a provider
     usage limit.
   - **State of the Voice project during the trial:** its development app, which ran the
     prototype, was stopped. The Voice server was left running. Null does not use it, though the
     agent's own `voicebox` MCP server points at it, so what the agent does with that server
     stopped is untried.

9. **Remove the prototype from the Voice desktop**
   - What: delete the two prototype files, the marked edits in `main.rs`, the `tauri-nspanel`
     dependency and the `mini-spike` capability entry. The backend's harness layer, `/mini` routes
     and lock stay (ADR 0002).
   - Why: one mini, not two. The two no longer share a shortcut (the prototype opens on fn+Space,
     Null on Control+Space), but the prototype is still built into the Voice desktop.
   - Depends on: 8.
   - Status: Complete (2026-10-08)
   - **Done:** the two prototype files are deleted, and the five files the prototype had edited
     (`Cargo.toml`, `Cargo.lock`, `capabilities/default.json`, `gen/schemas/capabilities.json`,
     `src/main.rs`) are back to their content before it. `tauri/`, `app/` and `web/` are now
     identical to the first commit (`90a885c`), and the Voice desktop still passes `cargo check`.
   - **Kept:** the backend's harness layer, `/mini` routes and lock (ADR 0002), and
     `scripts/prototype-omp-spike.py`, which item 4 points to for the protocol's messages.
   - **Left on disk, outside git:** the prototype's log, `data/logs/mini-spike.log`.

## Verification

- Rust unit tests for the pure parts: update → event, config options → model list, MCP entry
  conversion, binary discovery (`cargo test`, 20 tests).
- Scripted checks, run from `Mini/` after `cargo build`. They stand in for the opt-in live test
  and write to `~/Library/Logs/Null/mini.log`:
  - `NULL_MINI_EXIT_WHEN_READY=1 ./target/debug/null-mini`: starts, registers the shortcut
    ("Control+Space opens the box" in the log), loads the page, quits. Add
    `NULL_MINI_NO_SHORTCUT=1` to leave the shortcut alone.
  - `NULL_MINI_SMOKE='Reply with exactly one word: pong' ./target/debug/null-mini`: the harness
    alone. Add `NULL_MINI_SMOKE_MODEL=<id>` or `NULL_MINI_SMOKE_STOP_AFTER=<seconds>`.
  - `NULL_MINI_NO_SHORTCUT=1 NULL_MINI_SELFTEST='<message>' ./target/debug/null-mini`: types into
    the real page and logs what the page shows.
- Reading the results: the self-test reports "window height asked 0" when the screen is locked or
  the box is hidden, because the page sizes its window in `requestAnimationFrame`, which does not
  fire then. That is not a fault in the page.
- Tooling: bundle with `cargo tauri`. `bunx tauri` fetches an unrelated npm package.
- Item 8's checklist, by hand, on the installed build.

## Validation

- No ADR is contradicted. ADR 0002 is what this plan carries out; folder names follow ADR 0001, with
  `src/` and `capabilities/` kept as tool-fixed names.
- The order holds: items 2 and 4 depend only on item 1 and can be built in either order.
- The "how" that was open inside item 4 is settled: a stop sent while a reply was in flight ended
  it as `cancelled` (item 4's table).
- Run so far without a person: start-up, the harness path and the page-to-harness path, by the
  scripted checks under "Verification". Run by the user: the shortcut, hiding, and (before it was
  removed) the Accessibility prompt. Not run by anyone: typing into the box on screen, dragging, the
  window growing, and a real approval request. Item 8 is where that happens.

## Decisions made (2026-10-08)

| Question | Decision |
|---|---|
| Quitting and settings | Typed commands in the box only (`/quit`), like `/model` and `/new`. No menu-bar icon. Since 2026-10-08 Ctrl+C with nothing running does what `/quit` does, and quitting closes the conversation: the next start opens a new one (the user: "ctrl + c should just close the Null terminal and wipe the current session"). While a reply runs Ctrl+C still only stops it |
| Where a conversation works | A folder of its own under the app's support directory |
| Name and identifier | "Null". First "Null Mini"; changed by the user on seeing it installed. The identifier stays `io.github.dominionism.null-mini` |
| Shortcut | Control+Space, registered as an ordinary macOS shortcut so that no permission is needed. Replaces fn+Space |
| Icon | Kept as built; the user likes it |
| macOS permission prompts | The user does not want them. Null is signed with a local certificate and given Full Disk Access, as the terminal has |

## Out of scope

- Voice, the pet, background tasks and other harnesses: later phases of `NullMini.md`, to be
  re-planned against this app when reached.
- Windows and Linux.
