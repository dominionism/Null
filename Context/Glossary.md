# Null

Null is a small macOS box that opens on Control+Space and drives an agent harness, so that asking a
question or handing over work does not depend on one provider. This glossary fixes the words its
plans, its code and the box itself use.

## Language

**Null**:
The app: one small box and the harness it drives.
_Avoid_: Null Mini, the mini (the app's first name; it survives only in names nobody sees, such as `Mini/` and `null-mini`)

**Box**:
Null's one window: a field and an arrow, and under them a conversation or a list when there is one to show.
_Avoid_: panel, overlay, window, pet

**Harness**:
The agent program behind the box: it runs the tools, keeps the conversations and holds the sign-ins. Today it is Oh-my-pi.
_Avoid_: backend, agent CLI, engine (the engine is the harness's program, not the part it plays)

**Engine**:
The harness's program file that Null carries inside the app, at the one version `Mini/Engine.toml` names.
_Avoid_: bundled binary, embedded omp, harness (when the file is meant)

**Built in**:
The engine Null runs unless told otherwise: the one inside the app, or a newer checked one that `/update` fetched.
_Avoid_: default harness, bundled

**Yours**:
An Oh-my-pi the person installed themselves, which Null runs only when it is chosen with `/harness`.
_Avoid_: system omp, external harness, own (in the box; the code says `own`)

**Provider**:
A company or service whose models answer. Null knows nothing about any one of them; the harness does.
_Avoid_: vendor, backend, model host

**Sign-in**:
The harness's own way of getting access to a provider: a browser page, a code or a key. Null shows it and keeps nothing of it.
_Avoid_: login (except as the command `/login`), authentication, credentials setup

**Account**:
A sign-in the harness lists in its usage report. A local model is not an account, and neither is a key the harness was handed some other way.
_Avoid_: user, subscription

**Limit**:
How much of a provider's allowance is used and when it starts over, as the harness reports it.
_Avoid_: quota, rate limit (a short rate limit is a different thing, and keeps the harness silent for minutes)

**Backup**:
A model to carry on with when the one in use stops answering, in the order the person set with `/backup`. The harness does the switching.
_Avoid_: fallback (the harness's own word for it), failover

**Refusal**:
A reply that carries a provider's "no" in place of an answer: a used-up limit, a bad sign-in, or a model the account may not use.
_Avoid_: error, failure (an error is the harness or the protocol breaking, which is something else)

**Conversation**:
One exchange between the person and the harness, kept by the harness. Null keeps only which one was open, and a title for each one made in the box.
_Avoid_: session (the protocol's word), chat, mini session, task, job (a conversation that is working is still a conversation, and the box has no other thing)

**Open**:
Said of the one conversation the box is showing.
_Avoid_: active, current, foreground, selected

**Working**:
Said of a conversation whose reply is still running, whether it is open or not.
_Avoid_: running (the reference product's word), busy (the code's word), in progress, at work

**Asking**:
Said of a conversation that waits for the person's answer before it goes on.
_Avoid_: needs input, waiting, blocked

**Done**:
Said of a conversation whose reply ended well.
_Avoid_: ready, complete, finished

**Failed**:
Said of a conversation whose reply ended with an error or a refusal.
_Avoid_: blocked, errored

**Stopped**:
Said of a conversation whose reply the person ended.
_Avoid_: cancelled (the protocol's word), interrupted, killed

**Cut off**:
Said of a conversation that was working when Null or the harness went away.
_Avoid_: orphaned, crashed, lost

**Unseen**:
Said of a conversation that ended or began asking while it was not open, and has not been opened since.
_Avoid_: unread, ready, new

**Now**:
The first heading in `/history`: every conversation that is working, asking or unseen.
_Avoid_: activity tray, tasks, running

**Work in the background**:
A conversation working while it is not open, or while the box is put away. It is not a mode, and not a thing of its own.
_Avoid_: background task, background job, background session

**Folder**:
Where a conversation works: the place its agent reads and changes files in. It is chosen when the conversation starts and does not change.
_Avoid_: workspace (the code's word), project, directory, cwd

**The box's own folder**:
The folder a conversation works in unless the person chooses another with `/folder`.
_Avoid_: default workspace, home folder, sandbox

**Harness check**:
The live tests that ask a real Oh-my-pi the things Null depends on, each named for one of them.
_Avoid_: compatibility tests, integration tests

**Checked version**:
A version of Oh-my-pi that passed the harness check with some Null. Null carries, and `/update` fetches, only checked versions.
_Avoid_: supported version, latest

**Stand-in provider**:
A made-up provider on the Mac that answers by the name of the model asked for, so Null can be tried with no real sign-in.
_Avoid_: mock, stub, fake provider

**Folder of its own**:
A folder a scripted run uses in place of the person's harness folder and Null's own files, so the run reads nothing of theirs.
_Avoid_: sandbox, profile (a profile is the harness's own named folder, a different thing)

**First opening**:
The one time Null opens by itself: when no account is signed in, to say so and show the sign-in list.
_Avoid_: first run, onboarding, welcome (in words; the code says `welcome`)

**Release**:
A Null built on GitHub for people who do not build it, published under a tag `null-v<version>`.
_Avoid_: download, distribution, build (a build is what any Mac makes)

**Signing identity**:
The certificate, made on each Mac and never leaving it, that Null is signed with so that macOS knows it again after an update.
_Avoid_: Developer ID (Null has none), certificate (alone, since Oh-my-pi's author has one too)
