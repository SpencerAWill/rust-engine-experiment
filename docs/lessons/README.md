# docs/lessons/

- **Status:** Living (2026-11-15). The workflow that writes and reads these is in [docs/workflow.md](../workflow.md).

A lesson is guidance an agent or contributor follows by default and may deviate from, saying why. Lessons are written when a chunk learns something non-obvious, and read when a later chunk's plan touches the same paths. Nothing loads all of them.

## When this applies

- You plan a chunk: `lessons-researcher` returns the lessons whose `applies_to` matches the paths the spec touches.
- You change files a lesson applies to, or write or edit a lesson (stage 6 of the [workflow](../workflow.md)).

## Contents

- [`bench.md`](bench.md): what the instruction-count gate counts as relevant.
- [`docs.md`](docs.md): keeping rules in one place across the documents agents read.
- [`ecs.md`](ecs.md): ECS prototypes and queries: path tests, per-query costs, sealed query traits, failure paths, borrows that must not resize, red checks through workloads, hot-path parity with sibling backends, GAT lifetimes in seal traits, fetch lengths that keep loops vectorized, fixed lengths for byte moves of runtime size.
- [`perf.md`](perf.md): inlining hot paths, and measuring heap bytes with `dhat` and wall clock against baselines.
- [`specs.md`](specs.md): writing specs and plans whose checks can pass.
- [`testing.md`](testing.md): writing tests that cannot hang or exhaust memory under mutation testing.
- [`tooling.md`](tooling.md): hooks, permission rules, skills and container scripts for agents.
- [`xtask.md`](xtask.md): writing `cargo xtask` commands that read the repository.

## Format

One file per topic. Its frontmatter lists the paths the lessons apply to; the planning stage matches the paths a spec touches against them.

```markdown
---
applies_to:
  - "libs/engine/src/collections/**"
---
# Lessons: <topic>

- Status: Living (<date>)

- **<topic>-<n>.** The lesson, in one or two sentences. *Why:* what went wrong without it. *Deviate when:* (optional) the case it does not fit. *Retire when:* (optional) the change that makes it obsolete. *From:* <chunk> · *Confirmed:* <date>
```

- IDs are never reused. A retired lesson is deleted; git history is the archive.
- **Capture gate:** write a lesson only if it is non-obvious from the code, likely to recur, and costly to miss. If a test, lint or `xtask` check can enforce it, write that instead.
- **Checked:** `cargo xtask check-docs` enforces this format; its globs use `*`, `?` and `**` only.
- **Edit one bullet at a time.** Search for an existing lesson on the same point and edit it rather than adding a second. Never regenerate a file.
- **Upkeep:** `/lessons-refresh` at each milestone boundary keeps, edits, merges or deletes each lesson against the code.
