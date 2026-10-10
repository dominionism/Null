# Null knows nothing about any one provider

Null lets a person sign in to a provider, see what is left of its limits, and carry on with another
when one runs out. Each of those could have been built provider by provider: a list of providers in
Null, the steps of each sign-in, a reader for each usage page. The owner asked on 2026-10-08, of a
sign-in that takes an extra step, "how can we have null figure out anything else someone needs to
authenticate". The answer settled then is that it does not try. The harness knows each provider and
keeps that current; Null shows whatever the harness asks and reads whatever the harness reports
(`Providers.md`, design rules 1 and 2).

So Null holds no list of providers and no steps. The sign-in list, the model list, the usage report
and the switch to a backup are all the harness's, and Null draws them. It checks results, not
steps: after a sign-in it looks for the provider's models, and "signed in" means an account in the
harness's report. A provider that changes its sign-in, or a new one, needs no change to Null.

The cost is accepted: Null is only as good as what the harness reports. It cannot tell a local
model from any other, or say why a listed model will not answer, unless the harness says.

Rejected: a harness adapter for each provider, which the first plan had (`NullMini.md`, Phase 2),
and a catalogue in Null of providers with their sign-in steps.
