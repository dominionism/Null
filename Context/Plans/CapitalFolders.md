# Capitalized folders and repo cleanup

> Recorded 2026-10-08 from the handoff of the session that did the work; not a `/blueprint`.
> Carries out `Context/ADR/0001-CapitalizedFolderNames.md`.
>
> Status: **Built on branch `capital-folders` (four commits on `171960b`); not merged, not pushed.
> The working copy still has the old names. Switching over waits for the user's go.**

## Goal

Every folder capitalized (ADR 0001) and the files Null does not need removed, without taking down
the Voice server or the Voice desktop on the way.

## Constraints

- **Verbatim (user):** "I want folders to be Capital here. For instance Mini/". The user chose the
  widest reading: every folder at every depth.
- **Verbatim (user):** "Let's keep the house clean here."
- Nothing is merged, switched or pushed until the user asks.
- The Null app is not at risk from the switch: it is installed in `/Applications` and its login
  item points there, not into this repo.

## Work items

1. **Remove what Null does not need**
   - Status: Complete on the branch (2026-10-08)
   - **Built:** `4ee80ac` Voicebox's websites (`docs/`, `landing/`), `b5d0566` Docker support
     files, `86d53fa` upstream paperwork and release tooling. 204 files deleted.
   - **Kept:** `CHANGELOG.md` (open decision 2).
   - **Moved, not deleted:** three README screenshots from `landing/public/assets/` to
     `.github/Assets/`, and `docs/content/docs/developer/tts-engines.mdx` to
     `.agents/skills/add-tts-engine/`.
   - **Left behind:** `README.md` still offers Docker in two places (lines 87 and 98).

2. **Rename every folder**
   - Status: Complete on the branch (2026-10-08)
   - **Built:** `eda74f8`. 632 files moved, 14 edited; with item 1, 861 tracked files became 657.
     The top level is `App/`, `Backend/`, `Context/`, `Data/`, `Scripts/`, `Tauri/`, `Web/`.
   - **Checks, as reported by the agent that did it and not re-run since:** pytest 133 passed on
     the nine named files; a wider run 297 passed with the same 5 failures as before; ruff,
     typecheck, the web build and `cargo check` unchanged.

3. **Bring the branch up to date**
   - What: rebase `capital-folders` onto `null-mini`, or merge. Apply the folder rule to anything
     new. Correct the paths named in `MiniApp.md` and `NullMini.md` (`tauri/src-tauri/` becomes
     `Tauri/SrcTauri/`, `backend/` becomes `Backend/`).
   - Depends on: the user's go (open decision 1).
   - Risk: `Mini/` is new and already follows the rule, so it will not conflict. The prototype has
     since been removed on `null-mini` (`MiniApp.md` item 9): two files deleted and five put back
     to their first-commit content. The branch renamed those same files, so the rebase will meet
     them: the two deleted files have to stay deleted, by hand, and the other five carry no
     prototype lines.
   - Status: Not started

4. **Switch the working copy**
   - What, in order:
     1. Quit the Voice desktop, then run the old `scripts/voicebox stop`. The new script does not
        recognise a server started as `backend.main`.
     2. On `null-mini` with a clean tree: `git merge --ff-only capital-folders`. A plain
        `git switch` is refused while the worktree holds the branch. If `git status` then shows
        deleted files, `git restore .`.
     3. Fix the names on disk: `git ls-tree -r -d --name-only HEAD > <a temp file>`, then
        `python3 Memories/rename-tools/fix_case.py <that file>`, with `--dry-run` first.
     4. Move what sits under folders that changed name: `Tauri/src-tauri/binaries/*` into
        `Tauri/SrcTauri/Binaries/` (or `bun run setup:dev` for placeholders), and
        `Tauri/src-tauri/target` to `Tauri/SrcTauri/target` (or accept a full rebuild). Then delete
        what is left of `Tauri/src-tauri`, `Backend/mcp_server`, `Backend/mcp_shim`,
        `Backend/pyi_hooks`, `docs/` and `landing/`.
     5. `bun install`.
     6. `Scripts/voicebox install`, then `Scripts/voicebox start`.
     7. In `~/.zshrc` line 22, the `vb` alias: `…/Null/scripts/voicebox` becomes
        `…/Null/Scripts/voicebox`.
     8. Optional: recreate `Backend/venv`. Its scripts carry old-case paths, which this disk still
        resolves.
     9. `git worktree remove .claude/worktrees/agent-a07675f34ba96cb77`.
   - Depends on: 3, and the user's go.
   - Risk:
     - Step 2 was never rehearsed; the permission system refused the rehearsal.
     - Git leaves the old-case name wherever a folder holds untracked files, and Python then cannot
       import `Backend`. Step 3 is the fix.
     - The server's login item (`~/Library/LaunchAgents/dev.voicebox.server.plist`) runs
       `backend/venv/bin/python … uvicorn backend.main:app` with KeepAlive, and would crash-loop on
       the renamed tree until step 6 rewrites it.
   - Source: the agent that did the rename (the steps), checked against the repo on 2026-10-08
     (the branch, the login item, the alias).
   - Status: Not started

5. **After it lands**
   - What: trim ADR 0001's exceptions (the docs and landing URL folders and the Next.js routing
     folders have no instance left). Push, and merge into `main`, when the user asks.
   - Status: Not started

## Open decisions (user-owned)

1. **When to switch.** The first reason to wait is gone: the box no longer depends on the Voice
   desktop. What is left is the order against removing the prototype (item 3's risk).
2. **`CHANGELOG.md`.** `app/plugins/changelog.ts` reads it at build time for Settings > Changelog,
   so deleting the file alone breaks the web build and CI, as the cleanup found. Keep it; delete
   it and let the page go blank; or delete the page too.
3. **The four moved files.** Whether moving them instead of deleting them is acceptable.

## Pointers

- Branch `capital-folders` at `eda74f8`, in the worktree `.claude/worktrees/agent-a07675f34ba96cb77`.
  It was branched from `171960b` by hand: the worktree itself was created at the initial commit.
- `Memories/rename-tools/`: the scripts the rename was done with. `Memories/` is not in git.
- `.gitignore`'s last two lines are UTF-16, so its `.claude/settings.local.json` rule does nothing.
  `.claude/` is untracked and holds the worktree; it must never be added.
