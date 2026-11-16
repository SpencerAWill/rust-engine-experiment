# M3.18: Issue tracking

- **Status:** Draft (2026-11-16)
- **Milestone:** M3 (not part of its exit criteria) · **Depends on:** M3.16 · **Decides:** no ADR (process and documentation; the Project's configuration is not code, see 4.6)
- **Estimated size:** about 500 changed lines of Markdown and YAML, about 60 lines in `infra/github/index.ts`, one new skill

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

The repository has issues, templates and labels but no defined path from "an idea exists" to "a chunk is scheduled", and no place to see or order unscheduled work. This chunk documents that path, adds a GitHub Project for triage, and adds a `/triage` skill that defines and orders issues using only the GitHub API and file reads, so it can run while a long test or benchmark occupies the machine. At the end, a new issue can be followed from Inbox to Done, and `/triage` can be run against the current backlog.

## 2. Goals and non-goals

Goals:

- One vocabulary and one set of relationships between GitHub concepts (issue, label, Project, tracking issue, branch, pull request) and repository concepts (roadmap, chunk, spec, plan, log, ADR, lesson), written once.
- A triage step that can run without building, testing or benchmarking.
- Issues that an agent can read and act on: a fixed body structure, scoped labels, a Definition of Ready.
- Keep the one-chunk-one-PR rule; multi-PR work is a group of chunks.

Non-goals:

- No change to the seven-stage chunk loop, the roadmap table's role or chunk IDs.
- No GitHub Milestones (the roadmap and tracking issues cover them; revisit if their progress bar is missed).
- No iterations or sprints, no stale-issue bot, no issue types (they appear to be organisation-only; unverified).
- The Project is not declared as code.

## 3. Guide-level explanation

**Vocabulary.** *Issue*: a request, not yet designed. *Chunk*: a PR-sized story with a spec (the repository's own term). *Tracking issue*: an issue whose sub-issues are the work of one effort (the pattern of #38). *Triage*: reviewing and defining unscheduled issues (backlog refinement). *Definition of Ready*: the checklist an issue meets before it can be scheduled. *Icebox*: accepted but deliberately unscheduled work (the `research` sub-issues).

**The path.** An idea starts in a Discussion or directly as an issue and enters the Project as **Inbox**. Triage labels it and defines it; when it meets the Definition of Ready it is **Ready**. The maintainer schedules it by adding a chunk row to the roadmap; the issue becomes **Scheduled** and from then on the roadmap owns its order. The chunk runs the loop on its own branch; its pull request says `Closes #N`; merging closes the issue and the Project shows **Done**. Follow-up issues found along the way start again at Inbox.

**Who is the source of truth.** Priority and what to schedule next: the Project. Order and dependencies of scheduled work: the roadmap. What a piece of work means: the chunk's spec once it exists. State of a chunk in flight: branch, plan checkboxes and `log.md`. Classification: labels.

**Cardinalities.**

| Relationship | Count |
|---|---|
| Chunk, branch, spec, plan, pull request | 1 : 1 : 1 : 1 : 1 |
| Roadmap milestone to chunks | 1 : many |
| Tracking issue to issues | 1 : many |
| Issue to chunk | normally 1 : 1 (batching: several small issues in one chunk; anything larger is a tracking issue with one sub-issue per chunk) |
| Pull request to issues closed | 1 : many |
| Chunk to follow-up issues | 1 : many |
| Label to issue | many : many |
| Issue to Project item | 1 : 1 (pull requests are not Project items) |

An issue never spans more than one pull request, so `Closes #N` is always correct. A chunk that proves too big is split by amending the roadmap: the first pull request keeps the chunk ID, the remainder gets a new one, IDs are never renumbered.

**Running triage during a long job.** `/triage` lists the Inbox, and for each issue proposes labels, Priority, Size and a drafted definition; the maintainer confirms; only then does it write. It never runs `cargo`, Valgrind, Docker or anything else that builds or measures.

## 4. Reference-level design

### 4.1 Labels

Existing labels stay and none is retired. **Type labels** are the existing `bug`, `enhancement`, `infrastructure`, `documentation` and `research`; every issue carries exactly one of them. `performance` and `determinism` are additive (an issue may carry them with a type). New: an `area:` family for code only, matching the `engine` modules and tools (`area:collections`, `area:genid`, `area:random`, `area:time`, `area:math`, `area:bench`, `area:xtask`); CI, docs and process are covered by `infrastructure` and `documentation`, so there is no `area:ci`, `area:docs` or `area:workflow` *(main session's call)*. Also new: `needs-measurement` (triage found the issue cannot be defined without a run), and `ai-assisted` (an assistant drafted the issue). Labels are declared in `infra/github/index.ts` (authoritative; labels not listed there are deleted). Status and priority are never labels.

### 4.2 The Project

One user-owned Project, "Engine backlog", with single-select fields:

- **Status:** Inbox, Ready, Scheduled, Done. (Fewer states first; add Defined or In progress only if wanted.) There is no Parked status: the icebox is the `research` issues, which are not Project items *(main session's call)*.
- **Priority:** Now, Next, Later (neutral wording, since contributors can see it).
- **Size:** S, M, L, XL (XL means split before scheduling).

Built-in workflows: auto-add issues from this repository except those labelled `research`; set Status to Done when an issue closes. Default views: Inbox, Ready by Priority, Scheduled, and a board. `research` issues, #38 included, are not items; when one is scheduled it loses `research` for a type label and is added. When `research` is added to an issue after it was created, `/triage` removes it from the Project.

### 4.3 Tracking issues

One per roadmap milestone and one per multi-chunk effort, with native sub-issues. A milestone tracking issue links effort-level tracking issues, which link the per-chunk issues. The shared design of a multi-chunk effort lives in the tracking issue's body while it is being scoped, and in an ADR when it would be expensive to reverse. Each chunk's spec links the tracking issue.

### 4.4 Definition of Ready

An issue is Ready when its body states: the problem and what it costs; the outcome that would count as done; the verification (a command or a named check); dependencies by issue or chunk ID; Size not XL; `needs-measurement` absent or the measurement named as the first step. It does not need a design: the spec (the grill) provides that.

### 4.5 Issue forms

`bug_report.md`, `feature_request.md` and `infrastructure.md` become YAML issue forms with the fields of 4.4 where they apply, keeping their labels and the existing `determinism_bug.yml` and `performance_report.yml` as they are. Issues filed with `gh issue create` follow the same section headings, documented in CONTRIBUTING.md, because the CLI bypasses forms. An issue drafted by an assistant says so and carries `ai-assisted`.

### 4.6 `/triage`

A skill in `.claude/skills/triage/`. Allowed commands: `gh issue`, `gh project`, `gh api` (read, and the writes below), and file reads. Forbidden: anything that builds, tests or measures, and any file edit outside the issue text. It proposes before writing: labels, Priority, Size, a drafted definition, possible duplicates, and a short reply for each external issue. It writes only after the maintainer confirms the batch. Its allowed writes are exactly: add or remove labels; set Project fields and add or remove Project items; edit the body of an issue filed by the maintainer or an assistant to add the definition; post a comment; close an issue as `duplicate` only with the maintainer's confirmation of that issue by number. It never edits the body of an issue filed by someone else (it comments instead) *(main session's call)*. Follow-up issues from chunk stage 7 are unchanged: already authorized. On start it checks the Project's field and option names against the list in 4.2 and stops with a clear error if they differ, since the Project is edited in the web interface and cannot be declared in Pulumi.

### 4.7 Hand-off to the chunk loop

The roadmap order still picks the chunk. A roadmap row that has an issue cites it as the existing rows do (`#40`). Two small edits to `.claude/skills/chunk/stages/1-brainstorm.md`: step 1 also reads the linked issue, and step 2 sets that issue's Project Status to Scheduled when the row goes to `Drafting`. This is the chunk's own issue, so it is covered by the chunk's authority. Stage 7 moves nothing by hand; the Project automation sets Done *(main session's call)*.

### 4.8 Contributors

Ready issues that are small and safe MAY carry `good first issue` or `help wanted`. A contributor claims an issue by assigning themselves or commenting before starting. CONTRIBUTING.md gains a short "From issue to chunk" section linking this spec's vocabulary; `docs/workflow.md` gains the path in 3, as one paragraph that links here.

### 4.9 Migration

A one-time pass: give every open issue a type label (#27 and #30 get `infrastructure`), add `area:` labels where one applies, set Project fields on the non-`research` issues, and add the two milestone tracking issues (M3, M4). #38 stays standalone. Planned roadmap rows without an issue get none, and existing specs are not edited; a spec links its tracking issue when its chunk starts. Closed or merged chunks without issues are not backfilled *(main session's call)*.

## 5. Requirements

- **M3.18-R1** The Project MUST exist with the fields and options of 4.2, and `docs/` MUST list their exact names.
- **M3.18-R2** Every label in 4.1 MUST be declared in `infra/github/index.ts`.
- **M3.18-R3** `/triage` MUST NOT run a build, test, benchmark or container command, and MUST write to GitHub only after the maintainer confirms.
- **M3.18-R4** `/triage` MUST stop with an error naming the difference when a Project field or option is missing or renamed.
- **M3.18-R5** The three Markdown templates MUST become issue forms carrying the Definition of Ready fields.
- **M3.18-R6** CONTRIBUTING.md and `docs/workflow.md` MUST describe the path in section 3 and link to a single copy of it.
- **M3.18-R7** After the migration every open issue MUST have exactly one type label and an `area:` label where one applies, and every open issue without the `research` label MUST have a Project Status.
- **M3.18-R8** `cargo xtask ci --quick` MUST pass, including `check-docs`.

## 6. Cross-cutting concerns

- **Determinism, `unsafe`, concurrency, heap allocation, `no_std` and wasm, frozen formats, public API, FFI:** n/a, because no library or xtask code changes.
- **Performance:** n/a.
- **Documentation:** CONTRIBUTING.md, `docs/workflow.md`, the `.github` and `.claude` READMEs and `docs/README.md` gain entries; `check-docs` gates them.
- **Policy changes:** M3.9 set `hasProjects: false` in `infra/github/index.ts` ("wiki and Projects off"). Whether a user-owned Project needs that flipped is open (section 10). `infra/github/index.ts` (labels) needs `pulumi preview` then `pulumi up` after the merge. The Project is created by the maintainer in the web interface (the `gh` token needs `gh auth refresh -s project`). Creating the Project and the migration are outward-facing; this spec authorises them, so they are in stage 7, not before the spec is approved.

## 7. Alternatives considered

- **Labels and templates only:** no board and weak ordering; rejected.
- **Project only, no new labels:** loses greppable classification for agents and the Pulumi-versioned record; rejected.
- **GitHub Milestones:** duplicate the roadmap milestones; tracking issues give progress without a second system. Revisit if missed.
- **Status and Priority as labels:** two sources of truth with the Project; rejected.
- **Several pull requests per chunk:** would change the one-PR rule and need an ADR; rejected in favour of chunk groups.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `gh project field-list` output recorded in `log.md`; the names match the doc |
| R2 | `pulumi preview` shows the labels; `git grep` finds each in `index.ts` |
| R3 | The skill's allowed-command list; a dry run recorded in `log.md` |
| R4 | A dry run against a throwaway private Project with one option renamed, created and deleted for the purpose (authorised by this spec); the "Engine backlog" Project is never renamed to test |
| R5 | GitHub renders each form (checked in the chooser) |
| R6 | `cargo xtask check-docs` |
| R7 | `gh issue list --state open --json labels` shows no unlabelled issue; Project view shows no empty Status |
| R8 | `cargo xtask ci --quick` |

## 9. Done when

- [ ] R1 to R8 verified
- [ ] Pull request opened with the assistant noted and the Reviews section filled
- [ ] The maintainer has run `pulumi up` in `infra/github`

## 10. Open questions

- Does a user-owned Project linked to this repository require `hasProjects: true` (reversing M3.9's choice), or is that flag only the legacy repository Projects tab? To be checked against GitHub's behaviour before approval, not assumed.

## 11. Prior art and sources

Rust's label families (`A-` area, `E-` experience, `P-` priority, `S-` status) and its tracking issues for accepted proposals; GitHub's sub-issues and Projects documentation; agile backlog refinement and the Definition of Ready. Designs only, no code copied (ADR 0002).
