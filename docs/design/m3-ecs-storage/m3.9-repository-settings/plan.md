# M3.9 plan: Repository settings hardening

- **Status:** Draft (2026-11-14)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Eight commits on branch `m3.9-repository-settings`. The spec and plan are the first commit. The `check-workflows` rule comes before the workflows, so each new workflow lands together with its allowlist entry and is checked from the start. The Pulumi changes come after the workflows. They do nothing until `pulumi up` runs, and that happens only after merge, on the maintainer's go-ahead. The docs come last but one, and the closing record last.

Choices the spec leaves to implementation:

- **Probe before code (no commit).** Confirm what the spec took from documentation, and write any contradiction into §4 Deviations. Stop to ask if one changes a requirement.
  - The latest `@pulumi/github` (`pnpm view @pulumi/github version`), and whether its types have a private-vulnerability-reporting resource or `Repository` field (spec §4.2's upgrade path).
  - That Dependabot accepts `cooldown` for each of the five ecosystems in `dependabot.yml`, especially `devcontainers`.
  - That CodeQL supports `rust` with `build-mode: none` in the current `github/codeql-action` release.
  - The current releases and commit hashes of `github/codeql-action` and `ossf/scorecard-action`, and the permissions Scorecard documents for `publish_results: false`.
- **Testable allowlist.** `scan_workflow(file, text)` becomes `scan_workflow(file, text, allowlist: &[(&str, &str, &str)])`. `run()` passes `WRITE_ALLOWLIST`, so the tests can use their own list and do not depend on the real entries.
- **Job tracking.** While scanning lines, the scanner tracks whether it is inside `jobs:` (a key at indent 0) and the current job id (a key at indent 2 there). For a `permissions:` key it records whether it is at the top level (indent 0) or in a job (indent 4). The scope lines that follow, at greater indent, are checked against the block they belong to. An inline value on the `permissions:` line itself (`write-all`, `read-all`, or `{ a: write, ... }`) is checked on that line.
- **Dynamic resource shape.** `privateVulnerabilityReporting.ts` exports a class `PrivateVulnerabilityReporting extends pulumi.dynamic.Resource` with inputs `{ owner, repository, enabled }`. The provider object is defined in the same file, and its functions read `process.env.GITHUB_TOKEN` when they run, not at module load. Pulumi serializes dynamic providers, so only `fetch` and plain values are captured, never the token. `index.ts` passes the owner from the `github:owner` config.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`; commits that touch a workflow or `xtask/src/check_workflows.rs` also pass `cargo xtask check-workflows`; commits that touch `infra/github` also pass `pnpm typecheck` there.
- Commit messages are conventional, end with ` (M3.9)`, and end with the attribution lines the session requires. Stage by explicit path.
- No new dependency in `xtask/Cargo.toml` or `infra/github/package.json`, unless the probe finds a provider upgrade worth taking (then a version bump of `@pulumi/github` only, recorded in Deviations).
- No change under `libs/`, `examples/`, `bench/`, `fuzz/` or to `rust-toolchain.toml`.
- Stop and ask the maintainer before the first push, before opening the pull request, before `pulumi up`, and before merging.

**Review focus** (failure modes that no single requirement's test pins on its own)

1. **A write scope hidden in an inline mapping** (`permissions: { contents: write }`) or a flow-style value on the job line. Covered by `inline_permissions_mapping_with_write_is_flagged` in Step 1.
2. **A top-level block mistaken for a job block**, or the other way round, after a multi-line value such as a `run: |` block whose text contains `permissions:`. The scanner only treats `permissions:` as a key at exactly indent 0 or 4 and resets its state on each job key. Covered by `permissions_text_inside_a_run_block_is_not_a_key` in Step 1.
3. **Squash title for a one-commit pull request** still taking the commit subject, if `PR_TITLE` were not applied. Checked after `pulumi up` by reading the setting back (`gh api repos/... --jq .squash_merge_commit_title`).
4. **The token leaking into a Pulumi error or state.** The provider's thrown message is built from method, path, status and body only, and no output includes headers. Checked by the invalid-token run in Step 6 and by grepping the stack export for the token prefix (`pulumi stack export | grep -c gho_` prints `0`).
5. **Dependabot security updates enabled before alerts** (an API error on a fresh state). Handled by `dependsOn` in Step 5.

## 2. Steps

### Step 1: `check-workflows` rule 5 (R9)

- **Files:** `xtask/src/check_workflows.rs`.
- **Test first:** new unit tests, each on a small workflow string with a test-local allowlist `[("w.yml", "build", "security-events")]`:
  - `top_level_write_scope_is_flagged`: `permissions:\n  contents: write` at indent 0.
  - `top_level_write_all_is_flagged`: `permissions: write-all`.
  - `job_write_scope_outside_allowlist_is_flagged`: job `build` with `pull-requests: write`; the message names `build` and `pull-requests`.
  - `allowlisted_job_write_scope_passes`: job `build` with `security-events: write`.
  - `allowlist_is_per_file`: the same job and scope in `other.yml` is flagged.
  - `job_write_all_is_flagged_even_when_allowlisted`: job `build` with `permissions: write-all`.
  - `inline_permissions_mapping_with_write_is_flagged`: `permissions: { contents: read, issues: write }` in a job.
  - `permissions_text_inside_a_run_block_is_not_a_key`: a `run: |` body containing `    permissions: write-all` at a deeper indent.
  - `read_scopes_pass`: top-level and job-level `read` values.
  Each fails (compile error on the new parameter, then missing findings) before the change.
- **Change:** the job tracking and rule above; `WRITE_ALLOWLIST` added empty, with a doc comment giving the entry format and the "reason in a comment" rule; the module doc's rule list gains rule 5. Existing tests pass the empty allowlist.
- **Verify:** `cargo nextest run -p xtask check_workflows` passes; `cargo xtask check-workflows` passes on the repository (no workflow has a write scope today).
- **Commit:** `feat(xtask): check-workflows allows write scopes only for allowlisted jobs (M3.9)`

### Step 2: CodeQL workflow (R6, R8, R10)

- **Files:** `.github/workflows/codeql.yml` (new), `xtask/src/check_workflows.rs` (first allowlist entry).
- **Test first:** `cargo xtask check-workflows` with the workflow added and no allowlist entry fails on the job's `security-events: write`, naming `codeql.yml`, `analyze` and `security-events`.
- **Change:** the workflow as in spec §4.3, with pins from the probe; the allowlist entry `("codeql.yml", "analyze", "security-events")` with its reason comment.
- **Verify:** `cargo xtask check-workflows` passes. `requiredChecks` in `index.ts` is unchanged. The run itself is verified on the pull request.
- **Commit:** `ci: add CodeQL code scanning for workflows and Rust (M3.9)`

### Step 3: Scorecard workflow (R7, R8, R10)

- **Files:** `.github/workflows/scorecard.yml` (new), `xtask/src/check_workflows.rs` (second allowlist entry).
- **Test first:** as Step 2, failing on `scorecard.yml`, `analysis`, `security-events`.
- **Change:** the workflow as in spec §4.3; the allowlist entry with its reason comment. The allowlist now holds exactly the two spec entries (R10).
- **Verify:** `cargo xtask check-workflows` passes; `grep -n 'id-token' .github/workflows/scorecard.yml` prints nothing.
- **Commit:** `ci: add a weekly OpenSSF Scorecard run (M3.9)`

### Step 4: Dependabot cooldown (R11)

- **Files:** `.github/dependabot.yml`.
- **Test first:** none possible locally (GitHub validates the file after merge). The probe's findings decide which entries get `cooldown`.
- **Change:** `cooldown: { default-days: 7 }` on each supported entry, written in block style like the rest of the file. A comment on any entry left without it. The starter comment is replaced by one line.
- **Verify:** `yq '.updates[] | [.package-ecosystem, .cooldown."default-days"]' .github/dependabot.yml`, or a Python YAML load if `yq` is absent, lists every entry with `7` or a commented exception. After merge, the Dependabot status page shows no configuration error (recorded in Step 8).
- **Commit:** `ci(dependabot): wait 7 days before proposing a new version (M3.9)`

### Step 5: Repository settings and ruleset in Pulumi (R1, R2, R3, R12)

- **Files:** `infra/github/index.ts`.
- **Test first:** `pulumi preview --diff` before the change shows no changes, so the stack and the live repository agree before this chunk adds anything. If it shows drift, stop and ask.
- **Change:** the §4.1 table, the two ruleset fields, `const vulnerabilityAlerts = ...`, and the `RepositoryDependabotSecurityUpdates` resource with `dependsOn`. Remove `mergeCommitTitle` and `mergeCommitMessage`.
- **Verify:** `pnpm typecheck` passes. `pulumi preview --diff` shows exactly these updates on `repo` and `main-ruleset` and one create, and nothing else; save the output for the pull request. Run the R12 commands (wiki `git ls-remote`, `gh project list --owner SpencerAWill`) and save their output. Do **not** run `pulumi up`.
- **Commit:** `chore(infra): squash-only merging, linear history, security updates, no wiki or Projects (M3.9)`

### Step 6: Private vulnerability reporting resource (R4, R5)

- **Files:** `infra/github/privateVulnerabilityReporting.ts` (new), `infra/github/index.ts`.
- **Test first:** with the resource declared and the provider's `create` left throwing "not implemented", `pulumi preview` shows a create and `pnpm typecheck` passes. The real check is the error path below.
- **Change:** the provider (`create`, `read`, `update`, `delete`, `diff`) as in spec §4.2, and `new PrivateVulnerabilityReporting("private-vulnerability-reporting", { owner, repository: repo.name, enabled: true })` in `index.ts`.
- **Verify:** `pnpm typecheck` passes; `pulumi preview --diff` shows one create. `GITHUB_TOKEN=invalid pulumi preview --refresh` fails on the provider read with a message naming `GET`, the path and `401` and containing no token; save it for the pull request. (The resource is new, so refresh does not reach it until after the first `up`. If the preview does not exercise the provider, the same check moves to after `pulumi up` in Step 8, recorded in Deviations.)
- **Commit:** `chore(infra): declare private vulnerability reporting with a dynamic resource (M3.9)`

### Step 7: Documentation (R13)

- **Files:** `CONTRIBUTING.md`, `SECURITY.md`, `infra/github/README.md`, `docs/roadmap.md` (M3.9 row `In progress`, the "Later" row), `CHANGELOG.md`.
- **Test first:** `grep -n 'squash' CONTRIBUTING.md` and `grep -n 'WRITE_ALLOWLIST' CONTRIBUTING.md SECURITY.md` print nothing before the change.
- **Change:** spec §4.6, except the `maintainers.md` ticks, which wait for Step 8.
- **Verify:** `cargo xtask ci` (full) passes, including `typos`. The relative links in the touched files resolve (checked by hand).
- **Commit:** `docs: record squash merging, the write-scope allowlist and code scanning (M3.9)`

### Step 8: Closing record (R8, R14)

Runs after the maintainer approves the merge and `pulumi up`:

- **Files:** `docs/maintainers.md`, `docs/roadmap.md` (`Done (PR #n)`), `spec.md` (`Implemented (PR #n, date)`), `plan.md` (`Historical: ...`, Deviations).
- **Before the commit, on the maintainer's go-ahead:** `pulumi up`; then `pulumi preview --refresh` shows no changes (R14); `gh api .../private-vulnerability-reporting` prints `{"enabled":true}`; `gh api .../code-scanning/default-setup --jq .state` prints `not-configured` (R8); `gh workflow run scorecard.yml` and its run uploads results; post this evidence on the pull request.
- **Verify:** documentation review of the status lines.
- **Commit:** `docs: mark M3.9 done (M3.9)`. Following M3.0's precedent, this is a separate small pull request if the chunk's pull request is already merged.

## 3. Final verification

- `cargo xtask ci` passes; `pnpm typecheck` passes in `infra/github`.
- Every required check is green on the pull request; the CodeQL run is linked, with its duration.
- The pull request quotes: the `pulumi preview --diff` from Steps 5 and 6, the R12 commands, the invalid-token failure, and lists all eight reviews (public API, determinism and performance are expected to be `n/a`; portability covers the new workflows).
- The spec's Done-when checklist.

## 4. Deviations

None yet.
