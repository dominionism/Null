# Work in the background

> Blueprint: 2026-10-10, grilled with the owner the same day and planned again with what the
> grill settled. Step 3 of the five in `Context/Plans/NullMini.md` (Phase 3, items 15 to 17),
> planned for Null as it is now: its own app in `Mini/`, Oh-my-pi over the protocol, the box,
> history and the pet. It replaces those three items, which were written for a mini inside the
> Voice desktop app and its server.
>
> Status: **Planned and grilled, nothing built. The owner confirmed the goal and settled all
> twelve decisions. Next: item 1. Trials were run for the plan in scratch folders (see "What
> was found"); none of that code is in the repo.**

## Goal

Let a conversation go on working while the owner does something else in the box, show how each
conversation stands, let the owner choose the folder a conversation works in, and let history
be kept tidy and searched.

## Constraints

The owner dictates by voice and wants beginner-friendly words.

- **Verbatim (user, 2026-10-10):** asked "What is the most optimal direction to take here?", and
  to the answer (background work next, voice after it): "yes, and plan it".
- **The goal was the agent's reading, and the owner confirmed it.** It comes from `NullMini.md`:
  "long work should not hold the text box hostage" (item 15), "following work while doing
  something else" (item 16), and the later-phases line of its Acceptance, "background tasks with
  Running / Needs input / Ready / Blocked". Put to the owner as three parts (leave a job
  working; see what is going on; choose the folder), the answer was: **verbatim (user,
  2026-10-10):** "Yes, all three are what I want".
- **Verbatim (user, 2026-10-10, on what else belongs in this round):** "Let's also include
  deleting, renaming, or searching inside conversations".
- **Verbatim (user, 2026-10-10, on what is left out):** "make sure to put them somewhere safe
  for, you know, so that we don't lose track of the deferred tasks, right? Make sure of that.
  Guarantee me that you're keeping track of the deferred jobs." They are in the list that is
  named at every recall (`Context/Research/Research.md`, "Deferred by the owner").
- **Verbatim (user, `NullMini.md`):** "have my null mini be able to use omp to do all of the heavy
  work I want it to do." / "If I ask Null a question, then it answers. If I ask Null to do
  something, and then it does it."
- **Verbatim (user, 2026-10-08):** "All I really need to see is the text box and the arrow."
  Amended on 2026-10-10 for two quiet buttons and the pet (`PetAndBox.md`), and here by the
  owner's own word for an x on each row of `/history` (decision 9), with a pencil beside it
  (decision 10). Both show only on the row under the pointer or chosen with the keys. Nothing
  else here adds a button, a status line or a word that stays.
- **Verbatim (user, 2026-10-10, on the pet):** "Alignment should always be consistent."
- **Voice stays deferred** (user, 2026-10-10: "a big one (requiring a significant amount of
  planning -- let's defer for now)").
- **Machine budget:** Apple M1, 16 GB (`NullMini.md`).
- From the plans this builds on, still in force:
  - Null adds no approval layer; it mirrors the harness's own setting (`NullMini.md`).
  - Black and white, with amber (the agent is asking), red (errors) and blue (the box's own
    commands, and links). One typeface, SF Mono. No line down the left side of anything.
  - Everything the box does is a typed command. A button or a press is a second way in.
  - The box never makes Null the active app.
  - The page is one static file. Nothing a model, a file name or the harness writes becomes HTML.
  - Every height and width of the window is an exact contract between the page and `panel.rs`.
  - A feature is lost one at a time, never the whole box (`OwnHarness.md`, design rule 4).
  - No credential rests in Null (ADR 0004). Null knows nothing about any one provider (ADR 0005).
  - The harness keeps the conversations. Null keeps which one was open and a title for each one
    made in the box (`Glossary.md`).
  - Folder names are capitalized (ADR 0001).
  - Commits: Conventional Commits, split by concern, never an em dash, no tool or AI
    attribution. One pull request per concern. Sub-agents only when the owner asks.

## What was found (2026-10-10)

Read in the code, and tried on the carried Oh-my-pi 18.8.7 in folders of its own with a made-up
provider that answers a word a second. No real provider was reached. Of the owner's own
conversations only their number, their sizes and how long a look through them takes were read.

| Question | Finding |
|---|---|
| How many conversations Null holds today | One. `harness.rs` has one thread, one harness process and one conversation: one `busy`, one list of events, and anything the harness says about another conversation is dropped (`on_update`) |
| What refuses today while a reply runs | `/new` ("stop the reply before starting over"), `/history`, `/model`, and a second message |
| Can one Oh-my-pi run several conversations at once | **Yes.** Two conversations in two folders, in one process on one connection, answered at the same time, word by word in turn, and both ended well |
| Does the rest answer meanwhile | Yes. While both ran: the list of conversations in 0.01 s, a third conversation opened in 0.13 s, its model set, and a quick question in it answered at once |
| Does stopping one stop the others | No. One was stopped (`cancelled`) and the other went on to its end |
| A second message to a conversation still answering | The harness stops the first reply and answers the second. It does not queue and does not refuse. So Null must go on refusing it |
| Loading a conversation back while another is working | Works, in 0.01 s, and replays what was said |
| Loading back the conversation that is itself working | Answers "loaded" and the reply goes on, but the part of the reply said so far is **not** replayed. Null has to keep that itself |
| Closing a conversation | The harness offers it (`session/close`), and a closed one loads back in 0.11 s. Memory was not given back at once |
| Memory | One Oh-my-pi is about 260 MB when greeted. Each conversation that has answered adds 35 to 60 MB: six came to 633 MB. A second process would cost 260 MB each |
| Where conversations work | All in one folder, `…/Workspace` (`harness::workspace`). The harness takes a folder for each (`cwd` in `session/new`), and history already loads a terminal conversation in its own |
| A list of folders to choose from | `attach.rs` already walks the Mac's folders in a list in the box, and the harness's list of conversations names every folder one works in |
| Can the harness delete, rename or search a conversation | No. Over the protocol it lists, forks, resumes and closes; it has no command of its own for any of the three either |
| How the harness keeps a conversation | One file each, `sessions/<its folder>/<when>_<id>.jsonl` in the harness's folder, found by the id. A lock file sits beside it while a harness holds it open. Each line is one thing: a `title` line, a `session` line, and a `message` line for each thing said, with who said it (`user`, `assistant`) and the words as plain text |
| What happens when that file is taken away | The harness stops listing the conversation at once, does not write the file again, and will not load it. Put back, it is listed and loads again |
| How long it takes to look through every conversation for a word | On the owner's Mac, 275 conversations in 37 folders, 363 MB (half of them under 308 KB, the largest 41 MB): 0.03 s for a fast look that uses every core, 1.3 s for a simple one, both with the files in memory. Only how long it took was read, never what was found |
| The line for a mistyped command | It holds about 81 characters and the eleven commands take 78. A twelfth does not fit |
| The pet and the hidden box | `panel::hide` takes the whole window away. The pet is drawn by the page in a 46 px lane of the same window |

Not found out: the owner's own MCP servers with several conversations (each conversation attaches
them again, and one that runs as a program may be started once for each); a real provider's
limit with several replies at once; whether closing gives memory back after a while; whether a
file that Null moves to the Trash can be put back from there with the Trash's own "Put Back".

## Design rules

1. **One harness, many conversations.** Null keeps one harness process and one connection, and
   holds several conversations in it. It never starts a second harness to run a second job.
2. **Background is not a mode.** A conversation goes on working when it is left: by `/new`, by
   opening another, or by putting the box away. There is nothing to switch on.
3. **No new noun.** What is working is a conversation, and `/history` is where conversations are
   found, looked through, renamed and deleted. There is no "task", no "job" and no separate
   tray (decision 6).
4. **Status is the pet's and the list's.** No words stay in the idle box. The arrow speaks for
   the open conversation, the pet for all of them, and `/history` names each.
5. **Null keeps what a working conversation has said only while it runs,** in memory, so that it
   can be shown whole when opened. The harness stays the keeper of conversations.
6. **Nothing is stopped silently.** Quitting, a restart of the harness and a second message
   each say what they would stop before they do it.
7. **A folder is chosen when a conversation starts.** A conversation cannot move, since the
   harness ties it to its folder.
8. **Null may move a conversation's file and may read it, and never changes what is in it.**
   Deleting moves the file to the Trash; searching reads it and keeps nothing. Both lean on
   where and how the harness keeps its files, which the harness promises nobody, so each has
   its question in the harness check, and each is lost alone on a harness that keeps them
   another way.

## Decisions for the owner

All were put to the owner one at a time on 2026-10-10. Decisions 9 to 11 are the owner's own
additions.

1. **Whether the pet stays on the screen while work goes on with the box put away.** Today the
   box "hides completely", and `PetAndBox.md` left this out for this step. **Settled (owner,
   2026-10-10): "The first one, let's go with your recommendation".** The pet stays only while
   it has something to say (a conversation working, asking or unseen), a press on it opens the
   box on that conversation, and it goes when nothing is left. Never when the pet is put away
   with `/pet`. It is built as a prototype for the owner to live with for some days before it
   is promised, and no release waits for it (item 15).
2. **How many conversations may work at once. Settled (owner, 2026-10-10): four** ("Four is
   right"). Each costs memory and they share one provider's limit. A fifth is refused with one
   line, "four conversations are already working. wait for one to end, or stop one", and what
   was typed stays in the field. Conversations that are not working do not count.
3. **What `/quit` does while conversations are working. Settled (owner, 2026-10-10): it ends
   them, after one warning.** The first `/quit`, or Ctrl+C with the open conversation at rest,
   stops nothing: the box says "2 conversations are still working. /quit again stops them and
   quits." The second stops them and quits. With nothing working it quits at once, as today.
   - **How it was settled:** the owner first chose the other way (work going on after `/quit`,
     and Null quitting by itself once it had ended), then asked which was better ("I'm kind of
     in the middle here"), and to the recommendation said: "Yes, change the plan to that".
   - **Why:** quitting means stopping everywhere else. On this Mac the agent acts without
     asking, so work after a quit would go on with nobody watching, changing files and using
     up a provider's limit. Putting the box away already leaves work going. And Ctrl+C is a
     habit from terminals: without the warning one stray press would end every job.
4. **Where the folder of the open conversation shows. Settled (owner, 2026-10-10: "I agree
   with points 1 to 3").** Each time a conversation that works outside the box's own folder
   comes to the front (it is started, opened from `/history`, or is the one showing when the
   box is opened), the notice line says where for a few seconds, as in "in ~/Developer/Null".
   It is always in `/history`. Nothing shows in the idle box, and nothing for a conversation
   in the box's own folder.
   - **Why not once:** the plan first said it once, when the conversation starts. Set aside
     for this case: a job started in a project yesterday is still the open conversation
     today, the owner types "delete the old build files" thinking of somewhere else, and the
     agent, which acts without asking on this Mac, does it in that project.
   - **Set aside, always visible:** the folder's name, small and dim, at the end of the
     field. Safer still, but one more thing in a box the owner wants kept to the field and
     the arrow.
   - **Agreed with it:** `/folder` lists the folders that conversations already work in,
     most recent first, then a way to browse; `/folder` and a path goes straight there;
     choosing starts a new conversation in that folder. `/new` stays plain: the box's own
     folder, unless told otherwise.
5. **Whether Null may remember how a conversation it has not shown yet ended:** its id and one
   word (done, failed, cut off), beside the titles in `history.json`. That goes a little past
   the glossary's Conversation line again. Without it, a restart forgets what was waiting to be
   looked at. **Settled (owner, 2026-10-10): yes.** A conversation that ended and was not
   looked at is still under "now" after Null was quit or cut off. The glossary's line follows
   when this is built (item 13).
6. **The words. Settled (owner, 2026-10-10: "Yes, I agree with the words and conversation").**
   Open, working, asking, done, failed, stopped, cut off, unseen, now, folder and the box's
   own folder are in `Context/Glossary.md` as of the same day. The box says "conversation"
   everywhere: "2 conversations are still working". There is no "job" and no "task" as a
   thing of its own, though the owner and the agent may say "job" when they talk.
7. **What a mistyped command shows,** now that the commands no longer fit on one line (eleven
   take 78 of its 81 characters, and `/folder` and `/rename` make thirteen). **Settled (owner,
   2026-10-10: "Yes, I agree with the list").** A mistyped command opens the list of commands
   in the place the other lists use, each with a few words. What was typed narrows it, so
   that `/hist` leaves `/history` first and Enter runs it. A lone `/` and Enter shows them
   all. Set aside: one shortened line, which is no longer a whole answer and breaks again
   with the next command.
8. **Settled (owner, 2026-10-10): left out and deferred.** The agent starting background
   conversations by itself (`NullMini.md`, item 15, "delegate"), which belongs with voice, and
   a macOS notification or a sound when work ends. Both are named at every recall (Out of
   scope).
9. **Deleting a conversation. Settled (owner, 2026-10-10):** "I should be able to delete a
   conversation in history. There should be a x button for each conversation so that I can
   wipe it and keep my history clean if I don't need the conversation anymore. Each of the
   deleted conversations should go to the trash." The harness offers no way to delete, so
   Null moves the conversation's file to the Mac's Trash itself (design rule 8).
   - **The details (owner: "Yes, I agree with all five"):** the x shows only on the row under
     the pointer or chosen with the keys. Command+Delete on the chosen row does the same,
     since a button is never the only way. There is no "are you sure": the file goes to the
     Trash at once and the box says "moved to the Trash" with the title. Every row can be
     deleted, the terminal's conversations too, and one of those then leaves the terminal's
     own list as well. Refused: a conversation that is still working, and one that is open in
     a terminal at that moment (its lock file is there).
10. **Renaming a conversation. Settled (owner, 2026-10-10: "I agree with the two
    recommendations").** Null keeps the new title itself, beside the titles it already makes
    in `history.json`, and a title the owner typed comes before the harness's own. The
    harness's file is not changed, so a terminal goes on showing its own title. Two ways in:
    `/rename` and the new title, for the open conversation; and in `/history` a small pencil
    beside the x, with Command+R on the chosen row, which puts the title in the field to
    change (Enter keeps it, Esc leaves it alone).
    - **Set aside:** writing the title into the harness's file, which would reach the
      terminal but has Null change what is inside a file it does not own.
11. **Searching inside conversations. Settled (owner, 2026-10-10: "Just right into the plan
    that way. Let's go with your recommendation").** Words typed in `/history` narrow by
    title and folder at once, as today, and a moment later the conversations in which those
    words were said are added under a heading of their own, each with a short piece of the
    line they were found in. Null looks through the harness's files as it searches and keeps
    nothing: no index, and nothing of what is found in its log. The words may be anywhere in
    a conversation, in any order, in capitals or not. What is searched is what the owner
    wrote and what the agent replied, not what a tool read or gave back. Results are ranked
    by how often and where the words appear (the owner's idea: "term frequency"), a title and
    the owner's own messages counting for more, the newer first on a tie.
    - **Set aside, an index:** the owner asked for one, for speed. Measured on the owner's
      Mac, a fast look through all 275 conversations took 0.03 s with the files in memory, so
      an index would add nothing that can be felt, and Null would have to keep the words of
      every conversation. A simple look took 1.3 s: it has to be the fast kind.
    - **Deferred, search by meaning** (the owner's idea: "first semantic meaning"). It needs a
      language model inside Null, or every conversation sent to a provider, which ADR 0004
      and 0005 rule out. Named at every recall (Out of scope).
12. **One release or two. Settled (owner, 2026-10-10): "Let's go with two."** First the tools
    for history (items 1 to 6, Null 0.3.0), which are small and do not touch how Null holds
    conversations. Then work in the background and `/folder` (items 7 to 14, Null 0.4.0),
    which change the inside of `harness.rs`. So the change that can break the box goes out by
    itself, and a fault after either release has one place to be looked for. The cost, said
    to the owner: trying and releasing twice.

## Work items

Items 1 to 6 are the tools for history. Items 7 to 14 are work in the background. Item 15 is the
pet's prototype, outside both. The first group comes first because it is small, and because
both groups change the same list in the page: built one after the other, they do not collide.

1. **The list of commands**
   - What: in `Mini/Page/index.html`, each of the box's commands gets a few words beside it. A
     slash-word that is no command opens them as a list in the place the other lists use,
     narrowed by what was typed (a command that starts with it first), and Enter runs the one
     chosen. A lone `/` and Enter shows them all. The one line that answered a mistyped
     command goes.
   - Why: decision 7. `/rename` and `/folder` would not fit on that line.
   - Depends on: none.
   - Risk: a message that starts with a slash being taken for a command. Know it by: the rule
     stays what it is today, a lone slash-word with nothing after it.
   - Source: specified from user (decision 7) + inferred from codebase (`COMMANDS`, the list
     `/model` uses).
   - Status: Not started

2. **Renaming a conversation**
   - What: `Mini/src/history.rs`: `history.json` gains the titles the owner gave, by
     conversation, beside the ones Null makes; in the list a title the owner gave comes first,
     then the harness's, then Null's. A title goes when its conversation is gone from the
     harness's list, as today. `/rename` and a title renames the open conversation; with no
     title it says what the command is for. In `/history`, a small pencil at the end of the
     row under the pointer or chosen, and Command+R: the title goes into the field, Enter
     keeps it, Esc leaves it. A title is cut at 80 characters, as Null's own are.
   - Why: decision 10.
   - Depends on: 1 (a twelfth command needs the list).
   - Risk: none that reaches past the box. The harness's file is not touched.
   - Source: specified from user (decision 10) + inferred from codebase (`history::titles`).
   - Status: Not started

3. **Deleting a conversation, to the Trash**
   - What: `Mini/src/history.rs` finds a conversation's file by its id in the harness's
     folder. To delete: refuse one that is working; have the harness close it if this run
     holds it, so that the harness lets go of its lock; if a lock file is still beside it,
     another program has it open, and it is refused with a line; move the file to the Trash
     with macOS's own call for that; say "moved to the Trash" with the title. If it was the
     open conversation, the box goes to a new one. In the page: an x at the end of the row
     under the pointer or chosen, and Command+Delete. A question in the harness check: a
     conversation's file is found by its id, the harness stops listing it when the file is
     gone and lists it again when it is back. On a harness whose files are not found that
     way, the x does not show.
   - Why: decision 9.
   - Depends on: none. Best after 2, since both add to the same row.
   - Risk: a file moved to the Trash by an app may lack the Trash's "Put Back"; a newer
     Oh-my-pi keeping its files another way; a slip of the pointer. Know it by: trying "Put
     Back" on the installed app before anything else in this item; the harness check; and the
     Trash itself, which is why nothing is erased.
   - Source: specified from user (decision 9) + researched finding (the trial) + inferred
     from codebase.
   - Status: Not started

4. **Searching inside conversations**
   - What: a new module, `Mini/src/search.rs`. First a fast look through every conversation
     file for the words, on every core and in capitals or not. Then only the files that have
     every word are read line by line: the lines that say something (`message`, said by
     `user` or `assistant`, its plain text), counting each word. A rank from how often and
     where, a title and the owner's own messages counting for more, the newer first on a tie;
     and a short piece of the line around the first place found. A newer search ends the
     older one. In the page: typing in `/history` narrows by title and folder at once, and a
     quarter of a second after the last key the conversations found inside are added under
     their own heading. Null's log gets how many were found and how long it took, never a
     word. A question in the harness check: what was said in a conversation is in its file in
     the form the search reads.
   - Why: decision 11.
   - Depends on: none. Best after 3, for the same row and list.
   - Risk: a word that every conversation has; the first search after a restart; one file of
     41 MB; a newer Oh-my-pi writing its files another way, after which only titles are
     found. Know it by: timing on the owner's own conversations, a limit on how many are
     shown, and the harness check.
   - Source: specified from user (decision 11) + researched finding (the timing) + inferred
     from codebase.
   - Status: Not started

5. **Say it: the tools for history**
   - What: the README's commands and what Null keeps (the titles the owner gave). The research
     map, together with what `PetAndBox.md` item 7 left behind. The glossary's Conversation
     line, for the titles the owner gave.
   - Depends on: 1 to 4.
   - Status: Not started

6. **Null 0.3.0: tried by the owner first, then released** (decision 12)
   - What: in the order of `PetAndBox.md` item 8. An installed 0.2.0 can then see `/update` say
     that a newer Null is out, which nothing has shown yet (`OwnHarness.md`, item 7).
   - Depends on: 5.
   - Status: Not started

7. **The harness check asks what work in the background rests on**
   - What: `Mini/src/standin.rs` gains a fifth answer, `slow`: a reply that comes a word at a
     time with a pause between, written as it comes. `Mini/src/check.rs`: the check's `Agent`
     learns to have two messages on their way at once and to keep what it hears with the
     conversation it came from. New questions, each named for what Null relies on: two
     conversations in two folders answer at the same time; stopping one leaves the other
     working; a second message to a conversation still answering stops the first reply; a
     conversation loads back while another is working; a closed conversation loads back.
     `translate::abilities` says "closes a conversation" when the greeting offers it.
   - Why: every item below stands on these, and the weekly job (`null-engine.yml`) then asks
     every newer Oh-my-pi the same before Null moves to it.
   - Depends on: none.
   - Risk: a question that measures seconds fails on a slow runner. Know it by: the questions
     ask about order (a word of one arrives between two words of the other), not about time.
   - Source: researched finding (the trial) + inferred from codebase (`check.rs`).
   - Status: Not started

8. **`harness.rs` holds many conversations, and nothing looks different**
   - What: what is now one conversation's state (`Shared`: `busy`, the kept events and their
     numbering, `tools`, the waiting approvals, `model`, `ask`, `opening`) becomes one such
     record for each conversation this run knows, by id, beside what belongs to the whole
     connection (`open`, which was `session`; the models offered; `noted`; `counts_tokens`).
     Every event carries its conversation. `on_update` and `on_permission` go by the
     conversation the harness names, where today anything but the open one is dropped. `Send`,
     `Interrupt`, `SetModel` and `CheckModel` name their conversation. `events_since` asks for
     one conversation. The struct called `Conversation` is the connection and is named so. In
     the page, an event for a conversation that is not open is not drawn.
   - Why: design rule 1. Done by itself first, so that the change which can break the box is
     proven before anything new is built on it.
   - Depends on: 7.
   - Risk: the one-conversation path breaks where tests do not look, above all a conversation
     shown again after a restart. Know it by: the unit tests, the whole harness check, the
     scripted run of the real app that `PetAndBox.md` used (attach, `/new`, `/history`, a
     restart), and the same pictures as before.
   - Source: inferred from codebase.
   - Status: Not started

9. **Leaving a conversation working**
   - What: `/new` and choosing from `/history` no longer refuse while the open conversation is
     working: they move which one is open, and the box says once "still working. /history shows
     it". Opening a conversation this run holds draws it from what Null kept of it and goes on
     live (finding: the harness does not replay a reply in progress); any other loads from the
     harness as today. A second message to a working conversation is still refused (finding).
     At most four work at once (decision 2); one more is refused with its line. A conversation
     that has ended, is not unseen and is not open is closed in the harness and forgotten by
     Null; it stays in the harness's list.
   - Why: the goal. "Long work should not hold the text box hostage."
   - Depends on: 8.
   - Risk: what the page holds for one reply (the reply being drawn, the folded steps, the
     waiting approval, the quiet timer) leaking into another conversation. The owner's MCP
     servers started once for each conversation. Know it by: scripted runs with the `slow`
     stand-in (a slow job, `/new`, a quick question, back through `/history`, the slow reply
     whole), and memory and processes counted with the owner's own Oh-my-pi.
   - Source: specified from user (the goal) + researched finding + inferred from codebase.
   - Status: Not started

10. **Seeing what is going on**
    - What: Null keeps how each conversation stands (working, asking, done, failed, stopped,
      cut off) and whether it is unseen. A new event, `mini:work`, tells the page at each
      change, and a command gives the same to a page that has just loaded. In `/history` a
      first heading, "now", lists every conversation working, asking or unseen, most pressing
      first (asking, failed, done, working), with a mark where the open one has its dot: amber
      for asking, red for failed. On the notice line, when the box is open and another
      conversation ends or asks: "done: <title>", "asking: <title>" or "failed: <title>", and
      a press on it opens that conversation. The pet shows the most pressing state of all
      conversations; the arrow goes on speaking for the open one. Opening a conversation
      counts as having looked. A conversation that asks keeps its question until it is opened.
    - Why: "see what is working, asking, done or failed"; `NullMini.md`, item 16, with the
      reference product's order of what matters most.
    - Depends on: 9.
    - Risk: the pet changing too often with several conversations; an approval nobody notices.
      Know it by: pictures of every state, a scripted run, and the owner's eye.
    - Source: specified from user (the goal) + researched (`ChatGPTPetsAndMini.md`, the status
      order) + inferred from codebase (`history.rs`, `mark()` and `mood()` in the page).
    - Status: Not started

11. **Quitting, restarting, and being cut off**
    - What: `/quit`, and Ctrl+C with the open conversation at rest, do not quit at once while
      other conversations work: the box says how many, and a second `/quit` stops them and
      quits (decision 3). A restart of the harness (after a sign-in, `/harness`, `/update`)
      waits until nothing is working and says so. A harness that goes away by itself ends
      every working conversation with one line. Null writes down which conversations are
      working or unseen, an id and one word each, in `history.json` (decision 5); at a start,
      one that was working is cut off, and unseen.
    - Why: design rule 6. Background work makes quitting and restarting able to destroy work
      that is not on the screen.
    - Depends on: 10.
    - Risk: a way out that skips the warning (the install script, a crash). Know it by: a
      scripted run that kills Null under a slow job and finds it "cut off" at the next start.
    - Source: specified from user (decision 3) + inferred from codebase (`harness::restart`,
      the page's `/quit` and Ctrl+C).
    - Status: Not started

12. **Choosing the folder: `/folder`**
    - What: `/folder` opens a list in the box: first the folders that the harness's
      conversations already work in, most recently used first, then a way into the list of the
      Mac's folders that `/attach` uses. `/folder <path>` goes straight there. Choosing one
      starts a new conversation in it (`session/new` with that folder); the conversation open
      before is left as it stands. `/new` goes on meaning the box's own folder. The folder is
      said on the notice line each time such a conversation comes to the front (decision 4).
      Null keeps no list of folders: it reads the harness's. A question in the harness check:
      a conversation started in a folder works there.
    - Why: "choose which folder a conversation works in"; `NullMini.md`, item 17. In its folder
      the agent has that project's instructions and files, as in a terminal.
    - Depends on: 8 and 1. It does not need 9 to 11 and may come before them.
    - Risk: macOS asking about Desktop, Documents or Downloads when the list walks in (not
      proven for `/attach` either); a folder that has gone; the agent working in the wrong
      folder. Know it by: the folder said each time, and a trial by the owner.
    - Source: specified from user (the goal, decision 4) + inferred from codebase (`attach.rs`,
      `harness::workspace`, `history.rs`).
    - Status: Not started

13. **Say it: work in the background**
    - What: the README's commands and what Null keeps. The research map. The glossary's
      Conversation line, for how an unseen conversation ended (decision 5).
    - Depends on: 9 to 12.
    - Status: Not started

14. **Null 0.4.0: tried by the owner first, then released** (decision 12)
    - What: in the order of `PetAndBox.md` item 8.
    - Depends on: 13.
    - Status: Not started

15. **The pet stays while work goes on** (decision 1: a prototype first, outside the releases)
    - What: first a prototype for the owner to live with. Then: when the box is put away while a
      conversation is working, asking or unseen, the window becomes the pet alone, at the place
      the pet stands, a third size in the contract between the page and `panel.rs`. A press on
      the pet, or Control+Space, brings the box back, open on the most pressing conversation.
      When nothing is left to say, it goes. With the pet put away, nothing stays.
    - Why: with the box hidden there is otherwise no sign that work ended or is asking.
    - Depends on: 10.
    - Risk: a small thing on top of everything is unwelcome (the reference product's users
      complained that theirs cannot hide); the window taking presses meant for what is behind
      it. Know it by: the prototype, on the installed app, for some days.
    - Source: specified from user (decision 1; `PetAndBox.md`, Out of scope) + inferred from
      codebase (`panel.rs`).
    - Status: Not started

## Verification

- Unit tests (`cargo test`): the order of titles (the owner's, the harness's, Null's); a rank
  from made-up conversations; a file found by its id; events kept by conversation; which
  conversation is most pressing; what is written down and read back at a start.
- The harness check (`cargo test -- --ignored`): the questions of items 3, 4, 7 and 12.
- Scripted runs of the real app on a folder of its own with the stand-in: a conversation
  renamed and found by its new title; one deleted and gone from the list; words found inside
  one. With the `slow` answer: a slow job left with `/new`, a quick question meanwhile, the
  job opened again and shown whole; one stopped and the other going on; a fifth refused; Null
  killed under a job and "cut off" found.
- Pictures in the app's web engine: the list of commands; a row with its x and pencil; the
  heading for what was found inside; the "now" heading with each mark; each notice; the pet
  for each state while another conversation is open.
- By hand, the owner, on the installed app: a conversation deleted and put back from the
  Trash; one renamed; one found by something said in it; a real long job left running while
  asking something else; a job that ends while the box is away; `/folder` on a real project.

## Validation

- No ADR is contradicted. One harness process is still the carried one or "yours" (ADR 0003).
- **Null reaches into the harness's folder for the first time** (design rule 8): it moves a
  file to the Trash and it reads files to search them. It changes nothing inside one. Checked
  against "the harness keeps the conversations": the harness still does, and Null keeps
  nothing new but a title the owner typed.
- **"Everything the box does is a typed command" is bent for deleting,** which has a key
  (Command+Delete) and a button but no typed command. Put to the owner as "a key for it too",
  and agreed. Renaming has `/rename`; searching is typing in `/history`.
- **The glossary's Conversation line is widened twice:** for a title the owner gave (item 5)
  and for how an unseen conversation ended (item 13). Each follows when it is built.
- **"The box hides completely" is bent by item 15,** by the owner's decision, and only as a
  prototype until the owner has lived with it.
- The sizing contract does not change before item 15. Every list uses the room lists have.
- Still fuzzy, on purpose: the pet alone (item 15); how a row shows a piece of what was found
  (item 4, by pictures); how long "a moment later" is.
- The order holds: 1, 2, 3, 4 in a row, then 5 and 6; 7, 8, then 9, 10, 11 in a row; 12 any
  time after 8; 13 and 14; 15 after 10.
- The trials' scripts are scratch and not in the repo. Items 3, 4 and 7 turn them into
  questions of the harness check.
- No outside research was needed: the questions were about the carried harness and the
  owner's own Mac, and both were asked directly.

## Out of scope

Agreed with the owner at `/grill` on 2026-10-10.

**Deferred, and named at every recall** (`Context/Research/Research.md`, "Deferred by the
owner"), at the owner's word: "make sure to put them somewhere safe ... so that we don't lose
track of the deferred tasks".

- The agent starting background conversations by itself. Why: the owner starts each job, and
  an agent handing work to another belongs with voice (`NullMini.md`, item 15).
- A macOS notification or a sound when work ends. Why: the pet and `/history` are the signs.
- Voice. Why: it is step 4, after this one.
- Search by meaning in `/history`. Why: word search finds nearly everything among a few
  hundred conversations. Worth its cost with thousands of them, or with voice, where one asks
  by meaning. If the harness comes to offer a search of its conversations, Null shows that.

**Left out, and not deferred:**

- Other harnesses, and carrying a conversation from one to another. Why: a later phase of
  `NullMini.md`, and nothing here needs it.
- Work set to start later. Forking a conversation. Why: nobody asked for them.
- Work that goes on after Null quits. Why: the owner brought it in at the grill, for `/quit`
  and Ctrl+C only ("The first one, when I type quit, or I do Ctrl C, which is equivalent to
  quit"), and took it out again the same day: quitting ends the work, after one warning
  (decision 3). An update, a crash or a restart of the Mac cut work off as well (item 11).
- An index of what was said, and writing a title into the harness's own file. Why: decisions
  11 and 10.

**Brought in by the owner at the grill,** and planned above: deleting, renaming and searching
inside conversations (decisions 9 to 11, items 2 to 4).
