---
name: chunk
description: Run this repository's chunk loop - brainstorm, grill, plan, work, review, compound, close out - for the next chunk on the roadmap, or resume one in progress. Use when starting, continuing or finishing roadmap work (a chunk M<N>.<n>), or when asked "what's next".
argument-hint: "next | resume | <stage: brainstorm, grill, plan, work, review, compound, close-out>"
---

# Chunk loop

The loop is described for people in [docs/workflow.md](../../../docs/workflow.md). This skill runs it. You are the **main session**: you talk with the maintainer, write the documents, dispatch the other roles (`.claude/agents/`), check their work and commit it. You do not write product or test code in the work and review stages; a `builder` does.

Argument: `$ARGUMENTS` (empty means `resume` if a chunk is in progress on the current branch, else `next`).

## Two waits, nothing else

Wait for the maintainer only at:

1. **Spec approval**, at the end of the grill.
2. **Merge go-ahead**, after the pull request is open, its close-out commit pushed.

Between them, go from one stage to the next without asking. Stop and ask only for an escalation in [AGENTS.md](../../../AGENTS.md#the-loop) ("Stop and ask"); a wrong or silent spec is fixed by an amendment, grilled and approved like the spec. "Outward-facing" means anything beyond the chunk's branch, its pull request and its follow-up issues.

Mutation testing and other runs of mutated or untrusted code go through `/mutants-run` only; say when you start one.

## Dispatching roles

Use the agent files in `.claude/agents/` by name. If the session does not have them registered (they load at session start), dispatch a general-purpose agent with the file's `model`, and as its prompt: "Read `.claude/agents/<role>.md` and follow its body", then the brief. Note in `log.md` which way you dispatched.

A brief names the chunk folder, the spec path, the exact task (plan step, diff range, or spec path), and the lesson IDs from the plan's Lessons section. A reviewer's brief never includes your reasoning or the builder's.

## Plugins

Plugin techniques (brainstorming, test-driven development, debugging, verification) may be used inside any stage. Where a plugin's defaults differ, this loop wins: specs, plans and logs live in `docs/chunks/<id>-<slug>/` from `docs/chunks/_template/`, never in a plugin's own folder (such as `docs/superpowers/`), and the plan has no extra stop.

## next

1. Check the working tree is clean and you are on `main`, up to date with `origin/main`. If not, report and stop.
2. Read [docs/roadmap.md](../../../docs/roadmap.md). Pick the next chunk: if the chunk just merged has a spec whose header names a **Followed by** chunk, and that chunk is `Planned` with its dependencies `Done`, it goes next; otherwise the first `Planned` row of the current milestone, in the roadmap table's order, whose dependencies are all `Done`. The maintainer may name another. If no row is eligible (a milestone boundary, or everything blocked), say so, name the checklist in `docs/maintainers.md` that is due, and wait; do not draft the next milestone's rows yourself.
3. Create the branch `m<N>.<n>-<slug>` and run stage 1: [stages/1-brainstorm.md](stages/1-brainstorm.md).

## resume

Reconstruct state from disk only; do not trust memory of earlier conversation.

1. `git status --short`. If anything is uncommitted, list it and say what you think it is **before** touching anything; ask only if you cannot tell. Match each path against the plan's step file lists: changes in an unticked step's files are that step's work in progress (check them against the step, then finish, verify and commit it); changes in later steps' files are drafts, left uncommitted until their step; changes in a ticked step's files are a fix to it if `log.md` says so (commit it as a `fix:`); anything else is a stray to report.
2. Find the chunk: the branch name gives the ID; its folder is `docs/chunks/<id>-<slug>/`.
3. Read `log.md` (all of it), the spec's status line, and the plan's checkboxes, if a plan exists. `git log --oneline main..HEAD` shows the commits.
4. Decide the stage:
   - spec `Draft`: stage 1 or 2 (continue the grill if `log.md` records rounds);
   - spec `Accepted`, no plan: stage 3;
   - plan with unticked steps: stage 4, from the first unticked step;
   - all steps ticked, no Reviews recorded in `log.md`: stage 5;
   - reviews recorded, no PR: stage 6 then 7;
   - PR open: stage 7 (close-out commit, or waiting for the go-ahead).
5. Run `cargo xtask ci --quick`. If it fails, the failure is the first thing to fix (through a builder).
6. Say in one line where you are resuming and why, then continue that stage.

## <stage>

Run that stage's file and continue to the next stage unless it is a wait:

| Stage | File |
|---|---|
| brainstorm | [stages/1-brainstorm.md](stages/1-brainstorm.md) |
| grill | [stages/2-grill.md](stages/2-grill.md) |
| plan | [stages/3-plan.md](stages/3-plan.md) |
| work | [stages/4-work.md](stages/4-work.md) |
| review | [stages/5-review.md](stages/5-review.md) |
| compound | [stages/6-compound.md](stages/6-compound.md) |
| close-out | [stages/7-close-out.md](stages/7-close-out.md) |

## Always

- **Log as it happens:** append to `log.md` (`- <date> <tag> <text>`; tags `done`, `decision`, `surprise`, `dead-end`, `lesson?`) and commit it with the work it describes.
- **Verify before claiming:** run the command, read its output.
- **Stage by explicit path** (`git add <paths>`), never `-a`; check `git status` after adding.
