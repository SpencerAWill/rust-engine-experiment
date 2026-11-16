# M3.16 plan: Documentation layout

- **Status:** Accepted (2026-11-15)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

The checker comes first (steps 2 to 4), wired as a command but not yet into `ci`, so every later documentation step is verified by it: each step names the problems `cargo xtask check-docs` reports before it and the ones that must be gone after it. The gate goes in last (step 10), when the tree passes. Builders (Sonnet) write all Rust and the CI and Pulumi files; the main session writes the Markdown.

Choices the spec leaves to the plan:

- **Module layout.** `xtask/src/check_docs.rs` (the command, `Problem`, the file set, `AGENTS.md`), with submodules in `xtask/src/check_docs/`: `glob.rs` (§4.5), `markdown.rs` (parsing with `pulldown-cmark`: headings and anchors, links with byte offsets, top-level bullets and table rows, frontmatter), `links.rs` (§4.6), `frontmatter.rs` (§4.3, §4.4), `readme.rs` (§4.1, §4.2).
- **Testable core.** `pub(crate) fn check(files: &BTreeSet<String>, read: &dyn Fn(&str) -> Result<String, String>) -> Vec<Problem>` does every check on an in-memory tree, so the tests build fixture trees from string literals and touch no disk. `pub(crate) fn run() -> Result<(), String>` lists the files (`git ls-files -z --cached --others --exclude-standard`, deduplicated, dropping paths not on disk, which `--cached` lists for a deleted file), reads from disk, prints the sorted problems and the count line `check-docs: <n> problem(s)`, and returns `Err` when there is any.
- **`Problem`.** `pub(crate) struct Problem { path: String, line: Option<usize>, message: String }`, `Ord` by path, then line, then message; `Display` is `<path>:<line>: <message>` or `<path>: <message>`.
- **Parser options.** `Options::ENABLE_TABLES | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS`; without the second, `---` frontmatter parses as a thematic break and a setext heading. Line numbers come from `into_offset_iter()` byte offsets.
- **`pulldown-cmark = { version = "0.13", default-features = false }`** in `xtask/Cargo.toml` only.

## 2. Steps

### Step 1: Accept the spec and add this plan

- [x] **Who:** main session.
- **Files:** `plan.md` (this file), `log.md`, `docs/roadmap.md` (M3.16 row `In progress`).
- **Test first:** n/a (documentation).
- **Verify:** `git status --short` shows only these files.
- **Commit:** `docs: add the documentation layout plan (M3.16)`

### Step 2: Markdown does not trigger the instruction-count gate (R15)

- [x] **Who:** builder.
- **Files:** `xtask/src/bench_regress.rs`.
- **Test first:** a unit test `markdown_is_not_relevant` next to the existing `is_relevant` tests: `libs/engine/README.md`, `bench/regress/README.md` and `libs/engine/src/notes.md` are not relevant; `libs/engine/src/lib.rs`, `.cargo/mutants.toml` and `bench/regress/benches/genid.rs` still are. Run it and see the first three assertions fail.
- **Change:** `is_relevant` returns false for a path ending in `.md`, before the `RELEVANT` match; update the doc comment on `RELEVANT`. Also repoint the error message at line 291 from "(see CONTRIBUTING.md)" to "(see bench/README.md)" (R11; the file is written in step 7).
- **Verify:** `cargo xtask test` passes; `cargo xtask clippy` is clean.
- **Commit:** `feat(xtask): stop counting Markdown changes as relevant to bench-regress (M3.16)`

### Step 3: `check-docs`: files, globs, frontmatter, lessons, links, `AGENTS.md` (R4 to R8)

- [x] **Who:** builder.
- **Files:** `xtask/Cargo.toml`, `Cargo.lock`, `xtask/src/main.rs` (`Command::CheckDocs`, `"check-docs"` in `parse`, `execute`; **not** in `ci_steps` yet), `xtask/src/check_docs.rs`, `xtask/src/check_docs/{glob,markdown,links,frontmatter}.rs`.
- **Test first:** unit tests against fixture trees, written before the code and seen failing (compile error, then failures against stubs that return no problems):
  - glob: `*` stays in one segment; `?`; `**` at the start, middle and end, matching zero segments too (`docs/**` matches `docs/a.md`, `**/README.md` matches `README.md`); rejection of `{a,b}`, `[ab]`, `!x`.
  - frontmatter: a lesson file with no frontmatter, an extra key, an unquoted glob, a glob matching nothing, a bad ID (`docs-x`), an ID with the wrong stem, a duplicate ID, a lesson without `*Why:*`, a wrong heading, a missing status line; a rule file with `applies_to:` instead of `paths:`; a well-formed lesson file and rule file report nothing.
  - links: a missing target; `../` escaping the repository; `a%20b.md` resolving to `a b.md`; a directory target; a good anchor, a bad anchor, a same-file `#anchor`, a duplicate heading's `-1`; a heading with punctuation and code (`` ## `foo::bar` and baz! `` gives `foobar-and-baz`); a link in a code span, a fenced block and an HTML comment (ignored); `https:` and `mailto:` (ignored); a reference definition; an image.
  - `AGENTS.md` at 79 lines passes, at 80 fails.
  - two problems in two files come back sorted, and `Display` gives the §4.8 form.
- **Change:** the modules of §1 above. Error messages name what to do (for example `` link `x.md`: no such file; fix the path or remove the link ``).
- **Verify:** `cargo xtask test` passes; `cargo xtask clippy` is clean; `cargo xtask deny` passes; `cargo xtask check-docs` runs on the tree and reports the link problems of #58 among others (record the count per check in `log.md`).
- **Commit:** `feat(xtask): add check-docs for links, lessons, rule files and AGENTS.md (M3.16)`

### Step 4: `check-docs`: required READMEs and Contents (R1 to R3)

- [x] **Who:** builder.
- **Files:** `xtask/src/check_docs.rs` (`REQUIRED_READMES`, the spikes topics), `xtask/src/check_docs/readme.rs`.
- **Test first:** fixture-tree tests, seen failing: a missing required README; a missing `spikes/<topic>/README.md`; no status line; `Contents` before `When this applies`; a missing `Contents`; an uncovered child; an entry naming nothing; a wildcard matching nothing; `m*/` covering three folders; a table-form Contents whose first cell is a link wrapping a code span; a multi-segment entry (`skills/chunk/`) covering `skills`; `../x.md` and `https:` entries ignored; a prose bullet ignored; extra sections after Contents allowed; the root `README.md` not checked for shape.
- **Change:** `REQUIRED_READMES = ["docs", "docs/adr", "docs/chunks", "docs/lessons", ".claude", "infra/github", "libs/engine", "bench", "fuzz", "xtask", "spikes", "examples"]`, plus every directory directly under `spikes/` holding tracked files.
- **Verify:** `cargo xtask test` passes; `cargo xtask clippy` is clean; `cargo xtask check-docs` reports the missing READMEs and shapes (count in `log.md`).
- **Commit:** `feat(xtask): check required READMEs and their Contents (M3.16)`

### Step 5: Fix the broken links (R13)

- [x] **Who:** main session.
- **Files:** `docs/chunks/m2.0-roadmap-and-process/plan.md`, `docs/chunks/m2.1-fuzzing/spec.md`, and any other file `check-docs` reports a link problem in before any README work.
- **Test first:** `cargo xtask check-docs 2>&1 | grep 'link'` lists the #58 links.
- **Change:** as spec §4.10; any other broken link fixed the same way and listed in `log.md`.
- **Verify:** `cargo xtask check-docs 2>&1 | grep -c 'link'` prints 0.
- **Commit:** `docs: fix the broken relative links in historical records (M3.16)`

### Step 6: Library-facing rules into `CONTRIBUTING.md` (R11)

- [ ] **Who:** main session.
- **Files:** `CONTRIBUTING.md`.
- **Test first:** n/a (documentation); the rule map of step 8 checks it.
- **Change:** spec §4.9's first bullet, into "Tests" (coverage and mutants are report-only, so none of these is a CI gate): Coverage's two rules (no `cfg(coverage)`, `cfg(coverage_nightly)` or `#[coverage(...)]`; unit tests in inline `#[cfg(test)]` modules, with the reason), Mutation testing's "no `mutants::` attributes and no `mutants` dependency in library code", and Fuzzing's "turn a crash into a failing test first". The manual copies are removed in step 7.
- **Verify:** after step 7, `grep -c` for each rule's key phrase (`cfg(coverage`, `mutants::`, `crash`) over `CONTRIBUTING.md bench/README.md fuzz/README.md xtask/README.md` finds it in `CONTRIBUTING.md` only, apart from links.
- **Commit:** `docs: move the library-facing rules of the tool manuals into the policy sections (M3.16)`

### Step 7: Move the manuals and repoint references (R11)

- [ ] **Who:** main session.
- **Files:** `CONTRIBUTING.md` (manuals out; a "Tools" section linking each), `bench/README.md`, `fuzz/README.md`, `xtask/README.md` (commands table from the root `README.md`, Coverage, Mutation testing), `spikes/README.md` (Spike crates merged), `README.md` (command paragraph replaced by a link), and the references of spec §4.9: `AGENTS.md`, `.claude/agents/*.md` only where they cite a moved section, `.claude/skills/bench-run/SKILL.md`, `.claude/skills/mutants-run/SKILL.md`, `.cargo/mutants.toml`, `.github/ISSUE_TEMPLATE/performance_report.yml`, `docs/benchmarks.md`, `docs/mutation-triage.md`, `spikes/ecs_storage/README.md`.
- **Test first:** the §8 R11 grep lists the current references.
- **Change:** each manual in the §4.2 shape, with "How to" holding the commands. "Setting up" keeps the install line that `xtask/src/mutants.rs` tests.
- **Verify:** the §8 R11 grep finds no reference to `CONTRIBUTING.md`; `cargo xtask check-docs` reports no link problem and no problem in `bench/`, `fuzz/`, `xtask/` or `spikes/` READMEs; `cargo xtask test` passes (the install-line test).
- **Commit:** `docs: move the tool manuals next to their tools (M3.16)`

### Step 8: The rule map (R11, lesson specs-2)

- [ ] **Who:** main session.
- **Files:** `log.md`.
- **Change:** for each moved section of the `CONTRIBUTING.md` at `main` (`git show main:CONTRIBUTING.md`), one line per rule (each bullet and each normative sentence): where it is now (file and section). A rule not found is restored before committing.
- **Verify:** every bullet of the six moved sections at `main` has a line.
- **Commit:** `docs: record where each moved rule now lives (M3.16)`

### Step 9: The other READMEs, rule files and `AGENTS.md` (R1 to R3, R10, R12)

- [ ] **Who:** main session.
- **Files:** `docs/README.md` (shape; the README shape as a Conventions bullet), `docs/adr/README.md`, `docs/chunks/README.md`, `docs/lessons/README.md` ("Format" after Contents), `.claude/README.md` (`rules/`), `infra/github/README.md`, `libs/engine/README.md`, `examples/README.md`, `spikes/ecs_storage/README.md`, `.claude/rules/{engine,bench,fuzz,xtask,spikes,examples,docs,claude,ci}.md`, `AGENTS.md` ("Where to look").
- **Test first:** `cargo xtask check-docs` lists these READMEs as missing or misshapen.
- **Change:** spec §4.1, §4.2, §4.4, §4.7.
- **Verify:** `cargo xtask check-docs` passes (`check-docs: 0 problems`, exit 0); `wc -l AGENTS.md` is under 80.
- **Commit:** `docs: add a README in each directory that matters, and path-scoped rules (M3.16)`

### Step 10: The gate (R9)

- [ ] **Who:** builder.
- **Files:** `xtask/src/main.rs` (`Command::CheckDocs` in `ci_steps` after `CheckWorkflows`, with the test of `ci_steps` updated if there is one), `.github/workflows/ci.yml` (a `check-docs` job copied from `check-workflows`), `infra/github/index.ts` (`"check-docs"` in `requiredChecks`, after `"check-workflows"`).
- **Test first:** if `main.rs` has a test over `ci_steps`, extend it first and see it fail; otherwise n/a.
- **Verify:** `cargo xtask check-workflows` passes; `cargo xtask check-docs` passes; `cd infra/github && npx tsc --noEmit` passes if the toolchain is installed (else say so in the report).
- **Commit:** `ci: run check-docs in ci and require it on main (M3.16)`

### Step 11: The rule load trial (R14)

- [ ] **Who:** main session.
- **Files:** `log.md`.
- **Change:** run `claude -p --allowedTools Read --max-turns 3 "Read libs/engine/src/lib.rs. Then list, verbatim, the file paths of any project rules (.claude/rules) loaded into your context, or say none."` from the repository root; quote the answer in `log.md`. If `engine.md` is not loaded, stop and ask (spec §4.4).
- **Verify:** the log line quotes `engine.md`.
- **Commit:** `docs: record the rule load trial (M3.16)`

### Step 12: Changelog

- [ ] **Who:** main session.
- **Files:** `CHANGELOG.md` (Unreleased, Changed).
- **Verify:** `cargo xtask check-docs` passes.
- **Commit:** `docs: add the documentation layout to the changelog (M3.16)`

## 3. Final verification

`cargo xtask ci` (full) passes, which now includes `check-docs`; the spec's Done-when list, except the `pulumi up` item, which is the maintainer's after the merge.

## 4. Lessons

- **docs-1** followed: rule files are pointers (spec §4.4), and the library-facing rules have one copy in `CONTRIBUTING.md` (step 6).
- **specs-1** followed: the R11 grep names its paths and excludes `docs/chunks/`.
- **specs-2** followed: the rule map is its own step (8), and so is the load trial (11).
- **tooling-7** followed: the rule files are trialled before the chunk relies on them (step 11).

## 5. Deviations

- **Step 3, link resolution.** Spec §4.6 says targets resolve against the file's directory; a target with a leading `/` resolves from the repository root instead, as GitHub renders it. A `?query` is ignored and protocol-relative `//x` links are skipped. No such link exists today.
