# Null Mini is its own app

Null Mini was first built as a window of the Voice desktop app, with its connection to the agent
harness inside that project's background server, so fn+Space stopped working whenever the desktop app
was not running. The owner decided on 2026-10-08 that the two are separate for now: "fn + space
shouldn't be dependent on null desktop", and the desktop is "for cloning my voice so that it will be
the voice leveraged by my agents for the real-time voice-to-voice mode."

Null Mini therefore lives in `Mini/` as a small app that owns its window, its fn+Space shortcut and its
own connection to the harness. It starts at login and needs neither the Voice desktop nor its server.
The harness layer and the `/mini` endpoints already in the backend stay there, as the Voice project's
route to agents for voice mode.

Rejected: moving only the window and shortcut into their own app. The mini would still stop working
whenever the Voice server was down, so the two would not be separate.
