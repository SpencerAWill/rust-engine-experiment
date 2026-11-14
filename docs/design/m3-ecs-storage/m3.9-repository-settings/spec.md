# M3.9: Repository settings hardening

- **Status:** Draft (2026-11-14)
- **Milestone:** M3 (scheduled during M3; not part of its exit criteria, which are about the ECS storage decision) · **Depends on:** none · **Decides:** no ADR
- **Estimated size:** about 350 changed lines, excluding tests, lockfiles and generated files (`infra/github` about 90, workflows about 90, `xtask` about 60, docs about 110)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk closes the gaps between the repository's GitHub settings and what its documents promise or need. Private vulnerability reporting is turned on, because `SECURITY.md` and the issue chooser send reporters to it and it is off. The chunk also adds squash-only merging with linear history, Dependabot security updates, CodeQL and OpenSSF Scorecard code scanning, a Dependabot cooldown, and removes the unused wiki and Projects. It comes now because the repository is public and the vulnerability-reporting path is broken, and because each item is small and independent of the M3 storage work. At the end, `pulumi preview` shows no drift, the Security tab has a working "Report a vulnerability" button and CodeQL and Scorecard results, and `main` accepts only squash merges.

## 2. Goals and non-goals

**Goals**

- Every setting this chunk changes is declared in `infra/github/index.ts`, including the two that provider 6.15.1 has no resource for (§4.2).
- The documented vulnerability-reporting path works for someone outside the repository.
- `main` has one commit per pull request, and its subject is the pull request title.
- Code scanning covers the workflows (`actions`) and the Rust code, and supply-chain hygiene gets a recurring score, with results in the Security tab.
- No workflow job gets a write-scoped token unless `check-workflows` allows that job by name.

**Non-goals**

- **Release infrastructure** (a tag ruleset for `v*`, a protected `release` environment, crates.io trusted publishing). No release is planned yet; the roadmap's "Later" row records it (decided 2026-11-14).
- **Required reviews, code-owner review, signed commits, merge queue.** There is one maintainer, and GitHub does not let authors approve their own pull requests. Signed commits were offered and declined (2026-11-14).
- **Dependabot auto-merge.** Every merge stays manual (decided 2026-11-14); `allowAutoMerge` stays `false`.
- **Making CodeQL or Scorecard a required check.** Both are report-only in this chunk. A later chunk MAY require CodeQL once its alerts have been triaged to zero.
- **Publishing Scorecard results** to the public Scorecard API, and the README badge that needs it (§7).
- **CodeQL for `javascript-typescript`.** The only TypeScript is the Pulumi program, about 200 lines that run on the maintainer's machine with the maintainer's token. CodeQL adds no protection there that review does not already give (§7).
- **GitHub Pages**, stricter approval rules for workflows on fork pull requests, and the secret-scanning extras (non-provider patterns, validity checks). The first two were offered and declined (2026-11-14). The secret-scanning extras are left for when there are secrets to check.
- **A CI job that typechecks `infra/github`.** It stays a local step (`pnpm typecheck`), as today.
- No change under `libs/`, `examples/`, `bench/` or `fuzz/`, to any public API, frozen format or `rust-toolchain.toml`.

## 3. Guide-level explanation

**Contributors.** Pull requests are squash-merged. The pull request title becomes the commit subject on `main` and the branch's commit messages become its body. So the title MUST be a conventional commit message, and for a chunk it ends with the chunk ID, as CONTRIBUTING already requires. Small logical commits are still wanted on the branch, because reviewers read them.

**Reporters.** The Security tab now has **Report a vulnerability**, as `SECURITY.md` describes.

**Maintainer.** After this chunk merges:

```sh
cd infra/github
export GITHUB_TOKEN="$(gh auth token)"
pulumi preview --diff   # merge methods, linear history, wiki and Projects off, topics,
                        # Dependabot security updates, private vulnerability reporting
pulumi up
```

Then open the Security tab:

- **Code scanning** lists two tools. CodeQL runs on pull requests, on pushes to `main` and weekly. Scorecard runs weekly and on pushes to `main`. Alerts appear in the Security tab, and CodeQL also annotates pull requests, but neither blocks a merge.
- **Dependabot** opens a pull request when an advisory affects a dependency in `Cargo.lock`, `fuzz/Cargo.lock` or `pnpm-lock.yaml`. Merge it like any other pull request. Version updates wait 7 days after a release before Dependabot proposes them (the cooldown). Security updates do not wait.

## 4. Reference-level design

### 4.1 Pulumi resources and settings (`infra/github/index.ts`)

On `github.Repository`:

| Setting | Before | After |
|---|---|---|
| `hasWiki` | `true` | `false` |
| `hasProjects` | `true` | `false` |
| `hasIssues` | `true` | `true` (unchanged; issues are the backlog) |
| `allowMergeCommit` | `true` | `false` |
| `allowRebaseMerge` | `true` | `false` |
| `allowSquashMerge` | `true` | `true` |
| `squashMergeCommitTitle` | `COMMIT_OR_PR_TITLE` | `PR_TITLE` |
| `squashMergeCommitMessage` | `COMMIT_MESSAGES` | `COMMIT_MESSAGES` (unchanged) |
| `mergeCommitTitle`, `mergeCommitMessage` | set | removed (they configure a merge method that is off) |
| `topics` | none | `rust`, `game-engine`, `gamedev`, `simulation`, `ecs`, `deterministic` |

`PR_TITLE` matters for single-commit pull requests. With `COMMIT_OR_PR_TITLE`, GitHub uses the commit's subject instead, which may lack the chunk ID.

Before applying, the implementation MUST confirm that disabling the wiki deletes no content: the wiki has no pages (`git ls-remote https://github.com/SpencerAWill/rust-engine-experiment.wiki.git` fails or lists nothing). Projects MUST likewise be checked to be empty (`gh project list --owner SpencerAWill` shows none linked to this repository). If either has content, stop and ask.

On the `main` ruleset:

- `rules.pullRequest.allowedMergeMethods: ["squash"]`.
- `rules.requiredLinearHistory: true`.

These apply to admins too, except that the admin bypass still lets the admin push to `main` directly. A direct push must already be a fast-forward (`nonFastForward`), so it cannot add a merge commit.

New resource:

- `new github.RepositoryDependabotSecurityUpdates("dependabot-security-updates", { repository: repo.name, enabled: true }, { dependsOn: [vulnerabilityAlerts] })`. Security updates need alerts to be on, so the existing `RepositoryVulnerabilityAlerts` resource is bound to a `const`.

### 4.2 Settings the provider cannot manage

`@pulumi/github` 6.15.1 has no resource for private vulnerability reporting (`PUT`/`DELETE /repos/{owner}/{repo}/private-vulnerability-reporting`) or for code scanning default setup.

**Private vulnerability reporting** MUST be declared in `index.ts` as a Pulumi dynamic resource, `PrivateVulnerabilityReporting`, in a new file `infra/github/privateVulnerabilityReporting.ts`:

- **create / update:** `PUT` the endpoint above (expects `204`).
- **read:** `GET` the endpoint and return `{ enabled }`. Then `pulumi refresh` and `pulumi preview --refresh` show the setting drifting when someone turns it off in the UI.
- **delete:** `DELETE` the endpoint (expects `204`). Removing the resource from the program turns reporting off, as removing any other declared setting reverts it.
- **diff:** a change is needed exactly when `enabled` differs.
- It uses Node's built-in `fetch` with `process.env.GITHUB_TOKEN`, which the README's workflow already exports, and `Accept: application/vnd.github+json` with `X-GitHub-Api-Version: 2022-11-28`. A non-2xx response MUST throw with the method, path, status and response body, so `pulumi up` fails with the reason. The token MUST NOT appear in thrown messages or resource outputs.
- No new npm dependency.

If, at implementation time, a released `@pulumi/github` version has a native resource or `Repository` field for this setting, the implementation SHOULD upgrade and use it instead of the dynamic resource, and record that in the plan's Deviations.

**Code scanning** uses a workflow ("advanced setup", §4.3), not default setup. Default setup MUST stay `not-configured`: GitHub rejects advanced-setup uploads while default setup is on. The workflow is also in the repository, pinned, and checked by `check-workflows`, which default setup is not.

### 4.3 Workflows

Both workflows follow the existing rules: actions pinned to a full commit hash with a version comment, `persist-credentials: false` on checkout, top-level `permissions: contents: read`, no `pull_request_target`, and no `${{ github.* }}` expression from the event inside a `run:` step. The pins are the current release of each action at implementation time, resolved from the action's repository (`gh api repos/<owner>/<repo>/git/ref/tags/<tag>`), not copied from elsewhere.

**`.github/workflows/codeql.yml`**

- Triggers: `push` to `main`, `pull_request` to `main`, and a weekly `schedule` (Mondays 07:00 UTC, clear of the nightly run at 06:00 and the mutants run on Sundays at 08:00). No `paths` filter, so every pull request has a base analysis to compare against.
- Concurrency: the same group-per-ref, cancel-in-progress pattern as `ci.yml`.
- One job, `analyze`, with a matrix over `language: [actions, rust]` and `build-mode: none` for both (neither needs a build). Steps: checkout, `github/codeql-action/init` (with `languages` and `build-mode` from the matrix and the default query suite), then `github/codeql-action/analyze` (with `category: /language:${{ matrix.language }}`).
- Job permissions: `contents: read`, `security-events: write` (to upload results). `actions: read` is needed only on private repositories and MUST NOT be added.

**`.github/workflows/scorecard.yml`**

- Triggers: `push` to `main`, a weekly `schedule` (Mondays 07:30 UTC), `branch_protection_rule` (re-scores when rules change) and `workflow_dispatch`. No `pull_request` trigger: Scorecard scores the default branch.
- One job, `analysis`. Steps: checkout, `ossf/scorecard-action` with `results_file: results.sarif`, `results_format: sarif` and `publish_results: false`, then `github/codeql-action/upload-sarif` with `sarif_file: results.sarif`. An `actions/upload-artifact` step with the SARIF file and `retention-days: 30` MAY be added for offline reading.
- Job permissions: `contents: read`, `security-events: write`. Without `publish_results`, `id-token: write` is not needed and MUST NOT be added.

### 4.4 `check-workflows` rule 5: write scopes are allowlisted

So that "workflows use a read-only token" stays enforced rather than becoming a sentence with exceptions, `cargo xtask check-workflows` gains a fifth rule:

5. A `permissions:` block at the top level MUST NOT grant any scope `write`, and MUST NOT be `write-all`. A job-level `permissions:` block MAY grant a scope `write` only if `(workflow file name, job id, scope)` is in the allowlist constant `WRITE_ALLOWLIST` in `xtask/src/check_workflows.rs`. `write-all` is never allowed. Each allowlist entry carries a comment giving the reason.

The initial allowlist is exactly:

- `("codeql.yml", "analyze", "security-events")`: uploads CodeQL results.
- `("scorecard.yml", "analysis", "security-events")`: uploads Scorecard results.

The check stays line-based, like the other rules. A job id is a key at indent 2 under the top-level `jobs:` key (every workflow here indents by 2). A job-level `permissions:` key is at indent 4 inside a job, with its scopes at indent 6 or written inline (`permissions: write-all`, `permissions: { ... }`). An inline mapping (`{ ... }`) MUST be parsed for `write` values too, so it cannot be used to skip the check. A finding names the file, line, job and scope, and says to add a reasoned allowlist entry if the grant is intended.

### 4.5 Dependabot (`.github/dependabot.yml`)

- Every `updates` entry gains `cooldown: { default-days: 7 }`. A new version is proposed only once it is 7 days old, which leaves time for a compromised release to be found and yanked. The cooldown does not delay security updates.
- If GitHub rejects `cooldown` for an ecosystem (the `devcontainers` entry is the least certain), that entry goes without it, with a comment saying why. That is recorded in Deviations.
- The opening comment block (GitHub's starter text) is replaced by one line saying what the file covers.

### 4.6 Documentation

- `CONTRIBUTING.md`, "Commits and pull requests": pull requests are squash-merged; the title is the commit subject on `main` (conventional, ending with the chunk ID for a chunk) and the branch's commit messages are its body; small logical commits are still wanted on the branch for review.
- `CONTRIBUTING.md`, "Reviews before a pull request": the security review's workflow item also asks whether a new write scope is in `WRITE_ALLOWLIST` with a reason.
- `SECURITY.md`, "Our own safeguards": the token sentence says workflows use a read-only token except the two code-scanning jobs, which can write only security events and are allowlisted by `check-workflows`. Add one line each for CodeQL, Scorecard and Dependabot security updates.
- `infra/github/README.md`: a "Settings without a provider resource" note on the dynamic resource (what it calls, that it needs `GITHUB_TOKEN`, and the upgrade path in §4.2), and that code scanning is configured by workflows and default setup must stay off.
- `docs/maintainers.md`: tick "Enable private vulnerability reporting" and "Enable Dependabot security updates; set the repository description and topics", each pointing at `infra/github` (ticked in the closing commit, after `pulumi up` has run).
- `docs/roadmap.md`: an M3.9 row; the "Later" row of "Later milestones" gains "release infrastructure (a `v*` tag ruleset, a protected `release` environment, crates.io trusted publishing), when the first release is planned".
- `CHANGELOG.md`: one entry.

## 5. Requirements

- **M3.9-R1** `index.ts` MUST declare the repository settings in the §4.1 table, with wiki and Projects off, issues on, and squash as the only merge method with `PR_TITLE` and `COMMIT_MESSAGES`.
- **M3.9-R2** The `main` ruleset MUST allow only the `squash` merge method and MUST require linear history.
- **M3.9-R3** `index.ts` MUST enable Dependabot security updates, depending on the vulnerability-alerts resource.
- **M3.9-R4** `index.ts` MUST declare private vulnerability reporting as enabled, through a resource whose read reports the live value, so drift shows in `pulumi preview --refresh`.
- **M3.9-R5** The dynamic resource MUST fail `pulumi up` with the method, path, status and body on a non-2xx response, and MUST NOT expose the token in errors or outputs.
- **M3.9-R6** `codeql.yml` MUST analyse `actions` and `rust` with `build-mode: none`, on pull requests and pushes to `main` and weekly, with `security-events: write` on the job only.
- **M3.9-R7** `scorecard.yml` MUST run Scorecard on pushes to `main`, weekly, on `branch_protection_rule` and on demand, upload SARIF to code scanning, set `publish_results: false`, and grant `security-events: write` on the job only, without `id-token`.
- **M3.9-R8** Neither new workflow MAY be added to `requiredChecks`, and code scanning default setup MUST remain `not-configured`.
- **M3.9-R9** `check-workflows` MUST flag any write scope or `write-all` in a top-level `permissions:` block, any job-level write scope not in `WRITE_ALLOWLIST`, and any job-level `write-all`, in both block and inline form, naming file, line, job and scope.
- **M3.9-R10** `WRITE_ALLOWLIST` MUST contain exactly the two entries in §4.4, each with a reason comment.
- **M3.9-R11** Every `dependabot.yml` entry SHOULD carry a 7-day `cooldown`. An entry without one MUST carry a comment giving the reason.
- **M3.9-R12** Before `pulumi up`, the wiki and Projects MUST be confirmed empty.
- **M3.9-R13** The documents in §4.6 MUST be updated as listed.
- **M3.9-R14** After merge and `pulumi up`, `pulumi preview --refresh` MUST show no changes.

## 6. Cross-cutting concerns

- **Determinism:** n/a, because no library code changes.
- **`unsafe`:** n/a, because no Rust crate other than `xtask` changes, and `xtask` keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because nothing concurrent is added. The CodeQL workflow uses the existing cancel-in-progress concurrency pattern.
- **Heap allocation:** n/a, because no library code changes.
- **`no_std` and wasm:** n/a, because no library code changes.
- **Frozen formats:** n/a, because no PRNG stream or `Handle::to_bits` change.
- **Public API changes:** none.
- **FFI exposure:** n/a, because there is no FFI.
- **Performance:** CI minutes only. CodeQL adds two jobs per pull request; `build-mode: none` keeps them to a few minutes each (the first run is measured and recorded in the pull request). They are not required checks, so a merge never waits on them. No benchmark changes.
- **Documentation:** §4.6.
- **Policy changes:** yes, three, each recorded in CONTRIBUTING or SECURITY: (1) squash-only merging with the PR title as the commit subject; (2) workflow tokens may hold a write scope, but only for a job allowlisted in `check-workflows` with a reason; (3) Dependabot version updates wait 7 days. None is hard to reverse, so no ADR.

## 7. Alternatives considered

- **Code scanning default setup instead of a workflow.** It needs no file, but Pulumi 6.15.1 cannot declare it, so it would be a UI setting outside `infra/github`. It cannot be pinned or reviewed in a pull request, and `check-workflows` cannot see it. Rejected.
- **A manual `gh api` step for private vulnerability reporting**, written down in `maintainers.md`. Simpler, but invisible to `pulumi preview` and silently revertible in the UI, which is how it came to be off while the documents relied on it. Rejected in favour of the dynamic resource.
- **`@pulumi/command` running `gh api`.** It adds a dependency and a dependency on `gh` at deploy time, and its read cannot report the live value without extra scripting. Rejected.
- **Allowing rebase merges as well as squash.** That would keep each branch commit on `main`, but every commit would then have to be clean and conventional, and history would mix one-commit and many-commit pull requests. The recent history is already squash-shaped (`(#33)` suffixes). Squash only was chosen (2026-11-14).
- **Scorecard with `publish_results: true`.** It enables the public score and a README badge, but needs `id-token: write` and fixes the workflow's shape to what the Scorecard API accepts. Deferred until the score is worth showing; turning it on is a one-line change plus an allowlist entry.
- **CodeQL `javascript-typescript` as well.** Cheap to add, but it scans only the Pulumi program, which never runs in CI and handles no untrusted input. Left out; adding it later is one matrix entry.
- **No `check-workflows` rule, just a comment in each workflow.** That breaks the rule `SECURITY.md` states as enforced, and the next write scope would go unnoticed. Rejected.
- **A longer cooldown (14 or 30 days).** It gives more margin against a compromised release, but this project updates weekly and uses few dependencies, so 7 days keeps updates small. A security update is never delayed in any case.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1, R2, R3, R4 | `pnpm typecheck` in `infra/github`; `pulumi preview --diff` output quoted in the pull request, showing exactly these changes and no others |
| R5 | The provider is exercised by `pulumi preview`/`up`; error paths by running the provider's `create` once with an invalid token (`GITHUB_TOKEN=invalid pulumi preview --refresh`), and the failure message quoted in the pull request, showing status and body and no token |
| R6, R7 | `cargo xtask check-workflows` passes; the first CodeQL run on the pull request and the first Scorecard run after merge (a `workflow_dispatch` run on `main`) are linked in the pull request and both upload results |
| R8 | `requiredChecks` diff is empty; `gh api repos/SpencerAWill/rust-engine-experiment/code-scanning/default-setup --jq .state` prints `not-configured` after merge |
| R9, R10 | Unit tests in `check_workflows.rs`: top-level write flagged; top-level `write-all` flagged; job-level write outside the allowlist flagged with job and scope; allowlisted job passes; inline mapping with write flagged; job-level `write-all` flagged even for an allowlisted job; `read` values pass. `cargo xtask check-workflows` passes on the repository's workflows |
| R11 | `dependabot.yml` diff; the Dependabot "Last checked" status for each ecosystem shows no configuration error after merge |
| R12 | The two commands of §4.1 and their output quoted in the pull request |
| R13 | Documentation review |
| R14 | `pulumi preview --refresh` output after `pulumi up`, posted on the pull request; `gh api repos/SpencerAWill/rust-engine-experiment/private-vulnerability-reporting` prints `{"enabled":true}` |

## 9. Done when

- [ ] `cargo xtask ci` passes locally and every required check is green on the pull request.
- [ ] `pnpm typecheck` passes in `infra/github`.
- [ ] The `pulumi preview --diff`, wiki and Projects checks, and the CodeQL run are recorded in the pull request.
- [ ] All eight reviews are listed in the pull request.
- [ ] After merge, on the maintainer's go-ahead: `pulumi up`, then the R8, R14 and Scorecard evidence posted on the pull request, and the `maintainers.md` boxes ticked.

## 10. Open questions

None.

## 11. Prior art and sources

- GitHub REST API: [private vulnerability reporting](https://docs.github.com/en/rest/repos/repos#enable-private-vulnerability-reporting-for-a-repository), [code scanning default setup](https://docs.github.com/en/rest/code-scanning/code-scanning#get-a-code-scanning-default-setup-configuration).
- GitHub Docs: [rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets), [CodeQL advanced setup](https://docs.github.com/en/code-security/code-scanning/creating-an-advanced-setup-for-code-scanning), [Dependabot `cooldown`](https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference#cooldown-).
- OpenSSF Scorecard and `ossf/scorecard-action` documentation, for the workflow's required shape and permissions.
- Pulumi documentation on dynamic resource providers.
