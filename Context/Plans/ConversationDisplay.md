# How the box shows a conversation

> Blueprint: 2026-10-09. Builds on `Context/Plans/MiniApp.md` (the box as built, its sizing contract
> and the look the owner has approved) and `Context/Plans/Providers.md` (the lines the box says about
> switches and failures).
>
> Status: **The chosen look is built and installed (items 3 to 7, 2026-10-09): direction A, the null
> sign, literally everything on one left edge, nothing around code and diagrams. The owner has not
> yet used it. Not started: the look check in the repo (1), the note to the agent (8, the owner's
> call), more height (9, only if asked) and the content policy (10).**

## Goal

Make a conversation in the compact box pleasant and easy to read: the agent's reply laid out as what
it is (headings, lists, code, tables, diagrams) and not as raw terminal text, on one reading edge
where nothing wraps back under the arrow, without making the box bigger or busier.

## Constraints

- **Verbatim (user, 2026-10-09):** "Since the null terminal is compact, it's harder to display text
  more appropriately."
- **Verbatim (user, 2026-10-09):** "Right now the text wraps but starts aligned with the arrow."
- **Verbatim (user, 2026-10-09):** "Let's find a better way to display information and text (probably
  even diagrams or ascii) way better than this. Let's make it look aesthetic, easy to understand, and
  not like normal terminals where text is just so unappealingly protrayed."
- **Verbatim (user, 2026-10-09, on the drawn directions):** "What you have is really good. I just
  wanted everything to be aligned though. Everything that should always be aligned on the left
  side. Even more, the arrow inside the conversation section should be different. We already have
  an arrow in the typing section. Let's find something more suiting."
- **Verbatim (user, 2026-10-09, on the second drawing):** "The null sign looks acceptable.
  However, things are still not aligned. I literally meant everything should be aligned in the
  left. For some reason, the diagrams appears way before the text, making it unaligned with the
  rest. Also, I never want to see grayish lines on the left side. That is a convention I never
  want to see."
- **No line down the left side, ever.** Not beside a quote, not beside a message, not as a mark.
  The owner's rule, from the words above.
- **Verbatim (user, earlier):** "All I really need to see is the text box and the arrow." Words
  appear only for something the user has to know.
- **Verbatim (user, earlier):** "the primary colors are black and white." The only other colours are
  amber (the agent is asking), red (errors) and blue (the box's own commands, and links). So code is
  not coloured by syntax.
- **Verbatim (user, earlier):** "The null terminal is meant to be compact (while still looking modern
  and aesthetic)." The transcript stops at 212 px, ten lines, at the owner's request, and the box's
  heights are an exact contract with `src/panel.rs` (`MAX_HEIGHT` 314).
- The owner asked for a terminal typeface on 2026-10-08, and the box is SF Mono throughout. The new
  request pulls against that: see decision 1.
- The box is one static page with its style and script inline. No Node, no bundler (`MiniApp.md`).
- The box never takes activation; a press on text selects it and a press anywhere else moves the box.
- Same agent as in the terminal: instruction file, skills, MCP servers, model (`MiniApp.md`). Item 8
  touches this: see decision 3.
- **Safety:** the page can call every command of the app (`withGlobalTauri`, and no content policy
  in `tauri.conf.json`). Nothing a model writes may ever be put on the page as HTML.

## What the box does today (drawn 2026-10-09)

One realistic exchange, a long question, three tool steps and a reply with a heading, a list, a
command, a table and a diagram, was drawn in the app's web engine at the box's real size.

| What | Today |
|---|---|
| The owner's message | Its second line starts under the `›`, not under the text |
| The reply | Markdown shown as typed: `## `, `**`, backticks, code fences, the pipes of a table |
| Tool steps | One line each, with no space between them and the reply |
| Width | The 584 px box holds about 66 characters a line. A diagram 78 wide wraps and falls apart, and so does a table |
| Height | Ten lines. The view follows the reply to its end, so a long reply is left showing its last lines and has to be scrolled back to be read |
| Where the text comes from | `text_delta` events; the page appends them to one element as plain text (`white-space: pre-wrap`) |

One thing was tried on the harness: a note added to the agent's instructions at start
(`omp --append-system-prompt "…" acp`) does reach the model. A stand-in provider received it in the
request.

## Design rules

Proposed here; they decide most of what is below.

1. **Structure, not decoration.** A reply is shown as its parts: a heading reads as a heading, a list
   as a list, a command as something to copy. No ornament that carries no meaning.
2. **One reading edge, and literally everything on it.** The edge is where the typed text starts
   in the field, 33 px in. Everything starts exactly there, and so does everything that sets a
   thing off: the owner's message, the steps line, headings, paragraphs, the numbers of a list,
   code, tables, diagrams, quotes and the box's own notes. Nothing begins before the edge. So a
   block has no box of its own with an edge to its left, a word of code has no chip around it (a
   chip pushes its word in when it starts a line), and a quote has no rule beside it. A drawing
   whose lines begin with line-drawing characters is moved left by half a character, so that the
   frame it draws stands on the edge. The one thing left of the edge is the null sign beside the
   owner's message, under the arrow of the field.
3. **Wide things keep their shape.** Code, tables and diagrams are never wrapped. They sit in a panel
   of their own that can be moved sideways.
4. **The machinery is quiet.** What the agent did to get the answer takes one dim line, not one line
   a step.
5. **The answer starts where the eye is.** A finished reply is shown from its beginning.
6. **Nothing from the model becomes HTML.** The page builds every element itself and gives it text.

## Decisions for the owner

To be made by eye, on the pictures of item 2.

0. **The direction: A.** The owner saw the three and said "What you have is really good", asking
   for one left edge and a different mark. That is taken as A, the one shown first and the only
   drawn direction with an arrow in the conversation. It settles decisions 1 (SF Mono throughout)
   and 4 (tool steps folded) as A has them, unless the owner says otherwise.
5. **The mark beside the owner's message: the null sign** (owner, 2026-10-09: "The null sign
   looks acceptable"). Drawn, like the arrow, so that it is the same in any typeface. A dot, a
   bar, a hairline and nothing at all were also drawn; the bar is ruled out for good by the
   owner's rule on lines down the left side.
6. **How code and diagrams are set off: nothing around them** (owner, 2026-10-09: "nothing around
   them is perfect!"). Brighter type and space do it. A hairline above and below was the other
   drawing.

1. **The typeface of prose.** Keep SF Mono throughout, as asked on 2026-10-08, or set prose in the
   system's reading typeface and keep the terminal face for code, commands and diagrams.
   Recommended: keep SF Mono, and get the gain from structure. Direction C shows the other.
2. **Height.** Keep ten lines, or let the box grow for a long reply, or give it a key that makes it
   tall. Recommended: keep ten lines first and see how items 3 to 7 read; item 9 is only built if
   the owner wants more room.
3. **Telling the agent about the box** (item 8). Recommended: yes, a note of a few lines. It is the
   one thing that makes diagrams and tables fit by themselves. It makes the agent in the box differ
   a little from the agent in the terminal, which is why it is the owner's call.
4. **Tool steps.** Fold them into one line that opens on a click, or leave one line each.
   Recommended: fold.

## Work items

1. **The sample exchange and the look check, kept in the repo**
   - What: `Mini/Scripts/Look/`: the sample exchange above as data; a stand-in for the app's
     commands so the page can be drawn outside the app; the small Swift program that draws a page
     off-screen in the app's web engine and saves a picture; and `Mini/Scripts/look`, one command
     that draws every state at the real size, on a dark and a light backdrop, into `Mini/target/`.
   - Why: every look change so far was checked with scripts that lived in a session's scratch
     folder and were noted twice as possibly gone (`MiniApp.md`, item 8). This plan changes the look
     more than any before it and needs the same pictures before and after.
   - Depends on: none.
   - Risk: the stand-in drifts from the app's real commands. Know it by: the look check also loads
     the page as built and fails on a script error.
   - Source: inferred from codebase (`MiniApp.md`, "How the look was checked").
   - Status: Not started. Items 2 to 7 were drawn and checked with scratch tools once more, so
     the pictures behind this plan cannot be made again from the repo

2. **Three directions, drawn for the owner to choose**
   - What: the same sample exchange drawn three ways at the box's real size, capped at ten lines and
     also in full, and opened for the owner as one page of pictures:
     - **A, Gutter.** One reading edge with marks in a gutter; SF Mono throughout; headings in white
       and bold; code, tables and diagrams in quiet panels; tool steps folded into one line.
     - **B, Turns.** Each exchange is a block: the owner's message as a compact dim strip, the
       reply beneath it; panels with a hairline edge; more air between exchanges.
     - **C, Reader.** Prose in the system's reading typeface, a little larger, with the terminal
       face kept for code, commands and diagrams.
   - Why: "aesthetic" and "easy to understand" are judged by the owner's eye, and the four decisions
     above are easier to make looking at them. Recommended starting point: A.
   - Depends on: 1.
   - Risk: a picture reads differently from the live box, above all while a reply streams in. Know
     it by: the chosen direction is installed and tried before items 5 to 7 are called done.
   - Source: specified from user + inferred.
   - Status: Complete (2026-10-09). Chosen over three rounds of drawings: direction A, the null
     sign, one left edge for literally everything, nothing around code and diagrams. The drawings
     were scratch and are not in the repo
   - **Seen in the drawings:** each direction shows the whole sample exchange in less height than
     today (711 to 756 px against 945), because structure needs no blank lines and no fences. The
     78-wide diagram fits the box at 10 px type without scrolling. Its upright lines meet; its
     level lines still show fine gaps, which item 5 has to solve. The drawings were made with
     scratch tools and hand-built sample markup, not with the reader of item 3.
   - **Second round (2026-10-09), after the owner's words:** direction A redrawn with one left
     edge. Measured in the app's web engine, eleven things start at the same 50 px of the window,
     which is where the typed text starts: the owner's message, the steps line, a heading, a
     paragraph, a list's numbers, the text in a code panel, a table, a diagram, a quote and a
     note of the box's own. Five marks for the owner's message are drawn for the choice.
   - **Third round (2026-10-09):** the second round let a panel's background and a quote's rule
     begin before the edge, and the owner read that, rightly, as not aligned. Redrawn with nothing
     before the edge. Measured the same way: the start of every block and the start of what is in
     it are both at 50 px, for code, table, diagram and quote alike. A word of code is brighter
     type with nothing around it; a quote is dim words; a diagram's frame stands on the edge.

3. **Read the reply's Markdown into its parts**
   - What: `Mini/src/markdown.rs`, a pure function from a reply's text to a tree of parts:
     paragraph, heading, list (numbered, bulleted, nested, ticked), code block with its language,
     quote, table, rule; and within a line: strong, emphasis, code, link, struck-through. It uses
     the `pulldown-cmark` crate with tables, strikethrough and task lists on, and drops raw HTML to
     plain text. A command `layout(text)` returns the tree. The page keeps the reply's text as it
     arrives and asks for the layout at most once a frame while it streams, and once at the end.
   - Why: design rule 1. The parser is done in the app, not in the page, because Markdown from
     models is full of edge cases a hand-written reader gets wrong, the crate is the one the Rust
     world relies on, and the app's pure parts are already what `cargo test` covers.
   - Depends on: none.
   - Risk: text that is half-arrived looks odd for a moment (an open `**`, half a table). Know it
     by: unit tests on cut-off inputs; an open code fence must already be a code block, and
     anything not understood must come out as plain text, never vanish.
   - Considered and set aside: a Markdown library in the page that returns HTML (rule 6, and the
     page has no content policy); a hand-written reader in the page (no dependency, but the edge
     cases); sending the whole tree with every `text_delta` (the event buffer keeps 5,000 events).
   - Source: inferred from codebase (`translate.rs` is where the app already turns protocol into
     what the box shows) + known library.
   - Status: Complete (2026-10-09)
   - **Built:** `Mini/src/markdown.rs` (`pulldown-cmark` 0.13.4, its default features off) and
     the `layout` command in `main.rs`. Ten unit tests, among them HTML written by a model coming
     out as plain words, and half-arrived text: an open fence is already a code block, open
     emphasis shows as typed, a table grows row by row.

4. **Draw the parts: one reading edge**
   - What: in `Mini/Page/index.html`, a turn is a two-column row, gutter and text. The owner's
     message keeps its `›` in the gutter and wraps under its own first letter. The reply is built
     from item 3's tree as elements with text (`createElement`, `textContent`), styled to the
     direction chosen in item 2: headings, lists with hanging marks, inline code, links in the
     box's blue opened through the existing `open_url`, quotes as dim words. Notes,
     errors and the agent's questions move to the same edge. Space between turns comes from one
     rhythm, the 19 px line.
   - Why: the owner's own complaint, and design rules 1, 2 and 6.
   - Depends on: 2 (the direction), 3.
   - Risk: the rules for moving the box by dragging depend on where text is (`grip`, `onText`).
     Know it by: the drag and select checks of `MiniApp.md` item 8, run again on the installed app.
   - Source: specified from user + codebase.
   - Status: In progress — built and installed (2026-10-09); the owner has not yet used it
   - **Built:** the transcript's edge is 33 px in, where the typed text starts. The owner's
     message carries the null sign, drawn like the arrow, in the gutter. A reply is plain text
     the moment it arrives and is redrawn as its parts when the layout comes back, one asking
     at a time, so the last word is always drawn. A word of code is brighter type; a quote is dim
     words; a list's marks stand on the edge; a link is the box's blue and opens through
     `open_url`. The agent's question lost its amber box and stands on the edge as amber words
     with its buttons.
   - **Proven without a person:** drawn in the app's web engine with the app's own layout, seven
     states. In each, everything measured starts at the same 50 px of the window as the typed
     text: the owner's message, the steps line, a heading, a paragraph, a list's marks, code, a
     table, a quote and a note. The self-test, in the real app against a stand-in that streamed
     Markdown in 37-character pieces: the page showed the heading, the paragraph, the list, the
     command, the table, the diagram and the quote with no Markdown marks left.
   - **Not proven:** dragging the box and selecting text over the new elements; the owner's eye.

5. **Wide things keep their shape**
   - What: code blocks, diagrams and tables are never wrapped. Each starts on the reading edge
     with no box of its own (design rule 2), is set off as decision 6 settles, and is moved
     sideways with two fingers when it is too wide, its right-hand cut faded so it is plain there
     is more. A block a little too wide is first set in smaller type, no smaller than 10 px, and
     only then scrolls. Diagram lines are set at the line height at which line-drawing characters
     meet, found by drawing them, and a drawing whose lines begin with such characters is moved
     left by half a character. A table that fits is a real table with wrapping cells.
   - Why: "probably even diagrams or ascii", and design rule 3. Today both fall apart.
   - Depends on: 4.
   - Risk: sideways scrolling inside a box that is itself dragged and scrolled. Know it by: trying
     it on the installed app with the sample's 78-wide diagram.
   - Source: specified from user + inferred.
   - Status: In progress — built and installed (2026-10-09); sideways scrolling not yet tried by
     hand
   - **Built:** code, diagrams and tables are never wrapped and have nothing around them. A block
     too wide is set smaller in half-pixel steps down to 10 px, then moves sideways with its
     right-hand end faded until it is scrolled to. A block with line-drawing characters has the
     tighter line height, and one whose lines all begin with them is moved left by half a
     character. A table sits in a holder that scrolls the same way.
   - **Seen in the drawings:** the 78-wide diagram fits at 11 px, and at that size its level
     lines no longer show the gaps they had at 10 px. A 120-character error line and a six-column
     table fade at the right.

6. **Quiet tool steps** (decision 4)
   - What: while the agent works, its current step shows on one line that is replaced as steps
     pass. When the reply's text begins, the steps fold into one dim line ("3 steps"), which opens
     on a click. A step that failed stays in view, in red.
   - Why: design rule 4. In ten lines, three tool lines are a third of the view.
   - Depends on: 4.
   - Risk: hiding something the owner wanted to see. Know it by: the owner's eye; the fold opens.
   - Source: inferred.
   - Status: In progress — built and installed (2026-10-09); the owner has not yet used it
   - **Built:** as written. One step alone is shown as itself; two or more fold to "✓ 3 steps",
     or "2 steps, 1 failed" with the failed one left in view. Drawn: working, folded, opened, and
     with a failed step.

7. **The answer starts where the eye is**
   - What: the view follows a reply only while the reply still fits. Once it is taller than the
     view, its first line is held at the top and the rest grows below, with the bottom edge faded
     to say there is more. If the owner scrolls, the box stops following. A new message from the
     owner goes to the top of the view.
   - Why: design rule 5. Today a long reply ends with its last lines showing.
   - Depends on: 4.
   - Risk: the view jumping while text streams. Know it by: the installed app, with a slow model;
     pictures cannot show this.
   - Source: inferred.
   - Status: In progress — built and installed (2026-10-09); how it feels while a reply streams
     is unseen
   - **Built:** the view follows until the owner's latest message reaches the top and holds it
     there; the bottom fades while there is more below; a scroll by the owner stops the
     following until their next message. Outside a reply, a line the box says (a sign-in step, a
     note) is always brought into view.
   - **Deviation:** what is held at the top is the owner's message, not the reply's first line,
     so the question stays in sight above its answer.

8. **The agent knows the box** (decision 3)
   - What: a note of a few lines in a file of Null's own, passed at start with
     `--append-system-prompt`: the reply is shown in a small box about 60 characters wide; lead
     with the answer; keep it short; Markdown is drawn; a diagram or a table no wider than 60.
     Nothing of the owner's instruction files is touched.
   - Why: the display can only lay out what it is given. A diagram drawn to fit needs no scrolling.
   - Depends on: none. Best judged after 4 and 5, so that the note asks for what the box draws well.
   - Risk: the agent in the box answers differently from the agent in the terminal. Know it by: the
     owner's use; the note is one file and can be emptied.
   - Source: verified locally (the flag reaches the model, 2026-10-09) + inferred.
   - Status: Not started

9. **More room for a long reply** (only if decision 2 asks for it)
   - What: not designed. Either the transcript's limit rises while a reply is longer than ten lines,
     or a key makes the box tall and back. Both change `MAX_HEIGHT` and the height sums in
     `src/panel.rs` and the page's sizing comment.
   - Why: "since the null terminal is compact, it's harder to display text."
   - Depends on: 4 to 7, and the owner's choice.
   - Risk: the box stops being compact. Know it by: the owner's eye.
   - Source: specified from user (the problem) + inferred (the means).
   - Status: Not started

10. **A content policy for the page**
    - What: set `app.security.csp` in `tauri.conf.json` so the page runs only its own script and
      loads nothing from elsewhere, and check that the app's commands and events still work.
    - Why: rule 6 keeps the model's words from becoming HTML; this is the second lock on the same
      door. The page today has none, and it can call every command.
    - Depends on: none.
    - Risk: the policy blocks the page's own inline script or the app's commands. Know it by: the
      scripted start-up check and the self-test (`MiniApp.md`, Verification).
    - Source: inferred from codebase (`tauri.conf.json`).
    - Status: Not started

## Verification

- Unit tests (`cargo test`): the Markdown tree for each kind of part, for nesting, and for text cut
  off mid-way (item 3).
- The look check (item 1): pictures of every state, before and after, on both backdrops, at ten
  lines and in full: the sample exchange, a long reply, a wide diagram, a table that fits and one
  that does not, a failed reply, the agent's question, the model list.
- The self-test (`NULL_MINI_SELFTEST`): a message typed into the real page still shows the prompt
  line, the reply and a tool line.
- By hand, the owner, on the installed app: a real conversation with a slow and a fast model;
  dragging the box and selecting text; scrolling a wide panel sideways; whether it reads as wanted.

## Validation

- No ADR is contradicted. New folders are capitalized (`Mini/Scripts/Look/`), as ADR 0001 asks.
- One constraint is bent, on purpose and said here: item 1 uses Node and Swift for the look check.
  They draw pictures for checking; the app is still built without them.
- The order holds. Items 3, 8 and 10 depend on nothing and can be built beside the pictures.
- Still fuzzy, on purpose: the look itself (item 2 decides), and item 9, which is not designed
  until it is asked for.
- What pictures cannot settle: how the box feels while a reply streams in (items 6 and 7). Those
  are called done only after the owner has used them.
- No outside research was needed. The page, the harness flag and today's look were read or run
  here; the Markdown crate is a known one, and its exact version is read when item 3 is built.

## Out of scope

- Diagrams drawn from a description (Mermaid and the like). They need a large library and would
  put the model's text on the page as markup. To be looked at again once item 5 is in use.
- Pictures in a reply; colouring code by its syntax; showing the agent's thinking; showing what a
  tool printed or changed.
- A light theme. Voice and the pet.
