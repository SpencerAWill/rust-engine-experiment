//! `cargo xtask mutants`: mutation testing through `cargo-mutants`.
//!
//! The command checks the exclusion list in `.cargo/mutants.toml` ([`excludes`]), runs
//! `cargo mutants` on the triaged crates, and prints a per-crate summary ([`summary`]). It reports;
//! a missed mutant is not a failure. It fails only when no report could be produced.

use std::io::ErrorKind;
use std::num::NonZeroU32;
use std::path::Path;
use std::process::Command;

use crate::cmd::require_cargo_subcommand;

mod excludes;
mod summary;

/// The `cargo-mutants` version the results are comparable across. The workflow and the
/// devcontainer script repeat it, and a test fails when they differ.
pub(crate) const MUTANTS_VERSION: &str = "27.1.0";

/// The crates whose mutants have been triaged (see `docs/mutation-triage.md`). A later chunk that
/// triages another crate adds it here.
const TRIAGED: &[&str] = &["engine_collections", "engine_handle", "engine_random"];

/// Where `cargo-mutants` writes its output, and the summary beside it.
const OUT_DIR: &str = "target/mutants";

/// The configuration file, relative to the workspace root.
const CONFIG: &str = ".cargo/mutants.toml";

/// Options of `mutants [-p PACKAGE]... [-j N] [--list] [--in-diff FILE]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Options {
    /// Packages to test; empty means the triaged set.
    packages: Vec<String>,
    /// How many mutants to test at once; `None` leaves the tool's default (one).
    jobs: Option<NonZeroU32>,
    /// Check the exclusions and list the mutants instead of running them.
    list: bool,
    /// Test only the mutants in code that a unified diff file changes (`cargo mutants --in-diff`).
    in_diff: Option<String>,
}

/// Parses the arguments that follow `mutants`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let mut options = Options {
        packages: Vec::new(),
        jobs: None,
        list: false,
        in_diff: None,
    };
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        match *flag {
            "-p" => {
                let package = it.next().ok_or("`-p` needs a package name")?;
                options.packages.push((*package).to_string());
            }
            "-j" => {
                let value = it.next().ok_or("`-j` needs a number of jobs")?;
                if options.jobs.is_some() {
                    return Err("`-j` given more than once".to_string());
                }
                options.jobs =
                    Some(value.parse().map_err(|_| {
                        format!("`-j` needs a positive whole number, got `{value}`")
                    })?);
            }
            "--list" => options.list = true,
            "--in-diff" => {
                let file = it.next().ok_or("`--in-diff` needs a diff file")?;
                if options.in_diff.is_some() {
                    return Err("`--in-diff` given more than once".to_string());
                }
                options.in_diff = Some((*file).to_string());
            }
            other => return Err(format!("unknown flag `{other}` for `mutants`")),
        }
    }
    if options.list && options.in_diff.is_some() {
        return Err(
            "`--list` cannot be combined with `--in-diff`: the list is not filtered".to_string(),
        );
    }
    Ok(options)
}

/// `path` as an absolute path: a relative one is taken from `cwd`, the caller's directory. The
/// command runs `cargo mutants` from the workspace root, so a relative path would otherwise be
/// read from there.
fn absolute(path: &str, cwd: &Path) -> String {
    cwd.join(path).to_string_lossy().into_owned()
}

/// Removes the output of an earlier run. `cargo-mutants` writes no output at all when a diff
/// selects no mutant, and the summary must not then read an old run's files as this run's.
fn clear_previous_output(root: &Path) -> Result<(), String> {
    let dir = root.join(OUT_DIR).join("mutants.out");
    match std::fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(format!("{}: {e}", dir.display())),
        _ => Ok(()),
    }
}

/// The packages to test: those named, or else the triaged set.
fn packages(options: &Options) -> Vec<String> {
    if options.packages.is_empty() {
        TRIAGED.iter().map(|p| (*p).to_string()).collect()
    } else {
        options.packages.clone()
    }
}

fn package_args(packages: &[String]) -> impl Iterator<Item = String> + '_ {
    packages.iter().flat_map(|p| ["-p".to_string(), p.clone()])
}

/// The `cargo mutants` command line of a run. It passes no name filter (`-F`, `-E`, `-f`), so the
/// exclusions in the configuration apply unchanged; `--in-diff` only narrows the run to changed code.
fn run_args(options: &Options) -> Vec<String> {
    let mut args: Vec<String> = ["mutants", "--output", OUT_DIR].map(String::from).into();
    args.extend(package_args(&packages(options)));
    if let Some(jobs) = options.jobs {
        args.extend(["--jobs".to_string(), jobs.to_string()]);
    }
    if let Some(file) = &options.in_diff {
        args.extend(["--in-diff".to_string(), file.clone()]);
    }
    args
}

/// The command line that lists the mutants of `packages`, with or without the configuration (and
/// so with or without its exclusions).
fn list_args(packages: &[String], use_config: bool) -> Vec<String> {
    let mut args: Vec<String> = ["mutants", "--list"].map(String::from).into();
    args.extend(package_args(packages));
    if !use_config {
        args.push("--no-config".to_string());
    }
    args
}

/// The command line that lists every mutant in the workspace, ignoring the configuration.
fn workspace_args() -> Vec<String> {
    ["mutants", "--list", "--no-config", "--workspace"]
        .map(String::from)
        .into()
}

/// [`workspace_args`] with the mutants whose name matches `regex` removed. The difference in
/// length is the number of mutants an exclusion entry removes. This uses `-E` rather than `-F`
/// because `cargo-mutants` 27.1 never filters "delete field" mutants by name: `-F` lists them
/// whatever the regex, so a count of `-F` matches could never be zero.
fn match_args(regex: &str) -> Vec<String> {
    let mut args = workspace_args();
    args.extend(["-E".to_string(), regex.to_string()]);
    args
}

/// Maps the exit status of `cargo mutants`. Codes 0, 2 and 3 mean a report exists: all caught,
/// some missed, some timed out. `None` means the process was killed by a signal.
fn interpret(code: Option<i32>) -> Result<(), String> {
    let meaning = match code {
        Some(0 | 2 | 3) => return Ok(()),
        Some(1) => "usage error".to_string(),
        Some(4) => "the unmutated tests fail or hang (baseline failure)".to_string(),
        Some(5 | 6) => "a problem with the `--in-diff` file".to_string(),
        Some(70) => "internal error in cargo-mutants".to_string(),
        Some(_) => "unexpected exit code".to_string(),
        None => return Err("cargo mutants was stopped by a signal".to_string()),
    };
    Err(format!(
        "cargo mutants exited with code {}: {meaning}",
        code.unwrap_or_default()
    ))
}

/// Whether `cargo mutants --version` output names the pinned version.
fn version_matches(version_output: &str) -> bool {
    version_output.trim() == format!("cargo-mutants {MUTANTS_VERSION}")
}

/// The number of mutant names in the output of `cargo mutants --list`.
fn count_names(stdout: &str) -> usize {
    stdout.lines().filter(|l| !l.trim().is_empty()).count()
}

/// Pairs each entry with the number of mutants it matched. Returns the lines that report the
/// counts, and an error for each entry that matched none.
fn check_entries(entries: &[excludes::Entry], counts: &[usize]) -> (Vec<String>, Vec<String>) {
    let mut report = Vec::new();
    let mut errors = Vec::new();
    for (entry, count) in entries.iter().zip(counts) {
        report.push(format!("{count} mutants  {}", entry.regex));
        if *count == 0 {
            errors.push(format!(
                "{CONFIG}:{}: matches no mutant (stale entry?): {}",
                entry.line, entry.regex
            ));
        }
    }
    (report, errors)
}

/// Runs `cargo` with `args` in `root` and returns its standard output.
fn cargo_output(root: &Path, args: &[String]) -> Result<String, String> {
    let line = format!("cargo {}", args.join(" "));
    let output = Command::new("cargo")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to start `{line}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "command failed ({}): {line}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads one outcome file of the last run; a missing file is an error naming it.
fn read_outcome(root: &Path, name: &str) -> Result<String, String> {
    let path = root.join(OUT_DIR).join("mutants.out").join(name);
    std::fs::read_to_string(&path).map_err(|e| {
        if e.kind() == ErrorKind::NotFound {
            format!("missing {OUT_DIR}/mutants.out/{name}")
        } else {
            format!("{}: {e}", path.display())
        }
    })
}

/// Checks the exclusion list, runs `cargo mutants`, and prints and writes the summary.
pub(crate) fn mutants(options: &Options) -> Result<(), String> {
    let hint = format!("cargo install --locked cargo-mutants --version {MUTANTS_VERSION}");
    require_cargo_subcommand("mutants", &hint)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("cannot find the workspace root")?;

    let version_args = ["mutants".to_string(), "--version".to_string()];
    let version = cargo_output(root, &version_args)?;
    if !version_matches(&version) {
        eprintln!(
            "warning: found `{}`, but results are only comparable for `cargo-mutants {MUTANTS_VERSION}` (run '{hint}')",
            version.trim()
        );
    }

    let text = std::fs::read_to_string(root.join(CONFIG)).map_err(|e| format!("{CONFIG}: {e}"))?;
    let entries = excludes::parse(&text).map_err(|errors| errors.join("\n"))?;
    let everything = count_names(&cargo_output(root, &workspace_args())?);
    let mut counts = Vec::new();
    for entry in &entries {
        let left = count_names(&cargo_output(root, &match_args(&entry.regex))?);
        counts.push(everything.saturating_sub(left));
    }
    let (report, errors) = check_entries(&entries, &counts);
    for line in &report {
        println!("{line}");
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let selected = packages(options);
    let configured = cargo_output(root, &list_args(&selected, true))?;
    let excluded = count_names(&cargo_output(root, &list_args(&selected, false))?)
        .saturating_sub(count_names(&configured));
    if options.list {
        print!("{configured}");
        return Ok(());
    }

    let mut options = options.clone();
    if let Some(file) = &options.in_diff {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("cannot read the current directory: {e}"))?;
        options.in_diff = Some(absolute(file, &cwd));
    }
    clear_previous_output(root)?;
    let args = run_args(&options);
    println!("$ cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(&args)
        .current_dir(root)
        .status()
        .map_err(|e| format!("failed to start `cargo {}`: {e}", args.join(" ")))?;
    interpret(status.code())?;

    let header = summary::Header {
        version: version.trim(),
        packages: &selected,
        excluded_mutants: excluded,
        exclude_entries: entries.len(),
    };
    let out_dir = root.join(OUT_DIR);
    // A diff with no mutants in it leaves `cargo-mutants` nothing to run, and it writes no output.
    let table = if options.in_diff.is_some() && !out_dir.join("mutants.out").exists() {
        summary::render_no_mutants(&header)
    } else {
        render_outcomes(root, &header)?
    };
    let path = out_dir.join("summary.md");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    std::fs::write(&path, &table).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("\n{table}");
    Ok(())
}

/// Reads the four outcome files of the last run and renders them.
fn render_outcomes(root: &Path, header: &summary::Header<'_>) -> Result<String, String> {
    let (caught, missed) = (
        read_outcome(root, "caught.txt")?,
        read_outcome(root, "missed.txt")?,
    );
    let (timeout, unviable) = (
        read_outcome(root, "timeout.txt")?,
        read_outcome(root, "unviable.txt")?,
    );
    summary::render(
        header,
        &summary::Outcomes {
            caught: &caught,
            missed: &missed,
            timeout: &timeout,
            unviable: &unviable,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use excludes::Entry;

    fn opts(packages: &[&str], jobs: Option<u32>, list: bool) -> Options {
        Options {
            packages: packages.iter().map(|p| (*p).to_string()).collect(),
            jobs: jobs.and_then(NonZeroU32::new),
            list,
            in_diff: None,
        }
    }

    #[test]
    fn in_diff_is_passed_through_once() {
        let parsed = parse_flags(&["--in-diff", "target/pr.diff", "-j", "2"]).unwrap();
        assert_eq!(parsed.in_diff.as_deref(), Some("target/pr.diff"));
        assert!(parse_flags(&["--in-diff"]).is_err());
        assert!(parse_flags(&["--in-diff", "a", "--in-diff", "b"]).is_err());
        let args = run_args(&parsed);
        assert_eq!(
            &args[args.len() - 4..],
            ["--jobs", "2", "--in-diff", "target/pr.diff"]
        );
        assert!(!run_args(&opts(&[], None, false)).contains(&"--in-diff".to_string()));
    }

    #[test]
    fn parses_packages_jobs_and_list() {
        assert_eq!(
            parse_flags(&["-p", "a", "-p", "b", "-j", "2", "--list"]),
            Ok(opts(&["a", "b"], Some(2), true))
        );
        assert_eq!(parse_flags(&[]), Ok(opts(&[], None, false)));
        for bad in [
            &["-p"][..],
            &["-j"],
            &["-j", "0"],
            &["-j", "x"],
            &["-j", "-1"],
            &["-j", "2", "-j", "3"],
            &["--x"],
        ] {
            assert!(parse_flags(bad).is_err(), "{bad:?}");
        }
        let err = parse_flags(&["--x"]).unwrap_err();
        assert!(err.contains("unknown flag `--x` for `mutants`"), "{err}");
    }

    #[test]
    fn packages_default_to_the_triaged_set() {
        assert_eq!(
            packages(&opts(&[], None, false)),
            ["engine_collections", "engine_handle", "engine_random"]
        );
        assert_eq!(
            packages(&opts(&["engine_time"], None, false)),
            ["engine_time"]
        );
    }

    #[test]
    fn run_args_match_the_spec() {
        assert_eq!(
            run_args(&opts(&[], None, false)),
            [
                "mutants",
                "--output",
                "target/mutants",
                "-p",
                "engine_collections",
                "-p",
                "engine_handle",
                "-p",
                "engine_random"
            ]
        );
        let with_jobs = run_args(&opts(&["engine_random"], Some(2), false));
        assert_eq!(&with_jobs[with_jobs.len() - 2..], ["--jobs", "2"]);
        assert!(!run_args(&opts(&[], None, false)).contains(&"--jobs".to_string()));
        for args in [run_args(&opts(&[], None, false)), with_jobs] {
            for filter in ["-F", "-E", "-f", "--re", "--exclude-re", "--file"] {
                assert!(!args.contains(&filter.to_string()), "{filter} in {args:?}");
            }
        }
    }

    #[test]
    fn list_args_with_and_without_config() {
        let pkgs = ["a".to_string(), "b".to_string()];
        assert_eq!(
            list_args(&pkgs, true),
            ["mutants", "--list", "-p", "a", "-p", "b"]
        );
        assert_eq!(
            list_args(&pkgs, false),
            ["mutants", "--list", "-p", "a", "-p", "b", "--no-config"]
        );
    }

    #[test]
    fn match_args_remove_the_entry_from_the_whole_workspace_without_config() {
        // `-E`, not `-F`: cargo-mutants 27.1 never filters "delete field" mutants by name, so `-F`
        // lists them whatever the regex says. The entry's matches are the ones `-E` removes.
        assert_eq!(
            match_args("^libs/a"),
            [
                "mutants",
                "--list",
                "--no-config",
                "--workspace",
                "-E",
                "^libs/a"
            ]
        );
        assert_eq!(
            workspace_args(),
            ["mutants", "--list", "--no-config", "--workspace"]
        );
    }

    #[test]
    fn interpret_maps_every_documented_code() {
        for ok in [0, 2, 3] {
            assert_eq!(interpret(Some(ok)), Ok(()), "{ok}");
        }
        for (code, meaning) in [
            (1, "usage"),
            (4, "baseline"),
            (5, "--in-diff"),
            (6, "--in-diff"),
            (70, "internal"),
            (9, "unexpected"),
        ] {
            let err = interpret(Some(code)).unwrap_err();
            assert!(err.contains(&code.to_string()), "{err}");
            assert!(err.contains(meaning), "{code}: {err}");
        }
        assert!(interpret(None).unwrap_err().contains("signal"));
    }

    #[test]
    fn version_matches_the_pin() {
        assert!(version_matches("cargo-mutants 27.1.0\n"));
        assert!(!version_matches("cargo-mutants 27.0.0\n"));
        assert!(!version_matches(""));
    }

    #[test]
    fn counts_ignore_blank_lines() {
        assert_eq!(count_names("a\n\nb\n  \n"), 2);
        assert_eq!(count_names(""), 0);
    }

    #[test]
    fn entry_match_counts_are_printed() {
        let entries = [
            Entry {
                line: 4,
                regex: "^libs/a/".into(),
            },
            Entry {
                line: 8,
                regex: "^libs/b/".into(),
            },
        ];
        let (report, errors) = check_entries(&entries, &[3, 0]);
        assert_eq!(report, ["3 mutants  ^libs/a/", "0 mutants  ^libs/b/"]);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with(".cargo/mutants.toml:8:"),
            "{errors:?}"
        );
        assert!(errors[0].contains("matches no mutant"), "{errors:?}");
    }

    #[test]
    fn mutants_version_is_pinned_everywhere() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let read = |path: &str| std::fs::read_to_string(root.join(path)).unwrap();
        let workflow = read(".github/workflows/mutants.yml");
        let install = format!("cargo-mutants@{MUTANTS_VERSION}");
        assert!(workflow.contains(&install), "mutants.yml lacks `{install}`");
        let script = read(".devcontainer/install-tools.sh");
        let install = format!("cargo-mutants --version {MUTANTS_VERSION}");
        assert!(
            script.contains(&install),
            "install-tools.sh lacks `{install}`"
        );
    }

    #[test]
    fn in_diff_cannot_be_combined_with_list() {
        let err = parse_flags(&["--list", "--in-diff", "a.diff"]).unwrap_err();
        assert!(err.contains("--list"), "{err}");
        assert!(parse_flags(&["--in-diff", "a.diff", "--list"]).is_err());
    }

    #[test]
    fn a_relative_diff_path_is_made_absolute_against_the_callers_directory() {
        let cwd = std::env::temp_dir().join("caller");
        assert_eq!(
            absolute("pr.diff", &cwd),
            cwd.join("pr.diff").to_string_lossy()
        );
        // An absolute path stays as it is, whatever the directory.
        let elsewhere = std::env::temp_dir().join("elsewhere").join("pr.diff");
        let elsewhere = elsewhere.to_string_lossy().into_owned();
        assert_eq!(absolute(&elsewhere, &cwd), elsewhere);
    }

    #[test]
    fn previous_output_is_removed_before_a_run() {
        let root = std::env::temp_dir().join(format!("xtask-mutants-test-{}", std::process::id()));
        let out = root.join(OUT_DIR).join("mutants.out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("caught.txt"), "stale\n").unwrap();
        clear_previous_output(&root).unwrap();
        assert!(!out.exists());
        // Nothing to remove is fine too.
        clear_previous_output(&root).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn mutants_version_is_in_the_contributing_install_line() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let text = std::fs::read_to_string(root.join("CONTRIBUTING.md")).unwrap();
        let install = format!("cargo-mutants --version {MUTANTS_VERSION}");
        assert!(text.contains(&install), "CONTRIBUTING.md lacks `{install}`");
    }
}
