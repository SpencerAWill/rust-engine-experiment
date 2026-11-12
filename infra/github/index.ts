// GitHub configuration for SpencerAWill/rust-engine-experiment, declared with Pulumi.
//
// Run locally (state is kept in Pulumi Cloud; nothing runs there):
//   pnpm install && cd infra/github
//   export GITHUB_TOKEN="$(gh auth token)"
//   pulumi preview      # review the diff
//   pulumi up           # apply it
//
// Everything below is the desired state. Changing a setting in the GitHub UI creates drift that
// the next `pulumi up` reverts, so make changes here instead.

import * as github from "@pulumi/github";
import * as pulumi from "@pulumi/pulumi";

const config = new pulumi.Config();
const repoName = "rust-engine-experiment";

// ---------------------------------------------------------------------------------------------
// Repository settings
// ---------------------------------------------------------------------------------------------

const repo = new github.Repository(
    "repo",
    {
        name: repoName,
        description: "Experimenting with creating a real-time simulation engine in rust",
        visibility: "public",

        hasIssues: true,
        hasProjects: true,
        hasWiki: true,
        hasDiscussions: false,

        allowMergeCommit: true,
        allowSquashMerge: true,
        allowRebaseMerge: true,
        allowAutoMerge: false,
        allowUpdateBranch: false,
        deleteBranchOnMerge: true,
        squashMergeCommitTitle: "COMMIT_OR_PR_TITLE",
        squashMergeCommitMessage: "COMMIT_MESSAGES",
        mergeCommitTitle: "MERGE_MESSAGE",
        mergeCommitMessage: "PR_TITLE",

        // Free on public repositories: catch committed credentials and block pushes containing them.
        securityAndAnalysis: {
            secretScanning: { status: "enabled" },
            secretScanningPushProtection: { status: "enabled" },
        },

        // `pulumi destroy` must never delete the repository itself.
        archiveOnDestroy: true,
    },
    { protect: true },
);

new github.RepositoryVulnerabilityAlerts("vulnerability-alerts", {
    repository: repo.name,
    enabled: true,
});

// ---------------------------------------------------------------------------------------------
// Ruleset on the default branch
// ---------------------------------------------------------------------------------------------

// Job names in .github/workflows/ci.yml. Keep in sync when jobs are added, renamed or removed:
// a required check that never reports blocks every pull request.
const requiredChecks = [
    "fmt",
    "clippy",
    "test",
    "doc",
    "no-std",
    "wasm",
    "check-unsafe",
    "check-statics",
    "check-workflows",
    "hack",
    "typos",
    "deny",
    "bench-build",
    "os-matrix (macos-latest)",
    "os-matrix (windows-latest)",
];

// GitHub Actions' app id: only results posted by Actions satisfy the checks.
const githubActionsAppId = 15368;
// Built-in "Admin" repository role.
const adminRoleId = 5;

new github.RepositoryRuleset("main-ruleset", {
    name: "main",
    repository: repo.name,
    target: "branch",
    enforcement: "active",
    conditions: {
        refName: { includes: ["~DEFAULT_BRANCH"], excludes: [] },
    },
    bypassActors: [{ actorType: "RepositoryRole", actorId: adminRoleId, bypassMode: "always" }],
    rules: {
        deletion: true,
        nonFastForward: true,
        pullRequest: {
            // Sole maintainer: GitHub does not let an author approve their own pull request.
            requiredApprovingReviewCount: 0,
            dismissStaleReviewsOnPush: false,
            requireCodeOwnerReview: false,
            requireLastPushApproval: false,
            requiredReviewThreadResolution: false,
        },
        requiredStatusChecks: {
            strictRequiredStatusChecksPolicy: false,
            requiredChecks: requiredChecks.map((context) => ({
                context,
                integrationId: githubActionsAppId,
            })),
        },
    },
});

// ---------------------------------------------------------------------------------------------
// Labels (authoritative: labels not listed here are deleted)
// ---------------------------------------------------------------------------------------------

const labels: { name: string; color: string; description: string }[] = [
    { name: "bug", color: "d73a4a", description: "Something isn't working" },
    { name: "documentation", color: "0075ca", description: "Improvements or additions to documentation" },
    { name: "duplicate", color: "cfd3d7", description: "This issue or pull request already exists" },
    { name: "enhancement", color: "a2eeef", description: "New feature or request" },
    { name: "good first issue", color: "7057ff", description: "Good for newcomers" },
    { name: "help wanted", color: "008672", description: "Extra attention is needed" },
    { name: "invalid", color: "e4e669", description: "This doesn't seem right" },
    { name: "question", color: "d876e3", description: "Further information is requested" },
    { name: "wontfix", color: "ffffff", description: "This will not be worked on" },
    // Applied by Dependabot (.github/dependabot.yml); it recreates them if they are missing.
    { name: "dependencies", color: "0366d6", description: "Pull requests that update a dependency file" },
    { name: "rust", color: "000000", description: "Pull requests that update rust code" },
    { name: "github_actions", color: "000000", description: "Pull requests that update GitHub Actions code" },
    {
        name: "devcontainers_package_manager",
        color: "2753E3",
        description: "Pull requests that update devcontainers_package_manager code",
    },
];

new github.IssueLabels("labels", { repository: repo.name, labels });

// ---------------------------------------------------------------------------------------------
// GitHub Actions
// ---------------------------------------------------------------------------------------------

new github.ActionsRepositoryPermissions("actions-permissions", {
    repository: repo.name,
    enabled: true,
    allowedActions: "all",
    // Matches `cargo xtask check-workflows` rule 1: third-party actions are pinned to a commit.
    shaPinningRequired: true,
});

new github.WorkflowRepositoryPermissions("workflow-permissions", {
    repository: repo.name,
    defaultWorkflowPermissions: "read",
    canApprovePullRequestReviews: false,
});

// Plain Actions variables: `pulumi config set --path 'actionsVariables.NAME' value`.
const variables = config.getObject<Record<string, string>>("actionsVariables") ?? {};
for (const [name, value] of Object.entries(variables)) {
    new github.ActionsVariable(`variable-${name}`, { repository: repo.name, variableName: name, value });
}

// Actions secrets. Names are plain config so the set is known at preview time; each value is an
// encrypted config entry:
//   pulumi config set --path 'actionsSecretNames[0]' NAME
//   pulumi config set --secret 'actionsSecret-NAME' value
const secretNames = config.getObject<string[]>("actionsSecretNames") ?? [];
for (const name of secretNames) {
    new github.ActionsSecret(`secret-${name}`, {
        repository: repo.name,
        secretName: name,
        plaintextValue: config.requireSecret(`actionsSecret-${name}`),
    });
}

export const repositoryUrl = repo.htmlUrl;
