<p align="center">
  <img src=".github/Readme/hero.svg" width="100%" alt="Null — your agent, a shortcut away. A monochrome illustration of the floating input and Control+Space shortcut." />
</p>

<p align="center">
  <strong>A compact macOS interface to your own agent.</strong><br/>
  Ask a question. Put it to work. Keep your choice of models and providers.
</p>

<p align="center">
  <code>macOS 13+</code> &nbsp; <code>Oh-my-pi</code> &nbsp; <code>Rust + Tauri</code> &nbsp; <a href="LICENSE">MIT</a>
</p>

<p align="center">
  <a href="#get-started">Get started</a> &nbsp; / &nbsp;
  <a href="#use-null">Use Null</a> &nbsp; / &nbsp;
  <a href="#privacy-and-permissions">Privacy</a> &nbsp; / &nbsp;
  <a href="#under-the-hood">Architecture</a> &nbsp; / &nbsp;
  <a href="#development">Development</a>
</p>

<a id="small-surface-real-agent"></a>
<p align="center">
  <img src=".github/Readme/surface.svg" width="100%" alt="Small surface. Real agent. Press Control+Space from the app you are in: your tools, not just chat; your providers in one conversation; readable without getting bigger; today, a standalone macOS text app built from source." />
</p>

<a id="get-started"></a>
<a id="step-1-bring-your-harness"></a>
<p align="center">
  <img src=".github/Readme/step-harness.svg" width="100%" alt="1. Bring your harness: install Oh-my-pi, whose sign-ins, subscriptions and keys Null does not manage; Null looks on PATH and in known install folders, and does not install it." />
</p>

<p align="center"><sub><a href="https://github.com/can1357/oh-my-pi">Oh-my-pi</a></sub></p>

<a id="step-2-build-and-install"></a>
<p align="center">
  <img src=".github/Readme/step-build.svg" width="100%" alt="2. Build and install: macOS 13+, the Rust toolchain, Xcode Command Line Tools and Tauri CLI 2; xcode-select --install, cargo install tauri-cli, git clone, then Mini/Scripts/install, which prints that Null.app is installed and Control+Space opens the box." />
</p>

<p align="center"><sub><a href="https://rustup.rs/">Rust toolchain</a> &nbsp;·&nbsp; <a href="https://v2.tauri.app/start/prerequisites/#macos">Tauri macOS prerequisites</a></sub></p>

<a id="step-3-open-the-box"></a>
<p align="center">
  <img src=".github/Readme/step-open.svg" width="100%" alt="3. Open the box: Control+Space opens it, /login adds a provider, /model chooses one of its models, then type your request; no terminal has to stay running." />
</p>

<a id="uninstall"></a>
<p align="center">
  <img src=".github/Readme/uninstall.svg" width="100%" alt="Uninstall: Mini/Scripts/uninstall removes the app and its login item, leaving settings, workspace files and logs behind; OMP's own sign-ins and history are not removed." />
</p>

<a id="use-null"></a>
<p align="center">
  <img src=".github/Readme/use-null.svg" width="100%" alt="Use Null: Control+Space shows or hides the box, Enter sends or chooses, Esc closes or hides, Ctrl+C stops the reply or quits when idle, the up arrow recalls the last prompt, dragging moves the box." />
</p>

<a id="commands"></a>
<p align="center">
  <img src=".github/Readme/commands.svg" width="100%" alt="A few commands and no settings window: /model, /login, /usage, /backup, /new and /quit, with what each does and what an unknown command does." />
</p>

<a id="when-a-provider-stops-answering"></a>
<p align="center">
  <img src=".github/Readme/provider-stops.svg" width="100%" alt="When a provider stops answering: Null passes your backup order to OMP as an overlay without changing your settings, OMP decides when to retry and resends, the box reports switches and failed replies but never resends one." />
</p>

<a id="privacy-and-permissions"></a>
<p align="center">
  <img src=".github/Readme/privacy.svg" width="100%" alt="Privacy and permissions: a local interface is not an offline agent, Null has no listener or database, /login answers stay with the harness, the shortcut needs no Accessibility permission, and Full Disk Access is a separate choice." />
</p>

<a id="signing-and-permission-persistence"></a>
<p align="center">
  <img src=".github/Readme/signing.svg" width="100%" alt="Signing and permission persistence: grants follow the code signature, so an unsigned rebuild can ask again; with the local signing keychain the installer signs as Null Local Signing, and without it installs unsigned." />
</p>

<a id="what-null-keeps"></a>
<p align="center">
  <img src=".github/Readme/what-null-keeps.svg" width="100%" alt="What Null keeps: settings.json, backups.yml and Workspace/ under the app support directory, while OMP keeps the transcript and credentials; logs are in ~/Library/Logs/Null." />
</p>

<a id="under-the-hood"></a>
<p align="center">
  <img src=".github/Readme/under-the-hood.svg" width="100%" alt="Under the hood: Control+Space reaches Null's page, Tauri commands reach OMP over ACP, and text, tools and approvals come back; one native Tauri app with a static page and no web server." />
</p>

<a id="source-map"></a>
<p align="center">
  <img src=".github/Readme/source-map.svg" width="100%" alt="Source map: the page, main.rs, panel.rs and shortcut.rs, harness.rs and translate.rs, providers.rs, signin.rs and backups.rs, markdown.rs, and settings.rs, access.rs and log.rs, each with what it owns." />
</p>

<p align="center"><sub><a href="Mini/Page/index.html">Page/index.html</a> &nbsp;·&nbsp; <a href="Mini/src/harness.rs">harness.rs</a> &nbsp;·&nbsp; <a href="Mini/src/translate.rs">translate.rs</a> &nbsp;·&nbsp; <a href="Mini/src/markdown.rs">markdown.rs</a> &nbsp;·&nbsp; <a href="Mini/src/panel.rs">panel.rs</a> &nbsp;·&nbsp; <a href="Mini/src/settings.rs">settings.rs</a></sub></p>

<a id="development"></a>
<p align="center">
  <img src=".github/Readme/development.svg" width="100%" alt="Development: cd Mini and cargo test for offline unit tests, cargo build for a debug binary, cargo tauri build for the app bundle, and why not to use bunx tauri." />
</p>

<a id="development-switches-and-smoke-checks"></a>
<p align="center">
  <img src=".github/Readme/dev-switches.svg" width="100%" alt="Development switches and smoke checks: the six NULL_MINI_ environment variables, a startup-only check that leaves the installed shortcut alone, and the warning that profile isolation does not isolate MCP." />
</p>

<a id="optional-voice-tools"></a>
<p align="center">
  <img src=".github/Readme/voice.svg" width="100%" alt="Optional voice tools: Voicebox is a separate tool the box can use for voice, not part of its text path; your text path never calls the Voice API, and inference runs locally." />
</p>

<p align="center"><sub><a href="https://github.com/jamiepine/voicebox">Voicebox</a> &nbsp;·&nbsp; <a href="Context/ADR/0002-NullMiniIsItsOwnApp.md">ADR 0002</a></sub></p>

<a id="run-or-develop-the-voice-stack"></a>
<p align="center">
  <img src=".github/Readme/voice-run.svg" width="100%" alt="Run or develop the Voice stack: just setup, just dev, bun run dev:server, just test and the typecheck, with the API on 127.0.0.1:17493, narration on 17494, the MCP tools, and the warning that most endpoints have no authentication." />
</p>

<p align="center"><sub><a href="https://bun.sh">Bun</a> &nbsp;·&nbsp; <a href="https://github.com/casey/just">just</a> &nbsp;·&nbsp; <a href="Context/Research/Research.md">research map</a></sub></p>

<a id="status-and-direction"></a>
<p align="center">
  <img src=".github/Readme/status.svg" width="100%" alt="Status and direction: what is available in the box, and what is planned or incomplete." />
</p>

<a id="repository-layout-and-ongoing-cleanup"></a>
<p align="center">
  <img src=".github/Readme/layout.svg" width="100%" alt="Repository layout and ongoing cleanup: Mini/, the Voice directories app, tauri, web, backend, scripts and data, the inherited docs and landing sites, Context/, and CHANGELOG.md." />
</p>

<p align="center"><sub><a href="Context/Plans/CapitalFolders.md">cleanup plan</a> &nbsp;·&nbsp; <a href="Context/ADR/0001-CapitalizedFolderNames.md">ADR 0001</a></sub></p>

<a id="read-the-reasoning"></a>
<p align="center">
  <img src=".github/Readme/reasoning.svg" width="100%" alt="Read the reasoning: the app design, providers, conversation display and future-phase plans, the decisions that constrain changes, and the codebase map including what was never traced." />
</p>

<p align="center"><sub><a href="Context/Plans/MiniApp.md">App design</a> &nbsp;·&nbsp; <a href="Context/Plans/Providers.md">Providers</a> &nbsp;·&nbsp; <a href="Context/Plans/ConversationDisplay.md">Conversation display</a> &nbsp;·&nbsp; <a href="Context/Plans/NullMini.md">Future phases</a> &nbsp;·&nbsp; <a href="Context/ADR/">Decisions</a> &nbsp;·&nbsp; <a href="Context/Research/Research.md">Codebase research</a></sub></p>

<a id="license"></a>
<p align="center">
  <img src=".github/Readme/license.svg" width="100%" alt="License: MIT, clone only voices you own or have permission to use, and the responsible-use, security and contribution files." />
</p>

<p align="center"><sub><a href="LICENSE">MIT</a> &nbsp;·&nbsp; <a href="RESPONSIBLE_USE.md">Responsible use</a> &nbsp;·&nbsp; <a href="SECURITY.md">Security</a> &nbsp;·&nbsp; <a href="CONTRIBUTING.md">Contributing</a></sub></p>
