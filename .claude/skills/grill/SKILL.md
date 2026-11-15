---
name: grill
description: Grill a draft spec (or plan, ADR or design doc) with the maintainer until no decision is left open - a cold read first, then rounds of numbered questions with recommended answers, each answer written into the document as it is given. Use after drafting a spec and before asking for its approval, or when asked to "grill me" on a design.
argument-hint: "<path to the spec or design document>"
---

# Grill

Turn a draft into one the maintainer can approve without surprises. Method from Matt Pocock's [`grilling` skill](https://github.com/mattpocock/skills/tree/main/skills/productivity/grilling) (MIT), written for this repository; the workflow is in [docs/workflow.md](../../../docs/workflow.md#the-grill).

Document: `$ARGUMENTS` (default: the `spec.md` of the chunk on the current branch). Its chunk's `log.md` is the record. If the document is already `Accepted`, what you change is an amendment: the maintainer approves it like the original, and the status line notes the amendment and its date.

**Grilling probes the design, never the maintainer.** Every question is an open decision or a challenge to an assumption, tied to what it would change. Never ask a question whose answer you are testing for.

## 1. Cold read

Dispatch `spec-reader` (`.claude/agents/spec-reader.md`) with only the document's path. It returns the questions a builder would have to guess at, ranked.

## 2. Sort

For each question, and anything you noticed yourself:

- **Answered already** by the repository or the conversation: write the answer into the document.
- **Minor**, a sensible builder could settle it without changing what gets built: settle it, write it in marked as yours ("*(main session's call)*"), and keep a list of these calls. One `log.md` line lists them.
- **A fact** you can look up (in the code, the docs, or the web): look it up, or send a read-only agent. Facts are never asked.
- **The maintainer's:** only they can answer it, and the answer changes what gets built. These form the rounds.

## 3. Rounds

Each round asks every open decision whose prerequisites are settled. In one message:

- number them;
- for each: the question in plain words, why it matters (what it changes), your recommended answer and its trade-off; when there are named options, sketch each briefly;
- word each so that "yes" accepts your recommendation.

Before sending the round, commit the document and `log.md` with step 2's edits, so nothing is lost while you wait. Keep a round to about five questions; if more are open, ask the ones the others depend on first.

Then wait. After the answers, recompute what is open: an answer can settle other questions or open new ones (a split, a new dependency). Repeat.

## 4. Record as it goes

Before the next round, write every answer into the document (as decided, in its own section, not as a transcript) and add one `decision` line per answer, or one line per round, to `log.md`. Commit the document and log. A killed session must lose nothing.

## 5. End

When nothing is open:

1. List the calls you made yourself (step 2's minor list and anything else you decided), so the maintainer can check them while reading.
2. Give the document's path and ask for approval.

Approval is the spec wait of the chunk loop. On approval, return to the loop ([stage 2](../chunk/stages/2-grill.md)).
