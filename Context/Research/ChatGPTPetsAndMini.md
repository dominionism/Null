# Research: ChatGPT Pets and Mini

> Last updated: 2026-10-07 — external product research (web + primary sources). Not a codebase map;
> `Research.md` remains the codebase baseline.

Each claim below is one of three grades:

- **Confirmed** — read directly in OpenAI's docs, the `hatch-pet` skill, or the open-source Codex
  repository.
- **Reported** — from press coverage, OpenAI's social posts as quoted by third parties, or user
  forum posts. Not independently tested.
- **Inferred** — my reading of how confirmed pieces fit together. Marked inline.

Nothing here was tested hands-on in the ChatGPT desktop app.

## What is this?

"Pets" and "Mini" are two skins of one feature in the ChatGPT desktop app (macOS and Windows): a
small always-on-top overlay that shows what your chats are doing and lets you start a new chat by
typing or speaking without opening the main window.

- A **pet** is the overlay with an animated sprite above the controls.
- **Mini** is the same controls with no sprite.

Choosing a pet changes appearance only, not how ChatGPT completes tasks (confirmed, official docs).

## Timeline (2026)

| When | What shipped | Grade |
| --- | --- | --- |
| Feb 2–6 | Codex app launches on macOS (parallel chats, worktrees, skills, voice dictation) | Confirmed |
| Feb 9–13 | Chat forking and a floating, always-on-top pop-out chat window | Confirmed |
| ~May 1 | Pets launch in the Codex app: `/pet` to wake, `/hatch` to create, eight built-ins | Reported |
| Jul 9 | Codex app merges into the ChatGPT desktop app (macOS + Windows, every plan incl. Free) | Confirmed |
| Jul 20–24 | ChatGPT Voice, powered by GPT-Live, in the desktop app | Confirmed |
| ~Aug | "Click your pet to open Voice, check on work, and approve or stop tasks" | Reported |
| Sep 7–11 | Quick Chat from the pet's floating controls, plus the Mini option | Confirmed |
| Sep 21 – Oct 5 | User complaints about the shortcut and non-hideable overlay | Reported |

OpenAI's wording at each step (reported, via Gigazine, Digg and ETV Bharat):

- May 1, OpenAI Developers: "Use /pet to wake your pet."
- ~Aug, @ChatGPT: "Your pets have found a shortcut to ChatGPT Voice. In the desktop app, click your
  pet to open Voice, check on work, and approve or stop tasks without missing a beat."
- Sep 11, @ChatGPT: "Your pet has a job now. Pets help you keep track of your chats while you're
  away from the desktop app."

Before September the pet was status-only: clicking the pet returned you to ChatGPT and clicking its
status bubble opened that specific chat (reported, Penchan and One Cool Tip).

## The overlay

All confirmed from the official Pets page unless marked.

**Choosing and showing**

- Profile menu > **Pets**, or **Settings > Pets**. Pick a built-in pet, a custom pet, or **Mini**.
- Show with `/pet`, or command menu > **Show pet**. Hide with `/pet` again, right-click > **Hide**,
  command menu > **Hide pet**, or **Settings > Pets**.
- **Settings > Pets > Customize > Pet size** adjusts size; **Reset** restores the default.
- Selection and screen position persist across app restarts.
- A workspace admin can disallow pets.

**Global shortcut**

| Platform | Default |
| --- | --- |
| macOS | Option+Space |
| Windows | Windows+Alt+P |

The shortcut shows the controls and focuses Quick Chat. Pressing it again keeps the controls open
and re-focuses Quick Chat; it does not toggle them closed. The docs say the binding (**Show pet**)
can be changed in **Settings > Keyboard shortcuts**; see Limitations for conflicting user reports.

**Controls below the pet** (move the pointer over them to select one)

- **Pencil** — opens Quick Chat. Type, then Enter to send. `@` adds context, `$` picks a skill.
- **Voice icon** — starts a ChatGPT Voice conversation.
- **Bell** — opens the activity tray listing threads. Selecting a thread opens the full
  conversation in the main app. A chevron collapses the tray.

A chat started from these controls is **outside any project**. To use a project's context the chat
must be started from that project in the main app.

**Integrations**

- **Appshots (macOS):** pressing both Command keys captures the frontmost window (image plus
  available text, including text outside the visible scroll area). If the pet is visible and the
  main window is in the background, the appshot goes to the pet and starts a new chat. Works with
  Mini. Requires **Appshot destination = Automatic** and Appshots permissions (Screen & System
  Audio Recording, Accessibility). On Windows (both Alt keys) appshots open in the main app instead.
- **Computer Use (macOS):** the picture-in-picture window can attach to the pet and follows it when
  the pet is moved. Sending the window to a hidden pet shows the pet.
- **Reduced motion:** with the OS setting on, the pet shows a still frame (the first `idle` frame).
- **Activity tray vs notifications:** the tray is separate from system notifications. Desktop
  notifications are configured independently: turn-completion alerts never / only in background /
  always, with separate toggles for permission and question notifications.

## Status model

Four statuses (confirmed, official docs):

| Status | Meaning |
| --- | --- |
| Running | A chat is actively working. |
| Needs input | A chat needs your approval, answer, or another decision. |
| Ready | A chat has completed and has unread activity. |
| Blocked | A chat failed or encountered a system error. |

**Priority** when several chats have activity: Needs input, then Blocked, then Ready, then Running.

**Triggers, animation and expiry** — confirmed for the *terminal* pet from
`codex-rs/tui/src/pets/ambient.rs` and the chat widget call sites. The desktop overlay is closed
source; whether it uses the same lifetimes is unverified.

| Status | Internal kind | Trigger | Animation row | Default bubble text | Expires after |
| --- | --- | --- | --- | --- | --- |
| Running | `Running` | Turn starts | `running` | "Thinking" | 3 minutes |
| Needs input | `Waiting` | Exec/patch approval, permission request, user-input request, user verification, MCP elicitation | `waiting` | "Needs input" | 24 hours |
| Ready | `Review` | Turn completes (not on replay) | `review` | Preview of the agent's reply | 7 days |
| Blocked | `Failed` | Turn errors | `failed` | "Blocked" | 1 hour |

Behaviour of a status in the terminal implementation:

- Setting a status replaces the previous one and restarts the animation clock.
- The status row plays **three times**, then the pet settles into the `idle` loop while the bubble
  stays up until the status expires.
- After expiry the bubble disappears and the pet is plain `idle`.
- With animations disabled the first frame of the current animation is shown.

## Where status comes from

The desktop app, the IDE extension and the CLI's remote mode are clients of **Codex app-server**, an
open-source JSON-RPC interface (confirmed). Relevant protocol surface:

- `thread/status/changed` — emitted whenever a loaded thread's runtime status changes. Payload is
  `threadId` plus `status`.
- `status.type` is one of `notLoaded`, `idle`, `systemError`, or `active` with `activeFlags`
  (the documented example flag is `waitingOnApproval`).
- `turn/started` and `turn/completed`; a completed turn's status is `completed`, `interrupted`, or
  `failed`.
- `tool/requestUserInput` — the server asks the user 1–3 short questions (experimental).
- A thread with no subscribers and no activity is unloaded after 30 minutes.

**Inferred:** the overlay's four statuses are a projection of this stream — `active` with no flags
is Running, `active` with a waiting flag is Needs input, `systemError` or a failed turn is Blocked,
and a completed turn with unread activity is Ready. "Unread" is client-side state.

## Sprite contract

Confirmed from `hatch-pet/references/` and `codex-rs/tui/src/pets/catalog.rs`.

- One fixed atlas: **1536×1872**, PNG or WebP, transparent background.
- Grid: **8 columns × 9 rows**, cell **192×208**.
- Unused cells must be fully transparent. No labels, gutters, borders, grid lines or extra frames.
- The desktop webview animates by shifting CSS `background-position` across the fixed grid.
- Web upload limit: 20 MiB.

| Row | State | Columns used | Frame durations | Purpose |
| ---: | --- | ---: | --- | --- |
| 0 | `idle` | 0–5 | 280, 110, 110, 140, 140, 320 ms | Calm breathing/blinking loop; first frame is the reduced-motion still |
| 1 | `running-right` | 0–7 | 120 ms each, final 220 ms | Directional movement to the right (used while dragging) |
| 2 | `running-left` | 0–7 | 120 ms each, final 220 ms | Directional movement to the left (used while dragging) |
| 3 | `waving` | 0–3 | 140 ms each, final 280 ms | Greeting or attention gesture |
| 4 | `jumping` | 0–4 | 140 ms each, final 280 ms | Anticipation, lift, peak, descent, settle |
| 5 | `failed` | 0–7 | 140 ms each, final 240 ms | Error / deflated reaction (Blocked) |
| 6 | `waiting` | 0–5 | 150 ms each, final 260 ms | Expectant asking pose (Needs input) |
| 7 | `running` | 0–5 | 120 ms each, final 220 ms | Active task work: thinking, scanning, typing. Not foot-running |
| 8 | `review` | 0–5 | 150 ms each, final 280 ms | Focused inspecting loop (Ready) |

Notes:

- The terminal implementation slows `idle` to 1680, 660, 660, 840, 840, 1920 ms (6× the table).
- The terminal loader also registers aliases: `move_right`, `move_left`, `wave`, `bounce`, `sad`.
- The install deep link accepts `spriteVersionNumber=1` (default) or `2`. What format version 2 is
  is undocumented (unverified).

## Pet package and loading

Confirmed from `codex-pet-contract.md`, `model.rs`, `asset_pack.rs` and the commands reference.

**Custom pet on disk**

```text
${CODEX_HOME:-$HOME/.codex}/pets/<pet-name>/
├── pet.json
└── spritesheet.webp
```

```json
{
  "id": "pet-name",
  "displayName": "Pet Name",
  "description": "One short sentence.",
  "spritesheetPath": "spritesheet.webp"
}
```

- The app loads custom pets by folder name. A legacy layout,
  `~/.codex/avatars/<id>/avatar.json`, is still accepted by the terminal loader.
- `spritesheetPath` must be a relative path inside the pet folder; absolute and `..` paths are
  rejected.
- The terminal loader additionally accepts two optional keys not in the published contract:
  - `frame`: `{ width, height, columns, rows }` — must tile the sheet exactly; max 256 frames.
  - `animations`: `{ "<name>": { frames: [indices], fps, loop, fallback } }` — `fps` defaults to 8
    (max 60), `loop` defaults to true, `fallback` defaults to `idle`.
- The sheet must still be exactly 1536×1872 regardless of `frame`.
- Custom pets are local to the machine and do not sync to ChatGPT web.

**Built-in pets**

| id | Name | Description |
| --- | --- | --- |
| `codex` | Codex | The original Codex companion (default) |
| `dewey` | Dewey | A tidy duck for calm workspace days |
| `fireball` | Fireball | Hot path energy for fast iteration |
| `rocky` | Rocky | A steady rock when the diff gets large |
| `seedy` | Seedy | Small green shoots for new ideas |
| `stacky` | Stacky | A balanced stack for deep work |
| `bsod` | BSOD | A tiny blue-screen gremlin |
| `null-signal` | Null Signal | Quiet signal from the void |

- The CLI does not bundle built-in sheets. It downloads
  `https://persistent.oaistatic.com/codex/pets/v1/<id>-spritesheet-v4.webp` on first use (HTTPS
  only, 4 MiB cap, 60 s timeout), validates dimensions, and installs atomically into
  `$CODEX_HOME/cache/tui-pets/v1/assets/`.
- Updating a built-in means publishing a new versioned filename, never mutating one in place.

**Install deep link**

```text
codex://pets/install?name=<pet-name>&imageUrl=<https-image-url>
```

Optional `description=<text>` and `spriteVersionNumber=<1-or-2>`. Invalid names, non-HTTPS URLs,
unsupported versions or extra path segments make the link do nothing. Community galleries
(codex-pets.net, Petdex) offer an in-app install button, `npx codex-pets add <id>`, or a manual
zip unpacked into the pets folder (reported).

**Internal naming**

The feature's internal name is "avatar": the pet catalog is "ported from the Codex App avatar
catalog" (confirmed, source comment) and the shortcut command is `openAvatarOverlay` (reported).

## Custom pet generation (`hatch-pet`)

Confirmed from `openai/skills/skills/.curated/hatch-pet`.

**User flow:** **Settings > Pets > Create pet** installs the bundled skill, reloads skills and opens
a new chat. The user describes the pet (or supplies reference images or a brand name). When the
task finishes, **Refresh** in Settings > Pets lists the new pet.

**Pipeline**

1. `prepare_pet_run.py` creates a run folder, an image-generation job manifest
   (`imagegen-jobs.json`), prompts, a chroma key, and nine row-specific layout guide images.
2. Generate a **base** character image. It becomes the canonical identity reference.
3. Generate `idle` and `running-right` first as the identity and gait check.
4. `running-left` is mirrored from `running-right` only when that preserves identity; the mirror is
   done per frame slot so temporal order is not reversed. Otherwise it is generated.
5. Generate the remaining rows, one image strip per state, each grounded on the base image and its
   layout guide. `waiting`, `running`, `failed`, `review`, `jumping` and `waving` must each be
   generated as their own row, never derived from another.
6. Deterministic scripts: extract frames from strips, inspect them (`qa/review.json`), compose the
   atlas, validate it (`final/validation.json`), render a contact sheet and per-row preview GIFs.
7. A visual QA pass on the contact sheet and previews; identity or style drift blocks acceptance
   even when deterministic validation passes.
8. Package `pet.json` + `spritesheet.webp` into `~/.codex/pets/<pet-id>/`.

**Design rules worth noting**

- All image generation goes through the `$imagegen` skill; geometry is never trusted to the image
  model, only to the scripts.
- Style presets: `pixel`, `plush`, `clay`, `sticker`, `flat-vector`, `3d-toy`, `painterly`,
  `brand-inspired`, `auto`. Non-pixel styles are first-class.
- Effects must be attached to the silhouette. Shadows, glows, motion lines, detached sparkles,
  text and speech bubbles are rejected because they break transparent extraction.
- `idle` must be calm and low-distraction but not six identical frames.
- Repair the smallest failing scope: single frame, then one row, then the full atlas.
- A brand-only request triggers a small web-research step for visual and personality cues first.

## Voice

**From the pet:** the voice icon in the floating controls starts ChatGPT Voice (confirmed).

**ChatGPT Voice** (confirmed, official docs):

- Powered by GPT-Live. Works in Chat, Work and Codex in the desktop app, and through Codex on iOS
  after pairing with a desktop host.
- Plans: Plus, Pro, Business, Edu, Enterprise.
- Natural turn-taking: the user can interrupt a response, ask a follow-up, or change direction.
- Delegation: voice can start separate tasks for longer work, check existing tasks, and send
  follow-up instructions, bringing progress, blockers and results back into the conversation.
- Task switching by speech: "Let me talk to the task reviewing the tests", then "Take me back to
  the previous task."
- Voice in an existing task uses that task's conversation and selected model.
- Screen context (macOS): with **Settings > Voice > Screen context** on, "Take a look at this"
  takes an appshot of the frontmost window.
- Permissions follow the same rules as the tasks it directs.
- **Only one voice chat can be active across the desktop app at a time.**
- Shortcuts: Start voice chat is Ctrl+Shift+V on macOS; a custom shortcut can be set in
  **Settings > Voice > Voice chat hotkey**.

**Cost** (confirmed, pricing page): $0.05 per minute against the Codex usage budget. GPT-Live
manages the live conversation; the model doing the task is billed separately at token rates.
Credit-billed workspaces pay 1.25 credits per minute. Not available via API key.

**GPT-Live architecture** (reported; from search-result summaries of press coverage, pages not read
in full): full-duplex, so it listens and speaks at once and decides many times per second whether
to speak, pause, keep listening, interrupt, or call a tool. It back-channels ("mhmm", "got it") and
waits through pauses. Work needing search or deeper reasoning is handed to a background model
(GPT-5.5 at launch) while the conversation continues.

**Voice dictation** is a separate feature (confirmed): **Dictate** in the composer transcribes
speech into the text box for review before sending. The Codex app also has dictation cleanup and a
custom dictionary for names, file paths and code symbols.

## Related desktop surfaces

All confirmed.

- **Pop-out chat window:** any active chat can be popped into its own window with **Always on top**.
  This predates pets and is separate from the overlay.
- **Quick chat (main app):** Cmd+Option+N / Ctrl+Alt+N opens an ordinary ChatGPT chat from the Codex
  view. Different from the pet's Quick Chat box.
- **Activity view:** sidebar bell, Cmd+Option+U / Ctrl+Alt+U; lists chats that are unread, running,
  or waiting for a response. The pet's tray is the floating counterpart.
- **Codex Micro:** a limited-run Work Louder keypad that checks on chats, jumps between them, uses
  voice input and triggers actions from hardware keys.

## Other surfaces

| Surface | Behaviour | Grade |
| --- | --- | --- |
| Web | Pet appears inside supported Work chats only. No overlay, no activity tray, no `/pet`. Custom sheets can be uploaded in **Settings > Personalization > Pet**. | Confirmed |
| Codex CLI | `/pets` or `/pet` opens a picker; `/pets <name>` selects; `/pets off` disables. Reports the current session only. | Confirmed |
| IDE extension | No pet picker and no overlay. | Confirmed |
| Mobile | No native pet support documented. | Reported |

Terminal pet details (confirmed, source):

- Drawn through a terminal image protocol after the frame renders: Kitty graphics, iTerm2 3.6+
  (Kitty protocol via local file), or Sixel.
- Disabled in tmux and Zellij because terminal images do not stay pane-local there.
- Target height 75 px, anchored bottom-right above the composer; transcript text wraps around it.
- Config: `tui.pet` (pet id, or `disabled`), `tui.pet_anchor` (`composer` default, or
  `screen-bottom`), `tui.animations`.
- Frames are pre-extracted to PNGs under `$CODEX_HOME/cache/tui-pets/frame-cache/`.
- Hidden while a modal or popup is active.

## Limitations and complaints

**Confirmed (docs)**

- Quick Chat from the overlay has no project context.
- Custom pets do not sync between desktop and web.
- Windows: appshots open in the main app, not the pet. Computer Use attach is macOS only.
- One voice chat at a time.

**Reported (OpenAI community forum, Sep 21 – Oct 5, 2026; no staff replies)**

- **Mini is not hideable.** Even set to Mini, the Chat and Voice buttons stay on the desktop. Users
  want the launcher behaviour of the older "ChatGPT Classic" Mac app: Option+Space opens Quick Chat,
  and Option+Space again, Escape, or clicking away hides it completely.
- **Shortcut conflict.** Option+Space collides with Alfred and Todoist. Users said it could not be
  changed or disabled, which conflicts with the docs. Community workaround, untested:

  ```json
  [
    { "command": "openAvatarOverlay", "key": null }
  ]
  ```

  saved as `~/.codex/keybindings.json`.
- **Continuing a dialog costs a click.** A follow-up needs the button pressed and the input
  re-focused, which one user said defeats quick interaction.
- **Slower quick answers.** One user said the quick window was routed "through the codex models"
  and now "thinks" over trivial questions.

## Unverified / open questions

- The desktop overlay's window implementation (framework, always-on-top mechanism, hit-testing).
  It is closed source; only the webview/CSS detail in the pet contract is confirmed.
- Whether the desktop overlay uses the terminal pet's status lifetimes and three-plays-then-idle
  rule.
- Which rows the desktop overlay uses for `waving` and `jumping`, and when.
- Sprite format version 2.
- Whether the Option+Space binding is now changeable in Settings, as the docs state.
- The Reddit thread referenced on the forum and a YouTube explainer ("the project-context catch")
  could not be read directly.

## Sources

**Primary**

- [Pets](https://learn.chatgpt.com/docs/pets.md)
- [Desktop app settings](https://learn.chatgpt.com/docs/reference/settings.md)
- [Desktop app commands and deep links](https://learn.chatgpt.com/docs/reference/commands.md)
- [What's new](https://learn.chatgpt.com/docs/whats-new.md)
- [Notifications](https://learn.chatgpt.com/docs/notifications.md)
- [ChatGPT Voice](https://learn.chatgpt.com/docs/features/voice.md)
- [Prompting: voice dictation](https://learn.chatgpt.com/docs/prompting.md)
- [Appshots](https://learn.chatgpt.com/docs/appshots.md)
- [Codex App Server](https://learn.chatgpt.com/docs/app-server.md)
- [Pricing](https://learn.chatgpt.com/docs/pricing.md)
- [Projects and chats](https://learn.chatgpt.com/docs/projects.md)
- [Codex Micro](https://learn.chatgpt.com/docs/features/codex-micro.md)
- [hatch-pet skill, openai/skills](https://github.com/openai/skills/tree/main/skills/.curated/hatch-pet)
- [Terminal pets source, openai/codex](https://github.com/openai/codex/tree/main/codex-rs/tui/src/pets)

**Secondary**

- [Community: Pets and mini chat window — please fix it](https://community.openai.com/t/pets-and-mini-chat-window-please-fix-it/1401660)
- [Community: Allow Quick Chat to completely hide when dismissed](https://community.openai.com/t/allow-quick-chat-to-completely-hide-when-dismissed/1399635)
- [ETV Bharat: OpenAI adds Pets to ChatGPT desktop app](https://www.etvbharat.com/en/technology/openai-adds-pets-feature-to-chatgpt-desktop-app-what-is-it-and-how-to-setup-enn26091401158)
- [Digg: new-chat shortcuts for desktop pets and Mini](https://digg.com/ai/ich3puin)
- [Digg: pet shortcut to Voice](https://digg.com/tech/8dd5qi1d)
- [Penchan: ChatGPT/Codex pets guide](https://penchan.co/en/ai/coding/codex-pets/)
- [Gigazine: Codex pets launch](https://gigazine.net/gsc_news/en/20260507-openai-codex-pets)
- [TestingCatalog: animated pets in Codex](https://www.testingcatalog.com/openai-adds-animated-pets-and-config-imports-to-codex.md)
- [One Cool Tip: ChatGPT pet](https://www.onecooltip.com/2026/08/let-chatgpt-pet-keep-watch-over-your.html)
- [iGeeksBlog: GPT-Live](https://www.igeeksblog.com/openai-gpt-live-chatgpt-voice/)
- [Android Authority: GPT-Live](https://www.androidauthority.com/openai-gpt-live-voice-model-3685616/)
