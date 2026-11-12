#![forbid(unsafe_code)]
// developer tool, not simulation code
#![allow(
    clippy::print_stdout,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

//! `cargo xtask <command>`: the single entry point that CI and humans both run.

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
        "ci" => only(&["--quick"]).map(|()| Command::Ci {
            quick: has("--quick"),
        }),
        other => Err(format!("unknown command `{other}`")),
    }
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
        } => {
            require_tool("cargo-nextest", "cargo install --locked cargo-nextest")?;
            let mut args = vec!["nextest", "run", "--workspace", "--all-features"];
            if *release {
                args.push("--release");
            }
            if *include_ignored {
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
        Command::Doc => run(
            "cargo",
            &["doc", "--workspace", "--no-deps", "--all-features"],
            &[("RUSTDOCFLAGS", "-D warnings")],
        ),
        Command::Deny => {
            require_tool("cargo-deny", "cargo install --locked cargo-deny")?;
            run("cargo", &["deny", "check"], &[])
        }
        Command::Ci { quick } => {
            // `quick` is the only mode in this task; later tasks extend the full run.
            let _ = quick;
            execute(&Command::Fmt { check: true })?;
            execute(&Command::Clippy)?;
            execute(&Command::Test {
                release: false,
                include_ignored: false,
            })
        }
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
        assert!(
            parse(&["nope"])
                .unwrap_err()
                .contains("unknown command `nope`")
        );
    }
}
