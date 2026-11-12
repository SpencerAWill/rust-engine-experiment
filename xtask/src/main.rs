#![forbid(unsafe_code)]
// developer tool, not simulation code
#![allow(
    clippy::print_stdout,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

//! `cargo xtask <command>`: the single entry point that CI and humans both run.

mod check_statics;
mod check_unsafe;
mod cmd;

use std::process::ExitCode;

use cmd::{require_tool, run};

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
    NoStd,
    Wasm,
    BenchBuild,
    Bench {
        save: Option<String>,
        compare: Option<(String, String)>,
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
        "no-std" => only(&[]).map(|()| Command::NoStd),
        "wasm" => only(&[]).map(|()| Command::Wasm),
        "bench-build" => only(&[]).map(|()| Command::BenchBuild),
        "bench" => parse_bench(flags),
        "ci" => only(&["--quick"]).map(|()| Command::Ci {
            quick: has("--quick"),
        }),
        other => Err(format!("unknown command `{other}`")),
    }
}

/// Parses `bench [--save <name>] [--compare <a> <b>]`.
fn parse_bench(flags: &[&str]) -> Result<Command, String> {
    let (mut save, mut compare) = (None, None);
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
            other => return Err(format!("unknown flag `{other}` for `bench`")),
        }
    }
    Ok(Command::Bench { save, compare })
}

/// Keeps only the crates that exist yet, reporting the rest as skipped.
fn existing<'a>(names: &[&'a str]) -> Vec<&'a str> {
    let mut found = Vec::new();
    for name in names {
        if std::path::Path::new("libs").join(name).is_dir() {
            found.push(*name);
        } else {
            println!("skipping {name}: crate does not exist yet");
        }
    }
    found
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
    // `cargo test --doc` errors when the workspace has no library target yet.
    if std::path::Path::new("libs").is_dir() {
        run(
            "cargo",
            &["test", "--doc", "--workspace", "--all-features"],
            &[],
        )?;
    } else {
        println!("skipping doctests: no library crates yet");
    }
    Ok(())
}

fn no_std() -> Result<(), String> {
    let crates = existing(&["engine_handle", "engine_random", "engine_time"]);
    if crates.is_empty() {
        return Ok(());
    }
    let mut args = vec!["build"];
    for c in &crates {
        args.extend(["-p", c]);
    }
    args.extend(["--no-default-features", "--target", "thumbv7em-none-eabihf"]);
    run("cargo", &args, &[])
}

fn wasm() -> Result<(), String> {
    if !existing(&["engine"]).is_empty() {
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
    }
    let crates = existing(&[
        "engine_handle",
        "engine_random",
        "engine_time",
        "engine_math",
    ]);
    if crates.is_empty() {
        return Ok(());
    }
    require_tool("wasmtime", "cargo install --locked wasmtime-cli")?;
    let mut args = vec!["test", "--target", "wasm32-wasip1", "--lib", "--tests"];
    for c in &crates {
        args.extend(["-p", c]);
    }
    run(
        "cargo",
        &args,
        &[("CARGO_TARGET_WASM32_WASIP1_RUNNER", "wasmtime")],
    )
}

fn bench(save: Option<&str>, compare: Option<&(String, String)>) -> Result<(), String> {
    if let Some((a, b)) = compare {
        require_tool("critcmp", "cargo install --locked critcmp")?;
        return run("critcmp", &[a, b], &[]);
    }
    let mut args = vec!["bench", "--workspace"];
    if let Some(name) = save {
        args.extend(["--", "--save-baseline", name]);
    }
    run("cargo", &args, &[])
}

fn ci(quick: bool) -> Result<(), String> {
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
            Command::Doc,
            Command::NoStd,
            Command::Wasm,
            Command::CheckUnsafe,
            Command::CheckStatics,
            Command::Deny,
            Command::BenchBuild,
        ]);
    }
    steps.iter().try_for_each(execute)
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
        Command::NoStd => no_std(),
        Command::Wasm => wasm(),
        Command::BenchBuild => run("cargo", &["bench", "--workspace", "--no-run"], &[]),
        Command::Bench { save, compare } => bench(save.as_deref(), compare.as_ref()),
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
                compare: Some(("a".into(), "b".into()))
            })
        );
        assert!(
            parse(&["nope"])
                .unwrap_err()
                .contains("unknown command `nope`")
        );
    }
}
