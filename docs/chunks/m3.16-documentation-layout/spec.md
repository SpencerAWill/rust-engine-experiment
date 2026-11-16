# M3.16: Documentation layout

- **Status:** Draft (2026-11-15)
- **Milestone:** M3 (not part of its exit criteria) · **Depends on:** M3.15 (merged in PR #59; its spec §4.12 hands this scope on) · **Decides:** no ADR (documentation conventions, recorded in `docs/README.md` and enforced by `cargo xtask check-docs`)
- **Estimated size:** about 620 lines of Rust in `xtask` plus about 400 lines of tests; about 900 changed lines of Markdown, most of them moved
- **Closes:** #58

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

`CONTRIBUTING.md` is mostly tool manuals, the root README carries the reference for every `xtask` command in one paragraph, most directories have no README, and nothing checks that links, indexes or lesson files stay correct (#58 lists links that had already broken). This chunk gives each directory that matters a README in one shape, moves each manual next to the tool it describes, adds `.claude/rules/` files that point Claude Code at the right README when it touches a path, and adds `cargo xtask check-docs`, a required CI check over READMEs, links, lessons, rule files and the length of `AGENTS.md`. It is the first chunk built entirely with the M3.15 loop.

## 2. Goals and non-goals

**Goals**

- A contributor or agent working in a directory finds its rules and how-tos in that directory's README, and Claude Code is routed there without reading `CONTRIBUTING.md` whole.
- `CONTRIBUTING.md` holds policy (what CI enforces, tests, commits, reviews) and links to the manuals.
- Every rule in a moved manual still exists, once, in its new place.
- Broken relative links, stale indexes, malformed lessons and rule files whose paths match nothing fail CI.

**Non-goals**

- No change to the engine library, its API, frozen formats or CI-enforced code rules, and no change to what any moved rule requires (moves may shorten wording, never meaning). The one change to a gate is §4.11.
- No check of external (`http`, `https`) links: it needs the network and fails at random.
- No status-line check for documents other than READMEs and lesson files.
- No README in `.github/`, `.devcontainer/`, `.cargo/`, `docs/research/`, `docs/data/` or any source subdirectory; the root `README.md` keeps its own shape (§4.1).
- No change to how lessons are retrieved by `lessons-researcher` (M3.15 §4.8).

## 3. Guide-level explanation

A builder edits `libs/engine/src/collections/bit_set.rs`. Claude Code loads `.claude/rules/engine.md`, whose body says to read `libs/engine/README.md` and to follow the lessons in `docs/lessons/` whose `applies_to` matches the file. The README links to the library rules in `CONTRIBUTING.md` ("What CI enforces", "Tests") instead of copying them.

Someone renames a heading in `CONTRIBUTING.md`. `cargo xtask check-docs` fails, listing every link elsewhere that pointed at the old anchor:

```text
AGENTS.md:24: link `CONTRIBUTING.md#what-ci-enforces`: no heading in CONTRIBUTING.md has the anchor `what-ci-enforces`
check-docs: 1 problem
```

Someone adds `fuzz/fuzz_targets/` siblings or a new file in `xtask/` without adding it to the README's Contents, and the check names the README and the missing entry.

## 4. Reference-level design

### 4.1 Which directories have a README

These directories MUST have a `README.md` in the shape of §4.2 (decided with the maintainer):

| Directory | README | Holds |
|---|---|---|
| `docs/` | reshaped | the index of `docs/` and the documentation conventions |
| `docs/adr/` | new | an index of the ADRs |
| `docs/chunks/` | new | the folder layout; the roadmap is the index of chunks (§4.2, wildcard entries) |
| `docs/lessons/` | reshaped | as now |
| `.claude/` | reshaped | as now, plus `rules/` |
| `infra/github/` | reshaped | as now |
| `libs/engine/` | new | the crate layout, linking to the library rules in `CONTRIBUTING.md` |
| `bench/` | new | the Benchmarks and Instruction-count gate manuals |
| `fuzz/` | new | the Fuzzing manual |
| `xtask/` | new | the command reference (from the root `README.md`), and the Coverage and Mutation testing manuals |
| `spikes/` | reshaped | the Spike crates manual merged with what is there |
| every `spikes/<topic>/` | reshaped | as now (`spikes/ecs_storage/`) |
| `examples/` | new | one line per example |

`check-docs` holds this list as a constant, plus every directory directly under `spikes/` that contains tracked files. The root `README.md` is the project's front page: it keeps its shape, loses the command paragraph (replaced by a link to `xtask/README.md`), and is not checked for shape, only for links.

### 4.2 README shape

The shape is recorded once, in `docs/README.md` "Conventions"; `check-docs` enforces it. A required README:

1. starts with a level-1 heading;
2. has a status line before its first level-2 heading: a line that starts with `Status:` or `- **Status:**` (`Living (<date>)` for a README);
3. has the level-2 headings `When this applies` and then `Contents`, in that order; `How to` and other level-2 sections MAY follow `Contents`.

(Points 2 and 3's freedom for extra sections are my call: `docs/README.md` keeps "Conventions" and `spikes/README.md` keeps "Rules" and "Layout and names", and the existing status rule in `docs/README.md` already covers READMEs.)

**Contents** (decided with the maintainer). Contents is a bullet list or a table, or both.

- An entry is a top-level bullet, or a table row other than the header row. Its name is the first inline code span or link in it, whichever comes first; for a link, the name is its target. An entry with no code span or link is prose and is ignored. A name that starts with `../`, `/` or a URL scheme is a cross-reference and is ignored.
- A name is a path relative to the README's directory, with any trailing `/` and `#anchor` removed. The child it covers is its first path segment.
- Every tracked file or directory directly inside the README's directory, other than `README.md`, MUST be covered by an entry. "Tracked" means listed by `git ls-files --cached --others --exclude-standard` (files in the index plus untracked files that are not ignored), so a local run sees new files before they are staged and CI, whose checkout has no untracked files, sees the same set *(main session's call)*; a directory is tracked when it contains a tracked file. Every check of this spec uses this set.
- Every entry's name MUST be a tracked file or directory.
- A name containing `*` is a wildcard: `*` matches any run of characters within one path segment, never `/`. It covers every child it matches and MUST match at least one. `docs/chunks/README.md` lists `_template/`, `m0-m1-foundations/` and `m*/`.

### 4.3 Lesson files

Every `docs/lessons/*.md` other than `README.md` MUST have the format in `docs/lessons/README.md`, checked as follows (the checks are my call; the format is M3.15's):

- frontmatter delimited by `---` lines, holding exactly the key `applies_to:` followed by one or more lines `  - "<glob>"` (§4.5);
- a level-1 heading starting `Lessons: `, then the line `- Status: Living (<date>)`;
- each other top-level bullet is a lesson. It starts with `**<stem>-<n>.**`, where `<stem>` is the file name without `.md` and `<n>` a positive integer unique within the file, and contains `*Why:*`, `*From:*` and `*Confirmed:*`.

### 4.4 Rule files

`.claude/rules/*.md` route Claude Code to a README (decided with the maintainer). Each is a pointer, never a copy of a rule: frontmatter in the restricted form of §4.3 with the key `paths:` instead of `applies_to:`, and a body of a few lines saying which README to read and to follow the lessons in `docs/lessons/` whose `applies_to` matches the file being changed.

| File | `paths:` | Points to |
|---|---|---|
| `engine.md` | `libs/engine/**` | `libs/engine/README.md` |
| `bench.md` | `bench/**`, `libs/engine/benches/**` | `bench/README.md` |
| `fuzz.md` | `fuzz/**` | `fuzz/README.md` |
| `xtask.md` | `xtask/**` | `xtask/README.md` |
| `spikes.md` | `spikes/**` | `spikes/README.md` and the topic's README |
| `examples.md` | `examples/**` | `examples/README.md` |
| `docs.md` | `docs/**`, `AGENTS.md`, `CONTRIBUTING.md`, `**/README.md` | `docs/README.md` (conventions) |
| `claude.md` | `.claude/**` | `.claude/README.md` |
| `ci.md` | `.github/**`, `infra/github/**` | `CONTRIBUTING.md` "What CI enforces" (workflows), `infra/github/README.md` |

**The load trial (R14).** In the work stage the main session runs a headless `claude -p` session limited to read-only tools and a few turns, which reads a file under `libs/engine/src/` and reports whether `.claude/rules/engine.md` is in its context (decided with the maintainer). If it is not, that is a spec amendment (most likely to the frontmatter form, which would change §4.3 to §4.5 with it): the main session stops and asks; the builder does not improvise another form.

### 4.5 Globs

The globs in `applies_to:` and `paths:` use a subset that Claude Code's matcher and `check-docs` agree on: `*` (any characters except `/`), `?` (one character except `/`) and `**` as a whole path segment (zero or more segments). `check-docs` MUST reject braces, brackets and a leading `!`, and MUST report a glob that matches no tracked file. The matcher is our own code in `xtask`, with unit tests (my call: no dependency for some 40 lines, and the subset is what the check promises).

### 4.6 Links

`check-docs` checks every relative link in every tracked `*.md` file (decided with the maintainer):

- Inline links and images, and reference definitions, outside code spans, code blocks and HTML. Targets with a URL scheme (`https:`, `mailto:`) are skipped.
- The target, percent-decoded and resolved against the file's directory, MUST stay inside the repository and be a tracked file or directory.
- A `#anchor` on a `.md` target, or alone (the same file), MUST equal the anchor of one of that file's headings, using GitHub's rule: lowercase; drop every character that is not a letter, digit, space, `-` or `_`; spaces become `-`; the second heading with the same anchor gets `-1`, the third `-2`. Anchors on other targets (such as `#L10` on a `.rs` file) are not checked.

Markdown is parsed with `pulldown-cmark`, default features off (my call: code spans, nested brackets and reference links are where a hand parser would be wrong; it is MIT-licensed and used only by `xtask`).

### 4.7 `AGENTS.md`

`AGENTS.md` MUST have fewer than 80 lines (M3.15 §4.12). Its "Where to look" table MUST point at the READMEs of §4.1 instead of sections of `CONTRIBUTING.md`, except where the policy itself is the target (`What CI enforces`).

### 4.8 The command and the gate

- `cargo xtask check-docs` takes no flags, runs every check of §4.2 to §4.7, prints each problem as `<path>:<line>: <message>` (line omitted when there is none), and exits non-zero if there is any. It reports all problems, not the first.
- It runs in `cargo xtask ci` (the full run, after `check-workflows`), not in `--quick`.
- It is its own job in `.github/workflows/ci.yml`, built like `check-workflows`, and `"check-docs"` is added to `requiredChecks` in `infra/github/index.ts` (decided with the maintainer). The maintainer runs `pulumi preview` and `pulumi up` after the merge, not before: a required check that never reports blocks every open pull request.

### 4.9 Moving the manuals

| From `CONTRIBUTING.md` | To |
|---|---|
| Benchmarks | `bench/README.md` |
| Instruction-count gate | `bench/README.md` |
| Fuzzing | `fuzz/README.md` |
| Coverage | `xtask/README.md` |
| Mutation testing | `xtask/README.md` |
| Spike crates | `spikes/README.md` |
| (root `README.md`) the `cargo xtask` command paragraph | `xtask/README.md`, as a table |

- Rules in a moved section that bind code outside the tool's directory move into `CONTRIBUTING.md` "What CI enforces" or "Tests", whichever fits, and the manual links to them (decided with the maintainer): at least Coverage's two rules (no `cfg(coverage)`, `cfg(coverage_nightly)` or `#[coverage(...)]` in library code; unit tests in inline `#[cfg(test)]` modules), Mutation testing's "no `mutants::` attributes and no `mutants` dependency in library code", and Fuzzing's "a crash becomes a failing test first". The manuals keep what is particular to the tool: commands, flags, outcomes, triage classes, the label.
- Every rule in a moved section MUST exist in its destination, once; text MAY be shortened. A plan step records, in `log.md`, each rule of each moved section and where it now is (lesson specs-2).
- `CONTRIBUTING.md` keeps "How work is organized", "Issues", "Licence and originality", "Setting up" (its install line is read by a test in `xtask/src/mutants.rs`), "What CI enforces", "Tests", "Commits and pull requests", "Reviews before a pull request" and "Security", and gains a short "Tools" section linking each manual.
- Every reference to a moved section by name or anchor MUST be repointed: in `AGENTS.md`, `README.md`, `.claude/`, `.cargo/mutants.toml`, `.github/`, `docs/` outside `docs/chunks/`, `spikes/` and `xtask/src/` (an error message in `bench_regress.rs`). The absolute link in `.github/ISSUE_TEMPLATE/performance_report.yml` is not checked by §4.6 and is repointed by hand.

### 4.10 #58

The eight links in `docs/chunks/m2.0-roadmap-and-process/plan.md` quote text meant for other files; each is put in a code span, the only edit to that historical plan. The link to `libs/engine/tests/handle_model.rs` in `docs/chunks/m2.1-fuzzing/spec.md` §11 points at `genid_model.rs`, with "(renamed in M3.11)". Any other link the new check finds broken in historical records is fixed the same way and listed in `log.md`.

### 4.11 Markdown and the instruction-count gate

`is_relevant` in `xtask/src/bench_regress.rs` MUST return false for a path ending in `.md`, so a README or other Markdown under `libs/` no longer triggers the Valgrind comparison; everything else in `RELEVANT` is unchanged (decided with the maintainer). The change is made test first, and the gate's "When it runs" rule in `bench/README.md` says so. This chunk still triggers the gate, through `.cargo/mutants.toml` and `xtask/src/bench_regress.rs`; CI runs it as usual.

### 4.12 The first chunk on the M3.15 loop

Friction with the loop, its roles or its skills is logged as `lesson?` when it happens and handled in stage 6 like any other candidate.

## 5. Requirements

- **M3.16-R1** Every directory of §4.1 MUST have a `README.md`; `check-docs` MUST fail when one is missing.
- **M3.16-R2** `check-docs` MUST enforce the README shape of §4.2 (heading, status line, `When this applies` then `Contents`).
- **M3.16-R3** `check-docs` MUST fail when a tracked child of a required README's directory is not covered by a Contents entry, when an entry names nothing tracked, and when a wildcard entry matches nothing (§4.2).
- **M3.16-R4** `check-docs` MUST enforce the lesson format of §4.3.
- **M3.16-R5** `check-docs` MUST reject a glob outside the subset of §4.5 and a glob that matches no tracked file, in lesson and rule files.
- **M3.16-R6** `check-docs` MUST fail on a relative link whose target is missing or outside the repository, or whose anchor matches no heading, and MUST NOT check links in code or with a URL scheme (§4.6).
- **M3.16-R7** `check-docs` MUST fail when `AGENTS.md` has 80 lines or more.
- **M3.16-R8** `check-docs` MUST report every problem in the form of §4.8 and exit non-zero if there is any; it MUST pass on the branch at close-out.
- **M3.16-R9** `check-docs` MUST run in full `cargo xtask ci`, as a `ci.yml` job, and be listed in `requiredChecks`.
- **M3.16-R10** The nine rule files of §4.4 MUST exist, each a pointer to its README with no rule text of its own.
- **M3.16-R11** The manuals MUST be moved as in §4.9, with every rule present once in its destination, and every reference to a moved section repointed.
- **M3.16-R12** `AGENTS.md`'s "Where to look" table MUST point at the READMEs (§4.7).
- **M3.16-R13** The links of #58 MUST be fixed as in §4.10.
- **M3.16-R14** A rule file MUST be shown to load: a fresh Claude Code session that reads a file under `libs/engine/src/` has `engine.md` in its context (lesson tooling-7; §4.4).
- **M3.16-R15** `is_relevant` MUST return false for `.md` paths and be unchanged for every other path (§4.11).

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no library code changes; `check-docs` sorts its output by path and line so it is stable.
- **`unsafe`:** n/a, because none is added; `xtask` keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because `check-docs` is single-threaded.
- **Heap allocation:** n/a, because only `xtask` changes.
- **`no_std` and wasm:** n/a, because only `xtask` and documents change.
- **Frozen formats:** n/a, because none is touched.
- **Public API changes:** none; `xtask` gains a command.
- **FFI exposure:** n/a.
- **Performance:** not measured. `check-docs` reads every tracked `.md` file once; it is expected to take well under a second.
- **Documentation:** the subject of the chunk. `CHANGELOG.md` gets an entry under Unreleased, Changed.
- **Policy changes:** a new required check (`check-docs`) and the README shape as a CI-enforced convention. One new `xtask` dependency, `pulldown-cmark` (MIT), which `cargo xtask deny` must pass. The instruction-count gate stops treating `.md` paths as relevant (§4.11). Library-facing rules from the manuals move into `CONTRIBUTING.md`'s policy sections (§4.9), unchanged in meaning.

## 7. Alternatives considered

- **One README per `bench/` package, and coverage and mutants manuals in `docs/`.** Rejected with the maintainer: one `bench/README.md` covers both packages, and the coverage and mutants manuals live in `xtask/` with the code and flags they describe.
- **List every chunk folder in `docs/chunks/README.md`.** Rejected: a second index of chunks to update at every close-out, duplicating the roadmap.
- **An exemption marker for links in historical records.** Rejected: the links stay broken on GitHub, and a marker invites skipping a fix.
- **Rule files that carry the rule text, or that name lesson files.** Rejected: a second copy of each rule (lesson docs-1), or a second copy of each `applies_to` that nothing keeps in step.
- **`check-docs` outside the required checks.** Rejected: a check that can be merged past does not stop drift.
- **`git ls-files ':(glob)…'` for glob matching.** Rejected: its semantics are git's, not Claude Code's, and a process per pattern makes errors harder to report by line.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1, R2, R3 | unit tests in `xtask/src/check_docs.rs` on fixture trees: a missing README, a wrong heading order, a missing status line, an uncovered child, an entry naming nothing, a wildcard matching nothing, a wildcard covering children, a table-form Contents; `cargo xtask check-docs` passing on the branch |
| R4 | unit tests: a missing key, an extra key, a bad ID, a duplicate ID, a lesson without `*Why:*` |
| R5 | unit tests of the matcher (`*`, `?`, `**` at start, middle and end) and of rejection (braces, brackets, `!`, a glob matching nothing) |
| R6 | unit tests: a missing target, a target outside the repository, a percent-encoded target, a good and a bad anchor, a duplicate-heading anchor, a link in a code span and in a fenced block (ignored), an `https` link (ignored), a reference definition |
| R7 | a unit test at 79 and 80 lines |
| R8 | a unit test that two problems in two files are both reported, sorted; `cargo xtask check-docs` output on the branch |
| R9 | `ci_steps` in `xtask/src/main.rs`; the `check-docs` job in `ci.yml` passing on the pull request; `infra/github/index.ts`; `cargo xtask check-workflows` |
| R10 | the files in `.claude/rules/`; `check-docs` passing (globs match, links resolve); `review-docs` confirms no rule text in them |
| R11 | the rule map in `log.md` (§4.9), checked by `review-spec`; `git grep -n -E '"(Benchmarks\|Instruction-count gate\|Fuzzing\|Coverage\|Mutation testing\|Spike crates)"' -- AGENTS.md README.md .claude .cargo .github docs ':!docs/chunks' spikes xtask/src` finding no reference to `CONTRIBUTING.md` |
| R12 | `AGENTS.md`; `review-docs` |
| R13 | `check-docs` passing over `docs/chunks/`; the PR body closes #58 |
| R14 | a `log.md` entry from the trial (§4.4), quoting what the session reported |
| R15 | a unit test in `xtask/src/bench_regress.rs`, written first and seen failing: `libs/engine/README.md` and `bench/regress/README.md` are not relevant, `libs/engine/src/lib.rs` and `.cargo/mutants.toml` still are |

## 9. Done when

- [ ] `cargo xtask check-docs` exists, is tested, and passes on the branch.
- [ ] It runs in `cargo xtask ci`, as a `ci.yml` job, and is in `requiredChecks`.
- [ ] Every README of §4.1 exists in the shape of §4.2.
- [ ] The manuals are moved (§4.9), the rule map is in `log.md`, and no reference to a moved section is left.
- [ ] The nine rule files exist and one was shown to load.
- [ ] `AGENTS.md` is under 80 lines and its table points at the READMEs.
- [ ] #58's links are fixed.
- [ ] Markdown paths no longer trigger the instruction-count gate.
- [ ] `cargo xtask ci` passes.
- [ ] After the merge, the maintainer has run `pulumi up` in `infra/github`.

## 10. Open questions

None.

## 11. Prior art and sources

- Claude Code memory and path-scoped rules: [Manage Claude's memory](https://docs.claude.com/en/docs/claude-code/memory) (`.claude/rules/`, `paths:` frontmatter).
- GitHub's heading anchors, as observed on rendered READMEs and described in the [basic writing and formatting syntax](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links) guide.
- [Diátaxis](https://diataxis.fr/), for keeping reference (indexes) apart from how-tos, which the README shape follows loosely.
