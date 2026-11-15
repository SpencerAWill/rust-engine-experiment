# M<N>.<n> plan: <title>

- **Status:** Template (2026-11-12). Copy into the chunk folder; do not edit in place for a chunk. Plan statuses: `Draft (date)`, `Accepted (date)`, `Historical: executed in PR #n (date); the spec and the code are authoritative`. Set `Historical` in the chunk's own pull request, in the close-out commit after its checks pass (see docs/README.md).
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

Keep it short: a plan records the decisions a builder cannot make alone (files, names, signatures, the tests that prove each step). It does not restate the spec.

## 1. Approach

<!-- A few sentences, including any choices the spec leaves to implementation. -->

## 2. Steps

<!-- One step per commit, with a checkbox ticked in that commit. Each step names: who does it (builder, or main session for documentation); the requirements it satisfies; the files it touches; the test written first and the failure expected (for documentation, the check that fails before the change); the change; the verification command and its expected result; the commit message (conventional, ending with the chunk ID in parentheses). -->

### Step 1: <title> (R<k>)

- [ ] **Who:**
- **Files:**
- **Test first:**
- **Change:**
- **Verify:**
- **Commit:** `type(scope): message (M<N>.<n>)`

## 3. Final verification

<!-- `cargo xtask ci`, and the spec's Done-when checklist. -->

## 4. Lessons

<!-- The lesson IDs the lessons researcher returned for the paths this chunk touches, each "followed" or "deviated: <reason>". "None apply" if none matched. -->

## 5. Deviations

<!-- Filled in during implementation: what differed from the plan and why. "None" if nothing did. -->
