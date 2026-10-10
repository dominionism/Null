# Capitalized folders and repo cleanup

> Recorded 2026-10-08 from the handoff of the session that did the work; not a `/blueprint`.
> Carries out `Context/ADR/0001-CapitalizedFolderNames.md`.
>
> Status: **Built on branch `capital-folders`: five commits on `abb114d`, tip `4793f61`, rebuilt
> there on 2026-10-09. Not merged, not pushed. `main` has moved 16 commits since, so item 3 has to
> be done once more. The working copy still has the old names. The owner said "finish it now" on
> 2026-10-09.**

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
   - **Built:** `8d748d1` Voicebox's websites (`docs/`, `landing/`), `51e1258` Docker support
     files, `fe178c9` upstream paperwork and release tooling. 204 files deleted. The ids are those
     after the rebases of 2026-10-09 (item 3).
   - **Kept:** `CHANGELOG.md` (open decision 2).
   - **Moved, not deleted:** three README screenshots from `landing/public/assets/` to
     `.github/Assets/`, and `docs/content/docs/developer/tts-engines.mdx` to
     `.agents/skills/add-tts-engine/`.
   - **Left behind:** `README.md` on the branch still offers Docker in two places (lines 87 and
     98). The README on `main` was rewritten since and does not mention Docker, so nothing is
     fixed here: `main`'s file is taken when the branch is brought up to date (item 3).

2. **Rename every folder**
   - Status: Complete on the branch (2026-10-08)
   - **Built:** `b553674` (`eda74f8` before the rebases). By git's count 629 files moved and 15
     edited; with item 1, the 890 tracked files of `abb114d` become 686.
     The top level is `App/`, `Backend/`, `Context/`, `Data/`, `Scripts/`, `Tauri/`, `Web/`.
   - **Checks, as reported by the agent that did it:** pytest 133 passed on the nine named files;
     a wider run 297 passed with the same 5 failures as before; ruff, typecheck, the web build and
     `cargo check` unchanged. Run again on 2026-10-09 after the rebase (item 3).

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
   - Status: In progress. Done once on 2026-10-09 against `abb114d`; to be done again against
     `main`
   - **Done (2026-10-09), by the agent that did it:** rebased onto `null-mini` at `abb114d`. Three
     conflicts, all in the rename commit: the two prototype files stay deleted, and two passages
     of `NullMini.md` keep `null-mini`'s wording with `app/` as `App/`. Six paths corrected in
     `MiniApp.md` (`4793f61`). The folder rule holds on the whole tree: 110 tracked folders, 73
     capitalized, 37 exceptions with a stated reason; the only new exceptions are `Mini/src` and
     `Mini/capabilities`.
   - **Checks, the same on the branch as on its base:** Python tests 297 passed, 5 failed, 4
     skipped (the same 5: four need MLX, which is not installed, and
     `test_progress.py::test_hf_progress_tracker`); `ruff check` 1082 findings on both; typecheck
     and the web build pass, the built web files identical; `cargo check` for the Voice desktop
     passes; `cargo test` in `Mini/` 51 passed, 2 ignored.
   - **Still to do:** the working copy is now on `main`, which is 16 commits ahead of the
     branch's base. `README.md` was rewritten there and three branch commits change that file, so
     the next rebase will conflict in it: take `main`'s.

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
   - **Corrected by a rehearsal (2026-10-09):** steps 2 to 5 were run in scratch copies of the
     repo, and the list above is wrong in places (13 folders keep their old names after step 2;
     `Backend/pyi_hooks` and `docs/` are not left behind; `landing/` is). The corrected list,
     with an undo, is in `Memories/handoff-2026-10-09-0250-cleanup.md` under "Next action" and
     replaces the list above when this item is taken up. It names `null-mini`; the working copy
     is now on `main`. Never run anywhere: steps 1, 6, 7 and 9 (the server, its login item,
     `~/.zshrc`, removing the worktree).
   - Risk:
     - Step 2 was rehearsed on 2026-10-09 in a scratch copy only, never on the working copy.
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

1. **When to switch.** Settled by the owner on 2026-10-09: "finish it now". Each step that
   changes the working copy, the server or `~/.zshrc` is still done only when the owner asks.
2. **`CHANGELOG.md`.** `app/plugins/changelog.ts` reads it at build time for Settings > Changelog,
   so deleting the file alone breaks the web build and CI, as the cleanup found. Keep it; delete
   it and let the page go blank; or delete the page too. The owner said "I approve the
   CHANGELOG.md" on 2026-10-09. That was read as: keep the file. The reading was not confirmed
   with the owner.
3. **The four moved files.** Whether moving them instead of deleting them is acceptable.
   Recorded in the cleanup session's handoff as settled by the owner on 2026-10-09: they stay
   moved.

## Pointers

- Branch `capital-folders` at `4793f61`, in the worktree `.claude/worktrees/agent-a07675f34ba96cb77`.
  It was branched from `171960b` by hand (then at `eda74f8`) and rebuilt on `abb114d` on
  2026-10-09: the worktree itself was created at the initial commit.
- `Memories/handoff-2026-10-09-0250-cleanup.md`: the rebase, the checks, the rehearsal and the
  corrected steps for the switch.
- `Memories/rename-tools/`: the scripts the rename was done with. `Memories/` is not in git.
- `.gitignore`'s last two lines are UTF-16, so its `.claude/settings.local.json` rule does nothing.
  `.claude/` is untracked and holds the worktree; it must never be added.
