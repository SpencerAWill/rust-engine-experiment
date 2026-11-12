# M2.0 plan: Roadmap and process

- **Status:** Accepted (2026-11-12)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. "Spec §X" means "use the exact content of that section".

## 1. Approach

Documentation only, in six commits on branch `m2.0-roadmap-and-process`: the templates first (everything else follows their shape), then the roadmap, the restored M0+M1 plan, and the convention updates grouped by audience. Where the wording is a rule people or agents will follow, this plan gives the exact text; elsewhere the spec section says what must be covered and the wording is the implementer's.

There is no code, so "test first" means: run the step's check before the change and see it fail (a missing file, or a grep that finds stale wording), then make the change and see it pass.

**Global constraints**

- Every new document starts with a status line (`docs/README.md` convention).
- The M0+M1 spec keeps every section number; nothing in it is deleted (spec R9).
- ADRs and `docs/benchmarks.md` are not edited (spec §2).
- Prose follows the existing docs' style: plain sentences, no marketing words, British or American spelling as the file already uses.
- Commit messages are conventional and end with ` (M2.0)`, and end with the attribution lines the session requires.

**Review focus** (failure modes no step's check fully covers)

1. Relative links from the deeper folder `docs/design/m2-infrastructure/m2.0-roadmap-and-process/` use the wrong number of `../`. Covered by the link check in §3.
2. The roadmap and spec §4.2 disagree. The roadmap is the source of truth for status from now on; spec §4.2 is the approved snapshot. Check the roadmap's milestone and chunk tables against spec §4.2 line by line in Step 2.
3. Stale wording about plans being removed survives somewhere not listed in the spec. Covered by the repository-wide grep in Step 4.
4. `cargo xtask typos` flags new words (for example "Diátaxis", "EARS"). Fix by rewording or, for real terms, an entry in `_typos.toml` with a comment.
5. Markdown tables with `|` inside cells, or long tables, render badly on GitHub. Check the rendered files in the pull request's "Files changed" view before asking for review.

## 2. Steps

### Step 1: Spec and plan templates (R2, R3)

- **Files:** create `docs/design/_template/spec.md`, `docs/design/_template/plan.md`.
- **Test first:** `test -f docs/design/_template/spec.md && test -f docs/design/_template/plan.md` fails.
- **Change:** the spec template has the header and the eleven sections of spec §4.4, in that order and numbering; the plan template has the header and four sections of spec §4.5. Under each heading, one or two lines in italics or an HTML comment saying what goes there, taken from §4.4 and §4.5. The cross-cutting section lists all eleven concerns as a bulleted list with `n/a, because ...` placeholders. Each template starts with `Status: Template (2026-11-12). Copy into the chunk folder; do not edit in place for a chunk.` so it satisfies the status-line convention.
- **Verify:** the check passes; compare this chunk's `spec.md` and `plan.md` headings against the templates (`grep -n '^#' <file>` on each pair) and fix any drift in either direction (R3).
- **Commit:** `docs: add chunk spec and plan templates (M2.0)`

### Step 2: The roadmap (R1)

- **Files:** create `docs/roadmap.md`.
- **Test first:** `test -f docs/roadmap.md` fails.
- **Change:** the six parts of spec §4.2 in its order. Use the milestone table, M2 chunk table, later-milestone table and the Miri note from spec §4.2. M0+M1 has status `Done`, M2 `Current`, M3 `Next`, the rest `Later`. Chunk statuses: M2.0 `In progress`, M2.1 to M2.4 `Planned`. M2.0's row links its spec and plan. The later milestones may be one section each or the single table from the spec; pick whichever reads better at its length. The numbering note is a table: old number, new number(s).
- **Verify:** the check passes; read the roadmap side by side with spec §4.2 (review focus item 2).
- **Commit:** `docs: add the roadmap with renumbered milestones and M2 chunks (M2.0)`

### Step 3: Restore the M0+M1 plan (R10)

- **Files:** create `docs/design/m0-m1-foundations/plan.md`.
- **Test first:** `test -f docs/design/m0-m1-foundations/plan.md` fails. If `0dc227c` is not available locally, fetch it first: `git fetch origin pull/3/head:refs/remotes/pr/3`.
- **Change:** `git show 0dc227c^:docs/design/m0-m1-foundations/plan.md > docs/design/m0-m1-foundations/plan.md`, then replace the status blockquote (lines 3 to 6, starting `> **Status: executed, and historical.**`) with exactly:

  ```markdown
  > **Status: Historical: executed in PR #3 (2026-11-12); the spec and the code are authoritative.**
  > This plan was written before implementation and followed task by task. The code and the
  > [spec](spec.md) differ from it where the work taught us something (for example the allocator
  > became two types, see ADR 0010). It was removed when PR #3 merged and restored by M2.0, when
  > plans became permanent records.
  ```

- **Verify:** `diff <(git show 0dc227c^:docs/design/m0-m1-foundations/plan.md) docs/design/m0-m1-foundations/plan.md` shows only that block changed.
- **Commit:** `docs: restore the M0+M1 implementation plan as a historical record (M2.0)`

### Step 4: Docs index and the M0+M1 spec (R4, R9)

- **Files:** modify `docs/README.md`, `docs/design/m0-m1-foundations/spec.md`.
- **Test first:** `grep -rn "removed when\|removed at merge\|until that milestone merges\|until the milestone merges\|sits beside it" docs README.md AGENTS.md CONTRIBUTING.md .github --include='*.md' | grep -v 'm0-m1-foundations/plan.md\|m2-infrastructure/'` lists the stale lines in `docs/README.md` and the M0+M1 spec. (This chunk's own spec and plan quote the pattern, hence the exclusion.)
- **Change, `docs/README.md`:**
  - Add a first row to the table: `[roadmap.md](roadmap.md)`, "Milestones, their exit criteria, and the current milestone's chunks with their status", kind "Reference", trust "**Authoritative for what is planned and its status.**"
  - The `design/` row reads: "One folder per milestone, containing one folder per chunk with its `spec.md` and `plan.md`; [`_template/`](design/_template/) holds the templates. `m0-m1-foundations/` predates chunks and holds one spec and one plan for the whole milestone."
  - Replace the "Design folders" and "Plans are scaffolding" bullets with:
    - "**Design folders** are `design/m<N>-<topic>/m<N>.<n>-<topic>/`, holding the chunk's `spec.md` and `plan.md`, copied from [`design/_template/`](design/_template/). A spec keeps stable section numbers because code and config cite them (for example "spec §4.9"); add sections, do not renumber."
    - "**Plans are kept.** A plan is written before implementation and records what differed in its Deviations section. When its pull request merges, its status becomes `Historical`: it records how the work was done, and the spec, the ADRs and the code are authoritative."
    - "**Milestone numbers changed on 2026-11-12.** Documents dated before then (the M0+M1 spec, ADRs 0001 to 0011, `benchmarks.md`) use the old numbers; [the roadmap](roadmap.md) has the mapping."
- **Change, M0+M1 spec:**
  - Add a header bullet after **Scope**: "- **Milestone numbering:** this document uses the milestone numbers in force before 2026-11-12. [The roadmap](../../roadmap.md) maps them to the current ones and supersedes §11."
  - Reading guide, the sentence beginning "The implementation plan for this milestone was removed": replace with "The implementation plan is [plan.md](plan.md), kept as a historical record."
  - §3 tree, the `design/<milestone>/` line: replace its description with `design specs and plans, one folder per milestone or chunk (see docs/README.md)`, keeping the column alignment.
  - At the top of §11, before its table, add: "*Superseded by [the roadmap](../../roadmap.md) on 2026-11-12. Kept for the reasoning; the milestone numbers here are the old ones.*"
- **Verify:** the check-first grep prints nothing; `grep -n '^##' docs/design/m0-m1-foundations/spec.md` is unchanged from `main` (`diff <(git show main:docs/design/m0-m1-foundations/spec.md | grep '^##') <(grep '^##' docs/design/m0-m1-foundations/spec.md)` is empty).
- **Commit:** `docs: describe chunks, kept plans and the renumbering in the docs index (M2.0)`

### Step 5: Maintainer checklists and the pull request template (R5, R6)

- **Files:** modify `docs/maintainers.md`, `.github/PULL_REQUEST_TEMPLATE.md`.
- **Test first:** `grep -n "Remove the implementation plan" docs/maintainers.md` finds a line; `grep -n "Chunk" .github/PULL_REQUEST_TEMPLATE.md` finds nothing.
- **Change, `docs/maintainers.md`:** replace the section "Before merging a milestone" with:

  ```markdown
  ## Before merging a chunk

  - [ ] `cargo xtask ci` passes and the GitHub run is green, including the macOS and Windows jobs.
  - [ ] The pull request title ends with the chunk ID, for example `(M2.1)`.
  - [ ] The spec's status is `Implemented (PR #n, date)`; the plan's is `Historical: executed in PR #n (date)`, with its Deviations section filled in (or "None"); the roadmap row is `Done (PR #n)`.
  - [ ] `CHANGELOG.md` is updated.
  - [ ] Merge only on the maintainer's explicit go-ahead.

  ## When a milestone completes

  - [ ] Tag the commit that completed it: `git tag -a m<N> -m "M<N>: <goal>"`, then `git push origin m<N>`.
  - [ ] In the roadmap, mark the milestone `Done` and the next one `Current`, and break the next one into chunks.
  - [ ] Decide whether this is the boundary at which to run "Before the repository goes public".
  ```

  The pull request number is known only once the pull request exists, so the status updates in the first checklist are a final commit on the branch.
- **Change, `.github/PULL_REQUEST_TEMPLATE.md`:** insert above "## What and why":

  ```markdown
  ## Chunk

  <!-- Roadmap work: the chunk ID and links (see docs/roadmap.md). Otherwise "None", for example for a dependency update or a small fix. The title ends with the chunk ID, for example "(M2.1)". -->

  - Chunk:
  - Spec:
  - Plan:
  ```

  and append to the checklist: `- [ ] For a chunk: the spec is \`Implemented\`, the plan is \`Historical\` with its Deviations filled in, and the roadmap row is \`Done\`.`
- **Verify:** both check-first greps now give the opposite result; the Step 4 grep prints nothing.
- **Commit:** `docs: per-chunk merge checklist and chunk fields in the PR template (M2.0)`

### Step 6: Contributor and agent entry points, README, changelog (R7, R8, R11, R12)

- **Files:** modify `CONTRIBUTING.md`, `AGENTS.md`, `README.md`, `CHANGELOG.md`.
- **Test first:** `grep -c "roadmap" CONTRIBUTING.md AGENTS.md README.md CHANGELOG.md` is 0 for each.
- **Change, `CONTRIBUTING.md`:** a section "How work is organized" after the opening paragraph, of about six lines: work is planned in [docs/roadmap.md](docs/roadmap.md) as milestones broken into chunks, one pull request each; each chunk has a spec and a plan copied from [docs/design/_template/](docs/design/_template/); pull request titles end with the chunk ID; ideas and bugs that are not scheduled go into issues. Change "decisions are deliberately deferred (see `docs/design/`)" to point at the roadmap.
- **Change, `AGENTS.md`:** in the first paragraph, add "What is planned next, and its status, is in [docs/roadmap.md](docs/roadmap.md)." Under "How to work here", add a bullet:
  - "**Chunks and gates.** Work happens in chunks (see the roadmap). Write the chunk's spec and plan from [docs/design/_template/](docs/design/_template/) and stop for the maintainer to review both. If the chunk adds or changes an ADR, a public API, or a frozen format, stop after the spec, and write the plan only once the spec is accepted. Record anything that differs from the plan in its Deviations section."
- **Change, `README.md`:** in "Status", replace "Current scope is milestones M0 (workspace and policy) and M1 (foundation crates)." with "M0+M1 (workspace, policy and the foundation crates) is done; what comes next is in [docs/roadmap.md](docs/roadmap.md)."
- **Change, `CHANGELOG.md`:** under `[Unreleased]`, add a `### Documentation` subsection (after `### Added`): "- **Process:** work is now planned in `docs/roadmap.md` as small chunks, each with a spec and plan from `docs/design/_template/`. Milestones after M0+M1 are renumbered (the roadmap maps old to new numbers); plans are kept as historical records, and the M0+M1 plan is restored."
- **Verify:** the check-first grep is at least 1 for each file.
- **Commit:** `docs: point contributors and agents at the roadmap and templates (M2.0)`

### Step 7: Statuses at review and at merge (R4, R5)

- After the maintainer approves the spec and plan: set both to `Accepted (date)` in a commit `docs: accept the M2.0 spec and plan (M2.0)`. (They are approved before Steps 1 to 6 run, so this is the branch's first commit after this plan's own.)
- After the pull request is opened: the spec becomes `Implemented (PR #n, date)`, this plan `Historical: executed in PR #n (date); the spec and the code are authoritative`, the roadmap row `Done (PR #n)`. Commit `docs: mark M2.0 implemented (M2.0)`.

## 3. Final verification

1. `cargo xtask ci` passes.
2. The link check (R13) prints nothing. This plan is excluded because it quotes links meant for other files. Run from the repository root:

   ```sh
   git diff --name-only main...HEAD -- '*.md' | grep -v m2.0-roadmap-and-process/plan.md | python3 -I -c '
   import re, sys, pathlib
   bad = 0
   for name in sys.stdin.read().split():
       p = pathlib.Path(name)
       for target in re.findall(r"\]\(([^)\s]+)\)", p.read_text()):
           if re.match(r"[a-z]+:", target) or target.startswith("#"):
               continue
           path = target.split("#")[0]
           if not (p.parent / path).exists():
               print(f"{name}: broken link {target}"); bad = 1
   sys.exit(bad)'
   ```

3. Every item of spec §9 is checked off.
4. The review focus items in §1 have been checked.

## 4. Deviations

- **Roadmap later milestones:** one table (rough chunks and open question), with exit criteria in the overview table, as Step 2 allows.
- **Step 4 grep:** the fresh review found a stale "removed at merge" line in M0+M1 spec §13 that the grep's pattern missed; fixed in the review pass, and the pattern now includes `removed at merge`.
- **Step 6 changelog:** the entry went under `### Changed` (a Keep a Changelog category) instead of a new `### Documentation` heading.
- **Review pass:** a fresh review (Opus) also led to: the numbering-note wording ("written before this roadmap", because the old documents are dated up to 2026-11-12), the going-public note moved to the end of the roadmap, the row-status note, "Spec statuses" and "Plan statuses" in the templates, "exactly one pull request" in CONTRIBUTING, `Dropped` and `Template` in the status vocabulary, the stale "hosted service" line in `maintainers.md`, and this plan's link check excluding itself.
