#![forbid(unsafe_code)]
// developer tool, not simulation code
#![allow(
    clippy::print_stdout,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

//! `cargo xtask <command>`: the single entry point that CI and humans both run.

mod bench_doc;
mod bench_report;
mod check_statics;
mod check_unsafe;
mod check_workflows;
mod cmd;
mod prng_stats;

use std::process::ExitCode;

use cmd::{battery_guard, caffeinate_args, require_cargo_subcommand, require_tool, run};

/// A parsed `cargo xtask` subcommand.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Fmt {
        check: bool,
    },
    Clippy,
    Test {
        release: bool,
        include_ignored: bool,
    },
    Doc,
    Deny,
    CheckUnsafe,
    CheckStatics,
    CheckWorkflows,
    NoStd,
    Wasm,
    PrngStats {
        seed: u64,
        bytes: Option<u64>,
    },
    BenchBuild,
    BenchSmoke,
    BenchDoc(bench_doc::Options),
    Hack,
    Typos,
    BenchReport(bench_report::Options),
    Bench {
        save: Option<String>,
        compare: Option<(String, String)>,
        allow_battery: bool,
    },
    Ci {
        quick: bool,
    },
}

/// Parses command-line arguments (without the program name).
fn parse(args: &[&str]) -> Result<Command, String> {
    let (name, flags) = args
        .split_first()
        .ok_or_else(|| "missing command (try: fmt, clippy, test, doc, deny, ci)".to_string())?;
    let has = |flag: &str| flags.contains(&flag);
    let only = |allowed: &[&str]| -> Result<(), String> {
        match flags.iter().find(|f| !allowed.contains(f)) {
            Some(bad) => Err(format!("unknown flag `{bad}` for `{name}`")),
            None => Ok(()),
        }
    };
    match *name {
        "fmt" => only(&["--check"]).map(|()| Command::Fmt {
            check: has("--check"),
        }),
        "clippy" => only(&[]).map(|()| Command::Clippy),
        "test" => only(&["--release", "--include-ignored"]).map(|()| Command::Test {
            release: has("--release"),
            include_ignored: has("--include-ignored"),
        }),
        "doc" => only(&[]).map(|()| Command::Doc),
        "deny" => only(&[]).map(|()| Command::Deny),
        "check-unsafe" => only(&[]).map(|()| Command::CheckUnsafe),
        "check-statics" => only(&[]).map(|()| Command::CheckStatics),
        "check-workflows" => only(&[]).map(|()| Command::CheckWorkflows),
        "prng-stats" => parse_prng_stats(flags),
        "no-std" => only(&[]).map(|()| Command::NoStd),
        "wasm" => only(&[]).map(|()| Command::Wasm),
        "bench-build" => only(&[]).map(|()| Command::BenchBuild),
        "bench-smoke" => only(&[]).map(|()| Command::BenchSmoke),
        "bench-doc" => bench_doc::parse_flags(flags).map(Command::BenchDoc),
        "hack" => only(&[]).map(|()| Command::Hack),
        "typos" => only(&[]).map(|()| Command::Typos),
        "bench" => parse_bench(flags),
        "bench-report" => bench_report::parse_flags(flags).map(Command::BenchReport),
        "ci" => only(&["--quick"]).map(|()| Command::Ci {
            quick: has("--quick"),
        }),
        other => Err(format!("unknown command `{other}`")),
    }
}

/// Parses `prng-stats [--seed <n>] [--bytes <n>]` (defaults: seed 0, unlimited output).
fn parse_prng_stats(flags: &[&str]) -> Result<Command, String> {
    let (mut seed, mut bytes) = (0_u64, None);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let mut number = |name: &str| -> Result<u64, String> {
            let value = it
                .next()
                .ok_or_else(|| format!("`{name}` needs a number"))?;
            value
                .parse()
                .map_err(|_| format!("`{name}` needs a number, got `{value}`"))
        };
        match *flag {
            "--seed" => seed = number("--seed")?,
            "--bytes" => bytes = Some(number("--bytes")?),
            other => return Err(format!("unknown flag `{other}` for `prng-stats`")),
        }
    }
    Ok(Command::PrngStats { seed, bytes })
}

/// Parses `bench [--save <name>] [--compare <a> <b>] [--allow-battery]`.
fn parse_bench(flags: &[&str]) -> Result<Command, String> {
    let (mut save, mut compare, mut allow_battery) = (None, None, false);
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        match *flag {
            "--save" => {
                let name = it.next().ok_or("`--save` needs a baseline name")?;
                save = Some((*name).to_string());
            }
            "--compare" => {
                let (Some(a), Some(b)) = (it.next(), it.next()) else {
                    return Err("`--compare` needs two baseline names".to_string());
                };
                compare = Some(((*a).to_string(), (*b).to_string()));
            }
            "--allow-battery" => allow_battery = true,
            other => return Err(format!("unknown flag `{other}` for `bench`")),
        }
    }
    if save.is_some() && compare.is_some() {
        return Err("`--save` and `--compare` cannot be combined: run them separately".to_string());
    }
    Ok(Command::Bench {
        save,
        compare,
        allow_battery,
    })
}

fn run_tests(release: bool, include_ignored: bool) -> Result<(), String> {
    require_tool("cargo-nextest", "cargo install --locked cargo-nextest")?;
    let mut args = vec!["nextest", "run", "--workspace", "--all-features"];
    if release {
        args.push("--release");
    }
    if include_ignored {
        args.extend(["--run-ignored", "all"]);
    }
    run("cargo", &args, &[])?;
    run(
        "cargo",
        &["test", "--doc", "--workspace", "--all-features"],
        &[],
    )
}

/// The `no_std` leaf crates, built without default features for a bare-metal target.
fn no_std() -> Result<(), String> {
    run(
        "cargo",
        &[
            "build",
            "-p",
            "engine_handle",
            "-p",
            "engine_random",
            "-p",
            "engine_time",
            "--no-default-features",
            "--target",
            "thumbv7em-none-eabihf",
        ],
        &[],
    )
}

/// Builds the facade for `wasm32-unknown-unknown` (which also proves `SystemClock` is excluded
/// there), then runs the leaf crates' and `engine_math`'s tests on `wasm32-wasip1` under wasmtime.
fn wasm() -> Result<(), String> {
    run(
        "cargo",
        &[
            "build",
            "-p",
            "engine",
            "--target",
            "wasm32-unknown-unknown",
        ],
        &[],
    )?;
    require_tool("wasmtime", "cargo install --locked wasmtime-cli")?;
    run(
        "cargo",
        &[
            "test",
            "--target",
            "wasm32-wasip1",
            "--lib",
            "--tests",
            "-p",
            "engine_handle",
            "-p",
            "engine_random",
            "-p",
            "engine_time",
            "-p",
            "engine_math",
        ],
        &[("CARGO_TARGET_WASM32_WASIP1_RUNNER", "wasmtime")],
    )
}

fn bench(
    save: Option<&str>,
    compare: Option<&(String, String)>,
    allow_battery: bool,
) -> Result<(), String> {
    if let Some((a, b)) = compare {
        require_tool("critcmp", "cargo install --locked critcmp")?;
        return run("critcmp", &[a, b], &[]);
    }
    let args = bench_args(save);
    if cfg!(target_os = "macos") {
        // A run takes an hour: battery power throttles the CPU, and an idle sleep mid-run spoils
        // the results (it did once). Refuse battery, and keep the machine awake.
        let power = std::process::Command::new("pmset")
            .args(["-g", "batt"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        battery_guard(&power, allow_battery)?;
        let wrapped = caffeinate_args(&args);
        let wrapped: Vec<&str> = wrapped.iter().map(String::as_str).collect();
        return run("caffeinate", &wrapped, &[]);
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    run("cargo", &args, &[])
}

/// The `cargo bench` command line for `bench [--save <name>]`.
///
/// `--bench '*'` restricts it to the `[[bench]]` targets. Without it, flags after `--` (such as
/// `--save-baseline`) are also passed to the unit-test harness of every library crate, which does
/// not understand them and fails.
fn bench_args(save: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = ["bench", "--workspace", "--bench", "*"]
        .map(String::from)
        .into();
    if let Some(name) = save {
        args.extend(["--", "--save-baseline", name].map(String::from));
    }
    args
}

/// The `cargo hack` command line: check every package with each of its features alone, and with
/// none. `--no-dev-deps` keeps `engine_handle`'s self dev-dependency (which turns on
/// `test-invariants`) from hiding a feature-gated build break.
fn hack_args() -> Vec<String> {
    [
        "hack",
        "check",
        "--workspace",
        "--each-feature",
        "--no-dev-deps",
    ]
    .map(String::from)
    .into()
}

/// The `cargo bench` command line for `bench-smoke`: run every benchmark once, in criterion's test
/// mode, so a benchmark that panics or rots fails CI. It measures nothing.
///
/// As for [`bench_args`], `--bench '*'` keeps the flag away from the unit-test harnesses.
fn bench_smoke_args() -> Vec<String> {
    ["bench", "--workspace", "--bench", "*", "--", "--test"]
        .map(String::from)
        .into()
}

/// The steps of `ci`, in order. The full run mirrors the GitHub workflow, including the release
/// pass over ignored (statistical) tests.
fn ci_steps(quick: bool) -> Vec<Command> {
    let mut steps = vec![
        Command::Fmt { check: true },
        Command::Clippy,
        Command::Test {
            release: false,
            include_ignored: false,
        },
    ];
    if !quick {
        steps.extend([
            Command::Test {
                release: true,
                include_ignored: true,
            },
            Command::Doc,
            Command::NoStd,
            Command::Wasm,
            Command::CheckUnsafe,
            Command::CheckStatics,
            Command::CheckWorkflows,
            Command::Hack,
            Command::Typos,
            Command::Deny,
            Command::BenchBuild,
            Command::BenchSmoke,
        ]);
    }
    steps
}

fn ci(quick: bool) -> Result<(), String> {
    ci_steps(quick).iter().try_for_each(execute)
}

fn execute(command: &Command) -> Result<(), String> {
    match command {
        Command::Fmt { check } => {
            let mut args = vec!["fmt", "--all"];
            if *check {
                args.extend(["--", "--check"]);
            }
            run("cargo", &args, &[])
        }
        Command::Clippy => run(
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
            &[],
        ),
        Command::Test {
            release,
            include_ignored,
        } => run_tests(*release, *include_ignored),
        Command::Doc => run(
            "cargo",
            &["doc", "--workspace", "--no-deps", "--all-features"],
            &[("RUSTDOCFLAGS", "-D warnings")],
        ),
        Command::Deny => {
            require_tool("cargo-deny", "cargo install --locked cargo-deny")?;
            run("cargo", &["deny", "check"], &[])
        }
        Command::CheckUnsafe => check_unsafe::run(),
        Command::CheckStatics => check_statics::run(),
        Command::CheckWorkflows => check_workflows::run(),
        Command::NoStd => no_std(),
        Command::PrngStats { seed, bytes } => prng_stats::run(*seed, *bytes),
        Command::Wasm => wasm(),
        Command::BenchBuild => run("cargo", &["bench", "--workspace", "--no-run"], &[]),
        Command::BenchDoc(options) => bench_doc::run(options),
        Command::Hack => {
            require_cargo_subcommand("hack", "cargo install --locked cargo-hack")?;
            run(
                "cargo",
                &hack_args().iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
            )
        }
        Command::Typos => {
            require_tool("typos", "cargo install --locked typos-cli")?;
            run("typos", &[], &[])
        }
        Command::BenchSmoke => run(
            "cargo",
            &bench_smoke_args()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            &[],
        ),
        Command::Bench {
            save,
            compare,
            allow_battery,
        } => bench(save.as_deref(), compare.as_ref(), *allow_battery),
        Command::BenchReport(options) => bench_report::run(options),
        Command::Ci { quick } => ci(*quick),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match parse(&args).and_then(|c| execute(&c)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("xtask: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_commands() {
        assert_eq!(parse(&["fmt", "--check"]), Ok(Command::Fmt { check: true }));
        assert_eq!(parse(&["ci", "--quick"]), Ok(Command::Ci { quick: true }));
        assert_eq!(
            parse(&["test", "--release", "--include-ignored"]),
            Ok(Command::Test {
                release: true,
                include_ignored: true
            })
        );
        assert_eq!(
            parse(&["bench", "--compare", "a", "b"]),
            Ok(Command::Bench {
                save: None,
                compare: Some(("a".into(), "b".into())),
                allow_battery: false,
            })
        );
        assert!(
            parse(&["nope"])
                .unwrap_err()
                .contains("unknown command `nope`")
        );
    }

    #[test]
    fn parses_prng_stats() {
        assert_eq!(
            parse(&["prng-stats"]),
            Ok(Command::PrngStats {
                seed: 0,
                bytes: None
            })
        );
        assert_eq!(
            parse(&["prng-stats", "--seed", "7", "--bytes", "16"]),
            Ok(Command::PrngStats {
                seed: 7,
                bytes: Some(16)
            })
        );
        assert!(parse(&["prng-stats", "--bytes"]).is_err());
        assert!(parse(&["prng-stats", "--seed", "x"]).is_err());
    }

    #[test]
    fn check_workflows_parses_and_runs_in_the_full_ci_only() {
        assert_eq!(parse(&["check-workflows"]), Ok(Command::CheckWorkflows));
        assert!(parse(&["check-workflows", "--x"]).is_err());
        assert!(ci_steps(false).contains(&Command::CheckWorkflows));
        assert!(!ci_steps(true).contains(&Command::CheckWorkflows));
    }

    #[test]
    fn bench_accepts_allow_battery_and_defaults_to_refusing() {
        assert_eq!(
            parse(&["bench", "--save", "x"]),
            Ok(Command::Bench {
                save: Some("x".into()),
                compare: None,
                allow_battery: false,
            })
        );
        assert_eq!(
            parse(&["bench", "--allow-battery"]),
            Ok(Command::Bench {
                save: None,
                compare: None,
                allow_battery: true,
            })
        );
    }

    #[test]
    fn bench_rejects_save_combined_with_compare() {
        assert!(parse(&["bench", "--save", "m1", "--compare", "a", "b"]).is_err());
        assert!(parse(&["bench", "--compare", "a", "b", "--save", "m1"]).is_err());
    }

    #[test]
    fn full_ci_runs_the_release_ignored_pass_that_github_runs() {
        let release_ignored = Command::Test {
            release: true,
            include_ignored: true,
        };
        assert!(ci_steps(false).contains(&release_ignored));
        assert!(!ci_steps(true).contains(&release_ignored));
    }

    #[test]
    fn bench_smoke_runs_each_benchmark_once_in_test_mode() {
        assert_eq!(parse(&["bench-smoke"]), Ok(Command::BenchSmoke));
        assert!(parse(&["bench-smoke", "--x"]).is_err());
        assert!(ci_steps(false).contains(&Command::BenchSmoke));
        assert!(!ci_steps(true).contains(&Command::BenchSmoke));
        // Criterion's `--test` flag must reach only the `[[bench]]` targets, like `--save-baseline`.
        let args = bench_smoke_args();
        let separator = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(&args[..separator], ["bench", "--workspace", "--bench", "*"]);
        assert_eq!(&args[separator..], ["--", "--test"]);
    }

    #[test]
    fn bench_doc_parses_its_flags_and_is_not_part_of_ci() {
        assert_eq!(
            parse(&["bench-doc", "--baseline", "m2"]),
            Ok(Command::BenchDoc(bench_doc::Options {
                baseline: "m2".into(),
                env_files: Vec::new(),
                doc: "docs/benchmarks.md".into(),
            }))
        );
        assert!(parse(&["bench-doc", "--x"]).is_err());
    }

    #[test]
    fn hack_checks_each_feature_without_dev_dependencies() {
        assert_eq!(parse(&["hack"]), Ok(Command::Hack));
        assert!(parse(&["hack", "--x"]).is_err());
        assert!(ci_steps(false).contains(&Command::Hack));
        assert!(!ci_steps(true).contains(&Command::Hack));
        // `--no-dev-deps` matters: engine_handle's self dev-dependency would otherwise turn on
        // `test-invariants` everywhere and hide feature-gated breakage.
        assert_eq!(
            hack_args(),
            [
                "hack",
                "check",
                "--workspace",
                "--each-feature",
                "--no-dev-deps"
            ]
        );
    }

    #[test]
    fn typos_parses_and_runs_in_the_full_ci_only() {
        assert_eq!(parse(&["typos"]), Ok(Command::Typos));
        assert!(parse(&["typos", "--x"]).is_err());
        assert!(ci_steps(false).contains(&Command::Typos));
        assert!(!ci_steps(true).contains(&Command::Typos));
    }

    #[test]
    fn bench_args_select_only_bench_targets_before_passing_flags_through() {
        // Without `--bench '*'`, criterion flags such as `--save-baseline` also reach the unit-test
        // harness of every library crate, which rejects them.
        let args = bench_args(Some("m1"));
        let separator = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(&args[..separator], ["bench", "--workspace", "--bench", "*"]);
        assert_eq!(&args[separator..], ["--", "--save-baseline", "m1"]);
        assert_eq!(bench_args(None), ["bench", "--workspace", "--bench", "*"]);
    }
}
