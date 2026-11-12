//! Helpers for running cargo and external tool commands.

use std::process::{Command, Stdio};

/// Runs `program` with `args` and extra environment variables, streaming its output.
///
/// Returns `Err` with the failing command line if the process cannot be started
/// or exits unsuccessfully.
pub(crate) fn run(program: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let line = format!("{program} {}", args.join(" "));
    println!("$ {line}");
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .status()
        .map_err(|e| format!("failed to start `{line}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed ({status}): {line}"))
    }
}

/// Checks that `binary` is installed, returning `install_hint` if it is not.
pub(crate) fn require_tool(binary: &str, install_hint: &str) -> Result<(), String> {
    let found = Command::new(binary)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if found {
        Ok(())
    } else {
        Err(format!("{binary} missing: run '{install_hint}'"))
    }
}

/// Checks that `cargo <subcommand>` works, returning `install_hint` if it does not.
///
/// Some cargo plugins (cargo-hack, for one) only understand being run as `cargo <name>`, so
/// probing `cargo-<name> --version` directly would wrongly report them missing.
pub(crate) fn require_cargo_subcommand(subcommand: &str, install_hint: &str) -> Result<(), String> {
    let found = Command::new("cargo")
        .args([subcommand, "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if found {
        Ok(())
    } else {
        Err(format!(
            "`cargo {subcommand}` missing: run '{install_hint}'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cargo_subcommand_that_exists_is_found() {
        // `cargo fmt` ships with the pinned toolchain (rust-toolchain.toml lists rustfmt).
        assert_eq!(require_cargo_subcommand("fmt", "unused hint"), Ok(()));
    }

    #[test]
    fn a_missing_cargo_subcommand_returns_the_install_hint() {
        let err = require_cargo_subcommand("definitely-not-installed", "cargo install thing")
            .unwrap_err();
        assert!(err.contains("cargo install thing"), "{err}");
        assert!(err.contains("definitely-not-installed"), "{err}");
    }
}
