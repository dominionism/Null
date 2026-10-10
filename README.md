<p align="center">
  <img src=".github/Readme/hero.svg" width="100%" alt="Null — your agent, a shortcut away. A monochrome illustration of the floating input and Control+Space shortcut." />
</p>

<p align="center">
  <strong>A compact macOS interface to your own agent.</strong><br/>
  Ask a question. Put it to work. Keep your choice of models and providers.
</p>

<p align="center">
  <code>macOS 13+</code> &nbsp; <code>Oh-my-pi</code> &nbsp; <code>Rust + Tauri</code> &nbsp; <code>MIT</code>
</p>

<p align="center">
  <a href="#get-started"><img src=".github/Readme/link-nav-get-started.svg" height="30" alt="Get started" /></a>
  &nbsp;
  <a href="#use-null"><img src=".github/Readme/link-nav-use-null.svg" height="30" alt="Use Null" /></a>
  &nbsp;
  <a href="#privacy-and-permissions"><img src=".github/Readme/link-nav-privacy.svg" height="30" alt="Privacy" /></a>
  &nbsp;
  <a href="#under-the-hood"><img src=".github/Readme/link-nav-architecture.svg" height="30" alt="Architecture" /></a>
  &nbsp;
  <a href="#development"><img src=".github/Readme/link-nav-development.svg" height="30" alt="Development" /></a>
</p>

<a id="small-surface-real-agent"></a>
<p align="center">
  <img src=".github/Readme/surface.svg" width="100%" alt="Small surface. Real agent. Press Control+Space from the app you are in: your tools, not just chat; your providers in one conversation; readable without getting bigger; today, a standalone macOS text app built from source." />
</p>

<a id="get-started"></a>
<a id="step-1-the-harness-comes-with-null"></a>
<p align="center">
  <img src=".github/Readme/step-harness.svg" width="100%" alt="1. The harness comes with Null: nothing to install first; Null carries one checked version of Oh-my-pi and works from OMP's usual folder, with your existing sign-ins, skills and MCP servers; OMP manages providers, subscriptions and keys; /harness switches to your own copy and back; Apple Silicon Macs only, for now." />
</p>

<p align="center">
  <a href="https://github.com/can1357/oh-my-pi"><img src=".github/Readme/link-oh-my-pi.svg" height="30" alt="Oh-my-pi (opens https://github.com/can1357/oh-my-pi)" /></a>
</p>

<a id="step-2-build-and-install"></a>
<p align="center">
  <img src=".github/Readme/step-build.svg" width="100%" alt="2. Build and install: an Apple Silicon Mac with macOS 13+, the Rust toolchain, Xcode Command Line Tools and Tauri CLI 2; xcode-select --install, cargo install tauri-cli, git clone, then Mini/Scripts/install, which fetches the Oh-my-pi named in Mini/Engine.toml, builds, and prints that Null.app is installed and Control+Space opens the box." />
</p>

<p align="center">
  <a href="https://rustup.rs/"><img src=".github/Readme/link-rust.svg" height="30" alt="Rust toolchain (opens https://rustup.rs/)" /></a>
  &nbsp;
  <a href="https://v2.tauri.app/start/prerequisites/#macos"><img src=".github/Readme/link-tauri.svg" height="30" alt="Tauri macOS prerequisites (opens https://v2.tauri.app/start/prerequisites/#macos)" /></a>
</p>

<a id="step-3-open-the-box"></a>
<p align="center">
  <img src=".github/Readme/step-open.svg" width="100%" alt="3. Open the box: the first time, with no provider signed in, it opens by itself with the sign-in list; Control+Space opens it, /login adds a provider, /model chooses one of its models, then type your request; no terminal has to stay running." />
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
  <img src=".github/Readme/commands.svg" width="100%" alt="A few commands and no settings window: /model, /login, /usage, /backup, /harness, /update, /new and /quit, with what each does and what an unknown command does." />
</p>

<a id="when-a-provider-stops-answering"></a>
<p align="center">
  <img src=".github/Readme/provider-stops.svg" width="100%" alt="When a provider stops answering: Null passes your backup order to OMP as an overlay without changing your settings, OMP decides when to retry and resends, the box reports switches and failed replies but never resends one." />
</p>

<a id="privacy-and-permissions"></a>
<p align="center">
  <img src=".github/Readme/privacy.svg" width="100%" alt="Privacy and permissions: a local interface is not an offline agent, OMP asks outside services for model lists when it starts, Null has no listener or database, /login answers stay with the harness, the shortcut needs no Accessibility permission, Null passes on OMP's approval setting, whose default lets the agent act without asking, and Full Disk Access is a separate choice asked after the first reply." />
</p>

<a id="signing-and-permission-persistence"></a>
<p align="center">
  <img src=".github/Readme/signing.svg" width="100%" alt="Signing and permission persistence: grants follow the code signature, so an unsigned rebuild can ask again; the installer signs as Null Local Signing with an identity kept on your Mac, makes one without asking when there is none, and installs unsigned only if signing fails; the Oh-my-pi inside keeps its author's signature." />
</p>

<a id="what-null-keeps"></a>
<p align="center">
  <img src=".github/Readme/what-null-keeps.svg" width="100%" alt="What Null keeps: settings.json, harness.yml and Workspace/ under the app support directory, while OMP keeps the transcript and credentials; logs are in ~/Library/Logs/Null." />
</p>

<a id="under-the-hood"></a>
<p align="center">
  <img src=".github/Readme/under-the-hood.svg" width="100%" alt="Under the hood: Control+Space reaches Null's page, Tauri commands reach OMP over ACP, and text, tools and approvals come back; one native Tauri app that carries and launches OMP, with a static page and no web server." />
</p>

<a id="source-map"></a>
<p align="center">
  <img src=".github/Readme/source-map.svg" width="100%" alt="Source map: the page, main.rs, panel.rs and shortcut.rs, harness.rs and translate.rs, engine.rs, providers.rs and signin.rs, welcome.rs, backups.rs, markdown.rs, settings.rs, access.rs and log.rs, and check.rs and standin.rs, each with what it owns." />
</p>

<p align="center">
  <a href="Mini/Page/index.html"><img src=".github/Readme/link-page-html.svg" height="30" alt="Page/index.html" /></a>
  &nbsp;
  <a href="Mini/src/harness.rs"><img src=".github/Readme/link-harness-rs.svg" height="30" alt="harness.rs" /></a>
  &nbsp;
  <a href="Mini/src/engine.rs"><img src=".github/Readme/link-engine-rs.svg" height="30" alt="engine.rs" /></a>
  &nbsp;
  <a href="Mini/src/translate.rs"><img src=".github/Readme/link-translate-rs.svg" height="30" alt="translate.rs" /></a>
  &nbsp;
  <a href="Mini/src/markdown.rs"><img src=".github/Readme/link-markdown-rs.svg" height="30" alt="markdown.rs" /></a>
  &nbsp;
  <a href="Mini/src/panel.rs"><img src=".github/Readme/link-panel-rs.svg" height="30" alt="panel.rs" /></a>
  &nbsp;
  <a href="Mini/src/settings.rs"><img src=".github/Readme/link-settings-rs.svg" height="30" alt="settings.rs" /></a>
  &nbsp;
  <a href="Mini/src/check.rs"><img src=".github/Readme/link-check-rs.svg" height="30" alt="check.rs" /></a>
</p>

<a id="development"></a>
<p align="center">
  <img src=".github/Readme/development.svg" width="100%" alt="Development: Mini/Scripts/engine once to fetch the Oh-my-pi the build takes in, cd Mini and cargo test for offline unit tests, cargo build for a debug binary, cargo tauri build for the app bundle, why not to use bunx tauri, and the harness check." />
</p>

<a id="development-switches-and-smoke-checks"></a>
<p align="center">
  <img src=".github/Readme/dev-switches.svg" width="100%" alt="Development switches and smoke checks: the nine NULL_MINI_ environment variables, a startup-only check that leaves the installed shortcut alone, a run on a folder of its own with a stand-in provider, and the warning that profile isolation does not isolate MCP." />
</p>

<a id="optional-voice-tools"></a>
<p align="center">
  <img src=".github/Readme/voice.svg" width="100%" alt="Optional voice tools: Voicebox is a separate tool the box can use for voice, not part of its text path; your text path never calls the Voice API, and inference runs locally." />
</p>

<p align="center">
  <a href="https://github.com/jamiepine/voicebox"><img src=".github/Readme/link-voicebox.svg" height="30" alt="Voicebox (opens https://github.com/jamiepine/voicebox)" /></a>
  &nbsp;
  <a href="Context/ADR/0002-NullMiniIsItsOwnApp.md"><img src=".github/Readme/link-adr-0002.svg" height="30" alt="ADR 0002" /></a>
</p>

<a id="run-or-develop-the-voice-stack"></a>
<p align="center">
  <img src=".github/Readme/voice-run.svg" width="100%" alt="Run or develop the Voice stack: just setup, just dev, bun run dev:server, just test and the typecheck, with the API on 127.0.0.1:17493, narration on 17494, the MCP tools, and the warning that most endpoints have no authentication." />
</p>

<p align="center">
  <a href="https://bun.sh"><img src=".github/Readme/link-bun.svg" height="30" alt="Bun (opens https://bun.sh)" /></a>
  &nbsp;
  <a href="https://github.com/casey/just"><img src=".github/Readme/link-just.svg" height="30" alt="just (opens https://github.com/casey/just)" /></a>
  &nbsp;
  <a href="Context/Research/Research.md"><img src=".github/Readme/link-research.svg" height="30" alt="Research map" /></a>
</p>

<a id="status-and-direction"></a>
<p align="center">
  <img src=".github/Readme/status.svg" width="100%" alt="Status and direction: what is available in the box, and what is planned or incomplete." />
</p>

<a id="repository-layout-and-ongoing-cleanup"></a>
<p align="center">
  <img src=".github/Readme/layout.svg" width="100%" alt="Repository layout and ongoing cleanup: Mini/, the Voice directories app, tauri, web, backend, scripts and data, the inherited docs and landing sites, Context/, and CHANGELOG.md." />
</p>

<p align="center">
  <a href="Context/Plans/CapitalFolders.md"><img src=".github/Readme/link-cleanup-plan.svg" height="30" alt="Cleanup plan" /></a>
  &nbsp;
  <a href="Context/ADR/0001-CapitalizedFolderNames.md"><img src=".github/Readme/link-adr-0001.svg" height="30" alt="ADR 0001" /></a>
</p>

<a id="read-the-reasoning"></a>
<p align="center">
  <img src=".github/Readme/reasoning.svg" width="100%" alt="Read the reasoning: the app design, providers, own-harness, conversation display and future-phase plans, the decisions that constrain changes, and the codebase map including what was never traced." />
</p>

<p align="center">
  <a href="Context/Plans/MiniApp.md"><img src=".github/Readme/link-miniapp.svg" height="30" alt="App design" /></a>
  &nbsp;
  <a href="Context/Plans/Providers.md"><img src=".github/Readme/link-providers.svg" height="30" alt="Providers" /></a>
  &nbsp;
  <a href="Context/Plans/OwnHarness.md"><img src=".github/Readme/link-own-harness.svg" height="30" alt="Own harness" /></a>
  &nbsp;
  <a href="Context/Plans/ConversationDisplay.md"><img src=".github/Readme/link-display.svg" height="30" alt="Conversation display" /></a>
  &nbsp;
  <a href="Context/Plans/NullMini.md"><img src=".github/Readme/link-phases.svg" height="30" alt="Future phases" /></a>
  &nbsp;
  <a href="Context/ADR/"><img src=".github/Readme/link-adrs.svg" height="30" alt="Decisions" /></a>
  &nbsp;
  <a href="Context/Research/Research.md"><img src=".github/Readme/link-research.svg" height="30" alt="Research map" /></a>
</p>

<a id="license"></a>
<p align="center">
  <img src=".github/Readme/license.svg" width="100%" alt="License: MIT, clone only voices you own or have permission to use, and the responsible-use, security and contribution files." />
</p>

<p align="center">
  <a href="LICENSE"><img src=".github/Readme/link-license.svg" height="30" alt="MIT" /></a>
  &nbsp;
  <a href="RESPONSIBLE_USE.md"><img src=".github/Readme/link-responsible.svg" height="30" alt="Responsible use" /></a>
  &nbsp;
  <a href="SECURITY.md"><img src=".github/Readme/link-security.svg" height="30" alt="Security" /></a>
  &nbsp;
  <a href="CONTRIBUTING.md"><img src=".github/Readme/link-contributing.svg" height="30" alt="Contributing" /></a>
</p>
