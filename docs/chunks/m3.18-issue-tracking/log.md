# M3.18 log: Issue tracking

- **Status:** Living (2026-11-16)

<!-- Append-only. One or two lines per entry, written when it happens, not at the end:
- <date> <tag> <text>
Tags: done, decision, surprise, dead-end, lesson? (a candidate lesson for stage 6; see docs/lessons/README.md). -->

- 2026-11-16 decision Chunk ID M3.18: M3.17 (hierarchical bitset) is in flight on the main checkout and not yet on main; this branch is a worktree from main. Expect a trivial roadmap merge conflict (both add a row after M3.16).
- 2026-11-16 decision Brainstorm happened in conversation before `/chunk next`: Project for Status/Priority/Size, labels for type and area, tracking issues instead of GitHub Milestones, one chunk = one PR with multi-PR work as chunk groups, `/triage` machine-light and propose-then-confirm. Spec section 4 records it.
- 2026-11-16 decision Used the chunk folder, not the brainstorming skill's `docs/superpowers/specs/`, per the workflow's plugin rule.
- 2026-11-16 surprise Milestones: `gh api milestones` returns none; Projects are disabled in `infra/github/index.ts` (comment: "Unused"; M3.9 turned them off), so the Project needs that setting reversed. The `gh` token lacks `read:project`.
- 2026-11-16 decision Grill round 1, main session's calls from the cold read: type labels are bug/enhancement/infrastructure/documentation/research (exactly one), performance/determinism additive, no label retired; area labels for code only; no Parked status, research issues are not Project items; /chunk stage 1 reads the issue and sets Scheduled; /triage's allowed writes listed; R4 tested with a throwaway Project; migration limited to open issues plus M3 and M4 tracking issues.
- 2026-11-16 surprise A web search did not settle whether `hasProjects: false` blocks a user-owned Project linked to the repository; it must be tested, not assumed.
- 2026-11-16 decision Maintainer: `pulumi up` before the merge is fine (nothing else is under Pulumi). Rollout order is stage 7, before the go-ahead (spec 4.10); no split, no follow-up PR; `hasProjects` is tested first and flipped in this PR if needed.
- 2026-11-16 decision Maintainer asked whether the roadmap should be a GitHub Project; recommended keeping `docs/roadmap.md` (versioned with the PR, read offline by agents, chunks without issues, prose exit criteria); awaiting confirmation.
- 2026-11-16 decision Maintainer approved the spec as it stands, after being told the Project, area labels, ai-assisted and forms are the least certain value (assessed in conversation: keep vocabulary, tracking issues, Definition of Ready, /triage; Project is a bet on a growing backlog).
- 2026-11-16 decision Plan written: no builder (documentation, skill and configuration only); steps 1-5 on the branch, 6-9 are the stage 7 rollout with their own evidence steps. Lessons docs-1, specs-1, specs-2, tooling-7 followed (lessons-researcher dispatched through the registered agent).
- 2026-11-16 done Step 1: nine labels declared in `infra/github/index.ts`; each name matched once. TypeScript not type-checked locally: `infra/github/node_modules` is absent in the worktree and installing was skipped; step 7's `pulumi preview` is the check.
- 2026-11-16 done Step 2: bug, feature and infrastructure templates are now YAML forms (labels unchanged; feature and infrastructure gain required Done-when and verification fields, an optional Depends-on field and an assistant checkbox). The check failed on the three .md files first, then passed; ids are unique and every input has a label. GitHub's own rendering of the forms is not checked until the branch is pushed (R5 asks for the chooser): do it in stage 7.
- 2026-11-16 lesson? A form-structure check needs a YAML parser; this machine's Python has none (`yaml` missing), Ruby's is built in.
