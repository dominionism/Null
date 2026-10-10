# Null carries its own harness

> Blueprint: 2026-10-09. Builds on `Context/Plans/MiniApp.md` (the app as built) and
> `Context/Plans/Providers.md` (sign-in, limits and backups). It reverses decision 3 of
> `Providers.md` and takes over its item 6, first run.
>
> Status: **Items 1 and 2 are built, installed and proven by the owner (2026-10-09): Null carries
> Oh-my-pi and starts that one, and `/harness` chooses between it and the owner's own. Items 3, 4
> and 10 are built, merged and installed (2026-10-09, about 20:06): what a harness can do, the
> harness check, and `/update`. The carried version is 18.8.7. That installed build has not yet
> been checked by the owner's hand (see Verification). Item 5, the first opening, is built and
> proven by script, and not installed. Item 6 is begun: GitHub builds a ready-made Null, Null
> 0.1.0 is released, and fetched from there it runs on this Mac with no warning; the install
> command itself and the signing at install are not built. Item 9 is partly done: the README and
> the decision record. Not started: items 7 and 8.**

## Goal

Make Oh-my-pi a part inside Null, at one version Null chose, so that Null works the moment it is
installed and no update of Oh-my-pi can change it, while the owner's existing folder, skills and
MCP servers go on being used and a technical person can still put their own Oh-my-pi in its place.

## Constraints

- **Verbatim (user, 2026-10-09):** "my goal with Null is to keep implementation and software
  within it out (encapsulation) -- they just need to know it works. On the other hand, for people
  that are extremely technical and want to tweak things around they should be able to figure that
  out."
- **Verbatim (user, 2026-10-09):** "Null should use OMP as it's harness and should be installed in
  there automatically."
- **Verbatim (user, 2026-10-09):** "Is there a way that we can have null be unaffected by the
  update of omp?"
- **Verbatim (user, 2026-10-09):** "let's find a creative and intelligent way to achieve the
  ceiling of optimality architecture."
- **Verbatim (user, 2026-10-09):** "Let's blueprint that: own engine, my existing folder."
- **Verbatim (user, 2026-10-09):** "for people that will use Null, how can make that optimal for
  them as well? I fear that we don't have an optimal structure that they can use. That will
  definitely push people away. The goal with Null is to make things more accommodating to use, not
  have so many frictions."
- **Verbatim (user, earlier):** "OMP is just the harness that Null should have (the environment in
  which the specified agent will have access to and use as tools)."
- **Verbatim (user, 2026-10-09, on how other people get Null):** "people are just clone or npm
  install null." Not a download from a web page or a store, and no Apple account of any kind.
- The owner's skills, MCP servers, instructions and sign-ins must be there in Null as they are
  today. The owner asked this directly on 2026-10-09.
- Null never keeps a credential. Sign-ins stay in the harness's own folder (`Providers.md`).
- Null knows nothing about any one provider (`Providers.md`, design rule 1).
- Everything the box does is a typed command; words appear only for something the user has to
  know (`MiniApp.md`).
- The app is built without Node and without a bundler. Checks that run the real harness are
  `cargo test -- --ignored` (`MiniApp.md`, and the pattern in `signin.rs` and `providers.rs`).
- Folder names are capitalized (ADR 0001): new folders are `Mini/Engine/` and the like.
- The owner's instruction for commits: never an em dash in a commit message.

## What was established (2026-10-09)

Checked on this Mac and on `github.com/can1357/oh-my-pi`. Nothing was installed and no real sign-in
was touched; the two-version test used throwaway folders.

| Question | Finding |
|---|---|
| How Oh-my-pi is published | Tagged releases, each with a ready-made program for every system, a `SHA256SUMS.txt`, a `LICENSE` and third-party notices. For 18.4.3, `omp-darwin-arm64` is 214 MB and `omp-darwin-x64` 226 MB (corrected 2026-10-09 from the release page; 18.8.7's `omp-darwin-arm64` is 200 MB) |
| Who signed the Mac program | Its author, with an Apple Developer ID, and Apple has notarized it (`spctl`: "Notarized Developer ID", Can Boluk). Found 2026-10-09 while building item 1 |
| How often | Several releases a day: 18.8.7 on 2026-10-09, 18.8.5 and 18.8.6 the day before. Older versions stay available; 18.4.3 is still there |
| Licence | MIT. Shipping a copy is allowed with its licence and notices |
| The Oh-my-pi on this Mac | Byte for byte the published 18.4.3 Mac program: its checksum matches the release's |
| A copy run from another place | Same version, uses `~/.omp/agent`, and sees the owner's four sign-ins. So "own engine, existing folder" needs no setting at all |
| Whether a fresh Oh-my-pi looks for updates | Yes: `startup.checkUpdate` is on by default. It is off on this Mac only because the owner turned it off. Updating itself is a command, `omp update` |
| Settings handed over at start | `--config` with `{"startup":{"checkUpdate":false}}` is accepted, as the backup order already is |
| Two versions sharing one folder | 18.4.3 and 18.8.7 took turns on the same folder in both orders. Each opened a conversation, listed its models and commands and read the usage report every time. Not covered: a folder with real sign-ins, a sent message, and versions further apart |
| What differs between those versions | 66 commands against 69. Protocol version 1 and the usage report's shape are the same |
| Where skills live | Outside Oh-my-pi's folder (`~/.claude/skills`, `~/.agents/skills`). A separate folder still offered all 66 commands |
| Where MCP servers come from | Null reads `~/.omp/agent/mcp.json` itself and hands the servers over when a conversation opens (`harness.rs`) |

Two consequences shape the plan. First, Oh-my-pi changes several times a day, so a Null that uses
whatever is newest would be exposed to change constantly; choosing one version is not optional.
Second, the program and the folder are separate things: Null can own the program and leave the
folder where it is.

## What other people go through today, and after

| | Today | After this plan |
|---|---|---|
| Get the harness | Install Oh-my-pi in a terminal | Nothing. It is inside Null |
| Get Null | Install Apple's command line tools, Rust and the Tauri tool, clone the repo, run the install script | One `npm install`, with nothing to build. Or clone and run the install script, as now |
| First opening | The box waits. Without a sign-in a message just fails | The box says no provider is signed in and opens `/login` |
| Later | Any `omp update` changes what Null runs | Nothing changes until a new Null arrives |

The second row is the larger half of the friction and has little to do with Oh-my-pi. Item 6 is
where it is removed.

## Design rules

1. **The harness is a part of Null.** One version, chosen by Null, shipped with it, changed only
   by a new Null. Nobody has to know it is there.
2. **The folder is the user's.** Null's harness uses Oh-my-pi's ordinary folder. For the owner that
   is the existing one. For a newcomer it is made on first use, and it is the same folder a later
   terminal install would use.
3. **A technical person can take the part out.** One typed command points Null at their own
   Oh-my-pi. Null then says which version it is and that it was not checked with it.
4. **A feature is lost one at a time, never the whole box.** Null finds out what the harness in
   front of it can do and switches off only what cannot work, in a line of plain words.
5. **Keeping up happens in the repo.** A check that any Oh-my-pi version can be put through says
   whether Null still works with it. Null moves to a newer version only after it passes. A fix is a
   change to Null's code that the check has to pass. Nothing rewrites itself on a user's Mac.
   **Amended by the owner on 2026-10-09:** the user may move to the newest checked version
   themselves, by typing `/update` (item 10). Still nothing unchecked, and nothing by itself.

## Decisions for the owner

1. **How other people get Null: settled by the owner on 2026-10-09.** They clone the repo or run
   one `npm install`. No download from a web page, no store, and no Apple account. An earlier
   draft of this plan proposed an Apple Developer account; the owner rejected it.
   **The npm package is `@dominionism/null`** (owner, 2026-10-09). The plain name `null` is taken
   on npm by an unrelated package. Seen the same day: nothing is published under `@dominionism`
   and this Mac is not signed in to npm. Whether that scope already belongs to an account could
   not be seen without a sign-in; the owner needs an npm account or organisation of that name.
2. **The first version to carry: 18.4.3.** Everything Null does was checked against it.
   Recommended over 18.8.7; item 4 is what moves it forward safely. Built as recommended on
   2026-10-09.
   **Moved to 18.8.7 the same day, at the owner's word,** once item 4 existed:
   `Mini/Scripts/engine --to 18.8.7` fetched it, all 15 checks passed, and `Engine.toml` was
   written. The program is notarized and signed by the same author as 18.4.3. The built app is
   204 MB with the new checksum inside. It is the version the owner's terminal runs since the
   update, so "built in" and "yours" are the same version again. Installed on 2026-10-09,
   about 20:06.
3. **Apple Silicon first.** An Intel copy of the harness is another 226 MB. Recommended: Apple
   Silicon only until someone asks. Built as recommended on 2026-10-09; the owner has not yet
   said.
4. **The name of the command** that shows and chooses the harness. Proposed: `/harness`. Built
   under that name on 2026-10-09; the owner has not yet said.
5. **This reverses decision 3 of `Providers.md`** ("It does not download or run an installer").
   The owner asked for it on 2026-10-09; written as
   `Context/ADR/0003-NullCarriesItsOwnHarness.md`.
6. **How other people get a signing identity at install: settled by the owner on 2026-10-09.**
   The install makes one on each Mac, with one approval (item 6). Rejected: leaving Null
   unsigned, since macOS would then ask for everything again after every update. Still to
   work out: how to ask for that approval in words a person understands.
7. **What a newcomer's Null may do without asking: settled by the owner on 2026-10-09.** It
   keeps Oh-my-pi's own default, `yolo`: Null passes the harness's setting on and adds no
   approval layer of its own (`MiniApp.md`). The first opening says so in one plain line, and
   the README too (items 5 and 9). Rejected: starting newcomers on a stricter setting, which
   would mean more questions in a small box and a rule of Null's own.
8. **When Full Disk Access is asked for on a first run: settled by the owner on 2026-10-09.**
   After the first reply, not at the first start (item 5).
9. **What Null's release tags are called: settled by the owner on 2026-10-09.** `null-v…`,
   as in `null-v0.1.0`, until the Voice app's release workflow (`release.yml`, which answers
   to `v` tags) is removed with the cleanup (item 6).
10. **The first release: the owner gave the word on 2026-10-09, and it was published the same
    day.** Null 0.1.0, by the tag `null-v0.1.0` on `main` (item 6).

## Work items

1. **Choose one version and bring it into the build**
   - What: a file in git, `Mini/Engine.toml`, naming the version and the checksum of each program
     Null carries (`omp-darwin-arm64` first). `Mini/Scripts/engine` downloads that release's
     program from GitHub, checks it against the checksum in the file (not against the one it
     downloaded beside it), and puts it in `Mini/Engine/`, which git ignores. `tauri.conf.json`
     takes it into the app as a second program beside Null's own (`bundle.externalBin`), with the
     harness's `LICENSE` and notices as resources. `Mini/Scripts/install` runs the script first.
   - Why: design rule 1. A copy that comes with the build needs no installer at first run.
   - Depends on: none.
   - Risk: the local signing step signs only the outer app and macOS then refuses the inner
     program. Know it by: the installed app starting the built-in harness and answering.
   - Source: researched (the releases, checksums and licence above) + codebase
     (`Mini/Scripts/install`, `tauri.conf.json`).
   - Status: Complete (2026-10-09)
   - **Proven by the owner (2026-10-09):** two messages sent in the installed app were answered.
     The log: "starting the built-in harness: /Applications/Null.app/Contents/MacOS/omp",
     "harness ready: omp 18.4.3", and both replies ended `completed`. Afterwards the inner
     program still had the checksum of `Engine.toml` and the app still verified.
   - **Built:** `Mini/Engine.toml` (the version, and a checksum for the program, the licence and
     the notices); `Mini/Scripts/engine`; `Mini/Engine/` ignored by `Mini/.gitignore`;
     `bundle.externalBin` and `bundle.resources` in `tauri.conf.json`; `Mini/Scripts/install`
     runs the script first.
   - **Added to the plan's "What":** `Mini/build.rs` stops a build that has no program in
     `Mini/Engine/` and says to run `Mini/Scripts/engine`. Without it Tauri says only that a
     path does not exist. So `cargo build` and `cargo test` in `Mini/` need the script run once.
   - **Proven without a person:** the script fetched the three files in 9 seconds, and the
     program is byte for byte the published one and the one in `~/.omp/bin`. A second run
     fetches nothing. In a scratch copy: a wrong checksum is refused and the file not kept; a
     program changed on disk is replaced; a file with no checksum line stops the script. The
     built app is 217 MB and holds `Contents/MacOS/omp` with the checksum of `Engine.toml`, and
     the licence and notices under `Contents/Resources/Engine/`.
   - **The risk, as it turned out:** Tauri and the install script both leave the inner program
     as published, with its author's signature (see the table above); the install script signs
     the outer app only, and its seal then covers the inner program. On the installed app
     (2026-10-09): signed by "Null Local Signing", `codesign --verify --deep --strict` passes,
     the inner program has the checksum of `Engine.toml` and runs, and Full Disk Access is
     still on.

2. **Run the built-in harness, and let a technical person choose their own**
   - What: `harness::installed()` looks first beside Null's own program, then in `Mini/Engine/`
     for a development run. Only if neither is there does it look where it looks today, with a
     line in the log. A setting, `harness`, is empty for the built-in one or holds a path;
     `translate::find_installed` already takes a chosen path and is tested. `/harness` lists
     what there is: "built in, 18.4.3" and, when one is found on the Mac, "yours, 18.8.7" with its
     path. Enter chooses, and the running harness is let go as it is after `/backup`. Null always
     hands the harness a settings file of its own now (`harness.yml` in place of `backups.yml`):
     the update check off, and the backup order when there is one.
   - Why: design rules 1 to 3. The folder is not touched, so the owner keeps sign-ins,
     instructions and settings (established above).
   - Depends on: 1.
   - Risk: the built-in harness replacing itself. It looks for updates by default. Know it by:
     item 4 checks that the program's checksum is the same after a run.
   - Source: specified from user + codebase + verified locally.
   - Status: Complete (2026-10-09)
   - **Proven by the owner (2026-10-09):** `/harness` in the installed app listed "built in" and
     "yours". Choosing "yours" let the built-in harness go; the log then shows "the harness is
     now /Users/abdulwahid/.omp/bin/omp, version 18.4.3", "starting the user's own harness",
     the same conversation loaded back, and a reply that ended `completed`. `harness.yml` held
     the update check off and the owner's backup order, and `backups.yml` was gone.
   - **Built:** `Mini/src/engine.rs` decides which program runs: the owner's own when the
     setting `harness` holds a path that can be run, else the one beside Null's own program,
     else, with a line in the log, one found where it was looked for before. It reads the
     version Null carries from `Engine.toml` and asks each program for its own. The commands
     `harnesses` and `set_harness`, and `/harness` in the page: "built in" and "yours" with the
     version, the path of the owner's own, and "not checked with Null" for any version other
     than the carried one; choosing says so in one line. `harness.rs` writes `harness.yml`
     (the update check off, then the backup order) at every start and removes the old
     `backups.yml`. The log says "starting the built-in harness" or "starting the user's own
     harness".
   - **Deviation:** nothing looks in `Mini/Engine/` for a development run. The build already
     puts the program beside the debug app (`target/debug/omp`). Only the tests look there,
     because a test is a program in another folder.
   - **Deviation:** the live checks (`cargo test -- --ignored`) run the carried program and no
     longer the one in `~/.omp/bin`.
   - **Proven without a person:** 56 unit tests and 3 live checks. The app itself, under the
     probe profile against the stand-in provider of `Providers.md` item 2: the built-in harness
     started and answered; with the setting naming a downloaded 18.8.7, that one started and
     answered; back on the built-in 18.4.3, the conversation 18.8.7 had made was loaded back
     and answered. `harness.yml` held the update check off, alone and beside a backup order.
     The program's checksum was the same after a run. A message typed into the real page of
     the debug app was answered through the built-in harness. Nine states of `/harness` drawn
     in the app's web engine with stand-in data and read back.
   - **Proven by the owner, later the same day:** choosing "built in" again ("the harness is now
     /Applications/Null.app/Contents/MacOS/omp, version 18.4.3"). Then `omp update` in a
     terminal took the owner's own Oh-my-pi from 18.4.3 to 18.8.7. The program inside Null
     kept the checksum of `Engine.toml` and the app still verified. In Null the owner then
     went to "yours" (the log: "starting the user's own harness", "harness ready: omp 18.8.7",
     the conversation loaded back, a reply `completed`), back to "built in" ("starting the
     built-in harness", "harness ready: omp 18.4.3", the same conversation loaded back, a
     reply `completed`), and to "yours" again, where the setting was left.
   - **What that adds:** the two versions took turns on the owner's real folder, with real
     sign-ins and one conversation, in both directions. The harness check had only seen that
     in a throwaway folder.

3. **Find out what the harness can do, and lose one feature at a time**
   - What: at the first start of a harness Null notes, in the log and in memory: its version;
     whether a conversation has a model setting; whether another setting exists to ask the model
     with; whether the usage report can be read; whether the sign-in list can be read. Whether
     replies count tokens is learned from the first reply that does. Then: a failed reply is
     marked only once this harness has been seen to count tokens, so a harness that never counts
     them cannot turn every reply red; `/usage` and `/login` say in one line when this harness
     cannot do them; with the owner's own harness at a version other than the built-in one,
     `/harness` and one line after choosing say so.
   - Why: design rule 4. Today the rule for a failed reply rests on an unstable part of the
     protocol and would fail everywhere at once (`Providers.md`, item 7).
   - Depends on: 2.
   - Risk: a check that is itself wrong hides a feature that works. Know it by: unit tests on the
     noting, and item 4 running the same questions against two real versions.
   - Source: inferred from codebase (`translate::reply_failed`, `providers.rs`, `signin.rs`).
   - Status: In progress. Built, checked by script, merged and installed (2026-10-09); not yet
     checked by the owner's hand
   - **Built:** `translate::abilities`, and one line in the log at the first conversation of
     each harness version met in a run: "what this harness can do: a list of 19 models; says
     which model is in use when asked through thinking; loads a conversation back". Whatever
     is missing is said there with the feature it costs. `translate::judge_reply`: a reply
     without a token count is taken for a refusal only once this harness has been seen to
     count one. The version Null carries is trusted from the start, because the harness check
     asks exactly that of it. `/usage` on a harness whose report cannot be read says so in one
     line, where it used to list every provider as "no usage report".
   - **Already there:** `/model` on a harness with no model list says "this harness offers no
     model list"; `/login` on one that lists no providers says so; `/harness` and the line
     after choosing mark a version Null was not checked with (item 2).
   - **Deviation:** the usage report and the sign-in list are not tried at the first start.
     They are found out when `/usage` or `/login` is used, and a failure is not remembered.
     Trying at start would cost two more runs of the harness and a round to the providers
     every time, and a remembered "cannot" would hide a feature after one slow answer, which
     is the risk named above.
   - **Changed at the owner's word (2026-10-09, "we have keep things user-friendly and
     undertandable"):** which version was seen to count tokens is remembered between runs
     (the setting `counts_tokens`). A first cut forgot it at every restart, so on a version
     other than the carried one the first reply after each start, if it was a refusal, came
     in plain words and not in red.
   - **What is left of that:** the very first reply ever from a version Null has not met, when
     that reply is a refusal. Null cannot tell a refusal from a harness that never counts
     tokens until one reply has counted, and taking the reply for a refusal would paint every
     answer red on such a harness. On the built-in version it never happens.
   - **Added (same day):** while `/harness` is open the box says what the choice means: "built
     in stays until Null is updated. yours follows omp update". The owner had to ask what the
     difference was, which the box should have said.
   - **Proven without a person:** 59 unit tests, three of them new: the rule before and after
     tokens were seen; the noting, from a greeting and a conversation's settings; a harness
     that offers less. The app under the probe profile against the stand-in: on the built-in
     18.4.3 a bad sign-in as the first reply is marked ("reply ended: completed, with nothing
     from the model"); on the owner's 18.8.7 the same reply is not marked; and 18.8.7 after a
     working reply logs that it counts tokens and is remembered, so that after a fresh start a
     bad sign-in as the very first reply is marked. The 15 live checks still pass. `/harness`
     drawn again with its line of explanation, which goes when the list is put away.
   - **Not proven:** a harness that really lacks one of these, since none exists to try and
     the unit tests stand in for it; the `/usage` line on such a harness; the installed app,
     by the owner's hand.

4. **The harness check**
   - What: `Mini/tests/harness.rs`, run with `cargo test -- --ignored` against the built-in
     harness or any other (`NULL_MINI_ENGINE=<path>`), in a throwaway folder. It holds, in Rust, the
     stand-in provider that this month's work used from scratch scripts, and asks what Null
     depends on: protocol version 1 and a conversation that opens; a model setting; a working
     reply that counts tokens; a used-up limit, a bad sign-in and a refused model coming back as
     text with no tokens; a `default` backup list being followed, the message sent again, the
     failed model skipped; the model in use being told or answered when asked; the usage report's
     shape; the sign-in list and a dummy key being refused without being echoed;
     `config get retry.fallbackChains --json`; settings handed over at start being accepted; the
     program unchanged afterwards; two versions sharing a folder. `Mini/Scripts/engine --to
     <version>` downloads that version, runs the check, and writes `Mini/Engine.toml` only if
     everything passed.
   - Why: design rule 5. It is how a newer version is known to be safe before anyone gets it,
     and it puts back in the repo the checks that only existed as scratch.
   - Depends on: 1. It can be written beside 2 and 3.
   - Risk: the stand-in drifting from what real providers send. Know it by: the first real limit
     the owner meets, set beside what the stand-in sends (`Providers.md`, item 2).
   - Source: codebase (this month's spikes, recorded in `Providers.md`) + inferred.
   - Status: In progress. Built, and passed by 18.4.3 and 18.8.7 (2026-10-09). Not yet seen to
     stop a version that really breaks Null
   - **Built:** `Mini/src/check.rs`: the stand-in provider in Rust (three ways of talking; the
     model asked for picks the answer: `ok`, `limit`, `auth`, `noaccess`), a harness started as
     Null starts it and spoken to over the protocol, and twelve questions, one test each, named
     for what Null relies on. With the three live checks that were there before, that is 15
     tests, run by `cargo test -- --ignored` in about 18 seconds. `NULL_MINI_ENGINE=<path>`
     puts all 15 on another program. `Mini/Scripts/engine --to <version>` fetches that version,
     holds each file against the release's own `SHA256SUMS.txt`, runs the 15 on it, and only
     then writes `Mini/Engine.toml` and puts the program in place.
   - **The questions:** protocol version 1, a conversation that opens and can be loaded back,
     a model setting, and another setting to ask with; a working reply counts tokens (two ways
     of talking); a used-up limit, a bad sign-in and a refused model come back as replies with
     words and no tokens (three ways of talking, nine cases); a backup takes over, the message
     is sent again and the failed model is not tried for the next message; the model in use is
     answered when asked; Null's order goes ahead of a list the user already has, read with
     `config get retry.fallbackChains --json`; the setting that stops the update check is still
     a yes or no; started as Null starts it, the harness does not look for its own releases;
     the program has the same checksum after answering; the carried version and the one under
     test take turns on one conversation. The usage report and the sign-in with a dummy key are
     the two earlier live checks.
   - **Deviation:** the check is in `Mini/src/`, not `Mini/tests/`. Null is a program with no
     library, so a test under `tests/` cannot use Null's own code. In the crate, the harness's
     answers go through the very readers the app uses (`translate::reply_failed`,
     `models_from_config_options`, `other_option`, `event_from_update`, `backups::overlay`,
     `backups::lists_read`, `harness::own_settings`), so a pass means Null reads them, not that
     a copy of its rules does.
   - **Deviation:** the throwaway folder is a real temporary folder, given to the harness with
     `PI_CODING_AGENT_DIR`, not a profile under `~/.omp`. Seen on 2026-10-09: such a folder has
     no accounts and default settings, and the owner's folder is not touched. Each question
     makes its own and removes it; none was left behind.
   - **Deviation:** "settings handed over at start being accepted" could not be asked as
     written. The harness's own `config get` does not show a setting handed over for one run:
     it printed the stored value for the update check and for the backup lists alike, though
     the backup lists demonstrably take effect in a conversation. So the question became two:
     the setting still exists as a yes or no, and the harness does not go looking (below).
   - **Learned (2026-10-09):** in the mode Null uses, Oh-my-pi does not look for a newer
     version of itself at all. Kept behind a stand-in that wrote down every address it tried
     to reach, it asked for the same twelve with and without the setting, and none was GitHub
     or the npm registry. The update check belongs to the terminal's start. The setting is
     still handed over. This answers the open question whether the built-in harness ever
     replaces itself: it does not look, and its checksum is the same after a run.
   - **Also seen there:** at start the harness asks seven outside services for their model
     lists (api.commandcode.ai, api.kilo.ai, api.venice.ai, catalog.stencil.so,
     coding-intl.dashscope.aliyuncs.com, hyper.charm.land, zenmux.ai) and three ports on this
     Mac for local models (11434, 1234, 8080). That is the harness's doing, in the terminal as
     in Null; it is noted because "local" is easily read as "nothing leaves the Mac".
   - **Proven without a person:** 18.4.3 passed all 15, and so did 18.8.7, by
     `NULL_MINI_ENGINE` and again through `Mini/Scripts/engine --to 18.8.7`, which wrote the
     new version and checksum into `Engine.toml` and put the program in place. That was then
     undone: Null stays on 18.4.3 until the owner says otherwise (decision 2). A program that
     is no harness (`/bin/echo`) failed 14 of the 15 in half a second, each by its name; the
     one that passed is about the carried program itself. The script refuses a wrong word, a
     missing version and a version with no release, and says so when asked for the version
     already carried. The first form of the handed-over question failed on 18.4.3 and was
     reported by name, which is how its fault was found.
   - **Not covered:** a short rate limit, which keeps the harness silent for minutes; the
     usage report of a real account; a folder with real sign-ins; versions further apart than
     18.4.3 and 18.8.7; the stand-in set beside a real limit (the risk above still stands);
     `--to` on a version that fails, whose "nothing changes" branch has only been read.
   - **Added (2026-10-09):** `.github/workflows/null.yml` runs the unit tests and the harness
     check on GitHub, on a Mac, whenever `Mini/` changes. It passed on `main` at `89f960d`.
   - **Small things left:** `--to` takes a version number only; there is no `--to latest`. The
     two older live checks (the sign-in list and the usage report) still use the profile
     `null-probe` under `~/.omp`, not a throwaway folder, and the sign-in one still reaches
     DeepSeek with a dummy key.
   - **Added (2026-10-09):** the stand-in provider is now
     `Mini/src/standin.rs`, moved word for word out of the check, so that the app itself can
     use it. Two switches: `NULL_MINI_FOLDER` runs the app on a folder of its own (the
     harness's folder, its MCP servers, Null's own files and the log are all under it), and
     `NULL_MINI_STANDIN` offers the stand-in's models there. This replaces the scratch
     stand-in on port 18431 and the profile `null-probe`. The stand-in refuses to start
     without a folder and never overwrites a `models.yml` it did not write.
   - **Proven without a person:** 64 unit tests and the 15 live checks. The app on new
     folders: a message answered by `standin-anthropic/ok`; a second start loading the
     conversation back; a used-up limit marked ("reply ended: completed, with nothing from
     the model"); a message typed into the real page and "pong" read back from it; the
     stand-in without a folder refused; somebody's own `models.yml` left alone. The owner's
     settings, harness settings, `~/.omp/profiles` and harness config were the same
     afterwards, and a run added no line to the owner's log.

5. **First opening, for someone who has never used it**
   - What: takes over item 6 of `Providers.md`. When no provider reports and the model list has
     nothing but local models, the box opens by itself once, says "No provider is signed in" and
     opens `/login`. After a sign-in the box says which provider was added and which model it is
     on. Full Disk Access is asked for after the first reply, not at the first start as now
     (decision 8): the person sees Null work before being sent to System Settings, and until
     then macOS asks folder by folder. The first opening also says, in one plain line, that
     the agent may act without asking (decision 7).
   - Why: a newcomer's first minute. Today a message sent with nothing signed in only shows a
     provider's error.
   - Depends on: 2.
   - Risk: taking a Mac with only a local model for one with nothing signed in, or the reverse
     (`Providers.md`, item 1). Know it by: the scripted start under a throwaway folder, with and
     without a local model.
   - Source: specified from user + `Providers.md`.
   - Status: In progress. Built and proven by script (2026-10-09); not installed, and not yet
     tried by a person or on a fresh Mac account
   - **Seen (2026-10-09), on new folders of their own (`NULL_MINI_FOLDER`, item 4):**
     - This Mac runs Ollama, so a folder with nothing signed in still offers one model,
       `ollama/llama3.2:latest`. A message was answered by it after 33 seconds, with
       nonsense. So "nothing signed in" on this Mac is the "only a local model" case. The
       case with no model at all needs Ollama stopped, or the harness kept from this Mac's
       ports as `Folder::behind` does in the check.
     - The local model is not always in the first list. With the stand-in offered, a first
       start listed 12 models without it and a later start 13 with it.
     - Oh-my-pi's own default for approvals is `yolo`: a new folder, with the home folder
       elsewhere too, answers `tools.approvalMode` with `yolo`. Null passes the harness's
       setting on, so a newcomer's Null lets the agent do everything without asking
       (decision 7).
     - A conversation stays on the model it had when a provider appears, and a new
       conversation starts on it too. Seen with the stand-in added to a folder that had
       the Ollama model first. So a newcomer who signs in could go on talking to the small
       local model without knowing. Not seen with a real provider's sign-in.
   - **Built:** `Mini/src/welcome.rs`. At a start, once (the setting `welcomed`), Null reads
     the harness's usage report. With no account in it, the box opens by itself with the
     sign-in list and the line "no provider is signed in. choose one, or esc for later". With
     an account it does nothing and does not look again. With no report to read it looks
     again at the next start. After any sign-in the box says what was added and the model in
     use: "signed in to anthropic. now on …", or "still on … /model changes it" when the
     conversation was kept on the model it had; in the first opening the model list then
     opens on the new provider. When the first opening's sign-in is over, however it went,
     the box says "the agent may act without asking you first" if the harness's approval
     setting is `yolo`, and "no provider is signed in. /login signs in" if nothing was
     added. Full Disk Access is noted at start (`access::at_start`) and asked for after the
     first reply that ended well (`access::after_reply`). The README says all three.
   - **Deviation:** the model list is not looked at to decide. The harness gives no way to
     tell a local model from any other: `omp models --json` has no such field, and a key in
     the environment adds a provider's models but no account to the report. So the rule is
     the report alone, and no account means not signed in. A Mac with only a local model
     gets the first opening, as intended. Someone who gave the harness a key some other way
     sees it once too, and presses Esc.
   - **Proven without a person:** 68 unit tests, four of them new: what a report with no
     account, with an account, and no report at all mean; and when the user is sent to the
     Full Disk Access switch. The 15 live checks. The real app on new folders: a first start
     logs "no provider is signed in; opening the box to say so, this once" and then "the
     sign-in list was read: 82 providers", which only the page asks for; a second start on
     the same folder does neither; with the folder's settings saying `welcomed`, a message
     typed into the real page is answered as before. Nine states drawn in the app's web
     engine with stand-in answers and read back: the first opening; Esc; signed in while a
     local model was in use (the model list opens on the new provider); signed in with no
     model before; a sign-in that failed; a harness that asks before acting (no line about
     acting freely); the user already busy when it arrives (the lines only); and `/login`
     later, with and without a new provider.
   - **Not proven:** a real sign-in through the first opening; a report that holds an
     account (unit-tested only, until the owner's own Null starts on this build and logs "a
     provider is signed in, so there is no first opening"); the Full Disk Access question
     after a reply, since the test build reads it as on here and only the rule is tested; a
     fresh Mac user account; the installed app.

6. **One command to install** (decision 1)
   - What: two ways in, and the same Null at the end of both.
     - **Clone:** `Mini/Scripts/install`, as now, with item 1 added so that it also brings the
       harness. It builds Null on the Mac, so it needs Rust and Apple's free command line tools.
     - **npm:** a package that puts a ready-built Null on the Mac, so that nothing has to be
       built and nothing but Node is needed. The repo's own automation builds Null for each
       release; the package's install step fetches that build and the chosen Oh-my-pi, checks
       both against checksums the package carries, puts the app in place, makes it start at
       login and starts it. A small `null` command does the same again, and removes it.
     - First a spike, on a Mac user account that has never had Null: an app that arrives through
       npm opens with no warning from macOS. The warning is attached by web browsers to what
       they download; git and npm do not attach it. That is known in outline and unproven here.
     - The package's name is `@dominionism/null` (decision 1). The plain name `null` is taken.
   - Why: the larger half of the friction (the table above), in the two ways the owner named.
   - Depends on: 1, 2.
   - Risk: macOS refusing a ready-built app that npm put there. Know it by: the spike, before
     anything else in this item. The clone way does not have this risk: an app built on the Mac
     it runs on is that Mac's own.
   - Source: specified from user + inferred.
   - Status: In progress. The ready-built Null and most of the spike are done (2026-10-09). Not
     started: the npm package, the signing identity at install, and a first release
   - **Built:** `.github/workflows/null-release.yml`. On a Mac runner it fetches the carried
     Oh-my-pi, builds `Null.app`, seals it with no certificate (the Oh-my-pi inside keeps its
     author's signature), and packs it as `Null-<version>-macos-arm64.zip` with a
     `SHA256SUMS.txt`. A push that changes the workflow, or a run started by hand, keeps the
     app with the run for 14 days. A tag `null-v<version>` publishes it as a release, after
     checking that the tag names the version in `Cargo.toml` and `tauri.conf.json`.
   - **The spike, as far as this Mac goes (2026-10-09, macOS 26.5.2):** the Null that GitHub
     built was fetched with `gh`, which like npm attaches no quarantine mark. Its checksum
     matched, its seal held, and the Oh-my-pi inside had the checksum of `Engine.toml` and its
     author's Developer ID signature. Run from a scratch folder on a folder of its own, and
     never installed: it started and loaded its page; a message went through the Oh-my-pi it
     carries to the stand-in and came back; and opened the way a double-click opens an app
     (`open`), it started with no warning from macOS. `spctl` calls the app "rejected", which is
     what it says of any app without an Apple certificate, and that verdict is only acted on
     for a file that carries the quarantine mark.
   - **Released (2026-10-09, at the owner's word):** Null 0.1.0, by the tag `null-v0.1.0` on
     `main`. The workflow checked the tag against the files, built, and published
     `Null-0.1.0-macos-arm64.zip` (97 MB) with `SHA256SUMS.txt`. The Voice app's `release.yml`
     did not start. The published files were fetched with `gh` and tried as the first build
     was: the checksum matched, the seal held, the app started, answered through the
     Oh-my-pi it carries, and on a new folder showed the first opening. The release page says
     that the file is what an installer fetches and that a download through a browser is
     stopped by macOS.
   - **The install command (2026-10-09):** `Mini/Scripts/null`, a script that needs nothing
     but macOS. `null install` fetches the newest release of Null (or the one named), checks
     it, signs it if the Mac has Null's signing identity, puts it in `/Applications`, makes it
     start at login and starts it; `null uninstall` takes it away. It can be run from a clone
     or piped from the repository with `curl … | sh -s install`, and is what an npm package
     would run. It keeps nothing unless the file has the published checksum, the app's seal
     holds, the app is Null, and the Oh-my-pi inside carries a Developer ID signature. Two
     switches for trying it: `NULL_TRY_IN` puts the app in a folder and leaves the rest of the
     Mac alone, and `NULL_RELEASES` fetches from a folder in place of GitHub.
   - **Proven without a person, in a trial folder:** the newest release fetched from GitHub,
     placed, started from there, and removed again; the same when the script is piped into
     `sh`. From a folder of made-up releases, each refused with its own line and nothing
     placed: a checksum that is not the file's, an app changed after it was sealed, an app
     that is not Null, an Oh-my-pi that is not its author's, a version that does not exist,
     and a word that is not a version. Afterwards the installed Null, its login item, what
     was running and the keychain list were as before.
   - **Not proven:** the script's real path, which signs, replaces the app in `/Applications`
     and writes the login item (those lines are the ones `Mini/Scripts/install` has used); npm
     itself as the carrier; a Mac user account that has never had Null; a signing identity
     made at install (decision 6).
   - **Not settled:** what vouches for the published checksum. It is fetched from the same
     release as the app, so it proves the transfer and not who published. An npm package
     would carry the checksum itself; the script trusts GitHub and the repository, as a clone
     does.
   - **Found:** the repository already has `.github/workflows/release.yml`, which came with
     the Voice app. It answers to any tag that starts with `v`, and would try to build that
     app for three systems with certificates this repository does not have. So Null's tags are
     `null-v…`. The owner's call whether that workflow stays, and whether Null's tags should
     be plain `v…` once it is gone.
   - **Gap found (2026-10-09), settled by the owner the same day (decision 6):**
     `Mini/Scripts/install` signs the app only when a signing keychain is already on the Mac,
     and nothing makes one for someone else. Their Null would be installed unsigned, and macOS
     ties Full Disk Access and its folder answers to that exact build, so every update would
     ask again. So the install makes a local signing identity on each Mac, with one approval.
     Still to work out: how to do that without a prompt the person will not understand. To
     try on the same fresh Mac account as the spike.

7. **Getting the next Null**
   - What: no mechanism of Null's own. The next version comes the way the first did: `npm update`,
     or `git pull` and the install script. That is also how the harness moves forward. To design
     once item 6 exists: the box saying, once, that a newer Null is there.
   - **Amended (owner, 2026-10-09):** the harness alone can also move forward from the box, with
     `/update` (item 10). A new Null still comes the way the first did.
   - **Gap found (2026-10-09):** Null is version 0.1.0 everywhere and has no releases. Until
     its version moves, the box cannot say that a newer Null is there, and the `null` line in
     `Engine.toml` guards nothing (item 10, Limits). Not designed. The npm way of item 6
     already needs a build for each release.
   - **Since 2026-10-09:** a release can be built (item 6): a tag `null-v<version>` publishes
     the version the files name. The first is `null-v0.1.0`. Nothing moves the number yet,
     and nothing in the box looks for a newer Null.
   - Depends on: 6.
   - Status: Not started

8. **Keeping up without being asked**
   - What: not designed. Once a week a job takes the newest Oh-my-pi release, runs item 4, and
     opens a pull request moving `Mini/Engine.toml` forward when it passes. When it fails, it
     says which question failed, and an agent may draft the fix as a pull request that the same
     check has to pass. A person merges.
   - Why: design rule 5, and the owner's question whether Null can keep fixing itself.
   - Depends on: 4.
   - Risk: several releases a day would drown the repo. Hence once a week.
   - Source: inferred.
   - Status: Not started

9. **Say it**
   - What: the README's first step, from "Bring your harness" to what is true after item 2. An
     ADR that Null carries its own harness, recording that it reverses decision 3 of
     `Providers.md` and why. Glossary entries for harness, provider, backup and sign-in
     (`/grill`). `Providers.md` already points here from its decision 3 and its item 6.
   - Depends on: 2 for the README.
   - Status: In progress. The README is done (2026-10-09, pull request #11) and the decision
     record is written (`Context/ADR/0003-NullCarriesItsOwnHarness.md`). The glossary is not
     written; "engine" or "harness" is the first word to settle (`/grill`)

10. **`/update`: the newest checked harness, from the box**
    - **Decided (owner, 2026-10-09):** "Let's provide add a command that helps update the latest
      version of omp if the user chooses to do so. ... Like /update". Asked what it should
      install, the owner chose the newest version the repo's check has passed, not the very
      latest; and to build it after item 3.
    - What: a typed command. It reads `Mini/Engine.toml` on the repo's `main`, which anyone can
      read without signing in (seen 2026-10-09). If that names a newer version than the one
      Null runs as "built in", it fetches that version's program from Oh-my-pi's release, holds
      it against the checksum in that file, and keeps it in Null's support folder. It does not
      go inside the app, whose seal covers the program it came with. "Built in" then means the
      newest checked program Null has, and the one that came with the app stays as the
      fallback. One line says what happened: which version it is on now, or that it is already
      on the newest checked one. When the newer harness needs a newer Null, `/update` says so,
      says how to get it, and changes nothing.
    - Why: the owner's words above. People who never open the repo get the newest checked
      harness with one command, and only when they choose to.
    - Depends on: 3 (so that a harness that differs a little costs one feature, not the box)
      and 4. Most useful with 8, which is what keeps the checked version moving.
    - Risk: macOS refusing a program the app fetched itself; a version the repo checked against
      a newer Null than the user has. Know it by: a scripted run under a throwaway profile
      (fetch, checksum, start, answer), and the "needs a newer Null" line seen with a made-up
      file.
    - Open: how `Engine.toml` says which Null a version was checked with; what `/harness` calls
      a fetched program; whether `/update` also says that a newer Null is there (item 7).
    - Source: specified from user + inferred.
    - Status: In progress. Built, proven by script in the real app, merged and installed
      (2026-10-09); not yet used by the owner
    - **Built:** `/update` in the page and `update_harness` in `engine.rs`. It reads the
      repository's `Engine.toml` with the Mac's own `curl`, and moves only when that names a
      version newer than the built-in one. The fetched program is kept in
      `Engine/<version>/omp` in Null's support folder, only if it has the checksum that file
      gave, its signature holds, it is signed by whoever signed the program Null came with,
      and it says it is that version. One fetched version is kept. `engine::tidy` removes it at
      start once the app itself carries that version or a newer one. The box says what
      happened in one line, and while fetching says "fetching Oh-my-pi X, about 200 MB".
    - **The open points, as built:** `Engine.toml` has a line `null`, the oldest Null a version
      was checked with; `Mini/Scripts/engine --to` writes it from Null's own version when the
      check passes, and `/update` holds it against the running Null. A fetched program is still
      called "built in" in `/harness`, and counts as checked. `/update` says a newer Null is
      needed only when the version it wants needs one.
    - **Added beyond the item:** the same signer check in `Mini/Scripts/engine --to`, so that
      a version is never run by the check, or written into `Engine.toml`, unless it is signed
      by whoever signed the one carried now. A switch, `NULL_MINI_UPDATES`, names a made-up
      file in place of the repository's for scripted checks.
    - **Proven without a person:** 63 unit tests, among them what `/update` decides from a
      file: fetch, stay, needs a newer Null, and nothing without a checksum. In the real app
      under the probe profile, carrying 18.4.3, with made-up files: the version already
      carried ("already on the newest checked Oh-my-pi"); a version that needs Null 9.0.0
      (said, nothing changed); a wrong checksum (200 MB fetched, refused, nothing kept); the
      real 18.8.7 (fetched, kept, no quarantine mark, macOS rates it "Notarized Developer
      ID"). A message then started "the built-in harness" from the support folder as 18.8.7
      and was answered. A build that carries 18.8.7 removed the fetched copy at start. The six
      lines the box can say, drawn and read back. `--to 18.8.7` with the signer check wrote
      exactly the `Engine.toml` that was there before.
    - **Limits:** Null's own version is still 0.1.0 and nothing moves it yet, so the `null`
      line guards nothing until Null has releases (items 6 and 7). `/update` reads `main`, so
      it offers a version as soon as a change to `Engine.toml` is merged there. There is no way
      to stop a fetch once it has begun, short of quitting.
    - **Not proven:** the installed app, by the owner's hand; a real newer version on `main`,
      since none exists yet; the line said when the user is on "yours".

## Verification

- Unit tests (`cargo test`): reading `Mini/Engine.toml`; the order in which a harness is looked
  for; the noting of what a harness can do; a failed reply not being marked before tokens were
  ever counted.
- The harness check (item 4), against the built-in version and against the newest release.
- Scripted starts of the app under a throwaway folder: the built-in harness is the one started;
  a chosen path is the one started; the first opening with nothing signed in.
- By hand, the owner: after installing, a message is answered and the log names the built-in
  harness; skills, MCP servers and all four sign-ins are there; `omp update` in the terminal
  changes nothing in Null; `/harness` switches to the terminal's Oh-my-pi and back.
- By hand, the owner, owed on the build installed 2026-10-09: one message answered; `/harness`
  showing both rows as 18.8.7 with no "not checked" label, and the line "built in stays until
  Null is updated. yours follows omp update" under the list; `/update` saying "already on the
  newest checked Oh-my-pi, 18.8.7".
- By hand, the owner, once the first opening is installed: the box does not open by itself,
  and the log says "a provider is signed in, so there is no first opening". On a Mac user
  account that has never had Null: the box opens by itself with the sign-in list, a sign-in
  ends with the provider and the model said, the line about acting without asking is there,
  and System Settings opens only after the first reply.

## Validation

- No ADR is contradicted. One recorded decision is reversed on purpose, at the owner's word:
  decision 3 of `Providers.md`. ADR 0003 records it.
- The rules of `Providers.md` hold: no credential rests in Null, since the folder is the
  harness's; Null still knows nothing about any one provider.
- The order holds. Items 1 to 5 need nothing from the owner but decisions 2 to 4.
- Still fuzzy, on purpose: the npm way of item 6 until its spike, and items 7 and 8, which are
  not designed.
- Known limits of the evidence: the two-version test did not use a folder with real sign-ins and
  the two versions were four small steps apart. Since 2026-10-09 the first half is answered for
  18.4.3 and 18.8.7: they took turns on the owner's real folder (item 2). If versions far apart cannot share a folder, the
  owner's fallback is a folder of Null's own: one more sign-in, with skills and MCP servers still
  there.
- What this plan costs: Null grows from 12 MB to 217 MB (measured 2026-10-09), and Null takes
  on moving the harness forward. Item 4 is what makes that safe and item 8 is what makes it cheap.

## Out of scope

- A second harness (`Providers.md`, item 11).
- Windows, Linux and Intel Macs.
- Changing where Oh-my-pi keeps sign-ins, skills or MCP servers.
