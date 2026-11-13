//! `cargo xtask bench-regress`: compare the Gungraun instruction counts of a base commit with
//! the working tree, and `cargo xtask gungraun-version`, which prints the version CI and the
//! devcontainer install the runner at (spec M2.4 §4.4, §4.8).

// The command itself arrives in a later step of M2.4; until then most helpers below are only
// exercised by the tests. Remove this attribute when `bench_regress` uses them.
#![expect(
    dead_code,
    reason = "used by the command, which the next step of M2.4 adds"
)]

use std::fs;
use std::path::{Path, PathBuf};

/// Flags of `cargo xtask bench-regress`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// The revision to compare against; the merge base with `origin/main` when absent.
    pub(crate) base: Option<String>,
    /// A regression does not fail the command (the `perf-regression-accepted` label); a failure
    /// still does.
    pub(crate) report_only: bool,
}

/// Parses the arguments that follow `bench-regress`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let mut options = Options {
        base: None,
        report_only: false,
    };
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        match *flag {
            "--base" => {
                let rev = it
                    .next()
                    .filter(|rev| !rev.starts_with('-'))
                    .ok_or("`--base` needs a revision")?;
                if options.base.is_some() {
                    return Err("`--base` given more than once".to_string());
                }
                options.base = Some((*rev).to_string());
            }
            "--report-only" => {
                if options.report_only {
                    return Err("`--report-only` given more than once".to_string());
                }
                options.report_only = true;
            }
            other => return Err(format!("unknown flag `{other}` for `bench-regress`")),
        }
    }
    Ok(options)
}

/// The oldest Valgrind that Gungraun supports (spec §4.4 step 1), as (major, minor).
const MIN_VALGRIND: (u32, u32) = (3, 20);

/// How to install the runner; `gungraun-version` keeps it in step with `Cargo.lock`.
pub(crate) const RUNNER_INSTALL_HINT: &str =
    "cargo install --locked gungraun-runner --version \"$(cargo xtask gungraun-version)\"";

/// The `version` of the `[[package]]` named exactly `package` in a `Cargo.lock`.
///
/// `gungraun-runner` and `gungraun-macros` are packages of their own, with versions of their own,
/// so a prefix match would find the wrong one.
fn lock_version(cargo_lock: &str, package: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = false;
        } else if let Some(name) = quoted_value(line, "name") {
            in_package = name == package;
        } else if in_package && let Some(version) = quoted_value(line, "version") {
            return Some(version.to_string());
        }
    }
    None
}

/// The text between the quotes of a `key = "value"` line.
fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
    rest.trim().strip_prefix('"')?.strip_suffix('"')
}

/// (major, minor) from `valgrind --version`, which prints `valgrind-3.24.0`.
fn valgrind_version(output: &str) -> Option<(u32, u32)> {
    let mut numbers = output.trim().strip_prefix("valgrind-")?.split('.');
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    Some((major, minor))
}

/// The version from `gungraun-runner --version`, which prints `gungraun-runner 0.20.0`.
fn runner_version(output: &str) -> Option<String> {
    let version = output.trim().strip_prefix("gungraun-runner ")?.trim();
    (!version.is_empty()).then(|| version.to_string())
}

/// What the head run can see changed: a change to any of these can change an instruction count
/// (spec §4.4 step 2). A pattern ending in `/**` matches everything below that directory; any
/// other pattern matches the path exactly.
const RELEVANT: &[&str] = &[
    "libs/**",
    "bench-regress/**",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/**",
    "xtask/src/bench_regress.rs",
    "xtask/src/bench_regress/**",
    ".github/workflows/bench-regress.yml",
];

/// Whether a changed path (relative to the repository root, with `/` separators) can change an
/// instruction count.
fn is_relevant(path: &str) -> bool {
    RELEVANT
        .iter()
        .any(|pattern| match pattern.strip_suffix("**") {
            Some(directory) => path.starts_with(directory),
            None => path == *pattern,
        })
}

/// The effective Cargo target directory: `$CARGO_TARGET_DIR`, or `<repo>/target`.
fn target_dir(cargo_target_dir: Option<&str>, repo: &Path) -> PathBuf {
    match cargo_target_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => repo.join("target"),
    }
}

/// Why the base run does not happen (spec §4.4 step 4).
#[derive(Debug, PartialEq, Eq)]
enum BaseSkip {
    /// The base has no `bench-regress/Cargo.toml`: every benchmark is new.
    NoCrate,
    /// The base was built against another `gungraun`, whose binaries the installed runner cannot
    /// run: not compared.
    VersionChanged { base: String, head: String },
}

/// Decides whether the base run is skipped, from whether the base has the crate and from the two
/// `Cargo.lock` texts.
fn base_skip(base_has_crate: bool, base_lock: &str, head_lock: &str) -> Option<BaseSkip> {
    if !base_has_crate {
        return Some(BaseSkip::NoCrate);
    }
    let base = lock_version(base_lock, "gungraun");
    let head = lock_version(head_lock, "gungraun");
    (base != head).then(|| BaseSkip::VersionChanged {
        base: base.unwrap_or_else(|| "none".to_string()),
        head: head.unwrap_or_else(|| "none".to_string()),
    })
}

/// `cargo xtask gungraun-version`: prints the `gungraun` version in `Cargo.lock`, which is the
/// version `gungraun-runner` must have. The devcontainer and the CI action both follow it.
pub(crate) fn print_gungraun_version() -> Result<(), String> {
    let lock = fs::read_to_string("Cargo.lock").map_err(|e| format!("Cargo.lock: {e}"))?;
    let version = lock_version(&lock, "gungraun")
        .ok_or("Cargo.lock has no `gungraun` package; is `engine_bench_regress` still a member?")?;
    println!("{version}");
    Ok(())
}

/// `cargo xtask bench-regress`. Not implemented yet.
pub(crate) fn bench_regress(options: &Options) -> Result<(), String> {
    Err(format!(
        "bench-regress is not implemented yet (base: {:?}, report-only: {})",
        options.base, options.report_only
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base_and_report_only() {
        assert_eq!(
            parse_flags(&[]),
            Ok(Options {
                base: None,
                report_only: false
            })
        );
        assert_eq!(
            parse_flags(&["--base", "HEAD^1", "--report-only"]),
            Ok(Options {
                base: Some("HEAD^1".to_string()),
                report_only: true
            })
        );
    }

    #[test]
    fn rejects_unknown_missing_and_repeated_flags() {
        for flags in [
            &["--bogus"][..],
            &["--base"],
            &["--base", "--report-only"],
            &["--base", "a", "--base", "b"],
            &["--report-only", "--report-only"],
        ] {
            assert!(parse_flags(flags).is_err(), "{flags:?} should be rejected");
        }
    }

    #[test]
    fn lock_version_matches_the_exact_package_name() {
        let lock = r#"
    [[package]]
    name = "gungraun-runner"
    version = "0.19.0"

    [[package]]
    name = "gungraun-macros"
    version = "0.20.0"

    [[package]]
    name = "gungraun"
    version = "0.20.0"
    dependencies = ["gungraun-macros"]
    "#;
        assert_eq!(lock_version(lock, "gungraun").as_deref(), Some("0.20.0"));
        assert_eq!(
            lock_version(lock, "gungraun-runner").as_deref(),
            Some("0.19.0")
        );
        let without = "[[package]]\nname = \"gungraun-runner\"\nversion = \"0.19.0\"\n";
        assert_eq!(lock_version(without, "gungraun"), None);
    }

    #[test]
    fn parses_valgrind_and_runner_versions() {
        assert_eq!(valgrind_version("valgrind-3.24.0\n"), Some((3, 24)));
        assert_eq!(valgrind_version("valgrind-3.19.0"), Some((3, 19)));
        assert!(valgrind_version("valgrind-3.19.0") < Some(MIN_VALGRIND));
        assert!(valgrind_version("valgrind-3.24.0") >= Some(MIN_VALGRIND));
        assert_eq!(valgrind_version("garbage"), None);
        assert_eq!(valgrind_version(""), None);
        assert_eq!(
            runner_version("gungraun-runner 0.20.0\n").as_deref(),
            Some("0.20.0")
        );
        assert_eq!(runner_version("garbage"), None);
    }

    #[test]
    fn relevance_table() {
        for path in [
            "libs/engine_handle/src/pool.rs",
            "bench-regress/benches/prng.rs",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo/config.toml",
            "xtask/src/bench_regress.rs",
            "xtask/src/bench_regress/report.rs",
            ".github/workflows/bench-regress.yml",
        ] {
            assert!(is_relevant(path), "{path} should be relevant");
        }
        for path in [
            "docs/benchmarks.md",
            "README.md",
            "xtask/src/mutants.rs",
            "benches/benches/prng.rs",
            "libs",
            "examples/fixed_step_sim/src/main.rs",
        ] {
            assert!(!is_relevant(path), "{path} should not be relevant");
        }
    }

    #[test]
    fn target_dir_prefers_cargo_target_dir() {
        let repo = Path::new("/repo");
        assert_eq!(target_dir(Some("/target"), repo), PathBuf::from("/target"));
        assert_eq!(target_dir(None, repo), PathBuf::from("/repo/target"));
        assert_eq!(target_dir(Some(""), repo), PathBuf::from("/repo/target"));
    }

    #[test]
    fn base_skip_cases() {
        let v20 = "[[package]]\nname = \"gungraun\"\nversion = \"0.20.0\"\n";
        let v21 = "[[package]]\nname = \"gungraun\"\nversion = \"0.21.0\"\n";
        let none = "[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n";
        assert_eq!(base_skip(false, v20, v20), Some(BaseSkip::NoCrate));
        assert_eq!(base_skip(true, v20, v20), None);
        assert_eq!(
            base_skip(true, v20, v21),
            Some(BaseSkip::VersionChanged {
                base: "0.20.0".to_string(),
                head: "0.21.0".to_string()
            })
        );
        assert_eq!(
            base_skip(true, none, v20),
            Some(BaseSkip::VersionChanged {
                base: "none".to_string(),
                head: "0.20.0".to_string()
            })
        );
    }
}
