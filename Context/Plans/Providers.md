# Any provider: sign-in, limits and switching in Null

> Blueprint: 2026-10-08. Builds on `Context/Plans/NullMini.md` (the constraints, the provider-policy
> research and Phase 2, items 10 to 14, which were written for a mini inside the Voice desktop) and
> on `Context/Plans/MiniApp.md` (the app as built). This plan replaces those Phase 2 items.
>
> Status: **`/login` is built and installed (items 1, 3 and 5); the first real sign-in, by the owner,
> is its remaining proof. Not started: the limit state, `/usage`, first run, and the rest.**

## Goal

Let anyone use Null with the AI provider of their choice: sign in from inside the box, see which
providers are available and how much of each limit is left, and carry on with another provider when
one runs out, with Oh-my-pi as the harness and without Null ever keeping a credential.

## Constraints

- **Verbatim (user):** "when my usage limit is used up … my mini pet is not usable at all."
- **Verbatim (user):** "I don't rely only on ChatGPT, or a singular AI provider. I can switch whenever
  I want to manage usage."
- **Verbatim (user):** "This is going to be a personal tool, but I plan on open-sourcing this so that
  other people can use Null via BYOK (Bring Your Own Key), or authentication with an AI provider
  (Claude, Codex, or OpenCode)."
- **Verbatim (user, 2026-10-08):** "Not everyone will use the same AI provider. So, Null should
  accommodate for that. OMP is just the harness that Null should have (the environment in which the
  specified agent will have access to and use as tools)."
- **Verbatim (user, 2026-10-08):** "Usually, I would do /login to autheticate with an AI provider."
- **Verbatim (user, 2026-10-08):** "some setups may need additional time like OpenCode (which
  required me to get an API key even after logging in -- how can we have null figure out anything
  else someone needs to authenticate)?"
- **Verbatim (user, 2026-10-08):** "I just want to make sure that our software architecture, and
  decisions are the most optimal, and intelligent, and effective."
- **Verbatim (user, on the look):** "All I really need to see is the text box and the arrow." Words
  appear only for something the user has to know.
- Everything the box does is a typed command (`/model`, `/new`, `/quit`); there is no settings page
  (`MiniApp.md`, Decisions made).
- **Null never reads, stores or forwards a provider token** (`NullMini.md`). This plan asks the owner
  to restate it for keys: see decision 1.
- **Provider policy** (researched 2026-10-08, `NullMini.md`): Anthropic allows API keys in
  third-party products but not claude.ai sign-in or subscription limits without its approval. OpenAI
  allows Codex sign-in for local and open-source apps, not commercial or hosted ones.
- Null is its own app and does not depend on the Voice desktop or its server (ADR 0002).
- A login-launched app gets a minimal `PATH`; every harness command is run by absolute path.

## What was established on this Mac (2026-10-08)

Read from the installed tools; nothing was signed in or out.

| Question | Finding |
|---|---|
| How Oh-my-pi signs in | `omp login [PROVIDER]`, "terminal counterpart of /login". The argument is an OAuth provider id (`anthropic`, `openai-codex`); with none it picks interactively |
| Sign-in over the protocol Null speaks | Not offered. `omp acp` advertises one method, "Use existing local credentials", and its 66 slash commands include `model` and `usage` but no `login` |
| What the protocol itself provides | Two kinds of method: the agent signs in by itself, or the client "runs the configured agent program as a separate interactive process" (a terminal sign-in). So running the harness's own sign-in program is the standard route |
| Usage limits | `omp usage --json` reports every signed-in account: each limit's window, share used, reset time and status, and whether the limit is reached. Today: openai-codex 5 hours 0% and 7 days 2%; opencode-go 5 hours 0%, weekly 27%, monthly 23% |
| Which providers are signed in | `omp models --json` lists 53 models from `ollama`, `openai-codex` and `opencode-go`. The same grouping already reaches the box through the session's model list |
| Keys | `omp --api-key` and environment variables such as `ANTHROPIC_API_KEY`; a credential vault (`omp auth-broker`, with `login`, `logout`, `import`, `list`). How a key is saved for good is not yet known (item 1) |
| Never to be used | `omp token` prints a provider's key or token |
| A safe place to experiment | `omp --profile <name>` gives "an isolated profile for auth, sessions, settings, and caches" |
| Other harnesses | OpenCode 1.18.33 speaks the protocol (`opencode acp`) and advertises "Run `opencode auth login` in the terminal". Claude Code 2.1.293 and Codex 0.147.0 do not speak it; each has its own `login` |

One consequence shapes the plan. Oh-my-pi can itself sign in to Anthropic, to ChatGPT (as
`openai-codex`) and to opencode-go, so the providers the owner named are reachable through the one
harness. Accommodating any provider does not need a second harness.

## Design rules

These follow from the owner's question about sign-ins that take extra steps, and they decide most of
what is below.

1. **Null knows nothing about any one provider.** The harness knows how each provider signs in and
   keeps that current. Null shows the harness's own sign-in, whatever it asks for: a browser page, a
   code, a key fetched from a website afterwards. Null holds no list of providers and no steps of
   its own, so a provider that changes its sign-in, or a new one, needs no change to Null.
2. **Null checks results, not steps.** When a sign-in ends, Null looks for that provider's models and
   its usage report. If they are not there it says so plainly, and never reports success on the
   strength of the steps having run.
3. **No credential rests in Null.** Not in its settings, not in its log.

## Decisions (settled 2026-10-08)

The owner: "let's go with the most optimal direction. If you're directions are the most optimal,
then let's go with that." Each recommendation was looked at again against the design rules.

1. **Keys.** The rule is restated: Null never stores a credential, never logs it, and passes what is
   typed only to the harness's own sign-in on this Mac. Under design rule 1 a key is simply one of
   the things a sign-in may ask for, so it needs no mechanism of its own. To be written as an ADR
   (item 10).
2. **Sign-ins a provider restricts.** Null shows exactly what the harness offers and keeps no list of
   its own, by design rule 1. Oh-my-pi offers Anthropic's subscription sign-in, and Anthropic's
   terms do not allow a third-party product to offer it, so the README says that sign-ins belong to
   the harness and each provider's terms apply (item 10). Someone who uses a Claude subscription
   this way relies on Oh-my-pi's standing with Anthropic.
3. **A Mac without Oh-my-pi.** Null says how to install it, in one line with the command. It does
   not download or run an installer.

## Work items

1. **Spike: Oh-my-pi's sign-in, run by another program**
   - What: under a probe profile (`omp --profile null-probe …`), so the owner's real sign-ins are
     not touched:
     - confirm the profile is isolated: no accounts in `usage --json`, and what `models --json` and a
       new session's model list hold when nothing is signed in;
     - run `omp login openai-codex` with plain pipes, then under a pseudo-terminal. Record what it
       prints, whether it opens the browser by itself, whether it waits for something typed, how it
       ends (exit code, last line), and how it is cancelled;
     - run `omp login` with no provider: is the picker a full-screen terminal screen;
     - walk through `opencode-go`, the owner's example of a sign-in with an extra step (a key
       fetched after logging in), to see every step the harness asks for;
     - try Oh-my-pi's other mode, `--mode rpc-ui`, in which it asks the host app to show its
       dialogs: does sign-in run there, and what does the host have to draw;
     - find how a key-only provider is added and kept: a prompt in `omp login`, an environment
       variable only, `omp config`, or `omp auth-broker import`;
     - find a machine-readable list of the providers that can be signed in to;
     - send one prompt with nothing signed in and record the error Null receives.
   - Why: decides how `/login` is built (item 5) and what a first run looks like (item 6).
   - Depends on: none. The owner does the browser part.
   - Decision rule: take the first of these that carries every sign-in from start to finish,
     extra steps included. (1) The harness asks Null to show its dialogs (`rpc-ui`), and Null draws
     them in its own style. (2) The harness's sign-in program runs in a terminal view inside the
     box. (3) It opens in Terminal. All three keep design rule 1; they differ only in how the
     harness's questions reach the screen.
   - Risk: `omp login` refuses to run without a terminal, or the profile shares something with the
     real one. Know it by: the first check above, before any sign-in is attempted.
   - Source: verified locally (the commands and flags exist) + inferred.
   - Status: Complete (2026-10-08), except what a finished sign-in prints, which the first real
     one will show
   - **Decision: none of the three routes is needed.** Oh-my-pi's sign-in is a plain exchange of
     lines that runs without a terminal, so Null runs `omp login` on ordinary pipes and shows each
     line in the box in its own style. `rpc-ui` was not examined; the rule did not require it.
   - **How the sign-in behaves** (probe profile, nothing completed):

     | Step | What Oh-my-pi does |
     |---|---|
     | `omp login`, no provider | Prints "Select a provider:" and a numbered list of 78, then reads a number |
     | A key provider (`deepseek`, `opencode-go`) | "Open this URL in your browser:", the address, one line saying what to copy, then "Paste your … API key (sk-...): " with no line end |
     | The owner's example, `opencode-go` | "Log in to the OpenCode Zen console and copy your OpenCode Go API key": the extra step is spelled out by the harness |
     | A key that is wrong | "Validating API key..." then "Login failed: … validation failed (401): …", exit 1. The key is not echoed |
     | A browser sign-in (`openai-codex`, `anthropic`) | The address, "Local shortcut (this machine only): http://localhost:…/launch", "A browser window should open", "Waiting for browser authentication...", then "Paste the authorization code (or full redirect URL): " |
     | Input closed, or nothing in time | "Login failed: Login cancelled", exit 1 |
     | An unknown provider id | "Login failed: Unknown OAuth provider '…'. Run `omp login` to pick one." |

   - **Two things to build around:** a question ends with ": " and no line end, so output must be
     read as it arrives, not line by line; and an answer written before the question is asked is
     lost, so Null waits for the question.
   - **Established (2026-10-08):** `omp --profile null-probe` keeps its files in
     `~/.omp/profiles/null-probe/agent` and sees none of the real sign-ins: its usage report has no
     accounts, while the real profile still reports openai-codex and opencode-go. With nothing
     signed in it still offers one model, the local `ollama` one. So "nothing signed in" is not the
     same as "no models" on a Mac that runs a local model; item 6 has to tell the two apart.

2. **Spike: what a used-up limit and a missing sign-in look like**
   - What: under the probe profile, point one provider at a local stand-in that answers 429 and then
     401, through a config overlay (`omp --config`) or the provider's base-URL variable. Send one
     prompt over the protocol each time and record the error Null receives and what
     `omp usage --json` says at that moment.
   - Why: item 7 needs a rule that tells "limit reached" from any other failure. No limit has ever
     been seen in this app.
   - Depends on: 1 (the profile).
   - Risk: Oh-my-pi retries or moves to another account by itself, so the error never surfaces. Know
     it by: the prompt hanging or succeeding. Fallback: take the shapes from a real limit when one
     happens, and until then treat only the usage report's "limit reached" as the signal.
   - Source: inferred.
   - Status: Not started

3. **Run the harness under a chosen profile**
   - What: a development switch, `NULL_MINI_PROFILE=<name>`, that makes the app start
     `omp --profile <name>` for the protocol connection and for every command below.
   - Why: first-run and limit behaviour can then be checked by script without touching real
     sign-ins.
   - Depends on: 1 (that the flag works before `acp`).
   - Risk: low.
   - Source: verified locally (`--profile`).
   - Status: Complete (2026-10-08). `harness::extra_args()` adds it to every run of the harness.

4. **Provider state in the app**
   - What: `Mini/src/providers.rs`. Run `omp usage --json` off the main thread with a timeout; never
     `omp token`. Read it into: provider, each limit's label, share used, reset time and status, and
     whether the limit is reached. Join it with the providers in the session's model list. A
     command `providers`, and an event `provider_state` after a sign-in, after a failed reply, and
     when `/model` or `/usage` opens. Keep a result for 60 seconds: the call takes about a second
     and asks the providers.
   - Why: sign-in, first run, the limit state and `/usage` all need to know who is signed in and
     what is left.
   - Depends on: none (3 for its scripted checks).
   - Risk: the report's shape changes between Oh-my-pi versions, and some providers have no report
     (ollama, key-only ones). Know it by: unit tests on today's two captured reports and a
     hand-made "limit reached" one; anything unreadable becomes "unknown", never a failure.
   - Source: verified locally (shape captured 2026-10-08).
   - Status: Not started

5. **`/login` in the box**
   - What: a fourth command of the box, blue like the others. It starts the harness's own sign-in
     and shows what the harness asks, step by step, by the route item 1 chose: which provider, a
     browser page to finish, a code or a key to paste. Esc cancels at any step. When the sign-in
     ends Null checks the result (design rule 2): if the provider's models are there it refreshes
     the model list and says which provider was added; if not it says what is missing. `/logout`
     follows if the harness has a way to do it.
   - **Found on the way (2026-10-08):** the owner typed `/login` before it existed. The box passed it
     to the agent as a message, and the agent, taking it for one, began reading files to work out
     what was meant (reproduced). Fixed the same day: a lone slash-word that is not one of the
     box's commands is handed back with "no such command" and never sent. A sentence that merely
     starts with a slash still goes through.
   - **Left open by that fix:** Oh-my-pi lists 66 slash commands of its own, the owner's skills
     among them. A lone one is now refused by the box. Whether to let the listed ones through, and
     whether they work when sent this way, is untested and undecided.
   - Why: "Usually, I would do /login". Nobody should need a terminal to add a provider.
   - Depends on: 1, 4.
   - Risk: a sign-in left waiting for ever. Know it by: a time limit and the cancel path, both in
     the scripted checks.
   - Source: specified from user + item 1.
   - Status: In progress — built and installed; no real sign-in has been done through it yet
   - **Built (2026-10-08):** `Mini/src/signin.rs` runs `omp login` on ordinary pipes: it reads the
     harness's list, answers the harness's "Enter number" itself with the entry the user picked,
     then passes every line and question to the page (`mini:signin`) and the user's answers back.
     In the page, `/login` shows the list in the model list's style; the steps follow as lines,
     with web addresses shortened and clickable; the field hides what is typed; Esc or Ctrl+C
     cancels. The box stays up while a sign-in runs, as it does for Full Disk Access. When the
     harness reports success the app lets the running harness process go, so the next one sees
     the new provider, and the page compares the providers under `/model` before and after
     (design rule 2).
   - **Proven without a person:** unit tests for reading the list and the output as it arrives; a
     live check (`cargo test -- --ignored`) in which this code ran Oh-my-pi's sign-in under the
     probe profile, chose DeepSeek, gave a dummy key and was refused with "Login failed", the key
     appearing nowhere in what came back; and every stage of the page drawn in the app's web
     engine with stand-in data.
   - **Not yet proven:** a sign-in that succeeds, and so what the harness prints then, whether
     the fresh harness process shows the new provider, and whether the old process is really gone.
   - **Not built:** `/logout`; a time limit on a sign-in left waiting (Esc cancels it).

6. **First run**
   - What: when the harness is not installed, one line saying so and how to get it (decision 3).
     When nothing is signed in, one line ("No provider signed in. Type /login"), and the first time
     `/login` opens by itself. Asked once, in the way Full Disk Access is.
   - Why: other people start with nothing signed in; today they would see only a provider's error.
   - Depends on: 1, 5.
   - Risk: low.
   - Source: inferred + `MiniApp.md` (the Full Disk Access pattern).
   - Status: Not started

7. **Limit reached: carry on with another provider**
   - What: when a reply fails by item 2's rule, or the provider's report says its limit is reached,
     the box says one line ("openai-codex limit reached, resets 3:40 pm") and opens the model list
     with used-up providers dimmed and the others first. Choosing a model moves the conversation,
     which already works, and sends the failed message again; Esc leaves things as they are. One
     new event between app and page, `limit_reached`, carrying the provider and the reset time.
   - Why: the problem the project exists to solve.
   - Depends on: 2, 4.
   - Risk: an ordinary failure read as a limit. Know it by: the rule tested against item 2's
     recorded shapes; anything that does not match stays a plain error.
   - Source: specified from user + `NullMini.md` item 13.
   - Status: Not started

8. **`/usage`, and what is left in the model list**
   - What: `/usage` lists each signed-in provider with what is left and when it resets. The model
     list shows the same beside each provider's heading and dims providers with nothing left.
   - Why: "I can switch whenever I want to manage usage": before the wall, not only at it.
   - Depends on: 4.
   - Risk: more words in a box meant to stay bare. Know it by: the owner's eye; the figures appear
     only in these two lists, never in the idle box.
   - Source: specified from user + verified locally (the report).
   - Status: Not started

9. **Keys**
   - What: a key is one of the things a sign-in may ask for, so item 5 already carries it. What is
     left here: a field that does not show what is typed when the harness asks for a secret; any
     provider item 1 finds that takes a key only through an environment variable or a flag, with
     no sign-in to run; and the proof that nothing typed during a sign-in reaches Null's log or
     settings.
   - Why: "other people can use Null via BYOK".
   - Depends on: 1, 5.
   - Risk: a key reaching the log or the settings file. Know it by: a test that searches both after
     a sign-in with a marked dummy key.
   - Source: specified from user + decision 1.
   - Status: Not started

10. **Say it for other people**
    - What: a README section on what Null needs (Oh-my-pi), how sign-in works, that sign-ins live
      in the harness, and that each provider's terms apply (decision 2). An ADR for the credential
      rule as decision 1 restates it, one for design rule 1, and glossary entries for provider,
      harness, sign-in and limit (`/grill`).
    - Why: the open-source release; the rule is today only a line in two plans.
    - Depends on: none for the ADRs and the glossary; item 5 for the README.
    - Risk: low.
    - Source: inferred.
    - Status: Not started

11. **Other harnesses**
    - What: nothing now. If someone needs a harness other than Oh-my-pi: OpenCode is one more row
      (`opencode acp`, sign-in by `opencode auth login`); Claude Code and Codex need an adapter that
      speaks the protocol, or their own interfaces, and would be planned then.
    - Why: recorded so the earlier Phase 2 (items 10 to 14 of `NullMini.md`) is not picked up as
      written. Those items planned three adapters in the Voice server to reach three providers;
      Oh-my-pi reaches them already.
    - Depends on: a need.
    - Source: verified locally (the three tools) + `NullMini.md`.
    - Status: Not started

## Verification

- Unit tests: reading the usage report (item 4), the limit rule against recorded errors (item 7).
- Scripted, under the probe profile (item 3): a first run with nothing signed in; a reply that hits
  the stand-in's 429; `/login` cancelled; nothing of a dummy key left in the log or settings.
- By hand, the owner: `/login` to a provider not yet signed in (Anthropic is the obvious one), then
  `/model` to it; `/usage`; a switch after a limit, real or stand-in.

## Validation

- No ADR is contradicted. The credential rule and design rule 1 are not yet ADRs; item 10 makes
  them so.
- The order holds. Items 1 and 2 are spikes and decide how 5, 7 and 9 are built; item 4 depends on
  nothing and can be built beside them.
- Still fuzzy, on purpose: by which of three routes the harness's questions reach the screen
  (item 1 decides), and the limit rule (item 2 decides). None is specified past what is known.
- One dependency carries the whole plan: Oh-my-pi. Its sign-in, its usage report and its model list
  are read as they are today, and any of them can change with a new version. The report is read
  tolerantly (item 4), and a second harness stays one table row away (item 11).
- Pattern check: typed commands only, one line of words when needed, everything by absolute path,
  and no credential in Null's settings or log.
- No outside research was needed. The provider terms were researched earlier the same day
  (`NullMini.md`), and everything else was read from the tools installed here.

## Out of scope

- Voice, the pet and background tasks.
- An Apple Developer ID and anything else the release needs beyond providers.
- Windows and Linux.
