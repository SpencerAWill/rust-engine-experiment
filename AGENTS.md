# Guide for AI coding assistants

Read this first. The rules are in [CONTRIBUTING.md](CONTRIBUTING.md), the workflow in [docs/workflow.md](docs/workflow.md), and what is planned in [docs/roadmap.md](docs/roadmap.md).

## Commands

```sh
cargo xtask ci          # every check CI gates on; run it before saying work is done
cargo xtask ci --quick  # fmt check, clippy, and tests only
cargo xtask test        # nextest plus doctests
cargo xtask bench-regress  # the instruction-count gate (Valgrind: devcontainer); run it when you change libs/ or a benchmark
```

Coverage, the nightly fuzz run and the weekly mutants run are report-only and outside `ci`. The other commands are in the README.

## Rules that fail CI (do not work around them)

- One library crate: new library code is a module of `engine` (a new crate needs a criterion from [ADR 0004](docs/adr/0004-crate-splitting-rule.md)).
- No `unsafe` (`#![forbid(unsafe_code)]` in every crate root).
- Determinism: no `HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code. No global mutable state.
- Frozen formats: the PRNG output streams and `UntypedGenId::to_bits` (which `GenId::to_bits` forwards to) do not change without an ADR and updated golden tests.
- Every public item is documented and public types have runnable doctests. A documented panic has `#[track_caller]`, a `# Panics` section, an actionable message, and a fallible `try_` twin.
- No code copied from other projects ([ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md)).
- The rest (fmt, clippy with warnings denied, docs, `no_std` and wasm builds, workflow rules) is in [CONTRIBUTING.md](CONTRIBUTING.md#what-ci-enforces), "What CI enforces".

## The loop

Work is done in chunks from the roadmap, each through seven stages: brainstorm, grill, plan, work, review, compound, close out ([docs/workflow.md](docs/workflow.md)). In Claude Code, `/chunk next` starts one and `/chunk resume` continues one.

- **Wait for the maintainer only twice:** spec approval (at the end of the grill) and the merge go-ahead. Between them, move to the next stage without asking.
- **Roles.** The main session writes documents, dispatches agents (`.claude/agents/`) and commits. A `builder` (Sonnet) writes product and test code; independent reviewers check it and never see the builder's reasoning.
- **Stop and ask** when:
  - the spec is wrong or silent on something that changes what gets built;
  - a change would touch an ADR, a frozen format or a CI-enforced policy the spec does not name;
  - a reviewer and the builder still disagree after two fix rounds;
  - an action is outward-facing beyond the chunk's branch, pull request and follow-up issues.
- **Pushes to `main` and force pushes are blocked** for agents by a hook (`.claude/hooks/guard-push.sh`). Do not work around it; the maintainer runs such a push themselves.
- **Write it down as it happens** in the chunk's `log.md` (decisions, surprises, `lesson?` candidates). Agents get killed; what is not on disk is lost.
- **Lessons** in [docs/lessons/](docs/lessons/) are retrieved by path when planning. Follow them by default; say why when you deviate.
- **Plugins** may be used inside any stage, but this workflow decides where files go: specs, plans and logs live in `docs/chunks/<id>-<slug>/`, never in a plugin's own folder.

## Always

- **Test first.** For a bug fix, write the test, watch it fail for the reason you expect, then fix the code. Never weaken or delete a failing test without saying so.
- **Verify before claiming.** Run the command and read its output; do not infer success from the diff.
- **Small logical commits**, conventional messages ending with the chunk ID, files staged by explicit path. Say in the pull request that an assistant was used.
- **Report honestly**: a failed test, a skipped step or a noisy measurement is said, with the evidence. Benchmark claims need a saved baseline and the noise floor (`/bench-run`, `cargo xtask bench-report`).
- **Docs**: new documents get a status line; hard-to-reverse decisions get an ADR; spec section numbers stay stable; write the minimum and link instead of copying.
- Mutation testing and other runs of mutated code go through `/mutants-run` (a memory-capped container), never bare on the host.

## Where to look

| Working on | Read |
|---|---|
| `libs/engine/` | CONTRIBUTING.md "What CI enforces" and "Tests"; the module's lessons |
| benchmarks, `bench/` | CONTRIBUTING.md "Benchmarks" and "Instruction-count gate" |
| fuzzing, coverage, mutants | CONTRIBUTING.md, the section of that name |
| `spikes/` | [spikes/README.md](spikes/README.md) |
| CI, workflows, `infra/` | CONTRIBUTING.md "What CI enforces" (workflows); [infra/github/README.md](infra/github/README.md) |
| a chunk's documents | [docs/chunks/](docs/chunks/), templates in [docs/chunks/_template/](docs/chunks/_template/) |
