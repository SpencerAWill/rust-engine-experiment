//! `check-workflows`: guard rails for `.github/workflows/*.yml` (supply-chain and token hygiene).
//!
//! The rules, which keep a public repository's CI from being a way in:
//!
//! 1. Every `uses:` of a third-party action is pinned to a full 40-character commit hash, so a
//!    moved or hijacked tag cannot change what runs. Local actions (`./...`) are exempt, and a
//!    `docker://` image must carry a `sha256` digest.
//! 2. The workflow declares top-level `permissions:`, so the token never falls back to the
//!    repository default.
//! 3. Every `actions/checkout` sets `persist-credentials: false`.
//! 4. No workflow uses the `pull_request_target` trigger, which runs with secrets on code from
//!    forks.
//! 5. A top-level `permissions:` block grants no `write` scope, and no block is `write-all`. A
//!    job may grant a `write` scope only if `(file name, job id, scope)` is in
//!    [`WRITE_ALLOWLIST`], with a reason.
//!
//! The check reads the text line by line (there is no YAML parser in `xtask`), which is enough
//! for the plain block-style workflows this repository uses.

use std::fs;
use std::path::PathBuf;

/// One violation, with the file and 1-based line it was found on.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) message: String,
}

/// Where workflows live.
const WORKFLOW_DIR: &str = ".github/workflows";

/// The `(workflow file name, job id, scope)` triples that may hold a `write` token scope.
///
/// Each entry is a job-level grant that needs a reason, written in a comment above it. A
/// top-level grant and `write-all` are never allowed, so they have no entry here.
const WRITE_ALLOWLIST: &[(&str, &str, &str)] = &[
    // Uploads the CodeQL results to code scanning.
    ("codeql.yml", "analyze", "security-events"),
    // Uploads the Scorecard results to code scanning.
    ("scorecard.yml", "analysis", "security-events"),
];

/// The part of `line` before a YAML comment (a `#` at the start or after whitespace).
fn strip_comment(line: &str) -> &str {
    let mut previous_is_space = true;
    for (index, c) in line.char_indices() {
        if c == '#' && previous_is_space {
            return &line[..index];
        }
        previous_is_space = c.is_whitespace();
    }
    line
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The value of a step's `uses:` key, if this line is one (`- uses: x` or `uses: x`).
fn uses_value(trimmed: &str) -> Option<&str> {
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim_start();
    let value = body.strip_prefix("uses:")?.trim();
    Some(value.trim_matches(|c| c == '"' || c == '\''))
}

fn is_lowercase_hex(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Why `value` (a `uses:` reference) is not pinned, or `None` if it is.
fn pin_problem(value: &str) -> Option<String> {
    if value.starts_with("./") {
        return None; // a local action lives in this repository
    }
    if let Some(image) = value.strip_prefix("docker://") {
        let pinned = image
            .split_once("@sha256:")
            .is_some_and(|(_, digest)| is_lowercase_hex(digest, 64));
        return (!pinned)
            .then(|| format!("`{value}` is a container image without a `@sha256:` digest"));
    }
    let pinned = value
        .split_once('@')
        .is_some_and(|(_, reference)| is_lowercase_hex(reference, 40));
    (!pinned).then(|| {
        format!("`{value}` is not pinned to a full 40-character commit hash (use `owner/repo@<hash> # vX.Y.Z`)")
    })
}

/// Whether the step containing line `uses_index` sets `persist-credentials: false`.
fn disables_persisted_credentials(lines: &[&str], uses_index: usize) -> bool {
    let first = strip_comment(lines[uses_index]);
    let key_col = indent(first)
        + if first.trim_start().starts_with("- ") {
            2
        } else {
            0
        };
    let dash_col = key_col.saturating_sub(2);
    let mut start = uses_index;
    while !strip_comment(lines[start]).trim_start().starts_with("- ")
        || indent(strip_comment(lines[start])) != dash_col
    {
        if start == 0 {
            break;
        }
        start -= 1;
    }
    let mut end = uses_index + 1;
    while end < lines.len() {
        let code = strip_comment(lines[end]);
        if !code.trim().is_empty() && indent(code) < key_col {
            break;
        }
        end += 1;
    }
    lines[start..end].iter().any(|line| {
        strip_comment(line)
            .trim()
            .strip_prefix("persist-credentials:")
            .is_some_and(|value| value.trim() == "false")
    })
}

/// `text` without one pair of surrounding YAML quotes.
fn unquote(text: &str) -> &str {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')))
        .unwrap_or(text)
}

/// The `scope: value` pairs of an inline `permissions:` value (`{ a: read, b: write }`).
fn inline_pairs(value: &str) -> Vec<(&str, &str)> {
    value
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
        .filter_map(|pair| pair.split_once(':'))
        .map(|(scope, access)| (scope.trim(), access.trim()))
        .collect()
}

/// Where a `permissions:` block sits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Top,
    Job,
}

/// Checks the text of one workflow file. `file` labels findings and is matched, by its final path
/// component, against `allowlist` (see [`WRITE_ALLOWLIST`]).
pub(crate) fn scan_workflow(
    file: &str,
    text: &str,
    allowlist: &[(&str, &str, &str)],
) -> Vec<Finding> {
    let lines: Vec<&str> = text.lines().collect();
    let file_name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let mut findings = Vec::new();
    let mut add = |line: usize, message: String| {
        findings.push(Finding {
            file: file.to_string(),
            line,
            message,
        });
    };
    let mut has_top_level_permissions = false;
    let mut in_jobs = false;
    let mut job = String::new();
    // The `permissions:` block being read: its level and the indent of its key.
    let mut block: Option<(Level, usize)> = None;
    for (index, raw) in lines.iter().enumerate() {
        let code = strip_comment(raw);
        let trimmed = code.trim();
        if trimmed.is_empty() {
            continue;
        }
        let line = index + 1;
        let depth = indent(code);
        if block.is_some_and(|(_, key_indent)| depth <= key_indent) {
            block = None;
        }
        if depth == 0 {
            in_jobs = trimmed == "jobs:";
            job.clear();
        } else if in_jobs && depth == 2 {
            job = unquote(trimmed.split(':').next().unwrap_or("")).to_string();
        }
        let level = match depth {
            0 => Some(Level::Top),
            4 if in_jobs && !job.is_empty() => Some(Level::Job),
            _ => None,
        };
        if let (Some(level), Some(value)) = (level, trimmed.strip_prefix("permissions:")) {
            if level == Level::Top {
                has_top_level_permissions = true;
            }
            let value = value.trim();
            if value.is_empty() {
                block = Some((level, depth));
            } else if value == "write-all" {
                add(line, write_all_message(level, &job));
            } else if value.starts_with('{') {
                for (scope, access) in inline_pairs(value) {
                    if let Some(message) =
                        write_problem(level, &job, file_name, scope, access, allowlist)
                    {
                        add(line, message);
                    }
                }
            }
        } else if let Some((level, _)) = block
            && let Some((scope, access)) = trimmed.split_once(':')
            && let Some(message) = write_problem(
                level,
                &job,
                file_name,
                scope.trim(),
                access.trim(),
                allowlist,
            )
        {
            add(line, message);
        }
        if trimmed.contains("pull_request_target") {
            add(
                line,
                "the `pull_request_target` trigger runs with secrets on code from forks; use `pull_request`".to_string(),
            );
        }
        let Some(value) = uses_value(trimmed) else {
            continue;
        };
        if let Some(problem) = pin_problem(value) {
            add(line, problem);
        }
        if value.starts_with("actions/checkout@") && !disables_persisted_credentials(&lines, index)
        {
            add(
                line,
                "`actions/checkout` must set `persist-credentials: false` so the token is not left in git config".to_string(),
            );
        }
    }
    if !has_top_level_permissions {
        add(
            1,
            "no top-level `permissions:` block, so jobs get the repository's default token permissions (add `permissions: contents: read`)".to_string(),
        );
    }
    findings.sort_by_key(|f| f.line);
    findings
}

fn write_all_message(level: Level, job: &str) -> String {
    match level {
        Level::Top => "top-level `permissions: write-all` gives every job a write token; grant read scopes only".to_string(),
        Level::Job => format!("job `{job}` sets `permissions: write-all`, which is never allowed; list the scopes it needs"),
    }
}

/// Why granting `scope` the `access` level is not allowed here, or `None` if it is.
fn write_problem(
    level: Level,
    job: &str,
    file_name: &str,
    scope: &str,
    access: &str,
    allowlist: &[(&str, &str, &str)],
) -> Option<String> {
    if unquote(access) != "write" {
        return None;
    }
    match level {
        Level::Top => Some(format!(
            "top-level `permissions:` grants `{scope}: write` to every job; move it to the one job that needs it"
        )),
        Level::Job if allowlist.contains(&(file_name, job, scope)) => None,
        Level::Job => Some(format!(
            "job `{job}` grants `{scope}: write`; if that is intended, add (\"{file_name}\", \"{job}\", \"{scope}\") to `WRITE_ALLOWLIST` in xtask/src/check_workflows.rs with a reason"
        )),
    }
}

/// Checks every workflow in `.github/workflows` and prints the findings.
pub(crate) fn run() -> Result<(), String> {
    let entries = fs::read_dir(WORKFLOW_DIR).map_err(|e| format!("{WORKFLOW_DIR}: {e}"))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .collect();
    files.sort();
    let mut failures = Vec::new();
    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        for f in scan_workflow(&path.display().to_string(), &text, WRITE_ALLOWLIST) {
            failures.push(format!("{}:{}: {}", f.file, f.line, f.message));
        }
    }
    if failures.is_empty() {
        println!("check-workflows: ok ({} files)", files.len());
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "11d5960a326750d5838078e36cf38b85af677262";

    /// A workflow that follows every rule.
    fn good() -> String {
        format!(
            "name: CI\non:\n  push:\npermissions:\n  contents: read\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@{SHA} # v4\n        with:\n          persist-credentials: false\n      - uses: Swatinem/rust-cache@{SHA} # v2\n      - run: cargo build\n"
        )
    }

    fn messages(text: &str) -> Vec<(usize, String)> {
        scan_workflow("w.yml", text, &[])
            .into_iter()
            .map(|f| (f.line, f.message))
            .collect()
    }

    #[test]
    fn a_compliant_workflow_has_no_findings() {
        assert_eq!(scan_workflow("w.yml", &good(), &[]), Vec::new());
    }

    #[test]
    fn a_tag_or_branch_reference_is_flagged_with_its_line() {
        let text = good().replace(
            &format!("Swatinem/rust-cache@{SHA} # v2"),
            "Swatinem/rust-cache@v2",
        );
        let found = messages(&text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, 13);
        assert!(
            found[0].1.contains("Swatinem/rust-cache@v2"),
            "{}",
            found[0].1
        );
        assert!(found[0].1.contains("40-character"), "{}", found[0].1);
    }

    #[test]
    fn a_short_or_uppercase_hash_is_not_a_pin() {
        for bad in [&SHA[..39], &SHA.to_uppercase()] {
            let text = good().replace(
                &format!("Swatinem/rust-cache@{SHA}"),
                &format!("Swatinem/rust-cache@{bad}"),
            );
            assert_eq!(messages(&text).len(), 1, "{bad}");
        }
    }

    #[test]
    fn a_reference_without_any_ref_is_flagged() {
        let text = good().replace(
            &format!("Swatinem/rust-cache@{SHA} # v2"),
            "Swatinem/rust-cache",
        );
        assert_eq!(messages(&text).len(), 1);
    }

    #[test]
    fn local_actions_are_exempt_and_docker_images_need_a_digest() {
        let local = good().replace("- run: cargo build", "- uses: ./.github/actions/setup");
        assert_eq!(scan_workflow("w.yml", &local, &[]), Vec::new());
        let docker_tag = good().replace("- run: cargo build", "- uses: docker://alpine:3.20");
        assert_eq!(messages(&docker_tag).len(), 1);
        let docker_digest = good().replace(
            "- run: cargo build",
            &format!("- uses: docker://alpine@sha256:{}", "a".repeat(64)),
        );
        assert_eq!(scan_workflow("w.yml", &docker_digest, &[]), Vec::new());
    }

    #[test]
    fn commented_out_lines_are_ignored() {
        let text = format!(
            "{}# - uses: actions/checkout@v4\n# pull_request_target:\n",
            good()
        );
        assert_eq!(scan_workflow("w.yml", &text, &[]), Vec::new());
    }

    #[test]
    fn missing_top_level_permissions_is_flagged_even_if_a_job_sets_them() {
        let top_removed = good().replace("permissions:\n  contents: read\n", "");
        assert_eq!(messages(&top_removed).len(), 1);
        let job_level = good()
            .replace(
                "    runs-on: ubuntu-latest\n",
                "    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n",
            )
            .replace("permissions:\n  contents: read\njobs:", "jobs:");
        let found = messages(&job_level);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("top-level"), "{}", found[0].1);
    }

    #[test]
    fn checkout_must_disable_persisted_credentials() {
        let missing = good().replace("        with:\n          persist-credentials: false\n", "");
        let found = messages(&missing);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, 10);
        assert!(found[0].1.contains("persist-credentials"));
        let enabled = good().replace("persist-credentials: false", "persist-credentials: true");
        assert_eq!(messages(&enabled).len(), 1);
    }

    #[test]
    fn a_later_steps_setting_does_not_satisfy_an_earlier_checkout() {
        let text = good().replace(
            "        with:\n          persist-credentials: false\n      - uses: Swatinem",
            &format!("      - uses: Swatinem/rust-cache@{SHA}\n        with:\n          persist-credentials: false\n      - uses: Swatinem"),
        );
        let found = messages(&text);
        assert!(
            found.iter().any(|(_, m)| m.contains("persist-credentials")),
            "{found:?}"
        );
    }

    #[test]
    fn the_pull_request_target_trigger_is_flagged() {
        let text = good().replace("  push:\n", "  pull_request_target:\n");
        let found = messages(&text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("pull_request_target"));
    }

    const ALLOW: &[(&str, &str, &str)] = &[("w.yml", "build", "security-events")];

    fn write_findings(file: &str, text: &str) -> Vec<(usize, String)> {
        scan_workflow(file, text, ALLOW)
            .into_iter()
            .map(|f| (f.line, f.message))
            .collect()
    }

    /// `good()` with `block` inserted as the job `build`'s first key.
    fn with_job_permissions(block: &str) -> String {
        good().replace(
            "    runs-on: ubuntu-latest\n",
            &format!("    runs-on: ubuntu-latest\n{block}"),
        )
    }

    #[test]
    fn top_level_write_scope_is_flagged() {
        let text = good().replace("contents: read", "contents: write");
        let found = write_findings("w.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, 5);
        assert!(found[0].1.contains("contents"), "{}", found[0].1);
        assert!(found[0].1.contains("top-level"), "{}", found[0].1);
    }

    #[test]
    fn top_level_write_all_is_flagged() {
        let text = good().replace("permissions:\n  contents: read", "permissions: write-all");
        let found = write_findings("w.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("write-all"), "{}", found[0].1);
    }

    #[test]
    fn job_write_scope_outside_allowlist_is_flagged() {
        let text = with_job_permissions("    permissions:\n      pull-requests: write\n");
        let found = write_findings("w.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, 10);
        assert!(found[0].1.contains("`build`"), "{}", found[0].1);
        assert!(found[0].1.contains("pull-requests"), "{}", found[0].1);
        assert!(found[0].1.contains("WRITE_ALLOWLIST"), "{}", found[0].1);
    }

    #[test]
    fn allowlisted_job_write_scope_passes() {
        let text = with_job_permissions("    permissions:\n      security-events: write\n");
        assert_eq!(write_findings("w.yml", &text), Vec::new());
    }

    #[test]
    fn allowlist_is_per_file() {
        let text = with_job_permissions("    permissions:\n      security-events: write\n");
        let found = write_findings("other.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("security-events"), "{}", found[0].1);
        let in_dir = write_findings(".github/workflows/w.yml", &text);
        assert_eq!(in_dir, Vec::new(), "a path is matched by its file name");
    }

    #[test]
    fn allowlist_is_per_job_and_per_scope() {
        let other_job = good().replace("  build:", "  other:").replace(
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    permissions:\n      security-events: write\n",
        );
        assert_eq!(write_findings("w.yml", &other_job).len(), 1);
        let other_scope = with_job_permissions("    permissions:\n      contents: write\n");
        assert_eq!(write_findings("w.yml", &other_scope).len(), 1);
    }

    #[test]
    fn job_write_all_is_flagged_even_when_allowlisted() {
        let text = with_job_permissions("    permissions: write-all\n");
        let found = write_findings("w.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("write-all"), "{}", found[0].1);
    }

    #[test]
    fn inline_permissions_mapping_with_write_is_flagged() {
        let text = with_job_permissions("    permissions: { contents: read, issues: write }\n");
        let found = write_findings("w.yml", &text);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].1.contains("issues"), "{}", found[0].1);
        let top = good().replace(
            "permissions:\n  contents: read",
            "permissions: {contents: write}",
        );
        assert_eq!(write_findings("w.yml", &top).len(), 1);
    }

    #[test]
    fn permissions_text_inside_a_run_block_is_not_a_key() {
        let text = good().replace(
            "      - run: cargo build\n",
            "      - run: |\n          echo hi\n          permissions: write-all\n          contents: write\n",
        );
        assert_eq!(write_findings("w.yml", &text), Vec::new());
    }

    #[test]
    fn a_job_block_does_not_leak_into_the_next_job() {
        let text = format!(
            "{}  second:\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n    env:\n      contents: write\n",
            with_job_permissions("    permissions:\n      contents: read\n")
        );
        assert_eq!(write_findings("w.yml", &text), Vec::new());
    }

    #[test]
    fn read_scopes_pass() {
        let text = with_job_permissions(
            "    permissions:\n      contents: read\n      security-events: none\n",
        );
        assert_eq!(write_findings("w.yml", &text), Vec::new());
        let inline = with_job_permissions("    permissions: { contents: read }\n");
        assert_eq!(write_findings("w.yml", &inline), Vec::new());
        let read_all = with_job_permissions("    permissions: read-all\n");
        assert_eq!(write_findings("w.yml", &read_all), Vec::new());
    }

    #[test]
    fn quoted_write_values_and_quoted_job_ids_are_understood() {
        let quoted = with_job_permissions("    permissions:\n      contents: \"write\"\n");
        assert_eq!(write_findings("w.yml", &quoted).len(), 1);
        let single = with_job_permissions("    permissions: { contents: 'write' }\n");
        assert_eq!(write_findings("w.yml", &single).len(), 1);
        let top = good().replace("contents: read", "contents: 'write'");
        assert_eq!(write_findings("w.yml", &top).len(), 1);
        let id = good().replace("  build:", "  \"build\":").replace(
            "    runs-on: ubuntu-latest\n",
            "    runs-on: ubuntu-latest\n    permissions:\n      security-events: write\n",
        );
        assert_eq!(
            write_findings("w.yml", &id),
            Vec::new(),
            "allowlisted by its unquoted id"
        );
    }

    #[test]
    fn every_finding_names_its_file() {
        let found = scan_workflow(
            "ci.yml",
            &good().replace("rust-cache@", "rust-cache@x"),
            &[],
        );
        assert_eq!(found[0].file, "ci.yml");
    }
}
