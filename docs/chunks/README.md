# Chunks

Status: Living (2026-11-15). One folder per chunk of the [roadmap](../roadmap.md): its spec, plan and log. The roadmap is the index of chunks, with each one's status and links.

## When this applies

- You start, resume or review a chunk ([workflow](../workflow.md)).
- You want to know how something was designed, or why a spec says what it says.

A spec is a living design document: where it differs from the code or an ADR, those win. A plan and a log are historical once their pull request merges.

## Contents

- `_template/`: `spec.md`, `plan.md` and `log.md`, copied into a new chunk's folder.
- `m0-m1-foundations/`: the first milestone's spec and plan, from before chunks existed.
- `m*/`: one folder per chunk, `m<N>.<n>-<slug>/`, flat (the milestone is a field in the spec header, not a folder level), holding `spec.md`, `plan.md` and `log.md` (chunks before M3.15 have no log).
