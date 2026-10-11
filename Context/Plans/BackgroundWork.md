# Work in the background

> Blueprint: 2026-10-10, grilled with the owner the same day and planned again with what the
> grill settled. Step 3 of the five in `Context/Plans/NullMini.md` (Phase 3, items 15 to 17),
> planned for Null as it is now: its own app in `Mini/`, Oh-my-pi over the protocol, the box,
> history and the pet. It replaces those three items, which were written for a mini inside the
> Voice desktop app and its server.
>
> Status: **The tools for history are built and released, as Null 0.3.0 and then 0.3.1 (items
> 1 to 6, 2026-10-10): the list of commands, renaming, deleting to the Trash and searching
> inside, each by its own pull request (#42 to #45, and #47 and #49 for two faults the owner's
> use found). The owner's Mac runs 0.3.1 from the release. The owner used deleting; the log shows no title given and no search
> yet. Work in the background itself (items 7 to 14) has begun: the harness check asks what
> it rests on (item 7, pull request #51), and Null keeps a record for each conversation it
> holds (item 8, pull request #52). A conversation is left working with `/new` or
> `/history` and found again, four at most (item 9, pull request #53). The pet, "now" in
> `/history` and the notice line say how each conversation stands (item 10, pull request
> #54). Item 11, which stops no work silently (quitting, restarting, being cut off), is
> built and waits in pull request #55 for three scripted runs through the real page, which
> stand still while the Mac's screen is locked. Nothing of items 9 and 10 is to be released
> before it is merged. Next: those three runs and the merge, then item 12. The owner
> confirmed the goal and settled all twelve decisions.**

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
| How the harness keeps a conversation | One file each, `sessions/<its folder>/<when>_<id>.jsonl` in the harness's folder, found by the id. Beside it there may be a folder of the same name, for the conversations of the helpers the agent started (113 of the owner's 275 files are in such folders). Each line is one thing: a `title` line, a `session` line, and a `message` line for each thing said, with who said it (`user`, `assistant`) and the words as plain text |
| What happens when that file is taken away | The harness stops listing the conversation at once, does not write the file again, and will not load it. Put back, it is listed and loads again |
| How to tell that a program has a conversation open | Not by the lock file beside it: that file is empty, nothing locks it, and it stays when the harness has gone (169 of them lie in the owner's folder). A harness has the conversation's own file open for as long as it holds the conversation, and lets go when it closes it or ends; macOS says which program has a file open (`lsof`), in about a tenth of a second |
| A harness that still holds a conversation whose file was moved | It goes on answering and writes into the moved file, wherever that now is. So a conversation deleted while a terminal has it open would lose nothing, and putting it back would bring all of it |
| Whether the Trash can put back a file a program moved there | Yes, by the Trash's own record: a scratch file moved with macOS's call for it was written down there with the folder it came from |
| How long it takes to look through every conversation for a word | On the owner's Mac, 275 conversations in 37 folders, 363 MB (half of them under 308 KB, the largest 41 MB): 0.03 s for a fast look that uses every core, 1.3 s for a simple one, both with the files in memory. Only how long it took was read, never what was found |
| The line for a mistyped command | It holds about 81 characters and the eleven commands take 78. A twelfth does not fit |
| The pet and the hidden box | `panel::hide` takes the whole window away. The pet is drawn by the page in a 46 px lane of the same window |

Not found out: the owner's own MCP servers with several conversations (each conversation attaches
them again, and one that runs as a program may be started once for each); a real provider's
limit with several replies at once; whether closing gives memory back after a while. "Put Back"
itself was not pressed: that is the owner's hand.

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
     a terminal at that moment. How that is known was put wrongly here at first, as "its lock
     file is there": the lock file says nothing (see "What was found"). Null has its own
     harness let go of the conversation and then asks macOS whether any program still has the
     file open.
   - **Mended after the owner's first use (2026-10-10):** "For some reason, it doesn't delete
     the conversations all the time." / "It keep saying that something is opened in another
     program." Null refused whenever any program had the file open, and on the owner's Mac
     an Oh-my-pi that the Voice app's server started on 2026-10-08 was still running in the
     background with ten conversations open, where nobody could see it to close it. The rule
     is now what the owner agreed to, to the letter: only a conversation open in a terminal
     is left alone. A program in the background that still holds one does not stand in the
     way, and loses nothing (see "What was found": a harness goes on writing to the moved
     file).
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
   - Status: Complete (2026-10-10). On `main` with pull request #42 and in Null 0.3.0. Not
     known to have been tried by the owner: the log does not say when the list is opened
   - **Built:** in the page, each command has a few words (`ABOUT`), and a section of its own
     draws them as a list with the keys and the typing taken before the field's own handlers,
     as the history list does. A lone `/` and Enter lists all eleven. A slash-word that is no
     command lists the ones it could mean and stays in the field to be put right; typing
     narrows the list. Enter carries out the chosen row, or what is in the field when that is
     a whole command, with or without more after it (`/model opus`). Esc puts the list away.
     The one line of commands is gone.
   - **Decided by the agent:**
     - **The order of what is listed:** commands that start with what was typed, then those
       with it in their name, then those with a word beside them that starts with it (`/sign`
       finds `/login`, `/file` finds `/attach`), then those one or two slips away, a slip
       being a letter wrong, missing or added, or two swapped (`/histroy`, `/mdoel`).
     - **Nothing like it:** the heading says "no command like /xyz" and every command is
       listed under it, since the list is what was wanted.
     - **The look:** the names in the blue a command has when typed, the words dim, under the
       heading "commands". Eleven rows are more than the list's room, so it scrolls and fades
       like the others.
     - **It opens while a reply runs,** as `/usage` does: it changes nothing by itself.
   - **Proven without a person:** 84 unit tests, none new (the change is in the page).
     Seventeen states drawn in the app's web engine and read back, among them: `/`, `/hist`,
     `/histroy`, `/mdoel`, `/xyz`, `/h`, `/sign`, `/file`, `/conv`; narrowing by typing; the
     arrow keys to the last row; Enter on a row (`/hist` then Enter opened the history list,
     the ninth row put the pet away); a whole command typed in the open list; Esc over a
     reply, which came back; a message that starts with a path, which went to the agent as
     before. No row was cut off. Four pictures looked at, dark and light.
   - **Not proven:** the real app with a real key press and a real pointer. The page was
     drawn outside the app with its commands replaced by stand-ins, which is all this change
     touches.

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
   - Status: Complete (2026-10-10). On `main` with pull request #43 and in Null 0.3.0. Not
     tried by the owner as far as the log shows: no title was given
   - **Built:** `history.rs` keeps two kinds of title, the ones Null makes and the ones the
     owner gives (`Titles`), in `history.json` under `titles`, as before, and `renamed`. A
     file an older Null wrote reads as it did. A new command, `rename_conversation`, with no
     id means the open conversation. In the page: `/rename` and a title, with "renamed" and
     the title on the notice line; and in `/history` a pencil at the end of a row and
     Command+R on the chosen one, which put the title in the field, selected. Enter keeps
     what is there, Esc leaves the title as it was and the list open, and the words that
     narrowed the list come back. The arrow keys and Tab rest meanwhile, and so does a press
     on a row.
   - **Decided by the agent:**
     - **Taking a title away:** Enter on an empty field takes the owner's title away, and the
       title the conversation had before shows again. Nothing else undoes a rename.
     - **`/rename` with no title** says what the command is for and changes nothing.
     - **A fresh box:** with no conversation yet, `/rename` says there is none to rename.
     - **The log** says that a title was given or taken away, never the title.
     - **Room for the marks:** every row of `/history` keeps 19 px at its end for them, so
       that nothing moves when they show. The place and time of each row stand that much
       further left than they did.
   - **Proven without a person:** 87 unit tests, three new: the owner's title comes before
     the harness's and Null's, and taking it away brings the other back; the file as an
     older Null wrote it, and as it is written now; a title goes with its conversation
     whoever gave it. Fourteen states drawn in the app's web engine and read back: the
     pencil on the chosen row and on no other, a row's height unchanged at 23 px, Command+R,
     Enter with a new title (the list then showed it), Enter on an empty field, Esc, Esc
     again (the list goes), the narrowing words coming back, the arrow keys and a press on a
     row doing nothing meanwhile, the pencil pressed on another row, `/rename` with and
     without a title, and `/ren` through the list of commands. Three pictures looked at.
   - **Not proven:** the real app from end to end: a title typed, kept in the file and shown
     again after a restart. The pointer over a row (drawn by adding a rule, not by a
     pointer). Command+R as macOS hands it to the box.

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
   - Status: Complete (2026-10-10). On `main` with pull requests #44 and #47 and in Null
     0.3.0. Used by the owner on the installed build: more than twenty conversations went to
     the Trash, and one fault was found and mended
   - **Built:** `history.rs` finds a conversation's file by its id (`file_of`), asks macOS
     which programs have it open (`held_by`) and moves it to the Trash with macOS's own call
     (`to_trash`). A new command, `delete_conversation`: it refuses a conversation whose
     reply is running, has Null's own harness close the conversation (`harness::close`, a new
     order, which starts no harness to ask it), refuses if any program still has the file
     open, moves the file, and gives the box a new conversation if that one was open. In the
     page: an x after the pencil on the row under the pointer or chosen, and Command+Delete
     on the chosen row; the row goes, and the notice line says "moved to the Trash" with the
     title. A refusal is said on the notice line and the list stays as it was.
     `translate::abilities` now says whether a harness closes a conversation.
   - **Deviation, and the reason:** "open in a terminal" is not read from the lock file,
     which says nothing, but from macOS (decision 9; "What was found").
   - **Decided by the agent:**
     - **What is beside the conversation goes with it:** when the harness kept a folder of
       helpers' conversations beside the file, that folder is moved to the Trash too, as a
       second thing there. If it will not move, the conversation is deleted all the same and
       the log says so.
     - **Command+Delete deletes only with nothing typed in the field.** With words there it
       is the field's own key and clears them, as everywhere on a Mac, so that clearing a
       narrowing never deletes a conversation.
     - **The lock file is left where it is.** It is empty, and Null erases nothing.
     - **A run under a profile of the harness's own** offers no x: Null does not know that
       folder.
     - **The log** says that a conversation was moved to the Trash, never which.
   - **Proven without a person:** 89 unit tests, two new: a conversation's file is found by
     its id, with what is beside it, and nothing that is no id is looked for; a file that a
     program has open is known to be held, and by which. A new question in the harness
     check, on the carried Oh-my-pi: the file is found by its id; the harness has it open
     while it holds the conversation and lets go when asked; moved to the real Trash by
     Null's own code, the conversation is listed no longer and does not load; taken out
     again, it is listed and loads. Fifteen states drawn in the app's web engine and read
     back: the x beside the pencil, a row's height unchanged, Command+Delete on the chosen
     row, the x pressed on another, words in the field (nothing deleted), the open
     conversation deleted (the box emptied), the last one deleted (the list goes), a refusal
     said, a harness that offers no deleting (pencil only), and a title being typed (nothing
     deleted). Two pictures looked at.
   - **Not proven:** "Put Back" pressed in the Trash by the owner. The real app from end to
     end: the x pressed, and the row gone after a restart. A conversation that really is
     open in a terminal being refused (proven for a harness holding the file, not with the
     owner's terminal). The owner's own Oh-my-pi, "yours", which is what the installed Null
     runs.
   - **Found by the owner's use, and mended (2026-10-10, branch `delete-held-in-background`):**
     the refusal for a conversation "open in another program" came for conversations held by
     a forgotten background harness (decision 9). Now a holder counts only if it runs in a
     terminal, which macOS says of each program (`ps`); the words are "that conversation is
     open in a terminal. close it there first". Why a conversation was not deleted now goes
     in the log, without saying which, since the first look for this fault found nothing
     there. In the owner's first two minutes with the x, 23 conversations went to the Trash.
     One unit test more, 95 in all: a program in a terminal is told from one in the
     background. The question in the harness check also asks that a harness started by a
     program is in no terminal.
   - **What the owner will meet:** a conversation put back from the Trash has lost the title
     Null kept for it, made or given, once the list was shown without it; one made in the
     box is "untitled" until it is opened. A conversation with helpers' conversations is two
     things in the Trash, and both want putting back.

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
   - Status: Complete (2026-10-10). On `main` with pull request #45 and in Null 0.3.0. Not
     tried by the owner as far as the log shows: no search was made
   - **Built:** a new module, `Mini/src/search.rs`, in the two steps planned: every
     conversation's file looked through whole for the words, on every core, and only the
     files that have them all read line by line for who said what. A new command,
     `search_conversations`, in which a newer search ends an older one. In the page: a
     quarter of a second after the last key in `/history`, the conversations found are added
     under the heading "said inside", each with its title and, where the place and time
     stand on other rows, a piece of what was said. They are chosen, opened, renamed and
     deleted like any other row. One new dependency named in `Cargo.toml`, `memchr`, which
     the build already had.
   - **Measured on the owner's Mac, with Null's own code, built as a release:** the owner's
     162 conversations (the files right under a folder; the rest are helpers') were looked
     through in 15 ms for a word nobody said, 44 ms for "linker" (4 found), 70 ms for "the"
     (the best 40 of nearly all), and about 62 ms for two words. The first look took 60 ms.
     Only the numbers were read.
   - **Decided by the agent:**
     - **At least two letters** in all. One letter is in every conversation.
     - **Capitals do not matter for A to Z only.** A letter with an accent has to be typed as
       it was written.
     - **How a conversation is ranked:** a word in the harness's title counts five, in what
       the owner wrote three, in what the agent replied one; and the more often a word is
       said, the less each further time adds. A tie goes to the newer, by its id.
     - **Not searched:** what a tool read or gave back, the agent's thinking, and the
       conversations of the helpers an agent started.
     - **The best forty** are given, no more.
     - **Found by its title already,** a conversation is not shown a second time under "said
       inside".
     - **The log** says how many were found and how long it took, never a word.
   - **Proven without a person:** 94 unit tests, five new: what was typed becomes the words;
     a conversation is found by what was said in it and not by what a tool read; what stands
     first; the piece shown, cut between letters and never through one; a search no longer
     wanted gives nothing. A new question in the harness check, on the carried Oh-my-pi:
     what the owner wrote and what the agent replied are in the conversation's file as the
     search reads it. Eight states drawn in the app's web engine and read back: found by
     title and by what was said, capitals, one letter (nothing asked), nothing found, found
     only inside, the arrow keys and Enter on one found inside (it opened), typing on (only
     the newest words asked), and the pencil on one found inside. One picture looked at.
   - **Not proven:** the real app from end to end. The owner's eye on a found row: its title
     is cut short to make room for the piece. The first search after the Mac was started.
     The owner's own Oh-my-pi, "yours".

5. **Say it: the tools for history**
   - What: the README's commands and what Null keeps (the titles the owner gave). The research
     map, together with what `PetAndBox.md` item 7 left behind. The glossary's Conversation
     line, for the titles the owner gave.
   - Depends on: 1 to 4.
   - Status: Complete (2026-10-10)
   - **Done, the README:** six cards made again by `.github/Readme/make_cards.py`, and what
     the README says of each beside it. Commands: `/rename`, what `/history` now finds and
     does, and that an unknown command opens the list. Use Null: a lone slash. Privacy: Null
     reads the harness's conversation files to search them and keeps nothing, and deleting
     moves a file to the Trash. What Null keeps: a title the owner gave. Source map:
     `search.rs`. Status: the three tools are available and still wait for hands-on use, and
     work in the background is what is planned.
   - **Done, the research map:** a sixth addition. The box's commands and what an unknown one
     does; the modules `attach.rs`, `history.rs` and `search.rs`; 35 commands and the events
     `conversation_opened` and `mini:attached`; the window's width with the pet; `history.json`
     and how the harness keeps a conversation's file; 20 questions in the harness check; and
     the two plans, with which of them is active.
   - **Done, the glossary:** a conversation's title is one Null made "or named by the person".
   - **Left for item 13:** the new words of `PetAndBox.md` (pet, attach) are still not in the
     glossary; nothing here needed them.

6. **Null 0.3.0: tried by the owner first, then released** (decision 12)
   - What: in the order of `PetAndBox.md` item 8. An installed 0.2.0 can then see `/update` say
     that a newer Null is out, which nothing has shown yet (`OwnHarness.md`, item 7).
   - Depends on: 5.
   - Status: Complete (2026-10-10). Null 0.3.0 is released, and the owner's Mac runs the
     release
   - **Step 1:** pull request #46. `main` says 0.3.0 and the README is up to date.
   - **Step 2, the install:** at the owner's word ("Yes, let's build main and install the
     latest version to my mac"), `Mini/Scripts/install` on `main` at `e1b2ce7`. The box had
     been idle for two hours. Afterwards: 0.3.0, the seal holds, the same designated
     requirement, the checksum of the Oh-my-pi inside, the login item running, "started,
     version 0.3.0" and "Full Disk Access is on" in the log, and the keychain list as it was.
   - **Step 3, what the owner's use turned up:** the x did not always delete ("It keep saying
     that something is opened in another program"). Found, mended and installed again from
     `main` at `71dec65` (item 3; pull request #47).
   - **Step 4, the release:** at the owner's word ("New release."), the tag `null-v0.3.0` on
     `main` at `71dec65`. Only Null's own workflows answered the tag. GitHub published
     `Null-0.3.0-macos-arm64.zip` (97 MB) with `SHA256SUMS.txt`; fetched with `gh`, the
     checksum matched, the seal held, the app says 0.3.0, and the Oh-my-pi inside has the
     checksum of `Engine.toml` and its author's signature. Then the one-line install with no
     version named, after checking that the script GitHub serves is the one in the
     repository: it found 0.3.0 as the newest, signed it with the identity on the Mac and
     replaced the build made here. No reply was under way. Afterwards, the same checks as
     after step 2, all as they should be.
   - **Released before the owner had tried all of it.** The owner gave the word five minutes
     after the mended build was installed. By the log, the box was opened once in that time
     and nothing was deleted, so the mend for deleting was not seen to work by the owner
     before the release, and neither renaming nor searching had been used. Said to the owner
     with the release.
   - **After the release, found by the owner in 0.3.0 (2026-10-10):** "when I click on the x
     button to remove a chat, for some reason, it keeps resetting me to go the top of the
     chats. I have to keep scrolling down to find where I was deleting from which kind of
     annoying." The list was drawn afresh after each deletion, which starts it from its top,
     and then went to the chosen row, which the pointer had never moved. Mended on the branch
     `history-stays-in-place`: after a row is deleted or renamed the list is put back where
     it was, and the row that took the deleted one's place is the chosen one, so that the x
     can be pressed again at the same spot. Drawn with forty conversations and the list
     400 px down: it stayed at 400 after one deletion, after three in a row, and through a
     rename kept or left. Pull request #49; `main` built at `cb48e62` and installed.
   - **Null 0.3.1, released at the owner's word ("Release it.", 2026-10-10):** the tag
     `null-v0.3.1` on `main` at `cb48e62`. Only Null's own workflows answered. The published
     file's checksum matched, its seal held, it says 0.3.1, and the Oh-my-pi inside is the
     one `Engine.toml` names, with its author's signature. The one-line install found 0.3.1
     as the newest and replaced the build made here; afterwards the same checks as before,
     all as they should be. By the log, the box had not been opened between the install of
     the mended build and the word to release it, so the owner had not yet seen the list
     stay in place.
   - **Still not proven by anyone's hand:** `/update` on an installed 0.2.0 saying that 0.3.0
     is out (the owner's Mac went from 0.2.0 to builds of 0.3.0 without asking it).
   - **To try by hand, since nothing but the owner's hand proves it:** a lone `/`; a mistyped
     command; `/rename`; the pencil and Command+R in `/history`; the x and Command+Delete,
     and "Put Back" in the Trash; a word that was said in a conversation and is in no title.
     The installed Null runs the owner's own Oh-my-pi ("yours"), which none of the checks
     used.

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
   - Status: Complete (2026-10-10). On `main` with pull request #51. Nothing in the box
     changes with it
   - **Built:** the stand-in has a fifth answer, `slow` (`Mini/src/standin.rs`), in
     Anthropic's way of talking and in OpenAI's: it says back the last line of the message it
     was sent, a quarter of a second and then a word, written as it comes. A harness that
     stops the reply hangs up, and the rest is left unsaid. `ok` and `slow` are one reply
     now, sent whole or a word at a time. The check's `Agent` (`Mini/src/check.rs`) sends a
     message without waiting for the reply (`begin`, `ended`), stops one (`stop`), listens
     until something has been heard (`hear_until`), and keeps each thing it hears with the
     conversation it came from. Five new questions, 25 in all:
     - two conversations in two folders answer at the same time;
     - stopping one conversation leaves the other working;
     - a second message to a conversation still answering stops the first reply;
     - a conversation loads back while another is working;
     - a closed conversation loads back.
   - **Already there:** `translate::abilities` has said "closes a conversation" since item 3.
   - **Decided by the agent:**
     - **What `slow` says:** the message back, so that a reply shows which message it
       answers and lasts as long as the message has words. Ten words are two and a half
       seconds. Only the last line, because the harness puts a note of its own (the date and
       the folder) in front of the first message of a conversation.
     - **A quarter of a second for a word,** not the second of the trials, so that the check
       stays quick: the five questions take four seconds together.
     - **The ChatGPT way of talking has no `slow`,** as it has no `ok`: its stand-in only
       refuses.
     - **"While another is working" is asked with a conversation from an earlier run,** which
       the harness does not hold. That is what opening from `/history` is, and the question
       then does not lean on closing, which has a question of its own.
     - **A question says when it can no longer tell.** Each of the four that need a reply
       under way fails with "this can no longer be told" if the slow reply was over too
       soon, where it could have passed by luck.
   - **Proven without a person:** 96 unit tests, one new: `slow` says back the last line a
     word at a time in both ways of talking, and no faster than its waits. The harness check
     on the carried Oh-my-pi 18.8.7, 25 questions, four at a time as the runner asks them:
     all passed, in 20 seconds. On this Mac the word passed from one reply to the other 19
     times, where 2 would do; and when one conversation was stopped, or another loaded back,
     the reply under way had said one word of its ten. With the wait taken out of `slow`,
     the four questions that need a reply under way all failed and said why, so they ask
     what they say.
   - **Not proven:** the key check (`Mini/tests/keys.rs`) was left to the runner, since it
     opens the box on the owner's screen. The owner's own Oh-my-pi, "yours". A newer
     Oh-my-pi: the weekly job asks it these questions from its next run.
   - **Left for item 13:** the README's card of the development switches names four answers
     of the stand-in, not five.

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
   - Status: Complete (2026-10-10). On `main` with pull request #52. Nothing in the box
     looks or behaves differently
   - **Built:** in `Mini/src/harness.rs`, what belonged to "the" conversation is a record
     now (`Conversation`): the harness's id for it, whether the running harness holds it,
     its events, whether it is working, its model, its tools and its waiting approvals.
     `Shared` keeps one record for each conversation this run knows, and beside them what
     belongs to the whole connection: which one is open, the models offered, `noted` and
     `counts_tokens`. Every order to the harness thread (`Send`, `Interrupt`, `SetModel`,
     `CheckModel`) and every event names its conversation. What the harness says
     (`on_update`, `on_permission`) goes to the conversation it names, and is dropped only
     when this run does not hold that one. The struct that was called `Conversation` is the
     connection and is called `Connection`. `events_since` gives the open conversation's
     events and says which conversation that is. In the page, an event that names another
     conversation than the open one is not drawn.
   - **Deviations, and the reasons:**
     - **A conversation is known by a number Null gives it, not by the harness's id.** The
       id comes only when the first message opens the conversation in the harness, and
       events are kept before that: the message itself, and "working". With work in the
       background a second new conversation can begin while the first is still being
       opened, and the two have to be told apart. The id is kept in the record, and what
       the harness says is matched by it.
     - **Events keep one count between them, not one for each conversation.** They are kept
       by conversation; only the numbering is shared. So the page asks for what it missed
       with one number, whichever conversation it shows.
     - **`opening` stayed with the connection.** A conversation being opened from the list
       is not held until it has loaded, and the harness thread opens one at a time. While
       one is being opened, what the harness says of the others is no longer dropped, since
       from item 9 another may be working.
     - **`/new` no longer waits its turn on the harness thread.** There is nothing to ask
       the harness: the box turns to a new record at once, and the page is given its
       number.
   - **Decided by the agent:**
     - **Still one conversation at a time.** Starting over, or opening an earlier one, lets
       the last one go, as before, and each still refuses while a reply is under way. Item 9
       changes that.
     - **`busy`,** which `/update`, `/harness`, `/backup` and deleting ask, is true while
       any conversation works. Today that is the same thing.
     - **Two of the page's commands give something else:** `new_conversation` the number of
       the new conversation, and `delete_conversation` that number when the open one was
       deleted and nothing otherwise, where it gave yes or no.
   - **Proven without a person:** 101 unit tests, five new: what a conversation says is kept
     with it, under one count; a page is given only what the open one said; a conversation
     is found by the harness's id; starting over leaves one new conversation on the same
     model; nothing is loaded once the harness has gone. The harness check, 25 questions.
     Scripted runs of the real app on folders of their own with the stand-in. With no
     window: a message answered; the app started again and the conversation loaded back; a
     slow reply stopped after a second, which ended as stopped with three of its eight
     words; a slow reply left to its end; a used-up limit with a backup, and the box told
     of the move; a file read by the harness's tool. Through the real page: a message,
     `/new`, a message, `/history`, the first conversation opened again and continued
     ("one | pong | three | pong"); then the app started again on that folder, the
     conversation loaded back and answered, and the other was opened from history and
     continued. Eight states of the page drawn with the page as it was and as it is: seven
     the same byte for byte, the eighth a minute apart on the clock it shows.
   - **Not proven:** an approval asked and answered. On this Mac the agent acts without
     asking, and the stand-in asks for nothing that needs a yes even with the harness set
     to ask. Attaching a file through the real page, and `/model` typed in it: the Mac's
     screen had locked by then. The harness started again in the middle of a run (after a
     sign-in, `/harness`, `/update` or `/backup`). Deleting the open conversation in the
     real app: drawn with stand-ins only. The owner's own Oh-my-pi, "yours", and the
     owner's hand.
   - **Found on the way:** a scripted run through the page stands still for as long as the
     Mac's screen is locked, and goes on when it is unlocked. The first one took five
     minutes for that reason. Look at the log's "frontmost app" line before waiting on one.

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
   - Status: Complete (2026-10-10). On `main` with pull request #53. Not to be released
     before items 10 and 11, for what is listed at the end of this item
   - **Built:** in `Mini/src/harness.rs`, the box turning to another conversation
     (`turn_to`) no longer drops the one it showed: one that is working is kept and goes
     on, and one at rest is held no longer and closed in the harness. A reply that ends
     while the box shows another conversation is marked `unseen`, and its record is kept
     until that conversation is opened. `/new` and opening from `/history` no longer
     refuse while a reply runs. Opening a conversation this run holds is not asked of the
     harness: the box turns to it and the page is given its number
     (`open_conversation` now gives one, or nothing when the harness loads it). At most
     four work at once (`MAX_WORKING`, `begin_reply`); a fifth is refused with
     "four conversations are already working. wait for one to end, or stop one". In the
     page: `/new` and `/history` while a reply runs; "still working. /history shows it";
     a held conversation drawn again from what the app kept (`catchUp(true)`), with what it
     says next following; nothing of the left conversation's reply staying behind
     (`emptyBox`); a refused fifth put back in the field. `Mini/src/history.rs`: deleting
     asks whether that conversation is working, not whether any is.
   - **Decided by the agent:**
     - **"Unseen" begins here,** since it decides what Null keeps. Item 10 shows it.
     - **Only the conversation the box turns away from is ever let go,** and only at rest.
       Nothing is closed at another moment.
     - **The open conversation chosen again from `/history`:** at rest it is loaded afresh
       by the harness, as before. Working, the box shows what it has.
     - **"still working. /history shows it" is said each time** a working conversation is
       left, not only the first time.
     - **Deleting one that works but is not open** is refused with "that conversation is
       still working. open it and stop it first".
     - **A scripted run leaves a reply under way with " &"** at the end of a line: the
       next line does not wait for the reply (`SELFTEST_TYPING`).
   - **Proven without a person:** 105 unit tests, four new: a conversation left working
     goes on and one left at rest is let go; a reply that ends while the box shows another
     waits to be seen, and is seen once shown; one that ends in the open conversation has
     been seen; a fifth is not set to work, and there is room again when one ends. The
     harness check, 25 questions. Two runs of the app with no window: a message answered;
     started again, a slow reply stopped. Five states of the page drawn with stand-ins
     for the app: a working conversation left with `/new` (the box empty, the line said,
     the arrow at rest); what the left one says afterwards not drawn, and what the new one
     says drawn; a held conversation opened from `/history` and drawn from what was kept,
     with its next words following and the arrow and the pet at work; a fifth refused, back
     in the field; `/history` opened over a reply. Eight other states the same byte for
     byte as on `main`.
   - **Proven on the real app, by script (2026-10-10, at the owner's word, the screen
     unlocked):** two runs through the real page, each on a folder of its own with
     `{"model":"standin-anthropic/slow","welcomed":true,"asked_full_disk":true}` in its
     `null/settings.json`, started as
     `NULL_MINI_FOLDER=<folder> NULL_MINI_STANDIN=1 NULL_MINI_NO_SHORTCUT=1 NULL_MINI_SELFTEST=<lines> target/debug/null-mini`.
     The pull request was held back until both had passed.
     - **Left and found again, in 9 seconds.** The lines: twenty-four words and " &";
       `/new`; `/model`; `standin-anthropic/ok`; `quick`; `/history`; `alder`, the first of
       the twenty-four. The slow job was left; a second conversation was opened, moved to
       another model and answered; the box turned back to the first while it was still
       working, and its reply ended after that. The page showed the twenty-four words
       twice: the message, and the reply said back whole.
     - **A fifth refused, in 8 seconds.** The lines: four times sixty words and " &", then
       `/new`; then `five`. Four conversations were opened and working. The notice said
       "four conversations are already working. wait for one to end, or stop one", nothing
       was shown and the arrow was at rest.
     - **What such a run does to the screen:** a second box for those seconds, and the
       app that was in front (Ghostty) gave way to it meanwhile.
   - **Also not proven:** one conversation stopped while another goes on, in the real app
     (the harness check asks it of the harness). Memory and processes with the owner's own
     Oh-my-pi and MCP servers. The owner's hand.
   - **Open until items 10 and 11, and not to be released before them:** `/quit`, and
     Ctrl+C at rest, quit at once and end what works in the background without a word. A
     sign-in, `/harness`, `/update` or `/backup` while another conversation works starts
     the harness again under it. A conversation that ends or asks while it is not open
     shows nowhere but as a row in `/history`.

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
    - Status: Complete (2026-10-10). On `main` with pull request #54. Not looked at by the
      owner yet, which this item asks for
    - **Built:** `Mini/src/harness.rs` keeps how each conversation's last reply ended
      (`Ended`: done, failed, stopped) and gives each its word (`standing`: working, asking
      while it waits for an answer, or as it ended). `work()` lists every conversation that
      is working, asking or unseen, the most pressing first. The page is told at every
      change (`tell_work`, the event `mini:work`), each telling numbered so that a late one
      is not taken for the newest, and asks for the same at load (the command `work`). In
      the page: a first heading "now" in `/history`, each row with a sign where the open
      one has its dot (`?` asking, `✗` failed, `✓` done, `…` working) and the word for how
      it stands where other rows have their time, amber for asking and red for failed; the
      notice line saying "done: title", "asking: title" or "failed: title" for another
      conversation, and a press on it opening that conversation; and the pet making the
      most pressing thing any conversation has to say its own (`allSay`).
    - **Decided by the agent:**
      - **A failed reply** is one that ended with an error, with a provider's refusal, or
        with the model's own refusal. **Stopped** is one the person ended.
      - **The signs** are the ones the box already uses for a step of the agent's work.
        Each is drawn over the two spaces every row begins with, so that every title
        starts at the same place whatever the sign is.
      - **The pet and a conversation that is done and not yet seen:** it hops once and
        then stays glad, without moving, until that conversation is opened. One that
        failed keeps the pet sunk, with red eyes, until it is opened.
      - **The open conversation's own gestures are kept:** the hop when its reply ends
        and the sinking when it fails are made as before, and afterwards the pet goes
        back to what the others have to say.
      - **No news at load, and none while `/history` is open,** where "now" shows it.
      - **The line stays its usual four seconds,** for asking as well. The pet and "now"
        are what stay.
      - **"Cut off" is left to item 11,** where it is first known.
    - **Proven without a person:** 106 unit tests, one new: the page is told of what
      works, asks or waits to be seen, the most pressing first, and of nothing else. Seven
      states of the page drawn with stand-ins and read back: "now" with one asking, one
      failed, one done and the open one working (every title starting at the same place,
      rows 23 px, none cut off); news of one asking, one done and one failed, each with
      the pet's mood; the pet at work for another conversation while the arrow rests; a
      press on the line asking for that conversation; a telling that arrives late left
      aside. Twelve other states the same byte for byte as on `main`. Three pictures
      looked at, dark. On the real app, two scripted runs with the slow stand-in: a job
      left with `/new` ended while a second conversation was shown, and the notice line
      said "done:" with its title; then `/history` listed it first under "now" with the
      word "done", ahead of the open one under "today".
    - **Not proven:** the owner's eye on any of it: the signs, the colours, the pet when
      several conversations work. A conversation that asks, since on this Mac the agent
      acts without asking and the stand-in asks for nothing that needs a yes. A press on
      the line in the real app. The light look was drawn and not looked at.

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
    - Status: Built (2026-10-10), in pull request #55, **not on `main` yet**. It waits for
      the three scripted runs through the real page that are written out below
    - **Built:**
      - **Quitting.** The command `quit` (`Mini/src/main.rs`) gives how many conversations
        are working and does not quit, unless asked with `anyway`. The page (`quit` in
        `Mini/Page/index.html`) says "2 conversations are still working. /quit again stops
        them and quits." and asks with `anyway` the next time. Ctrl+C with nothing running
        in the box does the same.
      - **Starting the harness again.** `harness::restart` lets the harness go only when
        nothing is working. Otherwise it is put off (`restart_due`) until the last reply has
        ended, and the box says "that takes effect once nothing is working: the harness
        starts again then". The refusals of `/harness`, `/update` and `/backup` while a
        reply ran are gone from the app; the page still refuses each while the open
        conversation is working.
      - **Being cut off.** `history.json` gains `standing`: the id and one word of each
        conversation Null holds that is working or unseen, written at every change. At a
        start (`recall`) one that was working is "cut off" and unseen, and one that had
        ended is as it ended. A harness that goes away by itself cuts off every reply under
        way, each with one line. Opening one that was only recalled is asked of the
        harness, which has all of it.
    - **Decided by the agent:**
      - **`/quit` warns when the open conversation is the one working too,** not only for
        the others: decision 3 says "while conversations are working".
      - **Anything else typed after the warning** means the next `/quit` asks afresh.
      - **Quitting at the owner's word leaves no "cut off" behind.** What was working was
        stopped by the owner, so it is not written down; what still waits to be seen is.
      - **Quitting sends the harness no order to stop each reply.** The harness goes with
        Null, and the replies with it.
      - **Cut off counts with failed:** next after asking in what is most pressing, a red
        sign and word under "now", and the pet sunk.
      - **A message to a conversation that waits to be seen** means it has been seen.
    - **Found on the way:** the page was told of a new conversation's id only at the next
      change, so one that was still working could be missing under "now", and was not
      written down. It is told as soon as the harness has given the id. A unit test found
      a fault in the waiting restart before anything ran it.
    - **Proven without a person:** 109 unit tests, three new: how a conversation stood is
      written beside the titles and read back, and goes with its conversation; what was
      working when Null went away is cut off and unseen at the next start, the open one
      too, and quitting at the owner's word leaves only what waits to be seen; the harness
      is started again only once nothing is working, and once. The real app with no
      window, killed under a slow job after two seconds: `history.json` held that
      conversation's id and "working", and no harness was left running. Seven states of
      the page drawn with stand-ins: the warning for one and for two; `/quit` twice (the
      second asks with `anyway`); `/quit`, another message, `/quit` (asked afresh);
      "cut off" under "now" with its sign; the line "cut off: title"; the app's line about
      the restart. Fourteen other states the same byte for byte as on `main`.
    - **Not proven, and the reason it is not merged:** the real app through its page. The
      Mac's screen was locked. Each run is on a folder of its own with
      `{"model":"standin-anthropic/slow","welcomed":true,"asked_full_disk":true}` in its
      `null/settings.json`, and shows a second box for a few seconds.
      - **Cut off and found.** On the folder where the app was killed under a job whose
        message begins with `job`, the lines: `/history`; `job`. To see: "the list shows:
        NOW" with that conversation and "cut off", and in the log "opened the earlier
        conversation", not "which this run held".
      - **The warning.** The lines: sixty words and " &"; `/new`; `/quit`. To see: "the
        notice says: 1 conversation is still working. /quit again stops it and quits."
      - **Quitting anyway.** The same lines and `/quit` once more. To see: the app ends by
        itself with "quit from the box; the conversation is closed, and what was working
        is stopped" in the log, and no `standing` in that folder's `history.json`.
    - **Also not proven:** a restart that waits, in the real app (a sign-in, `/harness`,
      `/update` or `/backup` while another conversation works): the unit test and the
      drawn line only. A harness that goes away by itself under several replies.

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
