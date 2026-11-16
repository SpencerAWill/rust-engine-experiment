# GitHub configuration (Pulumi)

Status: Living (2026-11-15)

This Pulumi project declares the GitHub settings for `SpencerAWill/rust-engine-experiment`:
repository settings, the ruleset on `main`, labels, Actions permissions, Actions variables
and secrets, Dependabot security updates and private vulnerability reporting. `index.ts` is the source of truth. A setting changed in the GitHub UI is drift, and
the next `pulumi up` reverts it.

Commands run on your machine. Pulumi Cloud only stores the state (stack `SpencerAWill/github/prod`).

## When this applies

- You change a repository setting, the `main` ruleset, a required check, a label, or an Actions variable or secret.
- A pull request that changes this directory has merged: run `pulumi preview`, then `pulumi up`.

## Contents

- `index.ts`: every setting, the ruleset and its `requiredChecks`, the labels.
- `privateVulnerabilityReporting.ts`: a dynamic resource for the one setting the provider lacks (see Notes).
- `Pulumi.yaml`: the project.
- `Pulumi.prod.yaml`: the `prod` stack's configuration, secrets encrypted by the stack's key.
- `package.json`: dependencies (installed with `pnpm` from the repository root).
- `tsconfig.json`: TypeScript settings.
- `.gitignore`: keeps `node_modules/` and `bin/` out of git.

## How to

```sh
cd infra/github
pnpm install   # from anywhere in the repo; the workspace root is the repository root
export GITHUB_TOKEN="$(gh auth token)"   # needs admin on the repository
pulumi stack select prod
pulumi preview --diff                    # always read this first
pulumi up
```

## Notes

- **Required checks** are the job names in `.github/workflows/ci.yml`. When a job is added,
  renamed or removed, update `requiredChecks` in `index.ts` in the same pull request. A required
  check that never reports blocks every pull request.
- **Admin bypass** is limited to pull requests (`bypassMode: "pull_request"`). Direct pushes to
  `main` are refused for everyone, the admin included; the admin can still merge a pull request
  without waiting for its checks.
- **Settings without a provider resource.** `@pulumi/github` 6.15.1 has no resource for private
  vulnerability reporting, so `privateVulnerabilityReporting.ts` is a dynamic resource that calls
  `PUT`, `GET` and `DELETE /repos/{owner}/{repo}/private-vulnerability-reporting` with Node's
  `fetch` and `GITHUB_TOKEN`. Its read reports the live value, so `pulumi preview --refresh`
  shows the setting drifting if someone turns it off in the UI. When a released provider has a
  native resource, switch to it and delete the file.
- **Code scanning is configured by workflows**, `codeql.yml` and `scorecard.yml`, not by default
  setup. Default setup must stay off (`gh api repos/SpencerAWill/rust-engine-experiment/code-scanning/default-setup --jq .state`
  prints `not-configured`): GitHub rejects the workflows' uploads while it is on. Neither
  workflow is a required check.
- **Merging** is squash only, with the pull request title as the commit subject, in both the
  repository settings and the `main` ruleset.
- **Labels** are authoritative: a label not listed in `index.ts` is deleted on `pulumi up`.
- **The repository resource is protected** and has `archiveOnDestroy`, so `pulumi destroy`
  cannot delete the repository.
- **Variables and secrets**:

  ```sh
  pulumi config set --path 'actionsVariables.NAME' value
  pulumi config set --path 'actionsSecretNames[0]' NAME
  pulumi config set --secret 'actionsSecret-NAME' value
  ```

  Secret values are encrypted in `Pulumi.prod.yaml` by the stack's key.
