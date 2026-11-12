# GitHub configuration (Pulumi)

Status: Active (2026-11-12)

This Pulumi project declares the GitHub settings for `SpencerAWill/rust-engine-experiment`:
repository settings, the ruleset on `main`, labels, Actions permissions, and Actions variables
and secrets. `index.ts` is the source of truth. A setting changed in the GitHub UI is drift, and
the next `pulumi up` reverts it.

Commands run on your machine. Pulumi Cloud only stores the state (stack `SpencerAWill/github/prod`).

## Use

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
- **Admin bypass** is on, so the repository admin can still push to `main` directly. Everyone
  else needs a pull request with passing checks.
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
