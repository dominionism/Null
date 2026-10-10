# Any provider: sign-in, limits and switching in Null

> Blueprint: 2026-10-08. Builds on `Context/Plans/NullMini.md` (the constraints, the provider-policy
> research and Phase 2, items 10 to 14, which were written for a mini inside the Voice desktop) and
> on `Context/Plans/MiniApp.md` (the app as built). This plan replaces those Phase 2 items.
>
> Status: **`/login` is built, installed and proven by the owner's sign-in to Anthropic (items 1, 3
> and 5). The limit spike is done (item 2): a failure reaches Null as reply text, never as an error,
> and the harness can move to another model by itself, which is now the design (decision 4). That
> is built and installed (item 7): `/backup` sets the order, a switch and a failed reply are said
> in the box, and the owner's order is Claude Opus 5.5, then DeepSeek V4.1 Flash. `/usage` is
> built and installed too (item 8). No real limit has been seen yet. The first run moved to
> `OwnHarness.md`, item 5, and is built there. Item 9, keys, is proven by a test (2026-10-09), and
> item 10 is written down (2026-10-10). What is left is the owner's hands: `/backup`, `/usage` and
> a real limit.**

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
   **Reversed by the owner on 2026-10-09:** Null carries its own Oh-my-pi, at one version it
   chose. See `Context/Plans/OwnHarness.md`.
4. **Who switches when a provider runs out.** Oh-my-pi does (item 7). Its own fallback moves to
   the next model and sends the message again; Null supplies the order and says that it happened.
   Settled after item 2 showed the harness doing this over the protocol.

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
   - What: under the probe profile, point one provider at a local stand-in that answers 429, then
     401, then a refusal of the model (item 12), through a config overlay (`omp --config`) or the
     provider's base-URL variable. Send one
     prompt over the protocol each time and record the error Null receives and what
     `omp usage --json` says at that moment.
   - Why: item 7 needs a rule that tells "limit reached" from any other failure. No limit has ever
     been seen in this app.
   - Depends on: 1 (the profile).
   - Risk: Oh-my-pi retries or moves to another account by itself, so the error never surfaces. Know
     it by: the prompt hanging or succeeding. Fallback: take the shapes from a real limit when one
     happens, and until then treat only the usage report's "limit reached" as the signal.
   - Source: inferred.
   - Status: Complete (2026-10-08), against a stand-in only. No real limit has yet been seen
   - **Decision: there is no error to read, so item 7's rule cannot be written against one.**
     Oh-my-pi never answered a failed prompt with a protocol error. Every failure came back as an
     ordinary reply: the provider's refusal as the reply's text, the prompt ending `end_turn`.
     Today the box shows that text as if the model had said it.
   - **How it was run:** the probe profile was given a `models.yml` naming three stand-in
     providers, one for each way of talking that the owner's providers use (`anthropic-messages`,
     `openai-completions`, and `openai-codex-responses`, which the ChatGPT sign-in uses), each
     with a `baseUrl` on this Mac and a key that is not a key. A small local server answered
     according to the model asked for. Each case was one prompt over the protocol, with the
     harness started as Null starts it (`omp --profile null-probe --approval-mode yolo acp`).
     28 runs. The answers the server gave are close copies, not recordings, of the real ones.
   - **What reaches Null:**

     | The provider answers | What the harness does | What Null receives |
     |---|---|---|
     | A working reply | | The text, `end_turn`, and a `usage` count in the prompt's result |
     | ChatGPT's usage limit (429, `usage_limit_reached`, resets in 90 minutes) | 6 tries in 8 s | "You have hit your ChatGPT usage limit (plus plan). Try again in ~90 min." as reply text, `end_turn`, no `usage` |
     | An Anthropic limit with a long wait (429, `retry-after` 90 minutes) | 1 try | `429 {"type":"error","error":{"type":"rate_limit_error",…}} retry-after-ms=5400000` as reply text |
     | An OpenAI-style quota used up (429, `insufficient_quota`) | 6 tries in 16 s | "429 You exceeded your current quota…" as reply text |
     | A short rate limit (429, `retry-after` 2 s) | Tries again and again, telling Null nothing | Nothing at all while it tries. ChatGPT-style: 159 s of silence, then "ChatGPT rate limit exceeded. retry-after-ms=2000" as reply text. OpenAI-style: 280 s, then the same kind of text. Anthropic-style: still silent when stopped after 9 minutes; the stop took effect at once |
     | A bad sign-in (401) | 1 try | The provider's words as reply text, with the status in front for two of the three kinds |
     | A refused model (403, 404, ChatGPT's 400) | 1 try | The provider's words as reply text. For a 400 the harness adds `raw-http-request=` and the path of a file in its own logs (the file holds no key) |

   - **The one difference between a failure and a reply:** the prompt's result carries `usage`
     for a reply and not for a failure. True of all 14 failures and all 10 replies here; nothing
     promises it in another version.
   - **The harness can carry on by itself, and the message is not lost.** With a fallback named in
     its settings (`retry.fallbackChains`, for example `"openai-codex/*": ["anthropic/…"]`), the
     used-up ChatGPT-style model gave way after the same 8 s: the harness moved to the fallback,
     sent the same message again and the answer came back as a normal reply. It tells Null with a
     `config_option_update` naming the new model. It did this for every kind of failure tried
     (limit, bad sign-in, refused model), went down a list of two in order, and stayed on the
     fallback for the next message without trying the used-up model again. With every fallback
     used up too, the reply text is the last model's refusal and the conversation is left on that
     last model. The settings were passed for the run with `--config <file>`, which works in
     front of `acp`, so Null can hand the harness settings of its own without touching the
     owner's.
   - **Null today:** takes that model change in silence. `harness.rs::on_update` updates its own
     record of the model and tells the page nothing.
   - **The owner's settings today:** `retry.fallbackChains` is
     `"openai-codex/*": ["openai-codex/gpt-5.6-sol"]`, a fallback inside the same provider.
     Retries are on (10, waiting up to five minutes between them).
   - **The usage report:** empty for the stand-ins (`"reports": []`). A provider signed in by key
     alone has no report, so "limit reached" can come from the report only for the accounts it
     knows (openai-codex and opencode-go on 2026-10-08).
   - **Not established:** a real limit on a real sign-in. Everything here used keys; with a
     sign-in Oh-my-pi may also move between accounts, and it has settings that were not tried
     (`retry.usageAwareFallback`, `retry.waitForUsageReset`). Whether the conversation's earlier
     turns carry over to a fallback of another kind was not checked either: each run was one
     message.
   - **To run it again:** `models.yml` goes in the profile's folder
     (`~/.omp/profiles/null-probe/agent/`): under `providers:`, a name, `baseUrl`, `api`, `apiKey`
     and a `models:` list of `{ id, name, contextWindow, maxTokens }`. The session then offers
     them as `<name>/<id>`. The file was taken out of the profile afterwards and the server
     stopped; neither is in the repo.

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
   - Status: In progress — the report is read and served by a `providers` command, which
     `/usage`, `/model` and `/backup` ask (items 7 and 8)
   - **Built (2026-10-09):** `Mini/src/providers.rs`. It runs `omp usage --json --redact` off the
     main thread with a 15 second limit, and reads each provider's limits (label, share used,
     reset time, the harness's own status word, and whether the limit covers the whole provider)
     and whether nothing is left. It sets that beside the providers of the model list, in the
     model list's order; a provider with models and no report (a local model, a key alone) is
     there as unreported. A reading is kept for 60 seconds and dropped after a sign-in. Nothing
     that names an account is kept.
   - **How "nothing left" is decided:** the harness says so (`metadata.limitReached`, which
     openai-codex's report has and opencode-go's has not), or a limit that covers the whole
     provider is fully used. A limit on one tier of models does not count. With several accounts
     on one provider, the one with room is the one shown.
   - **Deviation:** no `provider_state` event. The page can ask when it opens `/model` or
     `/usage`, after a sign-in and after a failed reply; the event is added only if items 7 or 8
     find they must be told and cannot ask.
   - **Proven without a person:** 8 unit tests (the report captured on 2026-10-08 with what names
     the account taken out; an empty one; hand-made ones for a limit reached, a tier limit, two
     accounts and unreadable shapes; the join with the model list), and a live check
     (`cargo test -- --ignored`) in which this code ran the harness's report under the probe
     profile and read it as no providers. The app still starts and loads its page.
   - **Not proven:** a reading of the real sign-ins by this code. The installed app asks for it
     whenever `/model` or `/backup` opens, but that has not yet been watched.

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
   - Status: In progress — built, installed and proven by the owner's sign-in to Anthropic;
     `/logout` and a time limit on a waiting sign-in are not built
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
   - **Proven by the owner (2026-10-08):** a real sign-in to Anthropic through `/login`. The owner
     reported that the Claude models then appeared under `/model` and that they answer. A first
     attempt had ended without success after 12 seconds (the log: "not signed in"), most likely
     cancelled before the browser step was done.
   - **Learned from it:** being listed is not being usable. Claude Mythos appeared in the list and
     would not answer, while the other Claude models did. The harness lists a provider's whole
     catalogue, and what an account may use is the provider's to say. By design rule 1 the box
     does not try to know; what it owes the user is the provider's refusal shown plainly.
   - **Still not checked:** what the harness prints on success, and whether the harness process
     that was running before the sign-in is really gone afterwards. Neither has been looked at in
     the log.
   - **Not built:** `/logout`; a time limit on a sign-in left waiting (Esc cancels it).

6. **First run**
   - What: when the harness is not installed, one line saying so and how to get it (decision 3).
     When nothing is signed in, one line ("No provider signed in. Type /login"), and the first time
     `/login` opens by itself. Asked once, in the way Full Disk Access is.
   - Why: other people start with nothing signed in; today they would see only a provider's error.
   - Depends on: 1, 5.
   - Risk: low.
   - Source: inferred + `MiniApp.md` (the Full Disk Access pattern).
   - Status: Moved (2026-10-09) to `Context/Plans/OwnHarness.md`, item 5. The first line of
     "What" above no longer holds: the harness is inside Null

7. **Limit reached: carry on with another provider**
   - **Decided (owner, 2026-10-08, on item 2's findings): Oh-my-pi does the switching.** Null does
     not work out that a limit was reached and does not send the message again. It hands the
     harness the owner's fallback order and says what happened. This replaces the first design,
     in which Null spotted the limit, opened the model list and re-sent.
   - What:
     - A file of Null's own, in its support folder, holding the fallback order in the form the
       harness reads (`retry.fallbackChains`: model names only, no credential), passed with
       `--config` every time Null starts the harness. The owner's own Oh-my-pi settings are not
       touched.
     - A way to set that order from the box. Not designed yet: a typed command, or an order Null
       proposes from the signed-in providers. Until one is set Null passes nothing and the
       harness's own settings apply.
     - When the harness changes the model during a reply, the box says one line naming the model
       it left and the one it is on now. `harness.rs::on_update` already hears the change and
       today keeps it to itself.
     - When a reply comes back as a failure (item 2: no `usage` in the prompt's result), the box
       shows the harness's words as an error, not as the model's reply, and opens the model list
       with used-up providers dimmed (item 4). Choosing a model moves the conversation; Esc
       leaves things as they are. This is what happens with no fallback set, or with every
       fallback used up.
     - While the harness retries in silence, one line after some seconds saying it is still
       waiting on the provider. Ctrl+C stops it, as now.
   - Why: the problem the project exists to solve. The harness already moves to another model and
     sends the message again by itself (item 2), so by design rule 1 Null does not build a
     second way to do it.
   - Depends on: 2 (done), 4.
   - Risk: the harness falls back on any failure, a refused model or a bad sign-in included, so a
     switch must always be said and never silent. The "no `usage`" sign is an accident of this
     version of Oh-my-pi. Know it by: scripted checks under the probe profile against a stand-in
     (item 2 says how to make one): a fallback that answers, a list all used up, a short rate
     limit. A real sign-in's limit is still unseen; the first one will show whether it behaves as
     the stand-in did.
   - Open: whether Null also shortens the harness's silent retries (10, with waits of up to five
     minutes) through the same file; whether the failed message is sent again after the owner
     picks from the model list (today it is typed again, or brought back with the up arrow);
     whether earlier turns carry over when the fallback is another kind of provider.
   - Source: specified from user + item 2 (verified on this Mac against a stand-in).
   - Status: In progress — built and installed (2026-10-09). The owner has not yet tried
     `/backup`, and no real limit has been seen
   - **The owner's choice (2026-10-09):** "I really like the model command where I get to pick the
     order myself", and for the order itself Claude Opus 5.5, then DeepSeek V4.1 Flash. That order
     is set in the installed app (`anthropic/claude-opus-5-5`, `opencode-go/deepseek-v4.1-flash`).
     It is the owner's setting, not something built in: Null names no model of its own.
   - **Built:**
     - `/backup` (and `/backup <words>`), blue like the other commands. It opens the model list
       with the chosen models first, numbered in the order they will be tried. Enter adds the
       model under the cursor to the end or takes it out; Esc saves and says the order in one
       line.
     - `Mini/src/backups.rs` keeps the order in the app's settings and writes it to `backups.yml`
       beside them each time the harness starts:
       `{"retry":{"fallbackChains":{"default":[…]}}}`, passed with `--config`. Setting a new
       order lets the running harness go, so the next message starts one that has read it.
     - A switch is said: `harness.rs` publishes `model_switched` with the model left and the
       model now in use, and the box prints "X did not answer. now on Y".
     - A failed reply is said: when a reply ends in the ordinary way and counts no tokens, the
       app publishes `reply_failed`; the box turns the provider's words red and adds "no answer
       from the provider. /model picks another model".
     - After 20 seconds with nothing from the harness and no tool running, the box says "still
       waiting on the provider. ctrl+c stops it" until something arrives.
     - `/model` and `/backup` ask the app what each provider has left (item 4) once the list is
       open, and dim a provider with nothing left.
   - **What the harness does with the list** (stand-in, probe profile, 2026-10-09):
     - `default` is the list it turns to for any model that has no list of its own.
     - It skips the model that just failed, so a model may be in its own list.
     - It goes down the list of the model the reply started on, and does not look up the lists
       of the backups it passes through. Two models naming each other do not loop.
     - A model or provider it does not know is skipped; the harness still starts.
     - The file is added to the owner's own Oh-my-pi settings, not put in their place. A list
       the owner has for one model or one provider is tried first, then `default`.
     - It does not always tell Null that it moved: after a failure answered at once, the reply
       came from the backup with no notice at all. So Null asks after every reply, by setting
       the conversation's thinking level to the value it already has; the answer carries the
       model in use.
   - **Deviations:**
     - A failed reply does not open the model list. The list takes the transcript's place, and
       would cover the provider's words, which are the reason.
     - The token count at the end of a reply is an unstable part of the protocol; the app turns
       it on in the protocol library (`unstable_end_turn_token_usage`). Without it every reply
       looked like a failure.
     - A scripted check under `NULL_MINI_PROFILE` now keeps its own settings and backup files
       (`settings.<profile>.json`), so that a check no longer changes what the installed app
       remembers. `NULL_MINI_BACKUPS` (model ids with commas) names an order for one run.
   - **Proven without a person:** 41 unit tests. The app itself, under the probe profile against
     the stand-in: a backup that answers; a ChatGPT-style limit with the first backup used up
     too; every backup used up; a working model; a refused model with no backup. The real page,
     typed into by the self-test: the switch line, and the red words with their note. The nine
     new states of the box drawn in the app's web engine and looked at. One one-word message to
     each of the owner's three providers (2026-10-09): a working reply counts tokens on all
     three, so none is taken for a failure, and asking which model is in use changes nothing.
   - **Not proven:** a real limit on a real sign-in; `/backup` under the owner's hands; the
     dimming and the waiting line in the installed app (drawn, and their parts tested, but not
     seen live).
   - **Decided (owner, 2026-10-09): "make Null's order go first."** The owner's own Oh-my-pi
     settings send every ChatGPT model to `gpt-5.6-sol`, and the harness tries a list for one
     model or one provider before `default`, so a ChatGPT limit would have cost one more round
     of tries on the same provider before Opus was reached. Now, each time it starts the
     harness, Null reads the lists the owner has (`omp config get retry.fallbackChains --json`)
     and gives each one for a model or a provider again in its own file: Null's order, then
     whatever the owner's list adds to it. A list under the same name in Null's file takes the
     place of the owner's for that run. The owner's settings themselves are not changed, and
     lists for roles other than `default` are left alone.
   - **Proven for that** (stand-in, probe profile): with a list of the harness's own for the
     ChatGPT-style provider, the used-up model went straight to Null's backup; with Null's
     backup used up too, the harness's own entry was tried after it.

8. **`/usage`, and what is left in the model list**
   - What: `/usage` lists each signed-in provider with what is left and when it resets. The model
     list shows the same beside each provider's heading and dims providers with nothing left.
   - Why: "I can switch whenever I want to manage usage": before the wall, not only at it.
   - Depends on: 4.
   - Risk: more words in a box meant to stay bare. Know it by: the owner's eye; the figures appear
     only in these two lists, never in the idle box.
   - Source: specified from user + verified locally (the report).
   - Status: In progress — built and installed (2026-10-09); the owner has not yet used it
   - **Built:** `/usage`, blue like the other commands. It asks the harness afresh and lists each
     provider with one row a limit: the harness's own name for the limit, how much is used, and
     when it starts over (the time if that is soon, the day if it is this week, the date after
     that). A limit that is used up is dimmed and says so. A provider with no report says "no
     usage report". Enter or Esc puts the list away, and it may be opened while a reply runs.
     In `/model` and `/backup` each provider's name now carries its tightest limit ("37% left"),
     or "nothing left, resets 3:54 am".
   - **Checked:** drawn in the app's web engine with the figures the harness reported for the
     owner's three providers on 2026-10-09 (anthropic 54% and 63% used, openai-codex 0% and 2%,
     opencode-go 6%, 29% and 25%), and with one provider used up. All three reports name their
     limits differently, and each is shown as the harness names it.
   - **Not proven:** the installed app asking the harness itself; the drawings were fed the
     figures by a stand-in for the app's command.

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
   - Status: Complete (2026-10-09), merged the same day. Nothing is built for a
     key that is only in the environment, and nothing is needed (below)
   - **The field:** already there since item 5. While a sign-in is at its steps the field hides
     what is typed, and nothing typed then is drawn in the box.
   - **The proof:** `Mini/tests/keys.rs`, run with the live checks (`cargo test -- --ignored`).
     It starts the real app on a folder of its own, has it type `/login`, choose DeepSeek and
     type a marked key that is no key, which DeepSeek refuses. Then it searches everything Null
     printed, which is also its log and what the page showed, and every file Null keeps (the
     log, the settings, the settings it hands the harness) for the mark. It opens the box on
     the screen for a few seconds and reaches DeepSeek once.
   - **What made it possible:** `NULL_MINI_SELFTEST` now types into the real page a line at a
     time, each once the page has stopped working, so a command, a choice from a list and an
     answer to a sign-in can follow one another. Before, it sent one message and waited for a
     reply, so a typed command never ended it.
   - **Proven:** the test passes. With a line put into `signin_answer` on purpose that logged
     what was typed, it failed with "the key is in what Null printed", and the line was taken
     out again. By hand, the same run printed the harness's own lines ("Paste your DeepSeek API
     key (sk-...):", "Validating API key...") and the mark was in no file under the run's
     folder.
   - **A key only in the environment:** seen on 2026-10-09 with a made-up `ANTHROPIC_API_KEY`:
     the harness then lists that provider's models, with no account in its usage report. Null
     passes its own environment on to the harness and does nothing else with it. A Null started
     at login does not have the variables of a terminal, so such a key reaches it only if the
     person puts it in the harness's own settings. Nothing in Null reads, shows or keeps it.
   - **Not covered:** a sign-in that succeeds, since that needs a real key; a browser sign-in's
     pasted code, which takes the same path as a key.

10. **Say it for other people**
    - What: a README section on what Null needs (Oh-my-pi), how sign-in works, that sign-ins live
      in the harness, and that each provider's terms apply (decision 2). An ADR for the credential
      rule as decision 1 restates it, one for design rule 1, and glossary entries for provider,
      harness, sign-in and limit (`/grill`).
    - Why: the open-source release; the rule is today only a line in two plans.
    - Depends on: none for the ADRs and the glossary; item 5 for the README.
    - Risk: low.
    - Source: inferred.
    - Status: Complete (2026-10-10), on the owner's general word of that day (`OwnHarness.md`,
      Constraints), without `/grill`
    - **Written:** `Context/ADR/0004-NoCredentialRestsInNull.md` and
      `Context/ADR/0005-NullKnowsNoProvider.md`, from decision 1 and design rules 1 to 3 as they
      were settled on 2026-10-08. The glossary, `Context/Glossary.md`, with provider, harness,
      sign-in and limit among its twenty terms.
    - **The README** already said what Null needs and whose the sign-ins are: its first step
      (the harness comes with Null; providers, subscriptions and keys are managed by Oh-my-pi,
      and their terms apply) and its privacy card (answers to `/login` go to the harness's own
      sign-in and are never written to Null's settings or log).

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

12. **A provider's refusal, said plainly**
    - What: when a provider will not serve the chosen model (as with Claude Mythos on the owner's
      account), the box says one line with the model and the provider's own reason, and does not
      treat it as a limit. Null keeps no list of which models an account may use (design rule 1).
      First find what reaches Null today: the box prints any failure as "error: " plus the
      harness's message, and what it printed for Mythos was not recorded.
    - Why: being listed is not being usable (item 5, "Learned from it").
    - Depends on: 2, whose stand-in also answers with a refusal of the model, so that shape is
      recorded beside the limit and the bad sign-in.
    - Risk: a refusal read as a limit, or a limit as a refusal. Know it by: the rule tested
      against item 2's recorded shapes, as in item 7.
    - Source: inferred from the owner's report (item 5) + codebase (`Mini/Page/index.html`, the
      `error` case).
    - **After item 2 (2026-10-08):** the provider's own words do reach Null, as the text of a
      reply that carries no `usage`. Item 7 shows any such reply as an error, which covers this.
      With a fallback set the harness moves on from a refused model as it does from a limit, and
      item 7's line says so. Null cannot tell a refusal from a limit except by reading the
      provider's words, which design rule 1 rules out, so it shows them and does not sort them.
    - Status: Folded into item 7 (2026-10-08)

## Verification

- Unit tests: reading the usage report (item 4), telling a failed reply from a working one (item 7).
- Scripted, under the probe profile (item 3): a first run with nothing signed in; against a
  stand-in, a fallback that answers, a list all used up and a short rate limit (item 7); `/login`
  cancelled; nothing of a dummy key left in the log or settings.
- By hand, the owner: `/login` to a provider not yet signed in (Anthropic is the obvious one), then
  `/model` to it; `/usage`; a switch after a limit, real or stand-in.

## Validation

- No ADR is contradicted. The credential rule and design rule 1 are not yet ADRs; item 10 makes
  them so.
- The order holds. Items 1 and 2 are spikes and decide how 5, 7 and 9 are built; item 4 depends on
  nothing and can be built beside them.
- No longer fuzzy: the harness's questions reach the screen as plain lines (item 1), and there
  is no limit rule, because the harness does the switching (item 2, decision 4). Still fuzzy, on
  purpose: how the fallback order is set from the box (item 7).
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
