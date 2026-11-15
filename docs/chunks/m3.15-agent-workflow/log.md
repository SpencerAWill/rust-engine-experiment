# M3.15 log: agent workflow, lessons and documentation layout

- **Status:** Living (2026-11-15). Append-only; one or two lines per entry (spec §4.3).

- 2026-11-15 decision Approach B (own `.claude/` layer, compound-engineering ideas, no plugin); stops at spec approval and merge go-ahead only.
- 2026-11-15 decision Reviews by independent agents: code, security, conditional on Opus; docs, spec conformance on Sonnet. Builders on Sonnet.
- 2026-11-15 decision "Lessons", not "rules": followed by default, deviations recorded with a reason. Promoted by direct edits in the chunk PR.
- 2026-11-15 surprise Claude Code skips `AGENTS.md` when a `CLAUDE.md` exists; hence `CLAUDE.md` = `@AGENTS.md`.
- 2026-11-15 decision Builder check answered: depends on M3.14 (now merged), resume trial by a cold agent, mutants trial `--module random` in Docker, main-session model not pinned, push guard in Rust behind a sh pre-filter.
- 2026-11-15 decision Grill stage added between spec draft and approval, our own skill from Matt Pocock's method (option 3; ADR 0002 rules out a copy without an exception).
- 2026-11-15 done Worktree dropped after PR #56 merged; branch rebased onto `main` in the main checkout.
- 2026-11-15 decision Grill round 1 (all accepted): this chunk follows the new loop after spec approval (no plan stop); split into M3.15 (workflow) and M3.16 (documentation layout); `/simplify` once over the chunk diff before reviewers; plugin techniques allowed in any stage, but the workflow decides file locations and formats.
- 2026-11-15 done Spec rewritten for the split (§4.7 kept as a stub, §4.12 hands scope to M3.16, §4.13 bootstrapping); roadmap rows M3.15 and M3.16 added.
- 2026-11-15 decision Grill round 2 (all accepted): main session merges after the go-ahead and starts the next chunk; follow-up issues may be opened without asking; flat `docs/chunks/<id>-<slug>/`, existing folders moved in M3.15; IDs keep the `M<N>.<n>` form.
- 2026-11-15 done Spec accepted by the maintainer ("go ahead"); plan written (no plan stop, spec §4.13).
- 2026-11-15 done Step 2 (builder, Sonnet, dispatched as general-purpose: agent files not yet registered). 20 folders moved with `git mv`; links fixed in 10 files plus the moved docs; `cargo xtask ci --quick` passed.
- 2026-11-15 surprise The link check found 11 broken relative links before the move, not 0: 8 are quoted example links inside the M2.0 plan's prose, 1 is a stale `handle_model.rs` link in the M2.1 spec (renamed `genid_model.rs` in M3.11). 9 remain, all pre-existing in historical records; left for M3.16's `check-docs` to decide how to treat.
- 2026-11-15 lesson? `git log --follow` returns nothing for a path that exists only in the index: verify history after committing a move.
- 2026-11-15 lesson? Moving docs one level up costs more in relative `../` fixes inside the moved files than in links from outside them.
