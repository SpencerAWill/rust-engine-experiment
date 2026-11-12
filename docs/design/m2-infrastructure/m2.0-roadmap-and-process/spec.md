# M2.0: Roadmap and process

- **Status:** Accepted (2026-11-12)
- **Milestone:** M2 (process and verification infrastructure) · **Depends on:** none · **Decides:** no ADR (process conventions, recorded in `docs/README.md`, `CONTRIBUTING.md` and `AGENTS.md`)
- **Estimated size:** about 900 changed lines of Markdown, plus 1,483 lines restored verbatim (the M0+M1 plan)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

M0+M1 landed as one pull request of about 19,600 lines behind a 1,200-line spec, which was too large to review, slow to give feedback, hard to learn from, and expensive to plan. This chunk replaces milestone-sized work with **chunks**: one small pull request each, with its own short spec and plan, tracked in a new `docs/roadmap.md`. At the end, the roadmap, the spec and plan templates, and the updated conventions are in the repository, and this spec is the first one written to the new template.

## 2. Goals and non-goals

**Goals**

- A standing roadmap that shows every milestone's goal and exit criteria, with the current milestone broken into chunks.
- A spec template and a plan template that make each chunk's design precise enough to implement and review in one sitting.
- Conventions (folders, numbering, statuses, approval gates, pull request titles, issues, tags) written where people and agents will find them.
- The milestones renumbered into smaller goals, with a mapping so older documents stay readable.
- The M0+M1 plan restored as a historical record, since plans are now kept.

**Non-goals**

- No code, CI, xtask or GitHub configuration changes. In particular no labels, GitHub Milestones or Projects, and no `xtask check-specs` (requirement traceability stays in the spec documents for now).
- No edits to ADRs or `docs/benchmarks.md`. They predate the renumbering and are read through the mapping table (§4.2).
- No detailed chunking of M3 or later (rolling horizon, §4.1).
- The unrelated fixes found while planning are separate pull requests: the stale `docs/adr/0010-ffi-unsafe.md` example in `unsafe-allowlist.toml`, the `javascript` label missing from `infra/github/index.ts`, and Dependabot PR #10 (a TypeScript 5 to 7 major update).

## 3. Guide-level explanation

To find out what is next, open `docs/roadmap.md`. It lists the milestones (M2 to M9 and "Later"). The current milestone has a chunk table; the first row whose dependencies are `Done` and whose status is `Planned` is the next chunk.

To work on chunk M2.1:

1. Create the branch `m2.1-<topic>` and the folder `docs/design/m2-infrastructure/m2.1-<topic>/`.
2. Copy `docs/design/_template/spec.md` and `plan.md` into it and fill them in. Set the chunk's roadmap row to `Drafting`.
3. Ask the maintainer to review the spec and plan together (one gate). If the chunk adds or changes an ADR, a public API, or a frozen format, the spec is reviewed and accepted first, and the plan is written afterwards (two gates).
4. On approval, set both documents to `Accepted` and the roadmap row to `In progress`, and implement the plan step by step. Anything that differs from the plan is recorded in the plan's Deviations section.
5. Open a pull request titled like `feat(fuzz): fuzz HandlePool against a model (M2.1)`. The pull request template asks for the chunk ID and links to the spec and plan.
6. Before merging: the spec becomes `Implemented (PR #n, date)`, the plan becomes `Historical`, and the roadmap row becomes `Done (PR #n)`. The maintainer gives the merge go-ahead.

When the last chunk of a milestone merges, the maintainer tags that commit (`m2`, `m3`, ...) and the next milestone is broken into chunks in the roadmap.

Ideas, bugs and deferred questions that are not yet scheduled go into GitHub issues. When one is scheduled, a chunk spec links it and the chunk's pull request closes it.

## 4. Reference-level design

### 4.1 Chunks and milestones

- A **chunk** MUST be delivered as exactly one pull request. It SHOULD change about 500 to 1,500 lines, excluding tests, lockfiles and restored or generated files, introduce about one new concept, and end with something that can be run or observed (a test, an example, a benchmark result, a CI job, or a rendered document).
- A chunk is named `M<milestone>.<n>`, with `n` counting from 0 within the milestone. Chunk numbers MUST NOT be reused or renumbered. If a chunk is split during work, the split-off part takes the next free number in that milestone. Order is given by the dependency column, not by the number.
- A **milestone** is a goal with exit criteria, reachable in a handful of chunks. Only the current milestone is broken into chunks (the **rolling horizon**); later milestones carry a rough list of expected chunks, refined when they become current.

### 4.2 `docs/roadmap.md`

The roadmap MUST contain, in this order:

1. A status line and one paragraph explaining how to read it and how it relates to `docs/design/`.
2. A milestone overview table: milestone, goal, exit criteria, status (`Done`, `Current`, `Next`, `Later`).
3. For the current milestone, the **chunk table**: chunk ID, title, depends on, status, and links to its spec and plan once they exist. Chunk statuses are `Planned`, `Drafting`, `Accepted`, `In progress`, `Done (PR #n)` and `Dropped (reason)`.
4. For each later milestone, a short section with: rough chunks, exit criteria, and the open question it resolves.
5. A **milestone numbering note** with the mapping below, stating that documents dated before 2026-11-12 use the old numbers.
6. The note that going public is not a milestone but the checklist in `docs/maintainers.md`, run at a milestone boundary chosen by the maintainer.

Milestones, as approved on 2026-11-12:

| New | Goal | Exit criteria | Old number |
|---|---|---|---|
| M0+M1 | Engineering foundations and core primitives | Done (PR #3) | M0+M1 |
| M2 | Process and verification infrastructure | Roadmap and templates in place; nightly fuzzing green; coverage report in CI; first mutation triage recorded; Gungraun gate active on pull requests | "start of M2" in M0+M1 spec §4.12 |
| M3 | ECS storage decision | ADRs accepted for the storage model, component identity, storage of foreign and mod-defined components, and the transform component layout, backed by results recorded in `docs/benchmarks.md` | M2 (first half) |
| M4 | ECS core | World, entities, components and queries on the chosen storage; registry core; the dogfooding sample runs and its determinism test passes; core operations within noise of the winning M3 prototype | M2 |
| M5 | Reflection and persistence | The sample world saves, loads and reproduces its checksum; a separate tool prints a saved world without being compiled against its types | M2 |
| M6 | Change tracking and edits | Applying edits then undoing them restores the original world hash; `diff(A, B)` applied to A yields B | M2 |
| M7 | Scheduler | The sample runs its systems in parallel with identical checksums at thread counts 1 to N | M3 |
| M8 | C ABI and C# | A C# program drives the world and defines a component at runtime | M4 |
| M9 | Diagnostics and tooling | An external tool connects to the running sample, browses entities and edits a field | M5 |

M2 chunks:

| Chunk | Title | Depends on |
|---|---|---|
| M2.0 | Roadmap and process (this spec) | none |
| M2.1 | Fuzzing: pinned nightly, `fuzz/` targets for `HandlePool`, the allocators (operation sequences against a model, plus `check_invariants`) and `Handle::from_bits`; `cargo xtask fuzz`; nightly CI job | M2.0 |
| M2.2 | Coverage: `cargo-llvm-cov` through xtask; report uploaded in CI; no threshold gate | M2.0 |
| M2.3 | Mutation testing: `cargo xtask mutants`, scheduled report-only workflow, first triage of `engine_handle` and `engine_random` | M2.2 |
| M2.4 | Gungraun gate: Valgrind and gungraun-runner in the devcontainer and CI; benchmarks for handle and PRNG hot paths; merge-base against head comparison; fails above 5%; `perf-regression-accepted` label declared in Pulumi | M2.0 |

**Miri is deferred out of M2.** M0+M1 spec §4.12 placed it at the start of M2, but its trigger is the first allowlisted `unsafe` crate, and none exists. It arrives with the first chunk that proposes `unsafe` (M3 if the spike argues for type-erased columns, M8 at the latest). The roadmap MUST state this.

Rough chunks and open questions for later milestones, as approved on 2026-11-12:

| Milestone | Rough chunks | Open question |
|---|---|---|
| M3 | `engine_collections` (deterministic, fixed-seed map and set); spike harness and workloads (simple and fragmented iteration, add/remove, heavy compute, entity churn, 100k 2D sprites) with bevy_ecs and hecs baselines; sparse-set prototype; archetype prototype; per-component hybrid prototype; runtime-registered components in safe byte columns; decision ADRs and a `docs/benchmarks.md` section | Whether the prototype code is deleted or kept under `spikes/` (decided in its spec) |
| M4 | `engine_ecs`: World, Entity (on `engine_handle`), typed and runtime component registration, spawn/despawn, insert/remove; queries with with/without filters; invariant checks and readable `Debug`; debugger visualizers; Gungraun benchmarks for storage hot paths; the dogfooding sample (2D agents in a pixel-buffer window; the windowing crate is example-only) | Whether resources (singleton data) belong in M4 or M7. Entity relationships stay out |
| M5 | Reflection core traits at the bottom of the crate graph; field read and write by path; self-describing serialization; stable content hasher (a frozen format, so an ADR); persistent IDs with remapping on load | The serialization format: our own or an existing self-describing one |
| M6 | Change detection (added or changed since a tick); invertible edit commands (the basis for undo/redo); world diffing | Its first spec decides whether change detection is tick-based or relative to each system's last run; the latter moves M7 before M6 |
| M7 | Systems with access sets; explicit ordering and ambiguity detection; single-threaded mode; parallel execution on host-provided threads; loom, ThreadSanitizer, stress tests and the concurrency allowlist | The threading model (thread pool, fibers, work stealing) |
| M8 | `engine_ffi` (the first allowlisted `unsafe` crate, so Miri arrives here at the latest); generated header; C# binding and sample; FFI panic policy; evaluation of WASM component-model mods | Binding generator; ownership across the boundary; native or WASM mods |
| M9 | Tracing facade; profiler integration; remote inspection protocol; record/replay with desync detection | Protocol transport and encoding |
| Later | Windowing and input; 2D then 3D rendering; assets; audio; physics; mod loading; optional app runner; the editor | Unnumbered until scheduled |

### 4.3 Folders and files

- Milestone folders are `docs/design/m<N>-<topic>/`; chunk folders are `docs/design/m<N>-<topic>/m<N>.<n>-<topic>/` containing `spec.md` and `plan.md`. M2's folder is `docs/design/m2-infrastructure/`. The existing `docs/design/m0-m1-foundations/` stays as it is.
- The templates are `docs/design/_template/spec.md` and `docs/design/_template/plan.md`.
- Plans MUST be kept after merge, with their status set to `Historical`.

### 4.4 The spec template

The template MUST have a header (title `M<N>.<n>: <title>`, Status, Milestone, Depends on, Decides, Estimated size), the RFC 2119 note, and these sections, numbered as shown so they can be cited:

1. **Summary**: three sentences: what, why now, and what can be run at the end.
2. **Goals and non-goals.**
3. **Guide-level explanation**: how a user or contributor uses the result; code samples here become doctests.
4. **Reference-level design**: types, signatures, semantics, invariants, errors and panics; normative.
5. **Requirements**: numbered `M<N>.<n>-R<k>`, each one testable and using an RFC 2119 keyword.
6. **Cross-cutting concerns**: a fixed checklist answered line by line, with `n/a` and a reason where it does not apply: determinism; `unsafe`; concurrency; heap allocation; `no_std` and wasm; frozen formats; public API changes; FFI exposure; performance (what is measured, against which baseline); documentation; policy changes.
7. **Alternatives considered**, each with the reason it was rejected.
8. **Verification**: which test, command or review step proves each requirement.
9. **Done when**: an objective checklist.
10. **Open questions**: MUST be empty, or each item deferred to a named chunk, before the spec is `Accepted`.
11. **Prior art and sources.**

Spec statuses are `Draft (date)`, `Accepted (date)`, `Implemented (PR #n, date)`, `Superseded by <link> (date)` and `Dropped (reason, date)`. A spec is changed in place while it is a draft; after it is accepted, a change to its design is noted in the plan's Deviations section and, if it matters after merge, in the spec with the date.

### 4.5 The plan template

The template MUST have a header (title `M<N>.<n> plan: <title>`, Status, link to the spec) and these sections:

1. **Approach**: a few sentences, including any choices the spec leaves to implementation.
2. **Steps**: each step names the requirements it satisfies, the files it touches, the test written first and the failure expected (or, for documentation, the check that fails before the change), the change, the verification command and its expected result, and the commit message (conventional, ending with the chunk ID in parentheses).
3. **Final verification**: `cargo xtask ci`, and the spec's Done-when checklist.
4. **Deviations**: filled in during implementation: what differed from the plan and why.

Plan statuses are `Draft (date)`, `Accepted (date)`, and `Historical: executed in PR #n (date); the spec and the code are authoritative`.

### 4.6 Approval gates

- By default the maintainer reviews a chunk's spec and plan together, then gives a separate go-ahead to merge.
- A chunk that adds or changes an ADR, a public API, or a frozen format has its spec accepted before its plan is written.
- Merging always needs the maintainer's explicit go-ahead (unchanged).

### 4.7 GitHub conventions

- Pull request titles are conventional and end with the chunk ID: `feat(fuzz): fuzz HandlePool against a model (M2.1)`.
- Issues hold unscheduled work; the roadmap holds scheduled work. A chunk spec links any issue it addresses.
- The commit that completes a milestone gets an annotated tag `m<N>`, created by the maintainer.
- GitHub Milestones and Projects are not used while the roadmap is the source of truth; this is revisited when the repository is public and has outside contributors.

## 5. Requirements

- **M2.0-R1** `docs/roadmap.md` MUST exist with the content and order of §4.2.
- **M2.0-R2** `docs/design/_template/spec.md` and `docs/design/_template/plan.md` MUST exist with the structure of §4.4 and §4.5, with a short instruction under each heading saying what goes there.
- **M2.0-R3** This spec and its plan MUST follow those templates.
- **M2.0-R4** `docs/README.md` MUST describe the roadmap, the folder layout of §4.3, the templates, that plans are kept as `Historical`, and the old milestone numbering. The rule that plans are removed at merge MUST be gone.
- **M2.0-R5** `docs/maintainers.md` MUST replace "Before merging a milestone" with a per-chunk merge checklist (CI green, spec `Implemented`, plan `Historical`, roadmap row `Done`, `CHANGELOG.md` updated, maintainer go-ahead) and a "When a milestone completes" checklist (tag `m<N>`, chunk the next milestone in the roadmap). The step "Remove the implementation plan" MUST be gone.
- **M2.0-R6** `.github/PULL_REQUEST_TEMPLATE.md` MUST ask for the chunk ID and links to the spec and plan, and include the per-chunk documentation items of R5. Existing checklist items stay.
- **M2.0-R7** `CONTRIBUTING.md` MUST describe how work is organized (roadmap, chunks, templates, pull request titles with the chunk ID, issues for unscheduled work), linking to the roadmap and templates rather than repeating them.
- **M2.0-R8** `AGENTS.md` MUST point to the roadmap and templates and state the approval gates of §4.6.
- **M2.0-R9** The M0+M1 spec MUST gain a note near the top that it uses the old milestone numbering and that its §11 is superseded by `docs/roadmap.md`, and its reading guide and §3 workspace tree MUST no longer say that the plan was removed. Its sections MUST NOT be renumbered or deleted.
- **M2.0-R10** `docs/design/m0-m1-foundations/plan.md` MUST be restored from commit `0dc227c^` (reachable from `refs/pull/3/head`), with only its status block replaced by `Historical: executed in PR #3 (2026-11-12)`; the rest MUST be byte-identical.
- **M2.0-R11** The `README.md` status paragraph MUST point to the roadmap instead of naming M0 and M1 as the current scope.
- **M2.0-R12** `CHANGELOG.md` MUST record the process change under `[Unreleased]`.
- **M2.0-R13** `cargo xtask ci` MUST pass, and every relative link in the added or changed Markdown files MUST resolve to an existing file.

## 6. Cross-cutting concerns

- **Determinism:** n/a, documentation only.
- **`unsafe`:** n/a. The roadmap records that Miri moves to the first chunk that proposes `unsafe` (§4.2).
- **Concurrency:** n/a.
- **Heap allocation:** n/a.
- **`no_std` and wasm:** n/a.
- **Frozen formats:** n/a, none touched.
- **Public API changes:** none.
- **FFI exposure:** n/a.
- **Performance:** n/a, nothing measured.
- **Documentation:** this chunk is documentation. Every new document carries a status line (`docs/README.md` convention).
- **Policy changes:** yes, called out in the pull request: plans are kept instead of removed; milestones are renumbered; approval gates are written down; Miri is deferred out of M2 (an amendment to M0+M1 spec §4.12's timing, not to its trigger).

## 7. Alternatives considered

- **Keep milestones as the unit of work, with internal phases.** Rejected: that is what made M0+M1 too large to review and learn from.
- **Medium chunks (one crate or feature, about 3,000 lines).** Rejected: review and learning stay heavy.
- **Break every milestone into chunks now.** Rejected: most of it would be rewritten after the M3 storage decision.
- **Keep the old milestone numbers, with a large M2 whose second half is provisional.** Rejected: the restructured milestones are each reachable in a handful of chunks, and no code refers to milestone numbers, so renumbering only costs a mapping table.
- **Put the plan in the pull request description.** Rejected by the maintainer: a pull request description says what changed and why; a step-by-step plan does not belong there.
- **Delete plans at merge (the M0+M1 convention), or fold the plan into the spec.** Rejected: deletion costs a step on every small pull request and loses the record (the M0+M1 plan was only recoverable from a pull request ref after the squash merge); folding mixes design with sequencing.
- **Track chunks in GitHub issues, Milestones or Projects, alone or alongside the repository.** Rejected: specs would leave the repository, or status would live in two places.
- **Requirement IDs in test annotations, checked by `xtask check-specs`.** Deferred: the spec's Verification section maps requirements to tests by name; a checker is added if the mappings drift.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1, R2, R3 | Maintainer review against §4.2, §4.4 and §4.5 |
| R4 to R9, R11, R12 | Maintainer review of the diff; the stale-wording grep in plan Step 4 finds nothing |
| R10 | `diff <(git show 0dc227c^:docs/design/m0-m1-foundations/plan.md) docs/design/m0-m1-foundations/plan.md` shows only the status block |
| R13 | `cargo xtask ci`, and a one-off link check over the changed Markdown files (in the plan) |

## 9. Done when

- [ ] R1 to R13 are met and verified as in §8.
- [ ] The roadmap shows M2.0 as `Done (PR #n)`, this spec is `Implemented`, and its plan is `Historical`.
- [ ] The pull request calls out the policy changes listed in §6.
- [ ] The maintainer has given the merge go-ahead.

## 10. Open questions

None.

## 11. Prior art and sources

The spec template combines these:

- Rust RFC template (summary, motivation, guide-level and reference-level explanation, alternatives, prior art, unresolved questions): https://github.com/rust-lang/rfcs/blob/master/0000-template.md
- RFC 2119 and RFC 8174 (requirement keywords): https://www.rfc-editor.org/rfc/rfc2119, https://www.rfc-editor.org/rfc/rfc8174
- Design docs at Google (goals and non-goals, alternatives, cross-cutting concerns): https://www.industrialempathy.com/posts/design-docs-at-google/
- Kubernetes Enhancement Proposal template (test plan, graduation criteria): https://github.com/kubernetes/enhancements/tree/master/keps/NNNN-kep-template
- EARS requirement syntax: https://alistairmavin.com/ears/
- GitHub Spec Kit (spec, plan and tasks for AI-assisted development; requirement IDs): https://github.com/github/spec-kit
- Diátaxis (documentation kinds, already used by `docs/README.md`): https://diataxis.fr/
