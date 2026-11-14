//! `cargo xtask coverage`: line and function coverage through `cargo-llvm-cov`.
//!
//! The command runs the workspace tests under instrumentation, writes an HTML and an lcov report to
//! `target/llvm-cov/`, and prints a per-crate table computed by [`summary`].

use std::path::Path;
use std::process::Command;

use crate::cmd::{require_cargo_subcommand, run};

mod summary;

/// Where the reports go.
const OUT_DIR: &str = "target/llvm-cov";

/// Options of `coverage [--open]`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// Open the HTML report in a browser afterwards.
    pub(crate) open: bool,
}

/// Parses the arguments that follow `coverage`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let mut open = false;
    for flag in flags {
        match *flag {
            "--open" => open = true,
            other => return Err(format!("unknown flag `{other}` for `coverage`")),
        }
    }
    Ok(Options { open })
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_string()).collect()
}

/// Clears profiles from earlier runs, so they cannot mix into this one.
fn clean_args() -> Vec<String> {
    strings(&["llvm-cov", "clean", "--workspace"])
}

/// The tests of `cargo xtask test --include-ignored`, minus doctests (nightly only in
/// `cargo-llvm-cov`) and minus the packages that are tooling, not engine code.
fn test_args() -> Vec<String> {
    strings(&[
        "llvm-cov",
        "nextest",
        "--workspace",
        "--all-features",
        "--exclude",
        "xtask",
        "--exclude",
        "engine_benches",
        "--exclude",
        crate::REGRESS_PACKAGE,
        "--run-ignored",
        "all",
        "--no-report",
    ])
}

/// Flags shared by both reports. `cargo-llvm-cov` ignores every `examples/` directory by default,
/// which would drop the example crate's sources, so its default is off and `regex` does the work.
fn filter_args(regex: &str) -> Vec<String> {
    strings(&[
        "--no-default-ignore-filename-regex",
        "--ignore-filename-regex",
        regex,
    ])
}

fn html_args(regex: &str, open: bool) -> Vec<String> {
    let mut args = strings(&["llvm-cov", "report", "--html", "--output-dir", OUT_DIR]);
    if open {
        args.push("--open".to_string());
    }
    args.extend(filter_args(regex));
    args
}

fn lcov_args(regex: &str) -> Vec<String> {
    let mut args = strings(&["llvm-cov", "report", "--lcov", "--output-path"]);
    args.push(format!("{OUT_DIR}/lcov.info"));
    args.extend(filter_args(regex));
    args
}

/// The POSIX-extended regex (LLVM has no lookahead) for the files the reports leave out: tooling
/// (`xtask/`, `bench/`), every `tests/` and `benches/` directory of a library or example, and
/// the sources of dependencies and the standard library, which stop being filtered once the
/// default is off. Anchored at `root`, so a checkout under a directory named `tests` is safe.
fn ignore_regex(root: &Path) -> String {
    let mut escaped = String::new();
    for c in root.to_string_lossy().chars() {
        if ".[]()*+?{}|^$\\".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!(
        "^{escaped}/(xtask|bench|(libs|examples)/[^/]+/(tests|benches))/\
         |/(registry/src|git/checkouts)/|^/rustc/"
    )
}

/// Whether the `llvm-tools` component is missing for the active toolchain. `None` when `rustup`
/// cannot be asked (not installed), so the check is skipped.
fn llvm_tools_missing() -> Option<bool> {
    let output = Command::new("rustup")
        .args(["component", "list", "--installed"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(!text.lines().any(|l| l.starts_with("llvm-tools")))
}

fn cargo(args: &[String]) -> Result<(), String> {
    run(
        "cargo",
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &[],
    )
}

/// Runs the tests under instrumentation, writes the reports and prints the summary.
pub(crate) fn coverage(options: &Options) -> Result<(), String> {
    require_cargo_subcommand("llvm-cov", "cargo install --locked cargo-llvm-cov")?;
    if llvm_tools_missing() == Some(true) {
        return Err(
            "llvm-tools-preview missing: run 'rustup component add llvm-tools-preview'".to_string(),
        );
    }
    // Canonical, so it matches the resolved paths LLVM writes even through a symlinked checkout.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("cannot find the workspace root")?
        .canonicalize()
        .map_err(|e| format!("cannot resolve the workspace root: {e}"))?;
    let regex = ignore_regex(&root);

    cargo(&clean_args())?;
    cargo(&test_args())?;
    cargo(&html_args(&regex, false))?;
    cargo(&lcov_args(&regex))?;

    let lcov_path = format!("{OUT_DIR}/lcov.info");
    let lcov = std::fs::read_to_string(&lcov_path).map_err(|e| format!("{lcov_path}: {e}"))?;
    let table = summary::summarize(&root, &lcov, |path| {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    })?;
    let summary_path = format!("{OUT_DIR}/summary.md");
    std::fs::write(&summary_path, &table).map_err(|e| format!("{summary_path}: {e}"))?;
    println!("\n{table}");

    if options.open {
        cargo(&html_args(&regex, true))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_open_and_rejects_unknown_flags() {
        assert!(!parse_flags(&[]).unwrap().open);
        assert!(parse_flags(&["--open"]).unwrap().open);
        let err = parse_flags(&["--x"]).unwrap_err();
        assert!(err.contains("unknown flag `--x` for `coverage`"), "{err}");
    }

    #[test]
    fn test_args_match_the_spec() {
        assert_eq!(
            test_args(),
            [
                "llvm-cov",
                "nextest",
                "--workspace",
                "--all-features",
                "--exclude",
                "xtask",
                "--exclude",
                "engine_benches",
                "--exclude",
                "engine_bench_regress",
                "--run-ignored",
                "all",
                "--no-report"
            ]
        );
        assert_eq!(clean_args(), ["llvm-cov", "clean", "--workspace"]);
    }

    fn contains_pair(args: &[String], a: &str, b: &str) -> bool {
        args.windows(2).any(|w| w[0] == a && w[1] == b)
    }

    #[test]
    fn reports_share_the_regex() {
        let regex = "^/w/(xtask)/";
        let html = html_args(regex, false);
        let lcov = lcov_args(regex);
        for args in [&html, &lcov] {
            assert!(
                contains_pair(args, "--ignore-filename-regex", regex),
                "{args:?}"
            );
            // `cargo-llvm-cov` skips every `examples/` directory unless told not to.
            assert!(
                args.contains(&"--no-default-ignore-filename-regex".to_string()),
                "{args:?}"
            );
        }
        assert!(html.contains(&"--html".to_string()));
        assert!(contains_pair(&html, "--output-dir", "target/llvm-cov"));
        assert!(!html.contains(&"--open".to_string()));
        assert!(html_args(regex, true).contains(&"--open".to_string()));
        assert!(lcov.contains(&"--lcov".to_string()));
        assert!(contains_pair(
            &lcov,
            "--output-path",
            "target/llvm-cov/lcov.info"
        ));
    }

    #[test]
    fn ignore_regex_lists_the_excluded_dirs() {
        assert_eq!(
            ignore_regex(Path::new("/w")),
            "^/w/(xtask|bench|(libs|examples)/[^/]+/(tests|benches))/\
             |/(registry/src|git/checkouts)/|^/rustc/"
        );
    }

    #[test]
    fn ignore_regex_escapes_and_anchors_the_root() {
        let regex = ignore_regex(Path::new("/home/u/tests/repo.v2"));
        assert!(regex.starts_with("^/home/u/tests/repo\\.v2/"), "{regex}");
    }
}
