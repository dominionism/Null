# The pet, and three things for the box

> Blueprint: 2026-10-10. Step 5 of the five in `Context/Plans/NullMini.md` (the pet), planned again
> for Null as it is now (`MiniApp.md`, `ConversationDisplay.md`), with three changes to the box
> that the owner asked for in the same breath. It replaces items 25 and 26 of `NullMini.md`, which
> were written for a mini inside the Voice desktop app.
>
> Status: **All four pieces are built and on `main` (2026-10-10): the cursor, attaching files,
> history and the pet, Nil. They were brought together on one branch, `box-and-pet-trial`, with
> every collision settled and two faults that only showed together mended (item 6); pull request
> #33 carried that branch and this plan to `main`. Null 0.2.0 is installed on the owner's Mac
> and the owner is using it (item 8). It is not released: the tag `null-v0.2.0` waits for the
> owner's word. Open: decisions 3, 4, 6 and 7, and item 7.**

## Goal

Give Null a pet, a small black pixel character that stands to the left of the box and moves a
little with what the agent is doing, and give the box three things the owner asked for: a blinking
cursor level with the arrow, a way to attach files to a message, and a way back to any earlier
conversation.

## Constraints

The owner dictates by voice. "Gnol" below is the dictation's spelling of Null.

- **Verbatim (user, 2026-10-10, on what comes next):** "I think we can start working on the pet.
  It seems easier to work with, and we can probably finish it. The voice is definitely going to be
  a big one (requiring a significant amount of planning -- let's defer for now)."
- **Verbatim (user, 2026-10-10, the cursor):** "I noticed that the text region is not fully, you
  know, centered and aligned with the arrow. It seems like the text area is, or not necessarily
  the text, but more so the blinking stick when indicating where the next character would be is
  not aligned horizontally with the tip of the arrow on the text section. I was wondering if we
  can fix that."
- **Verbatim (user, 2026-10-10, files):** "I want to add something to the null text UI, perhaps
  like a plus button, which will be used for, you know, navigating files. So I can upload stuff. I
  can upload files to my Null Mini so that they can gain access and have context about a
  particular file that, you know, that I want them to have for a particular conversation."
- **Verbatim (user, 2026-10-10, history):** "I was also thinking perhaps having another button,
  perhaps to the all the way to the right, maybe like a small hamburger menu or some sort of icon
  that I can click on that is a history, right? So if I have a conversation, I don't want to,
  like, lose that conversation from Null in my Node. So I want you to have a particular, you know,
  history icon that I can click on, and perhaps we can have an organized history collection for
  Null, maybe categorizing the work by directory and an appropriate title. I don't know what. Feel
  free to get creative on the organization of my conversations. Don't limit yourself to just the
  category that I mentioned."
- **Verbatim (user, 2026-10-10, the pet):** "Let's get some inspiration and design the animations
  of the Gnol mini pet. I was more so thinking, you know, a black pixelated character that moves
  slightly, that is to the left of the Gnol text box. We can use Clyde design for this."
- **Verbatim (user, 2026-10-10, on the drawings):** "I really like the animations you made for
  Nil. Let's go with that. One thing I would change though is that it should be horizontally
  aligned with the arrow in the textbox. Alignment should always be consistent. In the image you
  provide, it looks like it's a little lower than the textbox. A part from that, it's perfect."
  So the rule: the pet's middle is level with the arrow's tip, as everything in the row is.
- **Verbatim (user, 2026-10-10, who does what):** "Lets keep this in the main agent and let others
  be sub-agents." "For the sub-agent work. Have them push their work as separate branches so that
  we can do one full check later on to ensure none of the branches create a merge conflict."
- **Verbatim (user, 2026-10-10, on landing it):** "We pushed things in separate branches. I want
  to know if they are all mergable without conflict. If not, I want you to merge them all for
  me into main." And, told that three of the four collide and that one pull request held them
  all: "Sounds good, let's commit the PetAndBox plan and merge to main."
- **This amends an older rule, at the owner's own word.** "All I really need to see is the text
  box and the arrow" (2026-10-08) now has two buttons and a pet beside it. They stay quiet: dim
  until the pointer is on them, no words, nothing that blinks for attention.
- **This sets aside a standing preference for these three items.** The last handoff recorded
  "Sub-agents: not for building". The owner asked for three by name here.
- From the plans this builds on, still in force:
  - Black and white. The only other colours are amber (the agent is asking), red (errors) and
    blue (the box's own commands, and links). One typeface, SF Mono.
  - No line down the left side of anything, ever. In the conversation everything starts on one
    edge, and nothing but the null sign stands left of it.
  - Everything the box does is a typed command. A button is a second way in, never the only one.
  - The box never makes Null the active app, and it hides completely.
  - The page is one static file with its style and script inline. No Node, no bundler.
  - Nothing a model, a file name or the harness writes is ever put on the page as HTML.
  - Null keeps no credential (ADR 0004) and knows nothing about any one provider (ADR 0005). The
    harness keeps the conversations; Null keeps only which one was open (`Glossary.md`).
  - Every height and width of the window is an exact contract between the page and `panel.rs`.
  - A feature is lost one at a time, never the whole box (`OwnHarness.md`, design rule 4).
  - Folder names are capitalized (ADR 0001).
  - Commits: Conventional Commits, split by concern, never an em dash, no tool or AI attribution.
    One pull request per concern.

## What was found (2026-10-10)

| Question | Finding |
|---|---|
| Is there pet art on this Mac | None. `~/.codex/pets` is empty and no sheet is cached |
| What the old sketch assumed | A React part and a `/pets` route in the Voice server, and sprite sheets in the Codex format (`NullMini.md`, items 25 and 26, architecture decision 9). None of that fits Null as built |
| What shows status today | The arrow alone: steady, pulsing while the agent works, amber while it asks (`mark()` in the page) |
| Where the cursor's fault may come from | The field is lifted one pixel (`top: -1px`) to put letters level with the arrow. That lift may be what puts the blinking cursor off the tip. To be measured, not assumed (item 1) |
| Can the harness list conversations | Recorded for Oh-my-pi 18.4.3 in `NullMini.md` (list, resume, fork, close). Never tried on the carried 18.8.7, and nothing in Null uses it (item 3) |
| Can the harness take a file | The same probe recorded image and embedded-context prompts. What the model really receives is not known (item 2) |
| What all four pieces share | One file, `Mini/Page/index.html`. All four change it, so the branches will not merge without a hand (item 6) |
| Room on the disk | About 12 GB. A cold build takes several, so each sub-agent works on a clone of the built dependencies |

## Design rules

1. **The pet is the status with a body, and says nothing.** It shows what the arrow shows:
   resting, working, asking, done, failed. No words, no bubble.
2. **The pet lives with the box.** It stands to the left of it and shows when the box shows. It
   does not stay on the screen when the box is hidden.
3. **The pet is drawn, not loaded.** Its frames are small grids of pixels kept in the page and
   drawn by the page, as the arrow and the null sign are drawn. No picture file.
4. **A button is quiet and has a typed twin.** `/attach`, `/history`, `/pet`.
5. **A file is attached, not uploaded.** It stays on the Mac; the harness is told about it with
   the message. Its content goes to the provider only when the agent reads it, as in a terminal.
6. **History is the harness's own list.** Null draws it and keeps no conversation's content.
7. **One sizing contract, changed once.** The pet widens the window. That change is made in one
   place, at the full check, with the page's comment and `panel.rs` together.

## Decisions for the owner

1. **Which pet: Nil** (owner, 2026-10-10: "I really like the animations you made for Nil. Let's
   go with that"), level with the arrow.
6. **Whether the typed letters now sit too low.** Levelling the cursor (item 1) took back a
   one-pixel lift of the field that the owner had asked for on 2026-10-08, when the complaint
   was text that did not sit centred. The cursor is now exactly on the arrow's tip and the
   letters are one pixel lower than they were. The typeface ties the two together, so both
   cannot be exact. By eye, on the installed app.
7. **Whether Null may keep titles.** History keeps a title for each conversation made in the
   box, the first line of its first message, in a file of Null's own (item 3), because the
   harness gives none over the protocol. That is a little of a conversation's words, and it goes
   past "Null keeps only which one was open" in the glossary and design rule 6 here. Without
   it, every conversation made in the box is "untitled". The owner's word is needed; the
   glossary line follows it.
2. **"Clyde design".** Taken as Claude Design, the canvas the drawings of item 4 are made on. If
   the owner meant the look of Claude Code's own pixel mascot, one of the drawn directions is
   close to it in build, and the owner says so. Shipping that mascot itself in an app for other
   people would be Anthropic's to allow, as `NullMini.md` already says of OpenAI's pets.
3. **Whether the arrow still pulses once the pet shows the work.** Two things moving for one
   status may be one too many. By eye, on the installed app (item 5).
4. **Whether history shows the terminal's conversations too,** under their folders. The
   sub-agent finds out whether the harness allows it (item 3); showing them is the owner's call.
5. **Loading other people's pets** in the Codex format. `NullMini.md` decided the pet "uses the
   Codex sprite contract". This plan draws Null's own pet first and leaves that format out
   (Out of scope). The owner says if it should come back.

## Work items

1. **The blinking cursor, level with the arrow's tip**
   - What: in `Mini/Page/index.html`, the field's own rules (`input`, `#ruler`) and the arrow's
     (`.caret`). Measure, in the app's web engine at 1x and 2x, where the arrow's tip, the
     cursor's middle and the middle of the letters are; then the smallest change that puts the
     cursor's middle on the tip while the letters still look level. A cursor drawn by the page is
     the last resort.
   - Why: the owner's own words above.
   - Depends on: none.
   - Risk: the letters and the cursor cannot both be exactly level, since the lift that centres
     one moves the other. Know it by: both measured before and after, and the choice said.
   - Source: specified from user + inferred from codebase (the comment on `top: -1px`).
   - Status: In progress. Built by a sub-agent on the branch `caret-level` (`525e8f1`,
     2026-10-10), which reaches `main` with pull request #33; not yet seen by the owner
   - **Built:** the field's one-pixel lift is gone (`position: relative; top: -1px` in the
     `input` rule), with a comment that records what was measured. Nothing else changed.
   - **Why the fault was there:** the web engine draws the cursor 16 px tall in the middle of
     the field's 22 px line, and the row already puts that line on the arrow's tip. The lift
     carried the cursor exactly one pixel above the tip.
   - **Measured in the app's web engine, in pixels from the top of the window (the tip is at
     30.00):** the cursor's middle went from 29.00 to 30.00, at 1x and at 2x, in an empty
     field, after text, in the middle of text and in a sign-in's hidden field. Small letters'
     middle went from 30.42 to 31.42, and capitals' from 29.25 to 30.25.
   - **The trade, and it was forced:** the typeface sets its cursor's middle 5.0 px above the
     baseline, capitals' 4.75 and small letters' 3.58. Line heights of 18, 22 and 27 px gave the
     same places, so nothing but an offset moves one against the other. Set aside: half a pixel
     (drawn unevenly at 1x), moving the arrow up instead (same result, but off the row's
     middle), and a cursor drawn by the page (it would have to follow selections and input
     methods). See decision 6.
   - **How it was seen:** a scratch program puts the page in a real web view in a window far
     off the screen that says it has the keyboard, so the engine paints its own cursor; fourteen
     pictures across a blink, and the ink measured. No permission was asked for.
   - **Not proven:** the owner's eye, on both questions. The real app and the real screen. A
     real 1x display (1x was an override). Text being composed by an input method.
   - **GitHub:** the unit tests and the harness check passed on the branch.

2. **Attach files to a message**
   - What: a plus button in the row, right of the field, and `/attach`. A way to find a file
     without leaving the box (a list in the box, built on the one `/model` uses; macOS's own file
     chooser only if a trial shows it neither makes Null the active app nor hides the box). What
     is attached shows before sending and can be taken off. The files go with the next message
     as the protocol's content blocks, in the form the carried Oh-my-pi is seen to pass on to
     the model. A new module, `Mini/src/attach.rs`. A question in the harness check.
   - Why: the owner's own words above.
   - Depends on: none.
   - Risk: the file chooser taking the box away; a file that reaches the harness but not the
     model. Know it by: the stand-in provider, which shows exactly what arrived.
   - Source: specified from user + inferred from codebase (`panel.rs`, `harness.rs`).
   - Status: In progress. Built by a sub-agent on the branch `attach-files` (three commits,
     `452c344`, `9cc2c31`, `2ced2f5`, 2026-10-10), which reaches `main` with pull request #33;
     not yet tried by the owner. The sub-agent was cut off once by the usage limit and resumed;
     nothing was lost
   - **Built:** `/attach` opens a list of the Mac's folders inside the box, on the list the box
     already has. Typing narrows, Enter opens a folder, attaches a file or takes an attached one
     off, `..` goes up, and a path typed from `/` or `~` goes straight there. It opens where it
     was last, for the run. `/attach <path>` attaches at once. The plus button, right of the
     field, opens the same list. What is attached stands on the notice line ("attached: a · b")
     until sent, and a press on a name takes it off. The sent message carries a dim line
     "attached: …" on the reading edge, and a conversation loaded back shows it again. A new
     module, `Mini/src/attach.rs`. No height changed.
   - **Decided, macOS's own file chooser is not offered.** Tried in two short runs of a debug
     build. With Null active, the chooser took the keyboard, the box lost it, and the chooser
     cancelled itself within a second. With another app active, which is the box's usual state,
     asking for the chooser never came back and the app hung until it was stopped. That says
     nothing certain about the signed, installed app; it is not to be tried again from a debug
     build.
   - **Decided, how a file reaches the agent:** as the protocol's link, for every kind of file,
     and Null reads no content. Seen on Oh-my-pi 18.8.7: a link gives the model only its title,
     so the title is "attached file: <full path>", and the agent then reads the file with its
     own tool (text by line, a PDF's words, a picture as a picture where the model takes
     them). Set aside: Oh-my-pi's own `@"path"` mention, which puts the content in the message
     at once and so goes against design rule 5; it is one function to change, `attach::blocks`.
   - **Where things go, said plainly:** the file's full path, with the user's name in it, goes
     to the provider with the message. The content goes only when the agent reads the file.
     Null's log holds only how many files went, never a name, a path or content.
   - **Edges, each one line in the box:** a file that is not there, one that cannot be read,
     one that went missing before sending (the message is held back), and a harness that
     refuses a message with files ("this harness does not take attached files. send the message
     again without them"). No size limit, since only the path travels.
   - **Deviation:** no line in `translate::abilities`. The protocol requires every agent to take
     a link, so nothing in the harness's greeting decides it; a refusal is met when it comes.
   - **Proven without a person:** 77 unit tests, eight new. A new question in the harness check,
     that the agent is told where an attached file is and can read it, with a file outside the
     conversation's folder and a name with both kinds of quote and Japanese letters; it fails
     by name when the title is taken away. The real app by script, with the stand-in: one file
     and a message, two files through the list, one taken off, a path that is not there, a file
     deleted before sending, and a restart that shows the earlier message with its line.
     Twenty states drawn dark and four light, all on the 50 px edge. On GitHub the unit tests,
     all 16 questions of the harness check and the key check passed.
   - **Not proven:** a real pointer on the installed app (that typing stays in the field, the
     look under the pointer). A real model on a real provider reading a real file. macOS asking
     about Desktop, Documents or Downloads when the list walks into them without Full Disk
     Access. A harness that asks before acting, and "yours" at another version. A folder of
     more than 20,000 entries.
   - **Learned, and it matters for item 3:** a conversation loaded back is drawn in the box only
     if the box is empty when it loads. After a restart, the first message drops the replay.
   - **Not built:** dragging a file onto the box. A way that might work: start the drag in
     Finder, press Control+Space with the file held, and drop it on the box. Not tried.

3. **History: the way back to any conversation**
   - What: a history button at the far right of the row, and `/history`. A list of earlier
     conversations from the harness's own list, each with a title, organised (by when, by
     folder, narrowed by typing; the sub-agent chooses the few that make a hundred conversations
     easy in ten lines). Choosing one loads it back and shows its messages. A new module,
     `Mini/src/history.rs`. A question in the harness check. No deleting.
   - Why: the owner's own words above. Today `/new`, `/quit` and Ctrl+C drop Null's pointer to
     the conversation, and there is no way back to it.
   - Depends on: none.
   - Risk: the carried harness not listing conversations, or not the terminal's. Know it by: the
     first trial, on a folder of its own. Then the feature shows Null's own, or says in one line
     that this harness cannot.
   - Source: specified from user + inferred from codebase (`harness.rs`, `settings.rs`).
   - Status: In progress. Built by a sub-agent on the branch `history` (eight commits, head
     `b061f18`, 2026-10-10), which reaches `main` with pull request #33; not yet tried by the
     owner. The sub-agent was cut off once by the usage limit and resumed; nothing was lost
   - **Built:** `/history`, `/history <words>` and a button last in the row. The list is the
     harness's own, in the space and the look of the box's other lists. It is sorted by when
     (today, yesterday, this week, then each month); Tab sorts it by folder instead, "the box"
     first, and Tab is offered only when there is more than one folder. Typing narrows by title
     and folder. A row has the title and, dim at the right, the folder's name for a terminal
     conversation and when it was last used; the open one is marked. Choosing a row loads the
     conversation in its own folder, draws what was said and continues it. A new module,
     `Mini/src/history.rs`, and `lists` in `translate::abilities`. No height changed.
   - **What Oh-my-pi 18.8.7 does, seen in folders of the sub-agent's own:** it offers a list,
     fork, resume and close, and no delete. The list needs no unstable part of the protocol,
     gives every conversation in every folder, fifty to a page (331 on seven pages took 27 ms),
     each with an id, a folder, when it was last used and how many messages; a title only when
     the harness has one, and it makes one only in the terminal, never over the protocol. A
     conversation loads only in its own folder.
   - **Decided, the terminal's conversations are in the list** (decision 4, still the owner's to
     confirm): listing them and loading one in its own folder both worked. It also mended a
     fault that would have come with it: at a restart Null opened the saved conversation in its
     own folder, so a terminal conversation would have been silently replaced by a new one.
   - **Decided, titles, and what Null keeps for them** (decision 7): the harness's title when
     there is one. Otherwise the first line of the first message sent from the box, cut to 80
     characters, kept in `history.json` beside the settings as a title for each conversation
     and nothing else. A title is dropped when its conversation is gone from the list.
   - **Decided, the icon:** a clock with an arrow going back round it, chosen from four drawn at
     real size. Three lines read as a general menu.
   - **Edges, each one line in the box:** asked for while a reply runs; no earlier
     conversations; nothing matches; a conversation that will not load (the box stays as it
     was); a harness that does not list.
   - **Proven without a person:** 75 unit tests, six new. Two new questions in the harness
     check: earlier conversations are listed with their folders and one loads back in its own;
     the list is read to its end a page at a time. Both fail by name against a program that is
     no harness. The real app by script, with the stand-in: two conversations left with `/new`
     and found again, one opened and continued, a restart, two conversations made as a terminal
     makes them in another folder and one of those opened and continued. Every state drawn. On
     GitHub the unit tests and the whole harness check passed.
   - **Not proven:** a real pointer on the button and Tab in the real app. The owner's real
     conversations: hundreds of them, a title the terminal really made, a conversation open in
     a terminal at the same moment, very long ones. Any harness version but 18.8.7.
   - **What the owner will meet first:** every conversation made in the box before this version
     is "untitled" until it is opened once. After opening a conversation left on another model,
     `/new` starts on that model.
   - **Set aside:** message counts in rows, search inside conversations, renaming, forking,
     deleting, a limit on very long replays.
   - **Two things the sub-agent said of itself:** it copied an older file over the main agent's
     scratch `snap.swift` (the built tool was not touched), and four of its scripted runs
     showed the box for about half a minute each, one of which took the keyboard from the
     owner's terminal.

4. **The pet's look: directions drawn for the owner to choose**
   - What: on a Claude Design canvas, a handful of small black pixel characters, each drawn at
     its real size to the left of the real box, on a dark and on a light backdrop, and each
     moving in five ways: resting, working, asking, done and failed, with the still frame for
     people who have asked macOS for less motion.
   - Why: "Let's get some inspiration and design the animations". A look is chosen by eye.
   - Depends on: none.
   - Risk: a black character is not seen on a dark desktop. Know it by: both backdrops in every
     drawing; a light edge or light eyes where needed.
   - Source: specified from user.
   - Status: Complete (2026-10-10). The owner chose A, Nil, and asked for one change: level
     with the arrow. The first board of the canvas shows Nil that way now
   - **Drawn:** four directions on one canvas, https://claude.ai/artifact/NmGWxNqHgwZxFjW4GAyAWm
     (private to the owner). One board shows all four resting beside the box at its real size,
     on a dark and on a light backdrop; then a board for each with its five moods, large and at
     real size.
     - **A, Nil:** the null sign with eyes and feet. The sign's stroke is its antenna and tail.
     - **B, Block:** a wide block on four legs, a cousin of Claude Code's mascot.
     - **C, Ghost:** a small ghost that hovers; its hem ripples.
     - **D, Cat:** a sitting cat; its paws knead and its tail asks.
   - **What all four share:** a grid of 16 by 16 pixels shown at three times its size, so 48 px
     beside the 44 px box, 10 px to its left, feet on the box's lower edge. A black body with a
     one-pixel light edge, as the box has, so it shows on a dark desktop. White eyes that turn
     amber when the agent asks and red when a reply fails. Movement is one pixel at a time:
     resting breathes and blinks, working taps its feet and looks about, asking looks up and
     waves, done hops once, failed sinks and shudders once.
   - **Seen by the owner:** the canvas drew and moved; the owner chose from it.

5. **The pet in the box**
   - What: in `Mini/Page/index.html`, the chosen character's frames as data, drawn as whole
     pixels in a lane to the left of `.box`; it follows the status the page already keeps
     (`mark('')` resting, `mark('working')`, `mark('asking')`, a short gesture at `message_done`
     and at `reply_failed` or `error`, then resting). In `Mini/src/panel.rs`, `WIDTH` grows by
     the lane, and the saved position goes on meaning where the box is, so that the box does not
     jump on the first start after the update. `/pet` shows and hides it, kept in the setting
     `pet`. With less motion asked for, one still frame. A press on the pet moves the box.
   - Why: the goal.
   - Depends on: 4.
   - Risk: the wider window takes presses meant for whatever is behind the empty part of the
     lane; the box jumping sideways for people who already have a saved position. Know it by:
     the installed app, and a unit test on the position.
   - Source: specified from user + inferred from codebase (`mark()`, `panel.rs`, `settings.rs`).
   - Status: In progress. Built on the branch `pet` (`33afe8b`, 2026-10-10), pushed at the
     owner's word the same day, which reaches `main` with pull request #33; proven by pictures
     and on the real window, not yet seen by the owner in the app
   - **Built:** in the page, Nil as drawn paths with its five moods and their movements as the
     owner approved them, 48 px, 4 px in from the window's left, with 10 px between it and the
     box. `mood()` follows `mark()`: the pet changes when the arrow changes. When a reply ends
     it hops once and rests; when one fails, or the model refuses, or an error comes, it sinks
     with flat red eyes and stays so until the arrow next changes; a reply the owner stopped
     ends with no gesture. `/pet` is among the box's commands. In `panel.rs`, `LANE` (46 px),
     the window's width with and without it, and two commands, `pet` and `set_pet`. The
     setting `pet` in `settings.rs`; nothing there means the pet shows.
   - **Level with the arrow, measured in the app's web engine (pixels from the top of the
     window):** the arrow's tip 30, the row's middle 30, and the pet's picture, its body, its
     eyes and its whole figure all 30. The pet's body, eyes and figure share one middle, so
     there was one line to put on the tip. The cursor branch does not move the arrow, so this
     holds after item 6.
   - **Deviation:** the remembered position is not the window's own corner any more. It is the
     corner the window has without the pet, which is what was always saved. So nothing has to
     be changed in anyone's settings, and the box does not jump at the first start.
   - **Deviation:** the gestures are made once. On the canvas they went round and round so
     that they could be seen.
   - **Decided by the agent:** the pet shows unless it is put away, for the owner and for
     anyone else. `/pet` says nothing, since the pet coming or going is the answer.
   - **Proven without a person:** 70 unit tests, one new: a saved position means the same
     place with the pet and without. Seven states drawn in the app's web engine and looked at:
     resting on a dark and on a light backdrop, working, asking (amber eyes, looking up), done
     (glad eyes), failed (flat red eyes, one pixel lower), stopped (resting), and put away (the
     box back at 16 px, `set_pet` asked for). The real app on a folder of its own with the
     stand-in: a message answered, then `/pet` three times, logged "the pet is put away", "the
     pet is shown", "the pet is put away", and the setting saved. The real window, read from
     the window server: with a saved position of (600, 400) it stands at x 254 and is 662
     wide with the pet, and at x 300 and 616 wide without, so the box's own edge is at 316
     both times.
   - **Not proven:** the owner's eye on the real thing, above all how the movements feel
     beside a real reply, and decision 3 (the arrow still pulses too). The window's place
     read back after a live `/pet` (only the two starts were read). A second screen with a
     different number of pixels to the point. The harness check was not run on this branch;
     nothing it asks was touched.
   - **Scratch, not in the repo:** the look tool of this session, in the session's scratch
     folder under `look/` (`prep.py`, `snap.swift`, `probe.js`, `scenarios.json`,
     `bounds.swift`). `ConversationDisplay.md`, item 1, is still where such a tool would be
     kept.

6. **One full check: the four branches together**
   - What: merge `caret-level`, `history`, `attach-files` and `pet` in that order into one trial
     branch. Settle by hand what collides in the page: the row's two buttons (one shared rule,
     `.tool`), the lists, the commands the page knows, and the one sizing contract. Then the
     unit tests, the harness check, the self-test on a folder of its own, and pictures of every
     new state. Only then one pull request per branch.
   - Why: the owner's own words: "one full check later on to ensure none of the branches create
     a merge conflict."
   - Depends on: 1, 2, 3, 5.
   - Risk: a collision settled wrongly passes the tests and looks wrong. Know it by: the
     pictures, and the owner's hands on the installed app.
   - Source: specified from user.
   - Status: Complete (2026-10-10). The check is done on the branch `box-and-pet-trial`, and
     pull request #33 carries that branch to `main` at the owner's word (Constraints). One pull
     request and not four, since the collisions would otherwise have to be settled again in
     each. Nothing is installed
   - **The trial branch:** `main`, then the four branches merged in the order above, then two
     small fixes the check found. Each branch's own commits are kept.
   - **Which branches collide, each pair tried with nothing else:** `caret-level` merges with
     each of the other three. `history` and `attach-files` collide, `history` and `pet`
     collide, and `attach-files` and `pet` collide. Each of the four alone merges with `main`.
   - **What collided:** `caret-level` and `history` merged by themselves. `attach-files`
     collided with `history` in nine places in three files, and `pet` with the rest in four
     places in the page. Every one was a place where both sides had added to the same list:
     the commands the page knows, the line for a mistyped command, the page's opening comment,
     the row's buttons, what `send` does with a command, two `use` lines and the line that
     builds a message for the harness. Each was settled by keeping both. The buttons' shared
     rule, `.tool`, merged by itself, being the same words on both sides. The plus comes
     first and the history button last.
   - **Found only with the branches together, and mended on the trial branch:**
     - The pet could go on showing a failed reply after another conversation was opened or a
       new one begun, since neither always changes the arrow. It rests at both now
       (`671d194`).
     - The line that answers a mistyped command no longer fitted. It holds about 81
       characters; on `main` it was already cut off before `/new` and `/quit`, and with eleven
       commands it ended at `/update`. It is now the commands alone, 78 characters, read from
       the page's own list (`5ed12dc`). Decided by the agent; the owner may want other words.
   - **Proven without a person, on the trial branch:**
     - 84 unit tests (69, and 1, 8 and 6 new).
     - The whole harness check, 18 questions, and the key check.
     - One line for the whole row, measured in the app's web engine: the arrow's tip, the
       row's middle, the field, the pet and both buttons are all at 30 px from the top of the
       window.
     - Eight states drawn and looked at, among them the row with the pet and both buttons,
       the history list, a reply with its "attached" line, and what is attached before
       sending.
     - The real app by script on a folder of its own with the stand-in: a file attached and
       read by the agent; a new conversation; history narrowed by typing and the first
       conversation opened in its own folder, shown with its "attached" line and continued;
       the pet put away; a mistyped command answered with the eleven commands.
   - **Not proven:** anything by the owner's hand on an installed Null. That is where the
     open decisions are judged (3, 4, 6 and 7).

7. **Say it**
   - What: `/grill` for the new words (pet, attach, history). The README's commands. The
     research map (`Research.md`): the new modules, commands and events.
   - Depends on: 6.
   - Status: In progress. The README is done (2026-10-10); the glossary and the research map
     are not
   - **Done, the README:** its cards now say the three commands and what a mistyped one gets,
     the pet, that an attached file travels as its path and that Null reads none of its
     content, the titles Null keeps in `history.json`, and the two new source files. The pet,
     attaching and history moved from "planned" to "available", with the note that they still
     wait for hands-on use.
   - **Known to be behind once this lands:** `Research.md` (24 commands become 32; the events
     `conversation_opened` and `mini:attached`; 15 live tests become 18; `history.json` among
     the files Null keeps; the modules `attach.rs` and `history.rs`; the window's width). The
     glossary's Conversation line, by decision 7. The line about what a harness can do, quoted
     in `OwnHarness.md` item 3, which now also says "lists earlier conversations".

8. **Null 0.2.0: tried by the owner first, then released**
   - What, in this order, as the owner agreed on 2026-10-10 ("Go", to the order below):
     1. The version is set to 0.2.0 in `Mini/Cargo.toml` and `Mini/tauri.conf.json`, and the
        README is brought up to date (item 7). The number publishes nothing; only a tag does.
     2. `main` is built on the owner's Mac and installed over Null 0.1.1 with
        `Mini/Scripts/install`. The owner uses it.
     3. Whatever the owner's hands and eyes find is mended on `main`.
     4. The tag `null-v0.2.0` is pushed on `main`. GitHub builds and publishes the release, and
        the owner installs from it, so as to end on what everyone else gets.
   - Why: nobody has used these four things by hand, four decisions wait for the owner's eye
     (3, 4, 6 and 7), and a release is public. 0.2.0 and not 0.1.2, since it adds a pet and two
     features.
   - Depends on: 6.
   - Risk: what the owner tries is built on this Mac, not by GitHub. Know it by: step 4, which
     ends on GitHub's build. Also, an installed 0.2.0 will not see `/update` say that 0.2.0 is
     out, so that proof (`OwnHarness.md`, item 7) waits for the release after this one.
   - Source: specified from user.
   - Status: In progress. Steps 1 and 2 are done (2026-10-10). Step 3, the owner's own use, is
     under way: one find so far, mended and installed. Step 4 waits for the owner's word
   - **Step 1:** pull request #34. `main` says 0.2.0 and the README is up to date. The unit
     tests, the harness check and the page check passed on it.
   - **Step 2, the install:** `Mini/Scripts/install` on `main` at `94b4f39`. The owner had quit
     Null from the box a little before, so no reply was cut off. Afterwards: the app in
     `/Applications` says 0.2.0; its seal holds and it has the designated requirement it had
     before, so macOS knows it as the same app; the Oh-my-pi inside has the checksum of
     `Engine.toml`; the login item reports it running; the log says "started, version 0.2.0"
     and "Full Disk Access is on"; and the keychain list is as it was.
   - **Not proven by the install:** anything the owner has to see or do, starting with whether
     the box stands where it was left now that the pet has its room beside it.
   - **Step 3, what the owner's use turned up:**
     - **A row's highlight, from edge to edge** (owner, 2026-10-10: "when I am hovering over an
       item, would it be possible if the lightish highlight goes all the way through left and
       right instead of noticable border-radiused container over the hovered item?"). Built the
       same day: in every list the highlight, under the pointer or chosen with the keys, is a
       band from the box's left edge to its right, measured at both (63 and 645 px of the
       window, which are the box's own inner edges). The words of a row start where they did.
       One thing followed from it: a list long enough to scroll kept a lane for its scroll bar,
       which stopped the band 10 px short. So lists draw no scroll bar now and fade at the end
       where there is more, as the conversation does, and the chosen row is kept clear of the
       fade. That mended an older fault on the way: the chosen row was scrolled to before the
       rows after it were drawn, so with the keys it always came to rest against the list's
       very end. Decided by the agent, and the owner may want the scroll bar back.
       Pull request #35, merged (`5ab85e3`). The app was built from that branch's tip
       (`56f69dd`) and installed about half a minute before the merge; the merge changed no
       file, so the installed app is `main`'s code.

## Verification

- Unit tests (`cargo test`): whatever each branch adds, and the saved position after the window
  grows (item 5).
- The harness check (`cargo test -- --ignored`): the two new questions (items 2 and 3).
- The self-test on a folder of its own with the stand-in provider: a file attached and seen by
  the provider; a conversation left with `/new` and found again with `/history`.
- Pictures in the app's web engine at the real size, on a dark and a light backdrop: the row
  with both buttons, each new list, the pet in each of its five ways.
- By hand, the owner, on the installed app: the cursor against the arrow; attaching a real file
  and asking about it; finding yesterday's conversation; the pet while a real reply runs.

## Validation

- No ADR is contradicted. History keeps to "the harness keeps the conversations". Attaching
  keeps nothing of a file in Null.
- **Bent on purpose, at the owner's word:** items 1 to 3 are being built before their design is
  written here. The sub-agents decide inside the rules above and report; their decisions are
  written into this plan when they come back, and nothing is merged before item 6.
- **Collisions are expected, not only possible.** All four branches change the one page file.
  The two buttons were given to both sub-agents as the same lines, word for word, to keep the
  collision small.
- **A decision of `NullMini.md` is set aside** (decision 5 above): the Codex sprite format.
- **A glossary line needs care:** "Box ... Avoid: pet". The pet is a new thing beside the box,
  not another word for it. Item 7.
- Still fuzzy, on purpose: the pet's look (item 4 decides) and so the lane's exact width.
- No outside research was needed. Pixel animation is known ground, and the reference product is
  already in `Context/Research/ChatGPTPetsAndMini.md`.

## Out of scope

- The pet staying on the screen while the box is hidden. It is worth having once work runs in
  the background (`NullMini.md`, step 3).
- Other people's pets in the Codex format, a pet chooser, pet sizes.
- Dragging a file onto the box. Deleting a conversation.
- Voice, and work in the background.
