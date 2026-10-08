# Folder names are capitalized

Every folder in this repo is named in PascalCase (`Mini/`, `Backend/`, `McpServer/`), at every depth,
matching `Context/`, `Memories/` and the owner's other projects. The owner decided this on 2026-10-08
so the tree reads one consistent way. It applies to new folders immediately and to existing ones
through a single rename pass.

A folder keeps another name only where something outside our control fixes it:

- A tool looks for it or generates it by name: `.git`, `.github`, `node_modules`, `target`, `dist`,
  `venv`, `__pycache__`, a Rust crate's `src`, Tauri's `capabilities` and `gen`, a web app's `public`,
  Next.js routing folders.
- The name is part of a public address or identifier: URL segments of the docs and landing sites,
  locale codes such as `en` or `zh-CN`.
- It is a hidden folder: `.claude`, `.agents`.

Python packages are not an exception: they are renamed and their imports follow. Any other exception
must name the tool or address that forces it.
