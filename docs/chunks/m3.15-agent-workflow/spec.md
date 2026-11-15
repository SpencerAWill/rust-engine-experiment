# M3.15: Agent workflow and lessons

- **Status:** Accepted (2026-11-15); amended 2026-11-15 (§4.10, questions one at a time)
- **Milestone:** M3 (not part of its exit criteria) · **Depends on:** M3.14 (merged in PR #56; the seed lesson of §4.8 cites its test helpers) · **Decides:** no ADR (process conventions, recorded in `AGENTS.md`, `CONTRIBUTING.md`, `docs/workflow.md` and `docs/README.md`)
- **Estimated size:** about 800 changed lines of Markdown, about 250 lines of Rust in `xtask` plus tests, and the chunk folders moved by `git mv` (§4.14)
- **Followed by:** M3.16, the documentation layout (§4.12)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

Agents on this project stop to ask the maintainer what to do next, read about 30 KB of process text per session of which most does not apply, and lose what they learned when a session ends or is killed. This chunk defines one loop (brainstorm, grill, plan, work, review, compound, close out) with two stop points, assigns each stage to an agent role with a pinned model, and records progress and lessons on disk as they happen. At the end, `AGENTS.md` is short and describes the loop, `.claude/` holds the roles, skills and a push guard, `docs/lessons/` holds lessons that the planning stage retrieves by path, and every chunk's documents live in one flat `docs/chunks/` instead of under milestone folders. M3.16 then reorganizes the rest of the documentation, as the first chunk built with this loop.

## 2. Goals and non-goals

**Goals**

- An agent can run a chunk from an approved spec to an open pull request without asking what to do next.
- Work and review are done by different agents; reviewers never see the builder's reasoning.
- Each agent role runs on a model chosen for it, pinned in its agent file. The main session's model is documented, not pinned: a model in the shared `settings.json` would override every contributor's choice.
- Progress and lessons survive a killed agent, because they are written to disk and committed as they happen.
- Lessons from one chunk reach the next chunk's plan and review, without loading every lesson into every session.
- The always-loaded instructions shrink.

**Non-goals**

- No change to the engine library, its API, frozen formats or CI-enforced code rules.
- No directory READMEs, moving of tool manuals, path-scoped rules or `check-docs`: those are M3.16 (§4.12).
- No compound-engineering plugin or other workflow plugin as a project dependency (§7).
- No per-subagent persistent memory (`memory:` in agent files); lessons are the memory (§7).
- No automation that starts chunks without a person (scheduled agents, `/loop`).
- No rewrite of existing specs, plans or ADRs; they stay as written.

**Anti-goals** (failure even if everything technically works)

- Lessons that grow into a second, unreviewed rulebook that every session loads.
- Agents that skip the spec stop point because the loop "knows what is next".
- A workflow that only Claude Code can follow: everything an agent reads MUST also be readable as plain Markdown by a person or another tool.

## 3. Guide-level explanation

The maintainer opens a session and runs `/chunk next`. The main session (Opus) reads the roadmap, proposes the first `Planned` chunk whose dependencies are done, and brainstorms it with the maintainer. It writes `spec.md`, then runs `/grill`: a fresh agent reads the spec cold and lists what it would have to guess, and the main session works through those and the remaining open decisions with the maintainer in rounds, writing each answer into the spec as it is given. When nothing is left open, the maintainer approves the spec.

The main session then continues without stopping. It asks the lessons researcher which lessons apply to the paths the spec touches and writes `plan.md` listing them. For each plan step it hands the step to a builder agent (Sonnet), which writes the test first, makes the change, appends to `log.md` and reports back. The main session checks the result, ticks the step and commits it with the log.

When the steps are done, a builder runs `/simplify` over the chunk's diff. Then the reviewers run in parallel, each with only the spec, the diff, its checklist and the relevant lessons. Findings go to a builder; a fresh reviewer of the same kind checks the fix. The main session promotes what `log.md` taught into `docs/lessons/` (adding or editing single bullets), opens the pull request with its Reviews, Lessons and Follow-ups sections, pushes the close-out commit, and stops for the merge go-ahead. Once the maintainer gives it, the main session merges, deletes the branch, and starts the next chunk's brainstorm.

If the session dies at any point, a new one runs `/chunk resume`: it reads the git log, the plan's ticked steps and `log.md`, runs `cargo xtask ci --quick`, and continues from the first unticked step.

## 4. Reference-level design

### 4.1 The loop

| Stage | Done by | Model | Produces | Stops for the maintainer |
|---|---|---|---|---|
| 1. Brainstorm | main session, with the maintainer | Opus | `spec.md` (Draft), `log.md` | No (the maintainer is in the conversation) |
| 2. Grill (§4.10) | main session, with the maintainer; cold reader | Opus; Opus | spec edits, `log.md` entries | **Yes: spec approval, at its end** |
| 3. Plan | main session; lessons researcher | Opus; Haiku | `plan.md` | No |
| 4. Work | builder, one per plan step | Sonnet | one commit per step | No |
| 5. Review | builder runs `/simplify`; then reviewers (§4.2); builder for fixes | Sonnet; §4.2 | a simplify commit; the PR's Reviews section | No |
| 6. Compound | main session | Opus | edits to `docs/lessons/` and other docs | No |
| 7. Close out | main session | Opus | PR, close-out commit, follow-up issues; after the go-ahead, the merge | **Yes: merge go-ahead** |

- The main session MUST NOT write product or test code in stages 4 and 5; it dispatches, checks, commits and keeps the documents. It MAY edit documentation directly.
- Stage 1 picks the next chunk as the first `Planned` row of the current milestone, in ID order, whose dependencies are `Done`; the maintainer MAY name another.
- An approved spec authorizes everything it states, including public API changes. Pushing the chunk's own branch, opening its pull request, and opening issues for follow-ups found during the chunk (labelled, and listed in the pull request's Follow-ups section) are part of stage 7 and need no further approval.
- **After the merge go-ahead** the main session, in the same turn: squash-merges the pull request once its required checks pass (`gh pr checks <n> --required --watch` exits 0; the admin can bypass the ruleset, so this wait is what enforces them; then `gh pr merge --squash --delete-branch`; it MUST NOT use `--admin` or otherwise bypass a check); reminds the maintainer to run `pulumi preview` and `pulumi up` if `infra/github/` changed; and starts stage 1 for the next chunk. If a check fails after the go-ahead, it does not merge: it fixes the failure on the branch as in stage 5 and asks for the go-ahead again.
- The simplify pass in stage 5 runs once over the whole chunk's diff, before any reviewer, and is committed on its own (`refactor: simplify (M<N>.<n>)`, or no commit if nothing changed) so reviewers can see what it did.
- Compounding is not only stage 6: every agent writes candidate lessons to `log.md` as they happen (§4.3). Stage 6 promotes them.
- **Escalations.** Outside the two stop points, an agent MUST stop and ask when: the spec is wrong or silent on something that changes what gets built (the fix is a spec amendment, grilled and approved like the spec); a change would touch an ADR, a frozen format or a CI-enforced policy that the spec does not name; a reviewer and the builder still disagree after two fix rounds; or anything outward-facing beyond what stage 7 allows (pushing another branch, merging without the go-ahead, publishing, commenting on other issues or pull requests).
- Long jobs that run mutated or untrusted code (mutants) run only by the procedure in §4.6, and the agent tells the maintainer when it starts one.
- **Plugins.** Techniques from installed plugins (for example superpowers' brainstorming, test-driven development, systematic debugging and verification skills) MAY be used inside any stage. Where a plugin's defaults differ from this workflow, the workflow wins: specs, plans and logs go in the chunk folder from the repository's templates, never in a plugin's own location (such as `docs/superpowers/`). Nothing in the repository requires a plugin to be installed.

### 4.2 Roles

Roles are Claude Code agent files in `.claude/agents/`, each with `name`, `description`, `model` and `tools`. A role file MUST link to the checklist it applies (in `CONTRIBUTING.md` or `docs/workflow.md`) rather than copying it.

| Role | File | Model | Tools | Gets |
|---|---|---|---|---|
| Builder (work, simplify, fixes) | `builder.md` | `sonnet` | all except spawning agents | the plan step, the spec, the step's lessons |
| Code reviewer | `review-code.md` | `opus` | read-only plus Bash | spec, diff, checklist, lessons |
| Security reviewer | `review-security.md` | `opus` | read-only plus Bash | as above |
| Conditional reviewer (public API, determinism, performance, portability; told which apply) | `review-conditional.md` | `opus` | read-only plus Bash | as above |
| Documentation reviewer | `review-docs.md` | `sonnet` | read-only plus Bash | as above |
| Spec conformance reviewer | `review-spec.md` | `sonnet` | read-only plus Bash | as above |
| Lessons researcher | `lessons-researcher.md` | `haiku` | read-only plus Bash | the list of paths and topics the spec touches (listed by the main session) |
| Cold reader (grill) | `spec-reader.md` | `opus` | read-only plus Bash | the spec path only |

- Reviewers MUST NOT edit files and MUST NOT receive the builder's or the main session's reasoning; the brief is the spec, the diff range, the checklist link and the lesson IDs.
- A re-review after a fix MUST be a fresh reviewer agent, not a continued one.
- Builders run in the chunk's working tree, one at a time. Agent files MUST NOT set `isolation: worktree` (it branches from `main`, not from the chunk).
- Each review's result goes in the pull request's Reviews section (one line each, or `n/a: <reason>`), as today, naming the agent file that did it.

### 4.3 Progress on disk: `log.md` and the plan's steps

Each chunk folder gets a `log.md`, copied from `docs/chunks/_template/log.md` when the spec is started.

- It is append-only during the chunk. Each entry is one or two lines: `- <date> <tag> <text>`, where tag is one of `done`, `decision`, `surprise`, `dead-end`, `lesson?`.
- Every agent (builder, reviewer through its report, main session) MUST record surprises, decisions and candidate lessons when they happen, not at the end. A builder appends before it reports back; the main session commits `log.md` with the step.
- Each plan step has a checkbox, ticked in the step's commit.
- **Resuming** (`/chunk resume`): read `git log` since the branch point, the plan's ticked steps and `log.md`; run `cargo xtask ci --quick`; continue from the first unticked step. If the working tree has uncommitted changes, the session reports them before touching anything. Before a plan exists, resuming means continuing the brainstorm or grill from the spec and `log.md`.
- At close-out `log.md` becomes `Historical`, like the plan.

### 4.4 Lessons

A lesson is guidance an agent follows by default and may deviate from when it records why. Lessons live in `docs/lessons/<topic>.md`, one file per topic, indexed by `docs/lessons/README.md`.

```markdown
---
applies_to:
  - "libs/engine/src/**/tests/**"
  - "libs/engine/tests/**"
---
# Lessons: testing

- Status: Living (2026-11-15)

- **testing-1.** Tests that walk an iterator or the `Debug` output of code under test use `bounded` / `debug_bounded`. *Why:* a mutant that makes `next` return `Some` forever made a test allocate without bound and froze the host (M3.14). *Deviate when:* the iterator is a std type. *From:* M3.14 · *Confirmed:* 2026-11-15
```

- **IDs** are `<topic>-<n>`, never reused. A retired lesson is deleted; git history is the archive.
- **Fields:** the lesson in one or two sentences; *Why*; *Deviate when* (optional); *Retire when* (optional: the change that makes it obsolete); *From* (chunk); *Confirmed* (date it was last found true).
- **Capture gate.** A candidate from `log.md` becomes a lesson only if it is non-obvious from the code, likely to recur, and costly to miss. If a test, lint or `xtask` check can enforce it, that is preferred, and the lesson is not written (or is deleted once the check exists).
- **Edits are deltas.** An agent adds, edits or deletes single bullets. It MUST NOT regenerate a lesson file. Before adding, it MUST search the existing lessons for one that covers the same point and edit that one instead.
- **Retrieval.** In stage 3 the lessons researcher matches the paths the spec touches against `applies_to` and returns the matching lesson IDs with one line each. The plan's Lessons section lists each ID as followed or deviated from, with the reason. Reviewers receive the same IDs. No session loads all lessons.
- **Upkeep.** At each milestone boundary, `/lessons-refresh` checks every lesson against the code: keep (update *Confirmed*), edit, merge, or delete. M3.16's `check-docs` adds a mechanical check that every `applies_to` pattern still matches a tracked file.
- **Seed lessons.** This chunk moves the project knowledge now held only in the maintainer's local Claude memory into lessons, skills or docs (§4.8), and writes nothing else as seed.

### 4.5 Documentation in this chunk

- **`AGENTS.md`** MUST stay under 80 lines and contain only: the commands; the loop and its stop points in a few lines, linking `docs/workflow.md`; the escalations; the CI-failing rules (as today, shortened); a short "working in X, read Y" table pointing at today's documents (M3.16 repoints it at directory READMEs); and the plugin statement of §4.1. No repository overview.
- **`CLAUDE.md`** at the root contains only `@AGENTS.md`. Claude Code skips `AGENTS.md` when a `CLAUDE.md` exists; the import makes both paths load the same content.
- **`docs/workflow.md`** (new, Reference): §4.1 to §4.4 and §4.10 for people and agents. `CONTRIBUTING.md`'s "How work is organized" links it instead of restating the process, and its "Reviews before a pull request" says the reviews are done by agents other than the builder, naming the agent files.
- **`docs/lessons/README.md`**: what a lesson is, the format of §4.4, and the index of lesson files.
- **`.claude/README.md`**: one line per file and directory in `.claude/`, and how to use `/chunk` and `/grill`.
- **Writing rule** (added to `docs/README.md` Conventions; applied by the documentation reviewer): write the minimum the reader needs; link instead of copying; a plan does not restate its spec; no overview of what the reader can see in the code.
- `docs/README.md` lists `workflow.md` and `lessons/`.

### 4.6 `.claude/` contents

- `agents/`: §4.2.
- `skills/chunk/`: `SKILL.md` (under 150 lines) dispatches `/chunk next`, `/chunk resume` and `/chunk <stage>`; one reference file per stage holds that stage's steps, so only the current stage's text is loaded.
- `skills/grill/`: §4.10.
- `skills/lessons-refresh/`: the upkeep pass of §4.4.
- `skills/bench-run/`: the wall-clock benchmark protocol (AC power, `caffeinate -i`, no builds during the run, power-log check afterwards), moved from local memory.
- `skills/mutants-run/`: running mutants in a memory-capped container (`--memory=4g --memory-swap=4g`, `-j 1`) with a host-side watchdog, moved from local memory. The container recipe was lost with a session scratchpad and is rebuilt in this chunk, then tried once with `cargo xtask mutants --module random` in Docker Desktop on the maintainer's Mac; the agent says when it starts the run, and records its output in `log.md`.
- Each skill is tested before it is relied on, by running an agent against it (the method of superpowers' `writing-skills`), and the result is noted in `log.md`.
- `settings.json`:
  - a permission allowlist of read-only commands, built with `/fewer-permission-prompts` from the maintainer's past sessions and then reviewed by hand (nothing that writes, pushes or deletes);
  - a `PreToolUse` hook on Bash that blocks `git push` to `main` (including `HEAD:main` refspecs) and any force push (`--force`, `-f`, `--force-with-lease`, `+refspec`). The hook is a POSIX `sh` script, `.claude/hooks/guard-push.sh`, that exits 0 at once unless the input contains `push`, and otherwise hands the JSON to `cargo xtask guard-push`, which parses it with `serde_json` and decides. The decision logic is Rust with unit tests (inputs it must block and must allow), so it runs on every CI platform and needs no `jq`.
- `README.md`: §4.5.
- `.gitignore` gains `.claude/worktrees/`, `.claude/settings.local.json` and `.claude/agent-memory-local/`.

### 4.7 (Moved to M3.16)

The `check-docs` command first drafted here is part of M3.16 (§4.12). The number is kept so later sections keep theirs.

### 4.8 Memory migration

The maintainer's local Claude memory for this project holds project knowledge no other contributor can see. In this chunk:

| Memory | Goes to |
|---|---|
| Benchmark run protocol | `skills/bench-run/` |
| Long jobs need memory caps | `skills/mutants-run/`, and lesson `testing-1` (§4.4) |
| One PR per chunk, close-out in it | `docs/workflow.md` (stage 7) |
| Direct push to main: one-off only | the push guard (§4.6) |
| No copying other engines' code | already ADR 0002; nothing to add |
| Engine project goals | already in the roadmap, ADRs and specs; nothing to add |

Personal preferences (tone, collaboration style, "grill me", DX over API stability) stay in local memory. After the pull request merges, the migrated memory files are deleted or reduced to a pointer; that edit is outside the repository and is not part of the diff.

### 4.9 Templates and the pull request

- `docs/chunks/_template/plan.md`: a checkbox per step; a Lessons section (IDs followed or deviated from, with reasons); a length note.
- `docs/chunks/_template/spec.md`: a length note; sections 3, 7 and 11 marked optional for small chunks.
- `docs/chunks/_template/log.md`: new (§4.3).
- `.github/PULL_REQUEST_TEMPLATE.md`: a Reviews section (the eight reviews, one line each, naming the agent) a Lessons section (lessons added, edited or deleted, by ID) and a Follow-ups section (issues opened, by number) replace the checklist lines they duplicate.

### 4.10 Grilling the spec

A skill, `/grill`, written for this repository from the method of Matt Pocock's `grilling` skill (MIT; cited, not copied, per ADR 0002). It runs after the spec is drafted and before it is approved, with the maintainer in the conversation.

1. **Cold read.** Dispatch the cold reader (§4.2) with only the spec's path. It returns the questions a builder would have to guess at, where a wrong guess changes what gets built, ranked.
2. **Sort.** The main session answers from the repository or the conversation what it can, and writes the answer into the spec. It settles minor points itself and marks them in the spec as its call. What is left are decisions only the maintainer can make.
3. **Questions.** One at a time by default, each with the main session's recommended answer and its trade-off, worded so that "yes" accepts it, the next built on the answer. Up to three MAY be batched only when they are independent: no possible answer to one changes whether another is asked, what its options are, or what is recommended for it; the main session says why before batching, and asks one at a time when unsure. Facts are looked up (by the main session or a read-only agent), never asked. Questions probe the design; they never quiz the maintainer on it. *(Amended 2026-11-15 by the maintainer: rounds of every open question became one at a time, because dependent questions in one batch can be made irrelevant by an unexpected answer.)*
4. **Record as it goes.** Each answer is written into the spec and noted in `log.md` before the next question, so a killed session loses nothing.
5. **End.** When no decision is open, the main session lists the calls it made and asks for spec approval. That approval is the stop point of §4.1.

The skill MAY also be invoked on a plan, an ADR or any design document; the steps are the same.

### 4.11 Keeping the workflow current

The milestone checklist in `docs/maintainers.md` gains two items: run `/lessons-refresh`, and, if a new model shipped since the last milestone, re-check the model assigned to each role in §4.2 against one chunk's results.

### 4.12 M3.16: documentation layout (scope handed on)

Recorded here so the roadmap row and M3.16's brainstorm start from what was agreed; M3.16's own spec decides the details.

- A `README.md` in each directory that matters, in one shape: one line on what it is, "When this applies", "Contents" (an index of the directory), and an optional "How to".
- The tool manuals in `CONTRIBUTING.md` (benchmarks, instruction-count gate, fuzzing, coverage, mutation testing, spikes) move to the README of the directory they describe, shortened, with no rule lost.
- `.claude/rules/*.md` with `paths:` frontmatter route Claude Code to the right README and lessons when it touches matching files.
- `cargo xtask check-docs`, a CI gate: every required directory has a README; each README's Contents matches the directory; lesson files are well formed and their `applies_to` patterns match tracked files; `AGENTS.md` stays under 80 lines.
- `AGENTS.md`'s "working in X, read Y" table is repointed at the READMEs.

### 4.13 Bootstrapping this chunk

This chunk follows the new loop from its spec approval on: no plan stop. Its brainstorm and grill are the conversation of 2026-11-15, recorded in `log.md`. The plan's first step writes the agent files, and every later step is built by a builder and reviewed by the new reviewers. If Claude Code does not pick up the new agent files without a restart, the main session dispatches general-purpose agents with the agent file's model and body as their prompt, and notes it in `log.md`.

### 4.14 Chunk folders

- Every chunk's documents live in `docs/chunks/<id>-<slug>/` (for example `docs/chunks/m3.15-agent-workflow/`), flat, with the templates in `docs/chunks/_template/`. Milestones are goals with exit criteria in the roadmap and a field in each spec's header; they no longer group folders.
- Chunk IDs keep their form, `M<N>.<n>`, never reused or renumbered. `N` is the milestone that was current when the chunk was added, not a claim that the chunk serves its exit criteria; the roadmap's milestone tables keep listing chunks under the milestone in which they were added.
- This chunk moves every existing folder under `docs/design/` (including `m0-m1-foundations/`, which keeps its name) into `docs/chunks/` with `git mv`, so `git log --follow` keeps each file's history, and removes `docs/design/`.
- Every link to a moved path is updated, in 8 files outside the folders (41 references when counted on 2026-11-15), including ADRs 0003, 0004 and 0012. Changing a link path in an accepted ADR does not change its decision and is allowed; the pull request says so.
- `docs/README.md`'s table and Conventions describe `docs/chunks/` instead of `docs/design/`.

## 5. Requirements

- **M3.15-R1** `AGENTS.md` MUST describe the loop, its two stop points (spec approval, merge go-ahead), the escalations and the plugin rule of §4.1, and MUST stay under 80 lines.
- **M3.15-R2** `docs/workflow.md` MUST describe §4.1 to §4.4 and §4.10, and `CONTRIBUTING.md` MUST link it instead of restating the process.
- **M3.15-R3** `.claude/agents/` MUST define the eight roles of §4.2 with the models given there; reviewer, researcher and cold reader roles MUST NOT have edit tools.
- **M3.15-R4** The `chunk` skill MUST implement `next`, `resume` and per-stage invocation, with stage text in separate reference files, and MUST run the simplify pass of §4.1 before the reviewers.
- **M3.15-R5** `resume` MUST reconstruct state from git, the plan's checkboxes and `log.md` only (§4.3).
- **M3.15-R6** The templates MUST include `log.md`, plan checkboxes and the plan's Lessons section (§4.9).
- **M3.15-R7** `docs/lessons/` MUST follow §4.4, and the planning stage MUST retrieve lessons by `applies_to`.
- **M3.15-R8** The push guard MUST block pushes to `main` and force pushes, MUST allow other pushes, and its unit tests MUST run in `cargo xtask test` on every CI platform.
- **M3.15-R9** The project knowledge in §4.8 MUST be in the repository, and the mutants container procedure MUST complete one run of `--module random` with a report.
- **M3.15-R10** The CLAUDE.md audit (`claude-md-management:claude-md-improver`, a one-off check in this chunk, not a plugin contributors need) MUST be run against `AGENTS.md` and `CLAUDE.md` after the rewrite, and each finding fixed or answered in the pull request.
- **M3.15-R11** This chunk's own reviews MUST be run with the new reviewer agents.
- **M3.15-R12** The `grill` skill MUST implement §4.10, and the `chunk` skill MUST run it between brainstorm and spec approval.
- **M3.15-R13** The roadmap MUST list M3.15 and M3.16, with M3.16's row carrying the scope of §4.12.
- **M3.15-R14** Chunk folders MUST be moved as in §4.14, with no relative link to a moved path left broken.
- **M3.15-R15** The `chunk` skill MUST implement the merge and next-chunk steps of stage 7, MUST NOT bypass required checks, and MUST list follow-up issues it opened in the pull request.

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no library code changes.
- **`unsafe`:** none; `xtask` keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because none is added.
- **Heap allocation:** n/a, because no library code changes.
- **`no_std` and wasm:** n/a, because only `xtask`, `.claude/` and docs change.
- **Frozen formats:** none touched.
- **Public API changes:** none.
- **FFI exposure:** none.
- **Performance:** the push guard's `sh` pre-filter keeps `cargo` off every Bash call that is not a push; the plan measures the time of a push check.
- **Documentation:** §4.5 and §4.9.
- **Policy changes:** yes. The stop points change from "spec and plan reviewed together, then merge" to "spec (after a grill), then merge"; reviews must be done by agents other than the builder, after a simplify pass; lessons may be edited directly in a chunk's pull request and are listed in its Lessons section; plugin defaults yield to the workflow; the main session merges after the maintainer's go-ahead and may open follow-up issues; chunk documents move from milestone folders to `docs/chunks/`.

## 7. Alternatives considered

- **Install the compound-engineering plugin** (EveryInc). It has the researcher, overlap check and refresh step, but brings about 38 skills and 50 agents into every session's listing, uses its own plan format and `docs/solutions/` layout, overlaps with superpowers, changes under us on plugin updates, and is not installed for someone who clones the repository. Its ideas are adopted in §4.4; its code is not used.
- **Plugin for review and compound, our own for the rest.** Two vocabularies and two places for lessons, for little gain.
- **Keep the plan review as a stop point.** It adds a wait to every chunk; the spec is where the maintainer's intent matters most, and the plan is reviewed at merge with the rest.
- **Lessons in an always-loaded file** (a section of `AGENTS.md`). Measured context files raise cost and do not raise success much (ETH Zurich, 2026); lessons that load only by path keep the always-loaded part small.
- **Per-subagent persistent memory** (`memory: project`). It splits knowledge by role, is Claude-specific, and is loaded at every start of that role; lessons by path are shared and readable by people.
- **Install or copy Matt Pocock's `grill-me` / `grilling` skills.** The plugin brings many skills and updates itself outside a pull request; a copy would need an exception to ADR 0002. The method is adopted in §4.10 instead.
- **Regenerating lesson files with an LLM to keep them tidy.** ACE (ICLR 2026) measured that a full rewrite collapsed a playbook and dropped accuracy below no playbook at all; edits are deltas.
- **One chunk for workflow and documentation layout.** About 1,500 lines of Markdown and 400 of Rust, at the top of the chunk size; split so M3.16 is built entirely with the new loop.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `wc -l AGENTS.md`; documentation review |
| R2 | documentation review; spec conformance review |
| R3 | spec conformance review reads each agent file's frontmatter |
| R4, R5 | a resume trial in this chunk: midway through the work stage, a fresh agent with no conversation context follows the `resume` procedure read-only and reports the step it would continue from and any uncommitted changes; `log.md` records whether that matches the true state. A real killed session, when one first happens, is recorded the same way |
| R6 | documentation review |
| R7 | spec conformance review against §4.4; M3.16's plan lists retrieved lesson IDs (a follow-up check noted in `log.md`) |
| R8 | `cargo xtask test` (the guard's unit tests); a manual trial of the hook in a session, recorded in `log.md` |
| R9 | `log.md` entry for the `--module random` container run with its summary table; spec conformance review against §4.8 |
| R10 | the audit's output and responses in the pull request |
| R11 | the Reviews section names the agent file used for each review |
| R12 | this spec's grill, recorded in `log.md`; spec conformance review reads the skill against §4.10 |
| R13 | the roadmap diff |
| R14 | `git grep 'design/m[0-9]'` finds nothing; a link check over every Markdown file in the repository (a one-off script in the plan) reports no broken relative link |
| R15 | spec conformance review reads the skill against §4.1; this chunk's own merge, done by the main session after the go-ahead, recorded in `log.md` |

## 9. Done when

- [ ] `cargo xtask ci` passes, including the push guard's tests.
- [ ] Every requirement in §5 has its evidence from §8.
- [ ] The resume trial, the hook trial, the skill trials and the mutants container run are recorded in `log.md`.
- [ ] The CLAUDE.md audit was run and answered.
- [ ] The pull request lists all eight reviews, done by the new agents, and a Lessons section.
- [ ] The spec is `Implemented`, the plan and log are `Historical`, the roadmap row is `Done`.

## 10. Open questions

None.

## 11. Prior art and sources

- ETH Zurich, "Evaluating AGENTS.md", [arXiv 2602.11988](https://arxiv.org/abs/2602.11988) (2026): context files raise cost by over 20% and help little; hand-written beats generated; overviews do not help.
- "Agentic Context Engineering", [arXiv 2510.04618](https://arxiv.org/abs/2510.04618) (ICLR 2026): itemized playbooks updated by deltas; full rewrites cause context collapse.
- "Codified Context", [arXiv 2602.20478](https://arxiv.org/abs/2602.20478) (2026): a small always-loaded file, specialist agents, on-demand specs; stale docs caused conflicting code.
- EveryInc, [compound-engineering-plugin](https://github.com/EveryInc/compound-engineering-plugin) (checked 2026-11-15): the loop, a simplify step before review, `docs/solutions/` with frontmatter, a learnings researcher in planning, overlap check, capture gate, refresh.
- Matt Pocock, [`grilling` skill](https://github.com/mattpocock/skills/tree/main/skills/productivity/grilling) (MIT, checked 2026-11-15): a design tree worked in rounds of questions with recommended answers; facts looked up, decisions asked.
- Anthropic, [Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) (2025), [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) (2025), [Harness design for long-running apps](https://www.anthropic.com/research/harness-design-long-running-apps) (2026), [Components of agent memory](https://www.anthropic.com/research/components-of-agent-memory) (2026): progress files and commits per session, separate evaluator, re-test harness parts when models change, keep memory to what cannot be re-derived.
- Claude Code docs: [memory](https://code.claude.com/docs/en/memory) (AGENTS.md loading, path-scoped rules, under 200 lines), [subagents](https://code.claude.com/docs/en/sub-agents), [skills](https://code.claude.com/docs/en/skills).
