---
applies_to:
  - "AGENTS.md"
  - "CONTRIBUTING.md"
  - "docs/**"
  - ".claude/**"
---
# Lessons: documentation

- Status: Living (2026-11-27)

- **docs-1.** A rule that several entry files need (the waits, the escalations) has one copy; the other files link to it. *Why:* the escalation list was written three times with three wordings within one chunk (M3.15). In M3.20 three rules that other steps read (the chunk-ID rule, the stage that pushes the branch, the form of a link between issues) were each written in two or more files, and a later change updated only some of them (docs, spec and code reviews); when such a rule moves or changes, grep its key phrase across `docs/` and `.claude/` before committing. *From:* M3.15, M3.20 · *Confirmed:* 2026-11-23
- **docs-2.** A header field or section a stage tells agents to add belongs in the template they copy (`docs/chunks/_template/`), so its format has one copy; when a fix changes how many items a document lists, grep the plan for the old count. *Why:* chunk stage 1 told agents to add an `**Issue:**` line the template lacked, and plan step 6 still said "the two built-in workflows" after the setup grew to four (M3.18). *From:* M3.18 · *Confirmed:* 2026-11-16
- **docs-3.** A prediction (what a benchmark "will" show) keeps its "expected, unmeasured" wording wherever it is copied, such as from a spec into a roadmap row; a doc comment that ranks a backend ("best case", "floor", "fastest") is the same kind of claim, checked against the spec or a measurement before review, including when it copies a crate's premise; and a sentence saying what a benchmark row lets one read (a `CHANGELOG.md` entry, a README) carries the spec's caveat on what else the row contains. *Why:* M3.3's spec said the two sparse-set backends "will be close" until churn; the roadmap copy said they "differ mainly after churn", which reads as a result (docs review). M3.22's `VecManyQueries` doc copied `vec`'s crate premise and called itself the floor the ECS rows are read against, while spec §4.4 says its row pays a full pass per query (docs and performance reviews). M3.23's `CHANGELOG.md` entry and README said its row reads a wide row's move cost against `add_remove`'s, without spec §4.6's caveat that the difference also holds an unmeasured update pass, the larger share on `vec_aos` (performance review). *From:* M3.3, M3.22, M3.23 · *Confirmed:* 2026-11-27
- **docs-4.** Before the full `cargo xtask ci`, run `typos` over the chunk folder, and cite commits in `log.md` by the full hash when a short one reads as a word (or quote a flagged token nowhere, including the note about it). *Why:* in M3.21 a short hash in `log.md` failed `typos` twice, the second time through the note that explained the first; each failure stopped the full run before `deny`, `bench-build`, `bench-smoke` and `fuzz-build`, so it was run three times. *From:* M3.21 · *Confirmed:* 2026-11-24
