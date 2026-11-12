//! Helpers for running cargo and external tool commands.

use std::process::{Command, Stdio};

/// Runs `program` with `args` and extra environment variables, streaming its output.
///
/// Returns `Err` with the failing command line if the process cannot be started
/// or exits unsuccessfully.
pub(crate) fn run(program: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    run_with(None, program, args, env)
}

/// Like [`run`], but starts the process in the directory `dir` (relative to the current one).
pub(crate) fn run_in(
    dir: &str,
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<(), String> {
    run_with(Some(dir), program, args, env)
}

fn run_with(
    dir: Option<&str>,
    program: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<(), String> {
    let line = format!("{program} {}", args.join(" "));
    match dir {
        Some(dir) => println!("$ (in {dir}) {line}"),
        None => println!("$ {line}"),
    }
    let mut command = Command::new(program);
    command.args(args).envs(env.iter().copied());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let status = command
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

/// Whether `pmset -g batt` output says the machine is running on battery power. Only the first
/// line counts (`Now drawing from 'Battery Power'`): on AC power the second line still names the
/// battery. Output that cannot be read is not treated as battery.
pub(crate) fn on_battery(pmset_output: &str) -> bool {
    pmset_output
        .lines()
        .next()
        .is_some_and(|line| line.contains("Battery Power"))
}

/// Refuses to start a measurement on battery power, which throttles the CPU and can sleep the
/// machine mid-run, unless `allow_battery` is set.
pub(crate) fn battery_guard(pmset_output: &str, allow_battery: bool) -> Result<(), String> {
    if on_battery(pmset_output) && !allow_battery {
        return Err(
            "refusing to benchmark on battery power: results would be unreliable (plug in, or pass `--allow-battery`)"
                .to_string(),
        );
    }
    Ok(())
}

/// The `caffeinate` command line that runs `cargo` with `cargo_args` while preventing idle sleep.
pub(crate) fn caffeinate_args(cargo_args: &[String]) -> Vec<String> {
    ["-i", "cargo"]
        .map(String::from)
        .into_iter()
        .chain(cargo_args.iter().cloned())
        .collect()
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

    const AC: &str = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged; 0:00 remaining present: true\n";
    const BATTERY: &str = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t90%; discharging; 5:00 remaining present: true\n";

    #[test]
    fn battery_power_is_recognised_from_the_first_line_of_pmset_output() {
        assert!(on_battery(BATTERY));
        assert!(!on_battery(AC));
        // The second line mentions the battery on AC power too; only the first line counts.
        assert!(!on_battery(
            "Now drawing from 'AC Power'\n -InternalBattery-0 Battery Power\n"
        ));
        // Output we cannot read is not treated as battery (a desktop has none).
        assert!(!on_battery(""));
        assert!(!on_battery("pmset: command not found"));
    }

    #[test]
    fn the_guard_refuses_battery_unless_allowed() {
        assert_eq!(battery_guard(AC, false), Ok(()));
        assert_eq!(battery_guard(BATTERY, true), Ok(()));
        let err = battery_guard(BATTERY, false).unwrap_err();
        assert!(err.contains("battery power"), "{err}");
        assert!(err.contains("--allow-battery"), "{err}");
    }

    #[test]
    fn caffeinate_wraps_the_cargo_command_line() {
        let cargo_args = ["bench".to_string(), "--workspace".to_string()];
        assert_eq!(
            caffeinate_args(&cargo_args),
            ["-i", "cargo", "bench", "--workspace"]
        );
    }

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
