//! The `fuzz`, `fuzz-build` and `fuzz-toolchain` commands, and the nightly pin they share.
//!
//! The command lines are built by pure functions and unit-tested; only [`fuzz`] and
//! [`fuzz_build`] touch the machine.

use std::process::{Command, Stdio};

use crate::cmd::{require_cargo_subcommand, run_in};

/// The nightly toolchain every fuzz command uses. This is the only place the date is written:
/// CI and the devcontainer read it through `cargo xtask fuzz-toolchain`.
pub(crate) const FUZZ_NIGHTLY: &str = "nightly-2026-11-12";

/// The fuzz targets, in the order `fuzz all` runs them. A test checks this list against
/// `fuzz/fuzz_targets/*.rs`.
pub(crate) const TARGETS: [&str; 4] = [
    "handle_pool_ops",
    "handle_allocator_ops",
    "dense_allocator_ops",
    "handle_from_bits",
];

/// The directory of the `fuzz/` package, relative to the repository root.
const FUZZ_DIR: &str = "fuzz";

/// Seconds a named target runs when `--time` is not given.
const DEFAULT_TARGET_SECONDS: u64 = 60;

/// Seconds each target runs under `all` when `--time` is not given (the nightly setting).
const DEFAULT_ALL_SECONDS: u64 = 600;

const CARGO_FUZZ_INSTALL: &str = "cargo install --locked cargo-fuzz";

/// Which targets a `fuzz` invocation runs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    /// Every target in [`TARGETS`].
    All,
    /// One target from [`TARGETS`].
    One(&'static str),
}

/// What a `fuzz` invocation does with the selected target.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Fuzz for a fixed time.
    Run {
        /// The time limit per target, in seconds.
        seconds: u64,
    },
    /// Run one saved input (a crash artifact) and stop.
    Replay {
        /// The input file.
        path: String,
    },
}

/// The parsed arguments of `fuzz <target|all> [--time <secs>] [--replay <path>]`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// The targets to run.
    pub(crate) selection: Selection,
    /// How to run them.
    pub(crate) mode: Mode,
}

/// Parses the arguments that follow `fuzz`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let (name, rest) = flags.split_first().ok_or_else(|| {
        format!(
            "`fuzz` needs a target: all, {} (try `fuzz all --time 5`)",
            TARGETS.join(", ")
        )
    })?;
    let selection = if *name == "all" {
        Selection::All
    } else {
        let target = TARGETS.iter().find(|t| **t == *name).ok_or_else(|| {
            format!(
                "unknown fuzz target `{name}`; valid targets: all, {}",
                TARGETS.join(", ")
            )
        })?;
        Selection::One(target)
    };
    let (mut seconds, mut replay) = (None, None);
    let mut it = rest.iter();
    while let Some(flag) = it.next() {
        match *flag {
            "--time" => {
                let value = it.next().ok_or("`--time` needs a number of seconds")?;
                seconds =
                    Some(value.parse::<u64>().map_err(|_| {
                        format!("`--time` needs a number of seconds, got `{value}`")
                    })?);
            }
            "--replay" => {
                let path = it.next().ok_or("`--replay` needs an input file path")?;
                replay = Some((*path).to_string());
            }
            other => return Err(format!("unknown flag `{other}` for `fuzz`")),
        }
    }
    let mode = match (replay, seconds) {
        (Some(_), Some(_)) => {
            return Err(
                "`--replay` runs a single input; it cannot be combined with `--time`".into(),
            );
        }
        (Some(_), None) if selection == Selection::All => {
            return Err("`--replay` needs a named target, not `all`".into());
        }
        (Some(path), None) => Mode::Replay { path },
        (None, seconds) => Mode::Run {
            seconds: seconds.unwrap_or(match selection {
                Selection::All => DEFAULT_ALL_SECONDS,
                Selection::One(_) => DEFAULT_TARGET_SECONDS,
            }),
        },
    };
    Ok(Options { selection, mode })
}

/// The `+<toolchain>` argument that selects the pinned nightly.
fn pin_arg() -> String {
    format!("+{FUZZ_NIGHTLY}")
}

/// Flags shared by every `cargo fuzz` invocation: no sanitizer (the code under test is safe
/// Rust, so a sanitizer costs throughput and finds nothing), but debug assertions and overflow
/// checks stay on.
const COMMON_FLAGS: [&str; 3] = ["--sanitizer", "none", "--debug-assertions"];

/// The `cargo` arguments that fuzz `target` for `seconds`.
pub(crate) fn run_args(target: &str, seconds: u64) -> Vec<String> {
    let mut args = base_args("run");
    args.extend([
        target.to_string(),
        "--".to_string(),
        format!("-max_total_time={seconds}"),
    ]);
    args
}

/// The `cargo` arguments that run `target` once on the saved input at `path`.
pub(crate) fn replay_args(target: &str, path: &str) -> Vec<String> {
    let mut args = base_args("run");
    args.extend([target.to_string(), path.to_string()]);
    args
}

/// The `cargo` arguments that compile every target without running it.
pub(crate) fn build_args() -> Vec<String> {
    base_args("build")
}

fn base_args(subcommand: &str) -> Vec<String> {
    [pin_arg(), "fuzz".to_string(), subcommand.to_string()]
        .into_iter()
        .chain(COMMON_FLAGS.map(String::from))
        .collect()
}

/// The error shown when a fuzz command is run on Windows.
pub(crate) fn windows_message() -> String {
    "fuzzing is not supported on Windows (libFuzzer needs a non-MSVC toolchain); \
     use the devcontainer or WSL"
        .to_string()
}

/// Checks that fuzzing can run here: not on Windows, and with the tools present.
///
/// `tools` is only called when `windows` is false.
pub(crate) fn preflight(
    windows: bool,
    tools: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if windows {
        return Err(windows_message());
    }
    tools()
}

/// Checks for `cargo-fuzz` and the pinned nightly toolchain, returning the install line for
/// whichever is missing.
fn require_tools() -> Result<(), String> {
    require_cargo_subcommand("fuzz", CARGO_FUZZ_INSTALL)?;
    let found = Command::new("rustup")
        .args(["run", FUZZ_NIGHTLY, "rustc", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if found {
        Ok(())
    } else {
        Err(format!(
            "toolchain {FUZZ_NIGHTLY} missing: run 'rustup toolchain install {FUZZ_NIGHTLY} --profile minimal'"
        ))
    }
}

fn host_preflight() -> Result<(), String> {
    preflight(cfg!(windows), require_tools)
}

/// What `fuzz-build` does on this machine.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BuildMode {
    /// Build the targets.
    Run,
    /// Print this note and carry on (the build is optional here).
    Skip(String),
    /// Stop with this error.
    Fail(String),
}

/// Decides what `fuzz-build` does given the [`preflight`] result. A `strict` build (run
/// directly, or under `CI`) fails when fuzzing is unavailable; a lenient one, inside a local
/// `ci`, skips with a loud note so unrelated work does not need a nightly toolchain.
pub(crate) fn build_mode(preflight: Result<(), String>, strict: bool) -> BuildMode {
    match preflight {
        Ok(()) => BuildMode::Run,
        Err(reason) if strict => BuildMode::Fail(reason),
        Err(reason) => BuildMode::Skip(format!("skipped fuzz-build: {reason}")),
    }
}

fn cargo_in_fuzz_dir(args: &[String]) -> Result<(), String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_in(FUZZ_DIR, "cargo", &args, &[])
}

/// Runs `fuzz <target|all> ...`.
pub(crate) fn fuzz(options: &Options) -> Result<(), String> {
    host_preflight()?;
    match (&options.selection, &options.mode) {
        (Selection::One(target), Mode::Run { seconds }) => {
            cargo_in_fuzz_dir(&run_args(target, *seconds))
        }
        (Selection::One(target), Mode::Replay { path }) => {
            cargo_in_fuzz_dir(&replay_args(target, path))
        }
        (Selection::All, Mode::Run { seconds }) => run_all(&TARGETS, |target| {
            cargo_in_fuzz_dir(&run_args(target, *seconds))
        }),
        (Selection::All, Mode::Replay { .. }) => {
            Err("`--replay` needs a named target, not `all`".to_string())
        }
    }
}

/// Runs `run_one` for every target even when some fail, then reports every failure at once so a
/// single night reveals all findings.
pub(crate) fn run_all(
    targets: &[&str],
    mut run_one: impl FnMut(&str) -> Result<(), String>,
) -> Result<(), String> {
    let failed: Vec<&str> = targets
        .iter()
        .copied()
        .filter(|target| run_one(target).is_err())
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} of {} fuzz targets failed: {}",
            failed.len(),
            targets.len(),
            failed.join(", ")
        ))
    }
}

/// Runs `fuzz-build`. `strict` makes missing tools an error instead of a skip note.
pub(crate) fn fuzz_build(strict: bool) -> Result<(), String> {
    match build_mode(host_preflight(), strict) {
        BuildMode::Run => cargo_in_fuzz_dir(&build_args()),
        BuildMode::Skip(note) => {
            println!("{note}");
            Ok(())
        }
        BuildMode::Fail(reason) => Err(reason),
    }
}

/// Prints the pinned toolchain name, for the workflows and the devcontainer.
pub(crate) fn print_toolchain() {
    println!("{FUZZ_NIGHTLY}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin() -> String {
        format!("+{FUZZ_NIGHTLY}")
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_named_target_defaults_to_sixty_seconds() {
        let options = parse_flags(&["handle_pool_ops"]).unwrap();
        assert_eq!(options.selection, Selection::One("handle_pool_ops"));
        assert_eq!(options.mode, Mode::Run { seconds: 60 });
    }

    #[test]
    fn all_defaults_to_ten_minutes_per_target() {
        let options = parse_flags(&["all"]).unwrap();
        assert_eq!(options.selection, Selection::All);
        assert_eq!(options.mode, Mode::Run { seconds: 600 });
    }

    #[test]
    fn time_overrides_the_default() {
        let options = parse_flags(&["handle_from_bits", "--time", "5"]).unwrap();
        assert_eq!(options.mode, Mode::Run { seconds: 5 });
        let options = parse_flags(&["all", "--time", "7"]).unwrap();
        assert_eq!(options.mode, Mode::Run { seconds: 7 });
    }

    #[test]
    fn replay_is_accepted_only_with_a_named_target() {
        let options = parse_flags(&["handle_pool_ops", "--replay", "fuzz/artifacts/x"]).unwrap();
        assert_eq!(
            options.mode,
            Mode::Replay {
                path: "fuzz/artifacts/x".into()
            }
        );
        let err = parse_flags(&["all", "--replay", "x"]).unwrap_err();
        assert!(err.contains("named target"), "{err}");
    }

    #[test]
    fn replay_and_time_cannot_be_combined() {
        // A replay runs one input; a time limit would be silently ignored.
        assert!(parse_flags(&["handle_pool_ops", "--replay", "x", "--time", "5"]).is_err());
        assert!(parse_flags(&["handle_pool_ops", "--time", "5", "--replay", "x"]).is_err());
    }

    #[test]
    fn an_unknown_target_lists_the_valid_ones() {
        let err = parse_flags(&["nope"]).unwrap_err();
        assert!(err.contains("unknown fuzz target `nope`"), "{err}");
        for target in TARGETS {
            assert!(err.contains(target), "{err}");
        }
        assert!(err.contains("all"), "{err}");
    }

    #[test]
    fn a_missing_target_or_a_bad_flag_is_an_error() {
        assert!(parse_flags(&[]).unwrap_err().contains("needs a target"));
        assert!(parse_flags(&["all", "--x"]).unwrap_err().contains("--x"));
        assert!(parse_flags(&["--time", "5"]).is_err());
    }

    #[test]
    fn time_needs_a_number() {
        assert!(
            parse_flags(&["all", "--time"])
                .unwrap_err()
                .contains("--time")
        );
        let err = parse_flags(&["all", "--time", "soon"]).unwrap_err();
        assert!(err.contains("--time") && err.contains("soon"), "{err}");
        assert!(parse_flags(&["all", "--replay"]).is_err());
    }

    #[test]
    fn run_args_pass_the_pin_the_sanitizer_and_debug_flags_then_the_time_limit() {
        assert_eq!(
            run_args("handle_pool_ops", 60),
            [
                pin().as_str(),
                "fuzz",
                "run",
                "--sanitizer",
                "none",
                "--debug-assertions",
                "handle_pool_ops",
                "--",
                "-max_total_time=60"
            ]
        );
    }

    #[test]
    fn replay_args_end_with_the_path_and_have_no_time_flag() {
        let args = replay_args("handle_pool_ops", "fuzz/artifacts/handle_pool_ops/crash-1");
        assert_eq!(
            args,
            strings(&[
                &pin(),
                "fuzz",
                "run",
                "--sanitizer",
                "none",
                "--debug-assertions",
                "handle_pool_ops",
                "fuzz/artifacts/handle_pool_ops/crash-1",
            ])
        );
        assert!(!args.iter().any(|a| a.contains("max_total_time")));
    }

    #[test]
    fn build_args_pass_the_pin_the_sanitizer_and_debug_flags() {
        assert_eq!(
            build_args(),
            strings(&[
                &pin(),
                "fuzz",
                "build",
                "--sanitizer",
                "none",
                "--debug-assertions"
            ])
        );
    }

    #[test]
    fn the_windows_message_points_at_the_devcontainer() {
        let message = windows_message();
        assert!(message.contains("devcontainer"), "{message}");
        assert!(message.contains("Windows"), "{message}");
    }

    #[test]
    fn preflight_refuses_windows_before_looking_for_tools() {
        let mut looked = false;
        let result = preflight(true, || {
            looked = true;
            Ok(())
        });
        assert_eq!(result, Err(windows_message()));
        assert!(!looked, "tools must not be probed on Windows");
        assert_eq!(preflight(false, || Ok(())), Ok(()));
        assert_eq!(
            preflight(false, || Err("cargo-fuzz missing".into())),
            Err("cargo-fuzz missing".into())
        );
    }

    #[test]
    fn fuzz_build_runs_when_the_tools_are_present_whatever_the_mode() {
        assert_eq!(build_mode(Ok(()), false), BuildMode::Run);
        assert_eq!(build_mode(Ok(()), true), BuildMode::Run);
    }

    #[test]
    fn a_lenient_fuzz_build_skips_loudly_and_a_strict_one_fails() {
        let missing =
            || Err("`cargo fuzz` missing: run 'cargo install --locked cargo-fuzz'".to_string());
        let BuildMode::Skip(note) = build_mode(missing(), false) else {
            panic!("a lenient build must skip");
        };
        assert!(note.starts_with("skipped fuzz-build: "), "{note}");
        assert!(note.contains("cargo install --locked cargo-fuzz"), "{note}");
        let BuildMode::Fail(reason) = build_mode(missing(), true) else {
            panic!("a strict build must fail");
        };
        assert!(
            reason.contains("cargo install --locked cargo-fuzz"),
            "{reason}"
        );
        // Windows is reported through the same path.
        assert!(matches!(
            build_mode(Err(windows_message()), false),
            BuildMode::Skip(_)
        ));
        assert!(matches!(
            build_mode(Err(windows_message()), true),
            BuildMode::Fail(_)
        ));
    }

    #[test]
    fn running_all_continues_past_a_failure_and_names_every_failed_target() {
        let mut ran = Vec::new();
        let result = run_all(&["a", "b", "c", "d"], |target| {
            ran.push(target.to_string());
            if target == "b" || target == "d" {
                Err(format!("{target} crashed"))
            } else {
                Ok(())
            }
        });
        assert_eq!(ran, ["a", "b", "c", "d"]);
        let err = result.unwrap_err();
        assert!(err.contains('b') && err.contains('d'), "{err}");
        assert!(err.contains("2 of 4"), "{err}");
        assert_eq!(run_all(&["a"], |_| Ok(())), Ok(()));
    }

    #[test]
    fn the_pin_is_a_dated_nightly() {
        assert!(FUZZ_NIGHTLY.starts_with("nightly-20"), "{FUZZ_NIGHTLY}");
    }

    /// The names of the `fuzz/fuzz_targets/*.rs` files on disk, sorted.
    fn target_files_on_disk() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../fuzz/fuzz_targets");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        on_disk
    }

    // Until every target exists (M2.1 Step 5) only this direction can hold. The full `ci` also
    // runs ignored tests, so the test cannot simply be `#[ignore]`d in the meantime.
    #[test]
    fn every_target_file_on_disk_is_listed() {
        for file in target_files_on_disk() {
            assert!(TARGETS.contains(&file.as_str()), "{file} is not in TARGETS");
        }
    }
}
