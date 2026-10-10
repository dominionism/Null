# No credential rests in Null

A provider is reached with a sign-in: a browser page, a code, or a key. Null could have kept keys
itself, in its settings or in the macOS keychain, as many apps do. The owner settled on 2026-10-08
that it does not: Null never stores a credential, never logs it, and passes what is typed only to
the harness's own sign-in on the same Mac (`Providers.md`, decision 1 and design rule 3).

So `/login` runs the harness's sign-in and shows its questions. The field hides what is typed, and
what is typed goes to that program's input and nowhere else. Sign-ins live in the harness's folder,
where the same Oh-my-pi finds them in a terminal. A test, `Mini/tests/keys.rs`, signs in with a
marked key that is no key and searches Null's log, its settings and what the box showed for it.

Rejected: a key field of Null's own with a store behind it. Null would then hold what an attacker
wants most, and would need to know, provider by provider, how each key is used.
