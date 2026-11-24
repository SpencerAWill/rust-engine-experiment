# Workflow

- **Status:** Accepted (2026-11-15), from [M3.15](chunks/m3.15-agent-workflow/spec.md). How one chunk goes from idea to merge, for people and agents. The rules a change must meet are in [CONTRIBUTING.md](../CONTRIBUTING.md); what is scheduled is on GitHub ([below](#from-issue-to-chunk)), and the milestone goals are in the [roadmap](roadmap.md).

## The loop

Each chunk goes through seven stages, and the next chunk starts in a new session. Only two stages wait for the maintainer.

| Stage | Done by | Model | Produces | Waits for the maintainer |
|---|---|---|---|---|
| 1. Brainstorm | main session, with the maintainer | Opus | `spec.md` (Draft), `log.md` | no (the maintainer is in the conversation) |
| 2. Grill | main session, with the maintainer; `spec-reader` | Opus | spec edits, `log.md` entries | **yes: spec approval, at its end** |
| 3. Plan | main session; `lessons-researcher` | Opus; Haiku | `plan.md` | no |
| 4. Work | `builder`, one per plan step | Sonnet | one commit per step | no |
| 5. Review | `builder` runs `/simplify`; then the reviewers; `builder` for fixes | Sonnet; see [Roles](#roles) | a simplify commit; the PR's Reviews section | no |
| 6. Compound | main session | Opus | edits to `docs/lessons/` and other docs | no |
| 7. Close out | main session | Opus | PR, close-out commit, follow-up issues; after the go-ahead, the merge | **yes: merge go-ahead** |

In Claude Code, `/chunk next` starts stage 1 for the next chunk, `/chunk resume` continues wherever the chunk is, `/chunk <stage>` runs one stage, and `/chunk chore #N` runs a chore ([its steps](../.claude/skills/chunk/SKILL.md#chore-n)). Without Claude Code, follow the stage files in [`.claude/skills/chunk/stages/`](../.claude/skills/chunk/stages/) by hand; they are plain Markdown.

- **Picking the next chunk:** in the current milestone (the open GitHub milestone with the lowest number), the scheduled chunk issue that is not in flight (no branch on origin, which stage 1 pushes at once, and no open pull request from this repository closing it) and has no open "blocked by" issue, with the lowest Order. The maintainer may name another; naming work that is not yet a scheduled chunk schedules it. The exact query is in [the chunk skill](../.claude/skills/chunk/SKILL.md#next).
- **The main session does not write product or test code** in stages 4 and 5. It dispatches, checks each result against the step's file list, commits, and keeps the documents.
- **An approved spec authorizes what it states**, public API changes included. Pushing the chunk's branch (in stage 1, as soon as it is created), opening its pull request (stage 7), and opening labelled issues for follow-ups found along the way (stage 7) are part of the loop.
- **Clear the context after the plan** (suggested, not a wait). The work and review stages do not need the brainstorm and grill conversation: the spec, the plan and `log.md` hold what was decided, and every later turn re-sends the whole context. Once the plan is committed (`docs: add the <title> plan (<id>)`), interrupt the session, run `/rename <id> spec and plan` so that conversation can be found again with `/resume`, then `/clear` and `/chunk resume`, which continues at stage 4 from disk. Unless interrupted, the loop goes straight on to stage 4.
- **The simplify pass** (behaviour-preserving cleanups: reuse, simplification, efficiency; `/simplify` in Claude Code) runs once over the whole chunk's diff, before any reviewer, and is committed on its own (`refactor: simplify (M<N>.<n>)`), so reviewers can see what it changed.
- **After the merge go-ahead**, in the same turn, the main session squash-merges the commit the required checks passed on (`gh pr merge --squash --delete-branch --match-head-commit <sha>`, never `--admin`), reminds the maintainer to run `pulumi preview` and `pulumi up` if `infra/github/` changed, names the chunk that comes next, and stops. The maintainer starts it with `/chunk next` in a new session, so each chunk begins with a clean context. If a check fails after the go-ahead, it fixes the failure as in stage 5 and asks again.

### When to stop and ask

Outside the two waits, an agent stops and asks only for the escalations listed in [AGENTS.md](../AGENTS.md#the-loop) ("Stop and ask"), which is the single copy of that list. A wrong or silent spec is fixed by an amendment, grilled and approved like the spec.

Long jobs that run mutated or untrusted code run only through `/mutants-run`, and the agent says when it starts one.

### Plugins

Techniques from installed plugins (such as superpowers' brainstorming, test-driven development and debugging skills) may be used in any stage. Where a plugin's defaults differ from this workflow, the workflow wins: specs, plans and logs go in the chunk folder from [the templates](chunks/_template/), never in a plugin's own location such as `docs/superpowers/`. Nothing here requires a plugin.

## From issue to chunk

An idea becomes a chunk through GitHub issues, before the loop above starts. Planning state (what is scheduled, its order, its milestone, its status) lives on GitHub; documents (specs, plans, logs, ADRs, lessons, milestone goals) live in the repository. The reasoning and the alternatives are in the [M3.18](chunks/m3.18-issue-tracking/spec.md) and [M3.20](chunks/m3.20-planning-on-github/spec.md) specs; the Project's setup is in [maintainers.md](maintainers.md#issue-tracking).

**Vocabulary.** An *issue* is a request, not yet designed. A *chunk* is a pull-request-sized story with a spec (this repository's own term). A *chunk issue* is the one issue of a chunk, titled `M<N>.<n>: <title>` and authored by the maintainer's account (another author's issue is linked from one, never retitled, and closed as its duplicate when it is scheduled). A *chore* is a scheduled issue without a chunk ID: Size S work done as one small pull request, not the loop. A *tracking issue* is an issue whose sub-issues are the work of one effort. *Triage* is reviewing and defining unscheduled issues. The *icebox* is accepted work that is deliberately unscheduled (the `research` issues).

**The path.**

1. An idea starts in a Discussion or directly as an issue, and enters the Project as Inbox.
2. Triage gives it labels, a Priority and a Size and defines it; when it meets the Definition of Ready it is Ready.
3. The maintainer schedules it, on GitHub. As a chunk: the next unused chunk ID in its title, the milestone, Status Scheduled, an Order, and "blocked by" links for what must land first. As a chore: the milestone, Status Scheduled and an Order, no chunk ID. An agent may make these writes when the maintainer asks; [maintainers.md](maintainers.md#issue-tracking) has the commands.
4. The chunk runs the loop above on its own branch, or the chore runs through `/chunk chore #N`. Its pull request says `Closes #N`; merging closes the issue, the Project shows Done, and the milestone's progress moves.
5. Follow-up issues found along the way (stage 7) start again at Inbox.

**Who is the source of truth.** Unscheduled work and its ranking: the Project's Status and Priority. Scheduled work and its order: the Project's Status Scheduled and Order, with "blocked by" links for dependencies (Priority ranks unscheduled work, Order ranks scheduled work; neither is read for the other). Milestone membership and progress: GitHub milestones. Milestone goals and exit criteria, and the history of chunks up to M3.20: [roadmap.md](roadmap.md). What a piece of work means, once a spec exists: the spec (the issue is only the request and is not edited to match). State of a chunk in flight: the branch, the plan's checkboxes and `log.md`. Classification: labels. Every chunk has exactly one chunk issue. Project fields have no history, so a change to a scheduled item's Order or dependencies comes with a one-line comment on the issue saying why.

| Relationship | Count |
|---|---|
| Chunk, branch, spec, plan, pull request | 1 : 1 : 1 : 1 : 1 |
| Milestone to chunks and chores | 1 : many |
| Chunk to chunk issue | 1 : 1 |
| Tracking issue (an epic, not a milestone) to issues | 1 : many |
| Pull request to issues closed | 1 : many |
| Chunk to follow-up issues | 1 : many |

An issue never spans more than one pull request, so `Closes #N` is always right. Work that needs several pull requests is several chunk issues in one milestone, grouped under a tracking issue if they form one effort, sequenced by Order and "blocked by" links. A chunk that proves too big is split: its issue and the first pull request keep the chunk ID, the remainder gets a new chunk issue with the next unused ID, and IDs are never renumbered or reused.

**Definition of Ready.** An issue is Ready when its body states: the problem and what it costs; the outcome that would count as done; the verification (a command or a named check); dependencies by issue number or chunk ID; a Size below XL (XL is split first); and either no `needs-measurement` label or the measurement named as the first step. It does not need a design: the spec, grilled and approved, provides that.

**Triage while a long job runs.** `/triage` (`.claude/skills/triage/`) reads and writes GitHub only and never builds, tests or measures, so it can run while a benchmark or mutation run is using the machine. It proposes a batch and writes only after the maintainer confirms.

## Roles

Agent files in [`.claude/agents/`](../.claude/agents/) pin each role's model. The main session's model is a recommendation (Opus), not a setting, so contributors keep their own choice.

| Role | File | Model | Edits files |
|---|---|---|---|
| Builder: plan steps, the simplify pass, review fixes | `builder.md` | Sonnet | yes |
| Code reviewer | `review-code.md` | Opus | no |
| Security reviewer | `review-security.md` | Opus | no |
| Conditional reviewer: public API, determinism, performance, portability | `review-conditional.md` | Opus | no |
| Documentation reviewer | `review-docs.md` | Sonnet | no |
| Spec conformance reviewer | `review-spec.md` | Sonnet | no |
| Lessons researcher | `lessons-researcher.md` | Haiku | no |
| Cold reader for the grill | `spec-reader.md` | Opus | no |

- A reviewer gets the spec, the diff range, its checklist and the lesson IDs, and never the builder's or main session's reasoning.
- A re-review after a fix is a fresh agent, not a continued one.
- Builders run one at a time in the chunk's working tree.
- The pull request's Reviews section gives each review's one-line result and the agent file that did it.

## Progress on disk

Agents get killed and sessions end; what is not on disk is lost.

- **`log.md`**, in the chunk folder, is append-only: `- <date> <tag> <text>`, tag one of `done`, `decision`, `surprise`, `dead-end`, `lesson?`. Every agent records surprises, decisions and candidate lessons when they happen. A builder appends before reporting; the main session commits the log with each step.
- **Plan steps** have checkboxes, ticked in the step's commit.
- **Resuming:** read `git log` since the branch point, the plan's ticked steps and `log.md`; run `cargo xtask ci --quick`; report any uncommitted changes before touching anything; continue from the first unticked step. Before a plan exists, continue the brainstorm or grill from the spec and the log.
- At close-out the log becomes `Historical`, like the plan.

## Lessons

[`docs/lessons/`](lessons/) holds guidance learned in earlier chunks, one file per topic, each with the paths it `applies_to`. The format and the capture gate are in its [README](lessons/README.md).

- **Written** in stage 6, from the `lesson?` entries in `log.md`, as single-bullet edits in the chunk's own pull request, listed in its Lessons section. Prefer a test or check over a lesson whenever one can enforce it.
- **Read** in stage 3: `lessons-researcher` returns the lessons whose `applies_to` matches the paths the spec touches. The plan lists each as followed or deviated from, with the reason; reviewers get the same list.
- **Kept current** by `/lessons-refresh` at each milestone boundary.

## The grill

`/grill` turns a draft spec into an approved one. It follows the method of Matt Pocock's [`grilling` skill](https://github.com/mattpocock/skills/tree/main/skills/productivity/grilling), written for this repository.

1. `spec-reader` reads the spec cold and returns the questions a builder would have to guess at.
2. The main session answers what the repository or the conversation already answers, settles minor points as its own call (marked in the spec), and keeps the rest for the maintainer.
3. Questions go one at a time, each with a recommended answer and its trade-off, worded so "yes" accepts it, and the next built on the answer. Up to three may be batched only if they are independent: no answer to one changes whether another is asked, its options or its recommendation. Facts are looked up, never asked. Questions probe the design; they do not quiz the maintainer.
4. Each answer goes into the spec and `log.md` before the next question.
5. When nothing is open, the main session lists its own calls and asks for approval.

It works the same on a plan, an ADR or any design document.
