# Workflow

- **Status:** Accepted (2026-11-15), from [M3.15](chunks/m3.15-agent-workflow/spec.md). How one chunk goes from idea to merge, for people and agents. The rules a change must meet are in [CONTRIBUTING.md](../CONTRIBUTING.md); what is planned is in the [roadmap](roadmap.md).

## The loop

Each chunk goes through seven stages, then the next chunk starts. Only two of them wait for the maintainer.

| Stage | Done by | Model | Produces | Waits for the maintainer |
|---|---|---|---|---|
| 1. Brainstorm | main session, with the maintainer | Opus | `spec.md` (Draft), `log.md` | no (the maintainer is in the conversation) |
| 2. Grill | main session, with the maintainer; `spec-reader` | Opus | spec edits, `log.md` entries | **yes: spec approval, at its end** |
| 3. Plan | main session; `lessons-researcher` | Opus; Haiku | `plan.md` | no |
| 4. Work | `builder`, one per plan step | Sonnet | one commit per step | no |
| 5. Review | `builder` runs `/simplify`; then the reviewers; `builder` for fixes | Sonnet; see [Roles](#roles) | a simplify commit; the PR's Reviews section | no |
| 6. Compound | main session | Opus | edits to `docs/lessons/` and other docs | no |
| 7. Close out | main session | Opus | PR, close-out commit, follow-up issues; after the go-ahead, the merge | **yes: merge go-ahead** |

In Claude Code, `/chunk next` starts stage 1 for the next chunk, `/chunk resume` continues wherever the chunk is, and `/chunk <stage>` runs one stage. Without Claude Code, follow the stage files in [`.claude/skills/chunk/stages/`](../.claude/skills/chunk/stages/) by hand; they are plain Markdown.

- **Picking the next chunk:** the chunk the merged spec names as **Followed by**, if it is `Planned` and its dependencies are `Done`; otherwise the first `Planned` row of the current milestone, in the roadmap table's order, whose dependencies are `Done`. The maintainer may name another.
- **The main session does not write product or test code** in stages 4 and 5. It dispatches, checks each result against the step's file list, commits, and keeps the documents.
- **An approved spec authorizes what it states**, public API changes included. Pushing the chunk's branch, opening its pull request, and opening labelled issues for follow-ups found along the way are part of stage 7.
- **The simplify pass** (behaviour-preserving cleanups: reuse, simplification, efficiency; `/simplify` in Claude Code) runs once over the whole chunk's diff, before any reviewer, and is committed on its own (`refactor: simplify (M<N>.<n>)`), so reviewers can see what it changed.
- **After the merge go-ahead**, in the same turn, the main session squash-merges the commit the required checks passed on (`gh pr merge --squash --delete-branch --match-head-commit <sha>`, never `--admin`), reminds the maintainer to run `pulumi preview` and `pulumi up` if `infra/github/` changed, and starts stage 1 of the next chunk. If a check fails after the go-ahead, it fixes the failure as in stage 5 and asks again.

### When to stop and ask

Outside the two waits, an agent stops and asks only for the escalations listed in [AGENTS.md](../AGENTS.md#the-loop) ("Stop and ask"), which is the single copy of that list. A wrong or silent spec is fixed by an amendment, grilled and approved like the spec.

Long jobs that run mutated or untrusted code run only through `/mutants-run`, and the agent says when it starts one.

### Plugins

Techniques from installed plugins (such as superpowers' brainstorming, test-driven development and debugging skills) may be used in any stage. Where a plugin's defaults differ from this workflow, the workflow wins: specs, plans and logs go in the chunk folder from [the templates](chunks/_template/), never in a plugin's own location such as `docs/superpowers/`. Nothing here requires a plugin.

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
