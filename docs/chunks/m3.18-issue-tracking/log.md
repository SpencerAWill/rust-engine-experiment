# M3.18 log: Issue tracking

- **Status:** Living (2026-11-16)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-16 decision Chunk ID M3.18: M3.17 (hierarchical bitset) is in flight on the main checkout and not yet on main; this branch is a worktree from main. Expect a trivial roadmap merge conflict (both add a row after M3.16).
- 2026-11-16 decision Brainstorm happened in conversation before `/chunk next`: Project for Status/Priority/Size, labels for type and area, tracking issues instead of GitHub Milestones, one chunk = one PR with multi-PR work as chunk groups, `/triage` machine-light and propose-then-confirm. Spec section 4 records it.
- 2026-11-16 decision Used the chunk folder, not the brainstorming skill's `docs/superpowers/specs/`, per the workflow's plugin rule.
- 2026-11-16 surprise Milestones: `gh api milestones` returns none; Projects are disabled in `infra/github/index.ts` (comment: "Unused"; M3.9 turned them off), so the Project needs that setting reversed. The `gh` token lacks `read:project`.
