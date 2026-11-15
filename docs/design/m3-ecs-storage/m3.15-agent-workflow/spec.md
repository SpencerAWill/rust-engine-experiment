# M3.15: Agent workflow, lessons and documentation layout

- **Status:** Draft (2026-11-15)
- **Milestone:** M3 (not part of its exit criteria) · **Depends on:** M3.14 (PR #56; this branch is rebased onto `main` after it merges, because the seed lesson of §4.8 cites its test helpers) · **Decides:** no ADR (process conventions, recorded in `AGENTS.md`, `CONTRIBUTING.md`, `docs/workflow.md` and `docs/README.md`)
- **Estimated size:** about 1,500 changed lines of Markdown (much of it moved, not new), about 400 lines of Rust in `xtask` plus tests, and one shell hook

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

Agents on this project stop to ask the maintainer what to do next, read about 30 KB of process text per session of which most does not apply, and lose what they learned when a session ends or is killed. This chunk defines one loop (brainstorm, plan, work, review, compound, close out) with two stop points, assigns each stage to an agent role with a pinned model, records progress and lessons on disk as they happen, and reorganizes the documentation so each reader loads only what applies. At the end, `AGENTS.md` is a short router, `.claude/` holds the roles, skills and path-scoped rules, `docs/lessons/` holds lessons that the planning stage retrieves, and `cargo xtask check-docs` keeps the indexes honest.

## 2. Goals and non-goals

**Goals**

- An agent can run a chunk from an approved spec to an open pull request without asking what to do next.
- Work and review are done by different agents; reviewers never see the builder's reasoning.
- Each agent role runs on a model chosen for it, pinned in its agent file. The main session's model is documented, not pinned: a model in the shared `settings.json` would override every contributor's choice.
- Progress and lessons survive a killed agent, because they are written to disk and committed as they happen.
- Lessons from one chunk reach the next chunk's plan and review, without loading every lesson into every session.
- A reader (person or agent) can find what applies to the directory they are working in, from that directory's README.
- Less text: the always-loaded instructions shrink, and tool manuals move next to the tools.

**Non-goals**

- No change to the engine library, its API, frozen formats or CI-enforced code rules.
- No compound-engineering plugin or other workflow plugin as a project dependency (§7).
- No per-subagent persistent memory (`memory:` in agent files); lessons are the memory (§7).
- No automation that starts chunks without a person (scheduled agents, `/loop`).
- No rewrite of existing specs, plans or ADRs to the new layout; they stay as written.

**Anti-goals** (failure even if everything technically works)

- Lessons that grow into a second, unreviewed rulebook that every session loads.
- Agents that skip the spec stop point because the loop "knows what is next".
- A layout that only Claude Code can use: everything an agent reads MUST also be readable as plain Markdown by a person or another tool.

## 3. Guide-level explanation

The maintainer opens a session and runs `/chunk next`. The main session (Opus) reads the roadmap, proposes the first `Planned` chunk whose dependencies are done, and brainstorms it with the maintainer. It writes `spec.md` and stops. The maintainer approves the spec.

The main session then runs `/chunk resume`. It asks the lessons researcher which lessons apply to the paths the spec touches, writes `plan.md` listing them, and starts the work without stopping. For each plan step it hands the step to a builder agent (Sonnet), which writes the test first, makes the change, appends to `log.md` and reports back. The main session checks the result, ticks the step and commits it with the log.

When the steps are done it dispatches the reviewers in parallel, each with only the spec, the diff, its checklist and the relevant lessons. Findings go to a builder; a fresh reviewer of the same kind checks the fix. Then the main session promotes what `log.md` taught into `docs/lessons/` (adding or editing single bullets), opens the pull request with its Reviews and Lessons sections, pushes the close-out commit, and stops for the merge go-ahead.

If the session dies at any point, a new one runs `/chunk resume`: it reads the git log, the plan's ticked steps and `log.md`, runs `cargo xtask ci --quick`, and continues from the first unticked step.

## 4. Reference-level design

### 4.1 The loop

| Stage | Done by | Model | Produces | Stops for the maintainer |
|---|---|---|---|---|
| 1. Brainstorm | main session, with the maintainer | Opus | `spec.md` (Draft) | **Yes: spec approval** |
| 2. Plan | main session; lessons researcher | Opus; Haiku | `plan.md`, `log.md` | No |
| 3. Work | builder, one per plan step | Sonnet | commits | No |
| 4. Review | reviewers (§4.2), then builder for fixes | §4.2 | the PR's Reviews section | No |
| 5. Compound | main session | Opus | edits to `docs/lessons/` and other docs | No |
| 6. Close out | main session | Opus | PR, close-out commit | **Yes: merge go-ahead** |

- The main session MUST NOT write product or test code in stage 3 or 4; it dispatches, checks, commits and keeps the documents. It MAY edit documentation directly.
- Stage 1 picks the next chunk as the first `Planned` row of the current milestone, in ID order, whose dependencies are `Done`; the maintainer MAY name another.
- An approved spec authorizes everything it states, including public API changes. Pushing the chunk's own branch and opening its pull request are part of stage 6 and need no further approval.
- Compounding is not only stage 5: every agent writes candidate lessons to `log.md` as they happen (§4.3). Stage 5 promotes them.
- **Escalations.** Outside the two stop points, an agent MUST stop and ask when: the spec is wrong or silent on something that changes what gets built (the fix is a spec amendment, approved like the spec); a change would touch an ADR, a frozen format or a CI-enforced policy that the spec does not name; a reviewer and the builder still disagree after two fix rounds; or anything outward-facing beyond the chunk's branch and pull request (merging, pushing another branch, publishing, commenting elsewhere).
- Long jobs that run mutated or untrusted code (mutants) run only by the procedure in §4.6, and the agent tells the maintainer when it starts one.

### 4.2 Roles

Roles are Claude Code agent files in `.claude/agents/`, each with `name`, `description`, `model` and `tools`. A role file MUST link to the checklist it applies (in `CONTRIBUTING.md` or `docs/workflow.md`) rather than copying it.

| Role | File | Model | Tools | Gets |
|---|---|---|---|---|
| Builder (work and fixes) | `builder.md` | `sonnet` | all except spawning agents | the plan step, the spec, the step's lessons |
| Code reviewer | `review-code.md` | `opus` | read-only plus Bash | spec, diff, checklist, lessons |
| Security reviewer | `review-security.md` | `opus` | read-only plus Bash | as above |
| Conditional reviewer (public API, determinism, performance, portability; told which apply) | `review-conditional.md` | `opus` | read-only plus Bash | as above |
| Documentation reviewer | `review-docs.md` | `sonnet` | read-only plus Bash | as above |
| Spec conformance reviewer | `review-spec.md` | `sonnet` | read-only plus Bash | as above |
| Lessons researcher | `lessons-researcher.md` | `haiku` | read-only | the list of paths and topics the spec touches |

- Reviewers MUST NOT edit files and MUST NOT receive the builder's or the main session's reasoning; the brief is the spec, the diff range, the checklist link and the lesson IDs.
- A re-review after a fix MUST be a fresh reviewer agent, not a continued one.
- Builders run in the chunk's worktree, one at a time. Agent files MUST NOT set `isolation: worktree` (it branches from `main`, not from the chunk).
- Each review's result goes in the pull request's Reviews section (one line each, or `n/a: <reason>`), as today.

### 4.3 Progress on disk: `log.md` and the plan's steps

Each chunk folder gets a `log.md`, copied from `docs/design/_template/log.md`.

- It is append-only during the chunk. Each entry is one or two lines: `- <date> <tag> <text>`, where tag is one of `done`, `decision`, `surprise`, `dead-end`, `lesson?`.
- Every agent (builder, reviewer through its report, main session) MUST record surprises, decisions and candidate lessons when they happen, not at the end. A builder appends before it reports back; the main session commits `log.md` with the step.
- Each plan step has a checkbox, ticked in the step's commit.
- **Resuming** (`/chunk resume`): read `git log` since the branch point, the plan's ticked steps and `log.md`; run `cargo xtask ci --quick`; continue from the first unticked step. If the working tree has uncommitted changes, the session reports them before touching anything.
- At close-out `log.md` becomes `Historical`, like the plan.

### 4.4 Lessons

A lesson is guidance an agent follows by default and may deviate from when it records why. Lessons live in `docs/lessons/<topic>.md`, one file per topic, indexed by `docs/lessons/README.md`.

```markdown
---
applies_to:
  - "libs/engine/src/collections/**"
  - "libs/engine/tests/**"
---
# Lessons: collections

- Status: Living (2026-11-15)

- **collections-1.** Tests that walk an iterator or `Debug` output of code under test use `bounded` / `debug_bounded`. *Why:* a mutant that makes `next` return `Some` forever made a test allocate without bound and froze the host (M3.14). *Deviate when:* the iterator is a std type. *From:* M3.14 · *Confirmed:* 2026-11-15
```

- **IDs** are `<topic>-<n>`, never reused. A retired lesson is deleted; git history is the archive.
- **Fields:** the lesson in one or two sentences; *Why*; *Deviate when* (optional); *Retire when* (optional: the change that makes it obsolete); *From* (chunk); *Confirmed* (date it was last found true).
- **Capture gate.** A candidate from `log.md` becomes a lesson only if it is non-obvious from the code, likely to recur, and costly to miss. If a test, lint or `xtask` check can enforce it, that is preferred, and the lesson is not written (or is deleted once the check exists).
- **Edits are deltas.** An agent adds, edits or deletes single bullets. It MUST NOT regenerate a lesson file. Before adding, it MUST search the existing lessons for one that covers the same point and edit that one instead.
- **Retrieval.** In stage 2 the lessons researcher matches the paths the spec touches against `applies_to` and returns the matching lesson IDs with one line each. The plan's Lessons section lists each ID as followed or deviated from, with the reason. Reviewers receive the same IDs. No session loads all lessons.
- **Upkeep.** At each milestone boundary, a refresh (`/lessons-refresh`) checks every lesson against the code: keep (update *Confirmed*), edit, merge, or delete. `check-docs` (§4.7) fails when an `applies_to` pattern matches no tracked file.
- **Seed lessons.** This chunk moves the project knowledge now held only in the maintainer's local Claude memory into lessons, skills or docs (§4.6), and writes nothing else as seed.

### 4.5 Documentation layout

- **`AGENTS.md`** MUST stay under 80 lines and contain only: the commands; the loop and its stop points in a few lines, linking `docs/workflow.md`; the CI-failing rules (as today, shortened); a "working in X, read Y" table; and the statement that the project workflow wins where a plugin's defaults differ (for example where a plugin writes specs). No repository overview.
- **`CLAUDE.md`** at the root contains only `@AGENTS.md`, so Claude Code versions that do not read `AGENTS.md` natively load the same content.
- **`docs/workflow.md`** (new, Reference): the full loop of §4.1 to §4.4 for people and agents. `CONTRIBUTING.md`'s "How work is organized" links it instead of restating the process.
- **`CONTRIBUTING.md`** keeps: issues, licence and originality, setting up, what CI enforces, tests, commits and pull requests, reviews before a pull request, security. The tool manuals move out (next bullet), each replaced by a one-line pointer.
- **Docs live next to what they describe.** The sections move as follows, shortened where the text restates the tool's own output:

  | Section of `CONTRIBUTING.md` | Moves to |
  |---|---|
  | Benchmarks | `bench/README.md` |
  | Instruction-count gate | `bench/regress/README.md` |
  | Fuzzing | `fuzz/README.md` |
  | Coverage, Mutation testing | `xtask/README.md` |
  | Spikes | `spikes/README.md` (exists; merged) |

- **Directory READMEs.** Each directory in the required set has a `README.md` with this shape, in this order:

  ```markdown
  # <dir>/
  <one line: what this is>
  ## When this applies
  <bullets: the situations in which a reader needs this directory>
  ## Contents
  <one bullet per tracked entry: `name`: one line>
  ## How to
  <only what is specific to this directory; optional>
  ```

  The required set is: `bench/`, `bench/regress/`, `bench/compare/`, `docs/`, `docs/adr/`, `docs/design/`, `docs/lessons/`, `examples/`, `fuzz/`, `infra/github/`, `libs/engine/`, `spikes/`, `xtask/`, and `.claude/`. The root `README.md` gains a Contents section in the same form for top-level directories. `docs/README.md` keeps its table, which counts as its Contents.
- **Writing rule** (added to `docs/README.md` Conventions, applied by the documentation reviewer): write the minimum the reader needs; link instead of copying; a plan does not restate its spec; no overview of what the reader can see in the code. The spec and plan templates gain a length note and mark optional sections as optional.
- **Path-scoped rules.** `.claude/rules/*.md` with `paths:` frontmatter route Claude Code to the right README and lessons when it touches matching files. Each rule file MUST be under 15 lines and MUST point to the README or lesson file rather than copy it. The plan picks the set; at least `libs/engine/**` (determinism, documentation and panic rules) and `bench/regress/**`.

### 4.6 `.claude/` contents

- `agents/`: §4.2.
- `skills/chunk/`: `SKILL.md` (under 150 lines) dispatches `/chunk next`, `/chunk resume` and `/chunk <stage>`; one reference file per stage holds that stage's steps, so only the current stage's text is loaded.
- `skills/lessons-refresh/`: the upkeep pass of §4.4.
- `skills/bench-run/`: the wall-clock benchmark protocol (AC power, `caffeinate -i`, no builds during the run, power-log check afterwards), moved from local memory.
- `skills/mutants-run/`: running mutants in a memory-capped container (`--memory=4g --memory-swap=4g`, `-j 1`) with a host-side watchdog, moved from local memory. The container recipe was lost with a session scratchpad and is rebuilt in this chunk, then tried once with `cargo xtask mutants --module random` in Docker Desktop on the maintainer's Mac; the agent says when it starts the run, and records its output in `log.md`.
- `rules/`: §4.5.
- `settings.json`: a permission allowlist of read-only commands (git and `gh` reads, `cargo xtask ci`, `test`, `check-*`), and a `PreToolUse` hook on Bash that blocks `git push` to `main` (including `HEAD:main` refspecs) and any force push (`--force`, `-f`, `--force-with-lease`, `+refspec`). The hook is a POSIX `sh` script, `.claude/hooks/guard-push.sh`, that exits 0 at once unless the input contains `push`, and otherwise hands the JSON to `cargo xtask guard-push`, which parses it with `serde_json` and decides. The decision logic is Rust with unit tests (inputs it must block and must allow), so it runs on every CI platform and needs no `jq`.
- `README.md`: the directory README of §4.5.
- `.gitignore` gains `.claude/worktrees/`, `.claude/settings.local.json` and `.claude/agent-memory-local/`.

### 4.7 `cargo xtask check-docs`

A new check, run by `cargo xtask ci` and in CI. It MUST fail, naming the file and the entry, when:

1. a directory in the required set (§4.5) has no `README.md`;
2. a README's Contents names an entry that does not exist in that directory;
3. a tracked file or directory directly inside that directory is missing from its Contents (entries MAY be excluded in a `<!-- check-docs: ignore name ... -->` comment, for generated or obvious files such as `Cargo.toml`);
4. a lesson file lacks `applies_to`, has a duplicate ID, or has an `applies_to` pattern that matches no tracked file;
5. `AGENTS.md` exceeds 80 lines.

It reads tracked files through `git ls-files`, so ignored and generated files never count. A new dependency for glob matching MUST pass `cargo xtask deny`; the plan MAY instead implement the small subset of glob syntax it needs.

### 4.8 Memory migration

The maintainer's local Claude memory for this project holds project knowledge no other contributor can see. In this chunk:

| Memory | Goes to |
|---|---|
| Benchmark run protocol | `skills/bench-run/` and `bench/README.md` |
| Long jobs need memory caps | `skills/mutants-run/`, and a lesson in `docs/lessons/testing.md` (the `bounded` helpers) |
| One PR per chunk, close-out in it | `docs/workflow.md` (stage 6) |
| Direct push to main: one-off only | the `guard-push.sh` hook |
| No copying other engines' code | already ADR 0002; nothing to add |
| Engine project goals | already in the roadmap, ADRs and specs; nothing to add |

Personal preferences (tone, collaboration style, "grill me", DX over API stability) stay in local memory. After the pull request merges, the migrated memory files are deleted or reduced to a pointer; that edit is outside the repository and is not part of the diff.

### 4.9 Templates and the pull request

- `docs/design/_template/plan.md`: a checkbox per step; a Lessons section (IDs followed or deviated from, with reasons); a length note.
- `docs/design/_template/spec.md`: a length note; sections 3, 7 and 11 marked optional for small chunks.
- `docs/design/_template/log.md`: new (§4.3).
- `.github/PULL_REQUEST_TEMPLATE.md`: a Reviews section (the eight reviews, one line each) and a Lessons section (lessons added, edited or deleted, by ID) replace the checklist lines they duplicate.

### 4.10 Keeping the workflow current

The milestone checklist in `docs/maintainers.md` gains two items: run `/lessons-refresh`, and, if a new model shipped since the last milestone, re-check the model assigned to each role in §4.2 against one chunk's results.

## 5. Requirements

- **M3.15-R1** `AGENTS.md` MUST describe the loop and its two stop points (spec approval, merge go-ahead) and the escalations of §4.1, and MUST stay under 80 lines.
- **M3.15-R2** `docs/workflow.md` MUST describe §4.1 to §4.4, and `CONTRIBUTING.md` MUST link it instead of restating the process.
- **M3.15-R3** `.claude/agents/` MUST define the seven roles of §4.2 with the models given there; reviewer roles MUST NOT have edit tools.
- **M3.15-R4** The `chunk` skill MUST implement `next`, `resume` and per-stage invocation, with stage text in separate reference files.
- **M3.15-R5** `resume` MUST reconstruct state from git, the plan's checkboxes and `log.md` only (§4.3).
- **M3.15-R6** The templates MUST include `log.md`, plan checkboxes and the plan's Lessons section (§4.9).
- **M3.15-R7** `docs/lessons/` MUST follow §4.4, and the planning stage MUST retrieve lessons by `applies_to`.
- **M3.15-R8** The tool manuals MUST move out of `CONTRIBUTING.md` as in §4.5, with no rule lost; each move SHOULD shorten the text.
- **M3.15-R9** Every directory in the required set MUST have a README in the shape of §4.5.
- **M3.15-R10** `cargo xtask check-docs` MUST implement §4.7, have unit tests for each failure kind, and run in `cargo xtask ci` and CI.
- **M3.15-R11** The push guard MUST block pushes to `main` and force pushes, MUST allow other pushes, and its unit tests MUST run in `cargo xtask test` on every CI platform.
- **M3.15-R12** The project knowledge in §4.8 MUST be in the repository, and the mutants container procedure MUST complete one run of `--module random` with a report.
- **M3.15-R13** The CLAUDE.md audit (`claude-md-management:claude-md-improver`) MUST be run against `AGENTS.md` and `CLAUDE.md` after the rewrite, and each finding fixed or answered in the pull request.
- **M3.15-R14** This chunk's own reviews MUST be run with the new reviewer agents.

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no library code changes. `check-docs` reads `git ls-files` output in sorted order so its messages are stable.
- **`unsafe`:** none; `xtask` keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because none is added.
- **Heap allocation:** n/a, because no library code changes.
- **`no_std` and wasm:** n/a, because only `xtask` and docs change.
- **Frozen formats:** none touched.
- **Public API changes:** none.
- **FFI exposure:** none.
- **Performance:** n/a; `check-docs` runs in well under a second on this repository (measured in the plan).
- **Documentation:** this chunk is mostly documentation; §4.5 and §4.9.
- **Policy changes:** yes. The stop points change from "spec and plan reviewed together, then merge" to "spec, then merge"; reviews must be done by agents other than the builder; lessons may be edited directly in a chunk's pull request and are listed in its Lessons section; `check-docs` becomes a CI gate.

## 7. Alternatives considered

- **Install the compound-engineering plugin** (EveryInc). It has the researcher, overlap check and refresh step, but brings about 38 skills and 50 agents into every session's listing, uses its own plan format and `docs/solutions/` layout, overlaps with superpowers, changes under us on plugin updates, and is not installed for someone who clones the repository. Its ideas are adopted in §4.4; its code is not used.
- **Plugin for review and compound, our own for the rest.** Two vocabularies and two places for lessons, for little gain.
- **Keep the plan review as a stop point.** It adds a wait to every chunk; the spec is where the maintainer's intent matters most, and the plan is reviewed at merge with the rest.
- **Lessons in an always-loaded file** (a section of `AGENTS.md`). Measured context files raise cost and do not raise success much (ETH Zurich, 2026); lessons that load only by path keep the always-loaded part small.
- **Per-subagent persistent memory** (`memory: project`). It splits knowledge by role, is Claude-specific, and is loaded at every start of that role; lessons by path are shared and readable by people.
- **Regenerating lesson files with an LLM to keep them tidy.** ACE (ICLR 2026) measured that a full rewrite collapsed a playbook and dropped accuracy below no playbook at all; edits are deltas.
- **Only `.claude/rules/` and no directory READMEs.** Claude-only; people and other agents would get nothing.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1, R2 | `check-docs` (line count); documentation review; spec conformance review |
| R3 | spec conformance review reads each agent file's frontmatter |
| R4, R5 | a resume trial in this chunk: midway through the work stage, a fresh agent with no conversation context follows the `resume` procedure read-only and reports the step it would continue from and any uncommitted changes; `log.md` records whether that matches the true state. A real killed session, when one first happens, is recorded the same way |
| R6, R9 | `check-docs`; documentation review |
| R7 | `check-docs` (lesson format); the plan of the next chunk lists retrieved lesson IDs (recorded as a follow-up check in `log.md`) |
| R8 | documentation review compares each moved section with the old text and lists anything dropped |
| R10 | `cargo xtask test` (unit tests per failure kind); `cargo xtask ci` |
| R11 | the hook's tests in `cargo xtask ci` |
| R12 | `log.md` entry for the `--module random` container run with its summary table; spec conformance review against §4.8 |
| R13 | the audit's output and responses in the pull request |
| R14 | the Reviews section names the agent file used for each review |

## 9. Done when

- [ ] `cargo xtask ci` passes, including `check-docs` and the hook tests.
- [ ] Every requirement in §5 has its evidence from §8.
- [ ] The resume trial and the mutants container run are recorded in `log.md`.
- [ ] The CLAUDE.md audit was run and answered.
- [ ] The pull request lists all eight reviews, done by the new agents, and a Lessons section.
- [ ] The spec is `Implemented`, the plan and log are `Historical`, the roadmap row is `Done`.

## 10. Open questions

None.

## 11. Prior art and sources

- Lulla et al. / ETH Zurich, "Evaluating AGENTS.md", [arXiv 2602.11988](https://arxiv.org/abs/2602.11988) (2026): context files raise cost by over 20% and help little; hand-written beats generated; overviews do not help.
- Zhang et al., "Agentic Context Engineering", [arXiv 2510.04618](https://arxiv.org/abs/2510.04618) (ICLR 2026): itemized playbooks updated by deltas; full rewrites cause context collapse.
- "Codified Context", [arXiv 2602.20478](https://arxiv.org/abs/2602.20478) (2026): a small always-loaded file, specialist agents, on-demand specs; stale docs caused conflicting code.
- EveryInc, [compound-engineering-plugin](https://github.com/EveryInc/compound-engineering-plugin) (checked 2026-11-15): the loop, `docs/solutions/` with frontmatter, a learnings researcher in planning, overlap check, capture gate, refresh.
- Anthropic, [Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) (2025), [Effective context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) (2025), [Harness design for long-running apps](https://www.anthropic.com/research/harness-design-long-running-apps) (2026), [Components of agent memory](https://www.anthropic.com/research/components-of-agent-memory) (2026): progress files and commits per session, separate evaluator, re-test harness parts when models change, keep memory to what cannot be re-derived.
- Claude Code docs: [memory](https://code.claude.com/docs/en/memory) (AGENTS.md loading, path-scoped rules, under 200 lines), [subagents](https://code.claude.com/docs/en/sub-agents), [skills](https://code.claude.com/docs/en/skills).
