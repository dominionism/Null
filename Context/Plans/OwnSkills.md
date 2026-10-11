# Null's own commands, and the skills it carries

> Planned 2026-10-11, **revised the same day at the owner's question:** "do we really need all of
> these different things? … let's keep the house clean and delete anything that has no use case."
>
> The first draft of this plan had a Kit (a directory of Null's own, in two halves, attached with
> `--plugin-dir`), plus notes about a conversation's own folder. All of that is **deleted**. What is
> left is one small change to the box, and one deferred trigger. Read
> `Context/Research/Research.md` first: the box's own typed commands (`/model`, `/folder`) are a
> different thing from **the harness's commands** (`/research`, `/skill:blueprint`).

## Goal

A person can type their own skill into the box and have it work — the same way, in the same place,
for the owner and for anyone else who installs Null.

## What we are not building, and why

| Idea from the first draft | Why it is gone |
| --- | --- |
| **The Kit** — a directory of Null's own in the support folder, writable, that the person adds to | No use case. A person's own skills never live there: they live in their own `~/.agents/skills`, which is the *same path on every Mac* and already works. A second place for the same files means two copies that drift. |
| **The second root inside `Null.app`** (`Resources/Kit`), with seeding, hashes and a manifest | Only existed to ship skills nobody has asked Null to ship. Nothing is shipped, so nothing needs a place to be shipped from. |
| **`--plugin-dir` and everything around it** | The mechanism for the two folders above. Not needed once they are gone. |
| **A paragraph about a conversation's own folder** (`.agents/skills` in the project a conversation runs in) | **Never ours to build.** The harness already reads it; that is how this repo's four skills appear when a conversation runs in this repo. Zero work, zero decision. Worth one line so a future agent does not think it must be built. |
| **ADR 0006 "Null carries its own Kit"** | An ADR for a thing we decided not to build. No new ADR: `ADR 0003` already says Null keeps the harness's folder for the person's own sign-ins, skills and MCP servers, and this plan does not change that. |
| **A new word in the glossary** (Kit) | The box will not say "Kit" to anyone. Nothing to name. |

Deleting those also deletes: seeding logic, profile-suffixed folders, build-time checks on shipped
skills, the precedence question for a shipped skill colliding with a person's own, and the "copy or
don't copy the Playbook" decision. Nothing of the sort is left to decide.

## Constraints

- **Same architecture for everyone.** One place for a person's skills — `~/.agents/skills` — the same
  path on every Mac, each person's own files. Null adds no second place.
- The harness's own project rule stays as it is: a conversation in a folder also sees that folder's
  `.agents/skills` and `.omp/skills`. Null does not touch it.
- **No symlinks, no copies**: satisfied by having nothing to link or copy.
- **ADR 0003**: null never writes the harness's folder (`~/.omp/agent`) — it reads `mcp.json` and the
  sessions there and writes nothing. **ADR 0004 / 0005**: no credential, no provider knowledge.
- **ADR 0001**: PascalCase folders; a name a tool looks for is the exception.
- **The box's rules**: everything the box does is a typed command, and a button is a second way in;
  the box says "conversation"; the page is one static file.
- **`BackgroundWork.md` decision 7** (owner, 2026-10-10): a mistyped command opens the box's list.
  Kept.
- **The engine rule**: the carried Oh-my-pi is 18.8.7 (`Mini/Engine.toml`) and moves only after the
  harness check passes, so anything relied on here should become a question in that check.
- Releases and installs happen at the owner's word; one small pull request per concern.

## What is already true (measured on this Mac, 2026-10-11)

- The harness **already finds** the person's skills: `~/.agents/skills` is a default-on provider
  (`agents`, priority 70, not treated as foreign), as is a conversation folder's own `.agents/skills`
  and `.omp/skills`. The agent inside a Null conversation can already read the Playbook by name
  (`skill://blueprint`).
- The harness **already names every command it has** to Null: a probe of `omp acp`
  (`initialize` → `session/new`, no prompt sent) received **65 commands**, 15 of them the Playbook as
  `skill:assess`, `skill:blueprint`, `skill:build`, `skill:commit`, `skill:diagram`, `skill:drive`,
  `skill:grill`, `skill:handoff`, `skill:init`, `skill:pr`, `skill:promote`, `skill:prototype`,
  `skill:recall`, `skill:research`, `skill:visuals`, each `{name, description, input.hint}`.
- **Null throws that list away** (`translate.rs::event_from_update` has no arm for
  `available_commands_update`; `Mini/src/history.rs:711` fabricates the same frame in a test only to
  drop it), **never draws it**, and **swallows a bare `/word`** (`Page/index.html:2892`,
  `/^\/[\w:-]*$/` → the box's own list, per decision 7). So `/research` does nothing, while
  `/skill:research the parser` — a form with a space — does reach the harness today.
- The harness **dispatches and expands** file commands and `/skill:<name>` itself, inside
  `AgentSession.prompt`; a client sends raw text and must not pre-expand; a `/word` matching nothing
  falls through as ordinary prompt text. **Seen, not just read (probe 2, below):** `/probe-cmd galaxy`
  sent as ACP prompt text was written into the conversation as the command's own body with `$1`
  filled in, and `/skill:probe-skill` produced a `skill-prompt` message naming the skill — found from
  the conversation folder's own `.agents/skills`, which also shows that a Folder's set follows the
  conversation.
- **Null can say something to the agent without any new folder**: `--append-system-prompt <file>`
  exists (`omp://system-prompt-customization.md`), and Null already writes files into its own support
  folder and passes flags before `acp` (`harness.rs:627-634`, `settings_args` at `683-698`). One
  limit to remember: append text reaches the **main session only** — a task subagent does not inherit
  it (the same doc, "Inputs by session type"). Skills *are* carried into subagents.

## The direction

**Two things, one of them already most of the way done.**

1. **Open the door.** Null reads the list it is already being sent, draws it in the command list the
   box already has, and lets a word through that the harness says it knows. This is what makes
   `/research` work — for the owner, and for anyone else, because the list is theirs.
2. **Let the box speak, if and when it has something to say.** Null's own words to the agent go in the
   file Null already writes and are passed with `--append-system-prompt`. No directory, no shipped
   skills, no new word.

Nothing is copied into Null, nothing is linked, and there is exactly one place a person's skills live.

### Deferred, with the trigger written down

**Null shipping a skill of its own.** Not now: nothing has asked for it, and the box's words are
already covered by item 2. When it happens — and the first real reason will be the agents Null is to
spawn, since append text does not reach them while skills do — the shape to revisit is: a folder of
text (`skills/<name>/SKILL.md`, one level, no code) inside `Null.app`, attached to the launch with one
flag, never written into the harness's folder, never merged with the person's own. Record the reason
when it is taken up; do not build the folder before there is a first skill to put in it.

## Work items

1. **The box takes the harness's commands** (the whole of the owner's ask; needs no decisions)
   - What: `translate.rs` gains `commands_said`, which turns the frame into
     `{ "type": "commands", "commands": […] }`, keeping the three fields it carries (`name`, and the
     harness's own words and hint) and nothing invented. The event rides the ordinary path, so it is
     kept in its conversation's own events and `events_since` replays it to a page that has just
     loaded; a frame that arrives while a conversation is being loaded back is kept for that one too
     (`harness.rs`, `open_earlier`). **No new command was needed:** the `harness_commands` command this
     plan first named is dropped, the kept event being the whole of it. `Page/index.html` draws the
     list in two sections, `the box` and then `the harness`, each lining its words up with its own
     widest name; a `skill:<name>` entry is offered by its bare name and sent as `/skill:<name>`, and
     everything else is sent as it is named.
   - The typed rule (line 2892) becomes: a bare slash-word that is the box's own command is carried
     out here, as now; a bare slash-word the last list knows is **sent**; anything else opens the
     box's list, exactly as decision 7 settled. So `/research` works, `/model` stays the box's, and a
     real slip still opens the list with "no command like it".
   - Why: the harness already offers everything and expands it; the box only has to stop dropping it.
   - Depends on: none.
   - Risk: low, and now measured (probe 2): the harness expands a typed command and dispatches a
     typed `/skill:<name>` when the box sends them, so the box's only job is to send them. The list
     must stay a scrolling pane: the notice line holds about 81 characters, the box's own thirteen
     already fill 78, and the harness's list is another ~65 rows.
   - Source: measured on this Mac (probe above) + researched finding
     (`omp://slash-command-internals.md` §5, §7; `omp://rpc.md`) + inferred from codebase.
   - Status: Complete (2026-10-11). Installed for the owner's trial as 0.5.0, mended as 0.5.1; not
     released, and no tag exists.
   - **Built:** `Mini/src/translate.rs` (`commands_said`, `command_from`, the match arm, one unit
     test); `Mini/src/harness.rs` (`open_earlier` keeps the frame a conversation's load brings);
     `Mini/src/history.rs` (`replayed` leaves a command frame out of a conversation's replay: it is
     what the harness can carry out, not what was said in it); `Mini/Page/index.html` (`harnessSaid`,
     `saying`, `harnessAbout`, `allAbout`; two sections in `drawCommands`; `commandsLike` ranks the
     box's own first and its keys walk the same order the sections are drawn in; a command typed out
     whole is sent in the harness's spelling; `send` rewrites a name the harness offered, bare or
     followed by arguments; `emptyBox` clears the list for a new conversation). No new command, no new
     file, and nothing written outside the app's own folder.
   - **Proved:** `cargo test` 112 pass, one more than before (`the_commands_a_harness_says_it_has_are
     _kept_for_the_box`); the count was 111 and no test was re-pinned. Three scripted runs of the real
     box on folders of its own: `/blueprint` typed bare is drawn and carried out as `/skill:blueprint`;
     a lone `/zzz` opens the list under `NO COMMAND LIKE /ZZZ` with `THE BOX` and `THE HARNESS`
     sections, so decision 7 stands; the harness's rows carry its own words, cut where the box has no
     room. Not yet seen by the owner's hand, and not run against a conversation in a project folder.
   - **Mended, 0.5.1 (2026-10-11), after the owner's first look:** "I don't see my commands". The box
     cannot know what the harness can carry out until a session has started, and a session starts
     with the first thing asked of it. A box the person had asked nothing of yet therefore offered
     its own commands only, and a bare `/word` typed as the very first thing was taken for a slip.
     `harness.rs` gains the order `Commands` and the command `harness_commands`, which opens the
     conversation and so starts the harness when nothing has been asked of it, letting the frame
     arrive; `Page/index.html` asks when the list is opened and before deciding that a lone `/word`
     is a slip, waits a moment for the answer, says `asking what it can carry out` in the harness's
     place meanwhile, and never waits on the app forever (the ask is raced with a timeout, and the
     wait ends as soon as the frame arrives). Proved: a run whose only action is opening the list
     starts the harness where it previously started nothing; a bare `/blueprint` typed as the very
     first thing is drawn as `/skill:blueprint` and carried out; the list that follows holds
     `THE BOX` and then `THE HARNESS`. `cargo test` 112 pass, no test re-pinned. The first `/` of a
     run now takes about a second while the harness starts, which is the cost of asking it.
   - **Look, 0.5.2 (2026-10-11), after the owner's second look:** "But they don't turn blue like the
     rest of the actual commands." The harness's commands were left plain in the list and untinted as
     they were typed, on the old rule that the command's colour meant the box's own commands. Blue is
     now a command the box can carry out, whichever of the two carries it out; a word neither of them
     knows stays plain, which is what a slip looks like. The palette rule in `ConversationDisplay.md`
     records the widening, the list's two headings still say whose each one is, and the now-unused
     list of the box's own names in the page was removed with it.

2. **The box's own words to the agent** (small, and only what the owner wants said)
   - What: when there is something the agent should know and the harness cannot tell it — starting
     with "what this box is", which is `ConversationDisplay.md` item 2, still open — Null writes it as
     one text file in its own support folder (as it already does for `harness.yml`) and passes
     `--append-system-prompt <that file>` in the argv it already builds. Nothing new is created in the
     harness's folder, and no command or skill is added.
   - Why: a fresh Null, on someone else's Mac, otherwise hands the agent nothing but a folder.
   - Depends on: none.
   - Risk: the flag may not reach `acp` from its place in the argv — it must be seen doing so. And
     append text does **not** reach task subagents, so nothing here may be treated as reaching them.
   - Source: researched finding (`omp://system-prompt-customization.md`) + inferred from codebase
     (`harness.rs` argv, `settings_args`).
   - Status: Not started.

3. **The questions the harness check must ask**
   - What: in `Mini/src/check.rs`, each named for what Null relies on: (a) the frame holds a
     conversation folder's own skills and commands, so a Null conversation in a project sees that
     project's set; (b) `--append-system-prompt` from Null's place in the argv reaches the session;
     (c) a skill invoked as `/skill:<name>` over ACP still arrives, so a newer Oh-my-pi cannot
     quietly stop dispatching what the box sends (probe 2 answers it for 18.8.7 today).
   - Why: the weekly `Null engine` job then asks every newer Oh-my-pi the same before Null moves to
     it, and (a) and (b) are not yet seen.
   - Depends on: items 1, 2.
   - Risk: none beyond the check's usual cost; `check.rs` already makes throwaway folders.
   - Source: the project's pattern (`BackgroundWork.md` item 7).
   - Status: Not started.

## Decisions the owner still owns

1. **One list or two in the box?** Recommended: one list, two sections, the harness's own words beside
   each entry, the box's own coloured as they are now.
2. **What the box calls them.** The harness's commands need a plain word in the list so they are not
   mistaken for the box's own ("from the harness"? "the agent's"?). Naming is the owner's.
3. **Whether item 2 is written now, or when the box first has something to say** (the note about the
   box is `ConversationDisplay.md` item 2, which has waited this long).

## Verification

- `cd Mini && Scripts/engine && cargo test` — the frame's framing, the typed rule, the new command.
- The box by hand: `/` shows both kinds; `/research` is sent and answers; `/model` still opens
  the box's own list; a slip still says so.
- `cargo test --bin null-mini -- --ignored --test-threads=4` — item 3's questions.
- A scripted run on a folder of its own (`NULL_MINI_FOLDER`), with and without the appended note.
- Nothing is added to the person's `~/.agents/skills`, and nothing is written to `~/.omp/agent`.

## Open questions and not established

- Whether `--append-system-prompt` keeps its effect when it sits before the `acp` subcommand
  (item 3b).
- What the frame holds when a conversation's folder is a project with its own commands (item 3a).
- The local `@oh-my-pi/pi-coding-agent` source copy here is 17.2.11 while the harness is 18.8.7, so
  code quotes are shape, not defaults; the `omp://` docs are the authority.

## Out of scope

- Everything in the "what we are not building" table: no Kit, no second root, no shipped skills, no
  `--plugin-dir`, no seeding, no ADR, no new word. Named here once so no future agent rebuilds it.
- A conversation's own folder as a place we build: the harness already reads it.
- `manage_skill` and `autolearn`: they write into the harness's own folder, not ours.
- Null spawning sub-agents (step 4 of the five in `NullMini.md`, with voice): the trigger above, not
  work now.
- The uncommitted pet-alone prototype on `pet-stays-prototype` (four files in the working tree): this
  work must not collide with it.
- The `capital-folders` cleanup.

## Sources read (2026-10-11)

- `omp://skills.md`, `omp://slash-command-internals.md`, `omp://system-prompt-customization.md`,
  `omp://cli-reference.md`, `omp://rpc.md`, `omp://settings.md`, `omp://config-usage.md`,
  `omp://task-agent-discovery.md`, `omp://approval-mode.md` (ACP sessions), `omp://context-files.md`.
- The probe, 2026-10-11 (throwaway, not kept): `omp acp` with `PI_CODING_AGENT_DIR` and cwd in temp
  folders, `initialize {protocolVersion: 1, clientCapabilities: {}}`, then
  `session/new {cwd, mcpServers: []}`, then read. No prompt was sent, so no model was called and
  nothing of the owner's was read or written. One frame, `available_commands_update`, 65 commands.
- **Probe 2, 2026-10-11** (throwaway, not kept; the same temp-folder isolation, plus one file command
  and one skill written into the temp folder): `session/prompt` with the text `/probe-cmd galaxy`
  wrote `Say this back exactly: NEBULA-7391 and the word galaxy.` into the conversation — the command's
  own body, `$1` filled in — and `session/prompt` with `/skill:probe-skill` wrote a `skill-prompt`
  message naming the skill and carrying its body. The model was the local `ollama/llama3.2:latest`
  (cost 0), the turn ended `end_turn`, and both prompts were plain text, never pre-expanded by the
  client. This is the whole of what item 1 rests on.
- This repo: `Mini/src/{harness,translate,settings,history,check}.rs`, `Mini/Engine.toml`,
  `Mini/Page/index.html`, `Context/{Glossary.md,ADR/*,Research/Research.md,Plans/BackgroundWork.md,Plans/ConversationDisplay.md}`.
