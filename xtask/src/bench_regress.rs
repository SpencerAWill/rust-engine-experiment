//! `cargo xtask bench-regress`: compare the Gungraun instruction counts of a base commit with
//! the working tree, and `cargo xtask gungraun-version`, which prints the version CI and the
//! devcontainer install the runner at (spec M2.4 §4.4, §4.8).

mod cargo_json;
mod report;

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cargo_json::BenchExe;
use report::Verdict;

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

/// Where the regression package (`crate::REGRESS_PACKAGE`) lives, relative to the repository root.
/// `RELEVANT` lists it too.
const REGRESS_DIR: &str = "bench/regress";

/// Where the regression package lived before, newest first. A base commit from before a move has
/// it there (for example the `m2` tag of the milestone check in `docs/maintainers.md`); its
/// benchmarks still compare with the head's, because Gungraun keeps baselines under
/// `GUNGRAUN_HOME/<target>/<group>/<function>`, not under the package's directory (see
/// `output_dir` in `fixtures/`). #28 moved it from `bench-regress/`.
const LEGACY_REGRESS_DIRS: &[&str] = &["bench-regress"];

/// Every location of the regression package a base may have, in order of preference: the current
/// one, then the earlier ones.
fn regress_dirs() -> impl Iterator<Item = &'static str> {
    std::iter::once(REGRESS_DIR).chain(LEGACY_REGRESS_DIRS.iter().copied())
}

/// Where the base has the regression package: the first of `regress_dirs()` whose `Cargo.toml`
/// appears in `listed` (the output of `git ls-tree --name-only` for those manifests). `None` when
/// the base has none of them.
fn base_regress_dir(listed: &str) -> Option<&'static str> {
    regress_dirs().find(|dir| {
        listed
            .lines()
            .any(|line| line.strip_suffix("/Cargo.toml") == Some(*dir))
    })
}

/// What the head run can see changed: a change to any of these can change an instruction count
/// (spec §4.4 step 2). A pattern ending in `/**` matches everything below that directory; any
/// other pattern matches the path exactly.
const RELEVANT: &[&str] = &[
    "libs/**",
    "bench/regress/**",
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
        Some(dir) if !dir.is_empty() => repo.join(dir),
        _ => repo.join("target"),
    }
}

/// Why the base run does not happen (spec §4.4 step 4).
#[derive(Debug, PartialEq, Eq)]
enum BaseSkip {
    /// The base has the regression package at none of `regress_dirs()`: every benchmark is new.
    NoCrate,
    /// The base was built against another `gungraun`, whose binaries the installed runner cannot
    /// run: not compared.
    VersionChanged { base: String, head: String },
}

/// Decides whether the base runs, from where the base has the regression package
/// (`base_regress_dir`) and from the two `Cargo.lock` texts: the directory to run the base from,
/// or why the base run is skipped.
fn base_plan(
    base_dir: Option<&'static str>,
    base_lock: &str,
    head_lock: &str,
) -> Result<&'static str, BaseSkip> {
    let dir = base_dir.ok_or(BaseSkip::NoCrate)?;
    let base = lock_version(base_lock, "gungraun");
    let head = lock_version(head_lock, "gungraun");
    if base == head {
        Ok(dir)
    } else {
        Err(BaseSkip::VersionChanged {
            base: base.unwrap_or_else(|| "none".to_string()),
            head: head.unwrap_or_else(|| "none".to_string()),
        })
    }
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

/// The Gungraun arguments of the base run: save the counts as the baseline `base`, with the cache
/// simulation on (spec §4.4 step 4).
fn base_args() -> Vec<String> {
    [
        "--bench",
        "--save-baseline=base",
        "--callgrind-args=--cache-sim=yes",
    ]
    .map(String::from)
    .into()
}

/// The Gungraun arguments of the head run: compare with the baseline, fail a benchmark whose
/// instruction count rose by more than 5%, and write JSON summaries (spec §4.4 step 5).
/// `--regression-fail-fast` is left out on purpose, so that every regressed benchmark of a
/// target is reported.
fn head_args() -> Vec<String> {
    [
        "--bench",
        "--baseline=base",
        "--callgrind-limits=ir=5%",
        "--save-summary=json",
        "--callgrind-args=--cache-sim=yes",
    ]
    .map(String::from)
    .into()
}

/// The `cargo` command line that builds the bench executables and reports them as JSON.
fn build_args() -> Vec<String> {
    [
        "bench",
        "-p",
        crate::REGRESS_PACKAGE,
        "--no-run",
        "--message-format=json",
    ]
    .map(String::from)
    .into()
}

/// Whether any path of `git diff --name-only` output (one per line) can change a count.
fn any_relevant(name_only: &str) -> bool {
    name_only.lines().any(is_relevant)
}

/// The one-line outcome for the job summary.
fn verdict_line(verdict: &Verdict, report_only: bool, skip: Option<&BaseSkip>) -> String {
    let outcome = match verdict {
        Verdict::Passed => "passed".to_string(),
        Verdict::Regressed if report_only => "regression accepted by label".to_string(),
        Verdict::Regressed => "regressed".to_string(),
        Verdict::Failed(reason) => format!("failed: {reason}"),
    };
    match skip {
        None => outcome,
        Some(BaseSkip::NoCrate) => {
            format!(
                "{outcome} (the base has no {} package: every benchmark is new)",
                crate::REGRESS_PACKAGE
            )
        }
        Some(BaseSkip::VersionChanged { base, head }) => {
            format!("{outcome} (not compared: gungraun changed from {base} to {head})")
        }
    }
}

/// The output of `program args`, when it runs and succeeds.
fn tool_output(program: &str, args: &[&str], dir: &Path) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Checks the platform and tools (spec §4.4 step 1) and returns the `gungraun` version.
fn preflight(repo: &Path, head_lock: &str) -> Result<String, String> {
    if std::env::consts::OS != "linux" {
        return Err(format!(
            "bench-regress needs Linux and Valgrind, and this is {}: run it in the devcontainer \
             (see CONTRIBUTING.md)",
            std::env::consts::OS
        ));
    }
    let valgrind = tool_output("valgrind", &["--version"], repo)
        .and_then(|output| valgrind_version(&output))
        .ok_or("valgrind is missing: run 'sudo apt-get install valgrind'")?;
    if valgrind < MIN_VALGRIND {
        return Err(format!(
            "valgrind {}.{} is too old: Gungraun needs {}.{} or later",
            valgrind.0, valgrind.1, MIN_VALGRIND.0, MIN_VALGRIND.1
        ));
    }
    let wanted = lock_version(head_lock, "gungraun")
        .ok_or("Cargo.lock has no `gungraun` package; is `engine_bench_regress` still a member?")?;
    let install = format!("cargo install --locked gungraun-runner --version {wanted}");
    let runner = tool_output("gungraun-runner", &["--version"], repo)
        .and_then(|output| runner_version(&output))
        .ok_or_else(|| format!("gungraun-runner is missing: run '{install}'"))?;
    if runner != wanted {
        return Err(format!(
            "gungraun-runner {runner} does not match gungraun {wanted} in Cargo.lock: run '{install}'"
        ));
    }
    Ok(wanted)
}

/// A detached worktree of the base commit, removed when the guard goes out of scope, so that
/// every way out of the base run (including an error) cleans up.
struct Worktree {
    repo: PathBuf,
    path: PathBuf,
}

impl Worktree {
    fn add(repo: &Path, path: &Path, base: &str) -> Result<Self, String> {
        // A worktree left by a run that was killed would make `git worktree add` fail.
        remove_worktree(repo, path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let path_arg = path.to_string_lossy();
        crate::cmd::run_in(
            &repo.to_string_lossy(),
            "git",
            &["worktree", "add", "--detach", &path_arg, base],
            &[],
        )?;
        Ok(Self {
            repo: repo.to_path_buf(),
            path: path.to_path_buf(),
        })
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        remove_worktree(&self.repo, &self.path);
    }
}

/// Removes the worktree at `path` and forgets it; does nothing when there is none.
fn remove_worktree(repo: &Path, path: &Path) {
    let status = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(path)
        .current_dir(repo)
        .stderr(Stdio::null())
        .status();
    if status.is_ok_and(|s| s.success()) {
        return;
    }
    // Not registered (or already gone): delete a leftover directory and drop stale entries.
    let _ = fs::remove_dir_all(path);
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo)
        .status();
}

/// A command for the base run, started in `dir` (the base's worktree).
///
/// `cargo xtask` runs under rustup's proxy, which exports `RUSTUP_TOOLCHAIN` to its children. That
/// variable outranks the worktree's `rust-toolchain.toml`, so without removing it the base would
/// be built with the head's compiler and a toolchain bump could never show.
fn base_command(program: &str, dir: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(dir)
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove("RUSTUP_TOOLCHAIN_SOURCE");
    command
}

/// Builds the bench targets in `dir` and returns their executables. `target_dir` overrides
/// `CARGO_TARGET_DIR` when given.
fn build(dir: &Path, target_dir: Option<&Path>, is_base: bool) -> Result<Vec<BenchExe>, String> {
    let args = build_args();
    println!("$ (in {}) cargo {}", dir.display(), args.join(" "));
    let mut command = if is_base {
        base_command("cargo", dir)
    } else {
        let mut command = Command::new("cargo");
        command.current_dir(dir);
        command
    };
    command.args(&args).stderr(Stdio::inherit());
    if let Some(target_dir) = target_dir {
        command.env("CARGO_TARGET_DIR", target_dir);
    }
    let output = command
        .output()
        .map_err(|e| format!("failed to start cargo: {e}"))?;
    if !output.status.success() {
        return Err(format!("the build failed ({})", output.status));
    }
    cargo_json::bench_executables(&String::from_utf8_lossy(&output.stdout))
}

/// Runs every executable with `args` from `package_dir`, one after the other, and returns each
/// target's exit code. A target that fails does not stop the next one. A code of `None` is a
/// signal; a process that cannot be started counts as `Some(-1)`.
///
/// `cmd::run` is not used: it turns a non-zero exit into an error, and here 3 is a result.
fn run_targets(
    package_dir: &Path,
    exes: &[BenchExe],
    args: &[String],
    env: &[(&str, String)],
) -> Vec<(String, Option<i32>)> {
    exes.iter()
        .map(|exe| {
            println!(
                "$ (in {}) {} {}",
                package_dir.display(),
                exe.path.display(),
                args.join(" ")
            );
            let status = Command::new(&exe.path)
                .args(args)
                .current_dir(package_dir)
                .envs(env.iter().map(|(k, v)| (*k, v)))
                .status();
            let code = match status {
                Ok(status) => status.code(),
                Err(error) => {
                    eprintln!("failed to start {}: {error}", exe.path.display());
                    Some(-1)
                }
            };
            (exe.name.clone(), code)
        })
        .collect()
}

/// Every `summary.json` below `dir`, in a stable order.
fn summary_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().is_some_and(|name| name == "summary.json") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The output of a `git` command that has to succeed; the error names the step and carries git's
/// own message (for example "dubious ownership").
fn git(repo: &Path, args: &[&str], what: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("{what}: failed to start git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "{what} (`git {}` failed: {})",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Appends `text` to the file named by `$GITHUB_STEP_SUMMARY`, if set.
fn append_step_summary(text: &str) {
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let written = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
    if let Err(error) = written {
        eprintln!("warning: could not write the job summary: {error}");
    }
}

/// Empties and recreates `GUNGRAUN_HOME`, so that a baseline left by an earlier run can never be
/// compared against.
fn clean_home(home: &Path) -> Result<(), String> {
    match fs::remove_dir_all(home) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", home.display())),
    }
    fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))
}

/// The rows of every readable `summary.json` below `home`. An unreadable one is reported and
/// skipped: it can lose a benchmark's name, never change the verdict (spec §4.5).
fn read_rows(home: &Path) -> Vec<report::Row> {
    let mut rows = Vec::new();
    for path in summary_files(home) {
        match fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|json| report::read_summary(&json))
        {
            Ok(row) => rows.push(row),
            Err(error) => eprintln!("warning: could not read {}: {error}", path.display()),
        }
    }
    rows
}

/// `cargo xtask bench-regress` (spec §4.4 to §4.6).
pub(crate) fn bench_regress(options: &Options) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("current directory: {e}"))?;
    let repo = PathBuf::from(git(
        &cwd,
        &["rev-parse", "--show-toplevel"],
        "not a git repository",
    )?);
    let head_lock =
        fs::read_to_string(repo.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;

    // 1. Preflight, then resolve the two commits.
    preflight(&repo, &head_lock)?;
    let rev = match &options.base {
        Some(rev) => rev.clone(),
        None => git(
            &repo,
            &["merge-base", "origin/main", "HEAD"],
            "could not find the merge base with origin/main (pass --base <rev>)",
        )?,
    };
    let base = git(
        &repo,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
        &format!("`--base {rev}` is not a commit"),
    )?;
    let head = git(&repo, &["rev-parse", "HEAD"], "no HEAD commit")?;
    let (short_base, short_head) = (&base[..base.len().min(7)], &head[..head.len().min(7)]);

    // 2. Relevance.
    let changed = git(
        &repo,
        &["diff", "--name-only", &base],
        "could not list the changes",
    )?;
    if !any_relevant(&changed) {
        let message = format!("bench-regress: skipped, no relevant changes since {short_base}");
        println!("{message}");
        append_step_summary(&step_summary_skipped(short_base, short_head, &message));
        return Ok(());
    }

    // 3. Clean: no baseline from an earlier run may be compared against.
    let work =
        target_dir(std::env::var("CARGO_TARGET_DIR").ok().as_deref(), &repo).join("bench-regress");
    let home = work.join("home");
    clean_home(&home)?;
    let env = [("GUNGRAUN_HOME", home.to_string_lossy().into_owned())];

    // 4. Base run.
    // Errors from `git` here must stop the command: reading them as "the base has no crate" would
    // quietly skip the comparison.
    let base_lock = git(
        &repo,
        &["show", &format!("{base}:Cargo.lock")],
        "could not read Cargo.lock of the base",
    )?;
    let manifests: Vec<String> = regress_dirs()
        .map(|dir| format!("{dir}/Cargo.toml"))
        .collect();
    let mut ls_tree = vec!["ls-tree", "--name-only", &base, "--"];
    ls_tree.extend(manifests.iter().map(String::as_str));
    let base_dir = base_regress_dir(&git(
        &repo,
        &ls_tree,
        "could not list the files of the base",
    )?);
    let plan = base_plan(base_dir, &base_lock, &head_lock);
    match &plan {
        Ok(dir) => {
            if *dir != REGRESS_DIR {
                println!("bench-regress: the base has the package at its earlier location {dir}/");
            }
            run_base(&repo, &work, &base, dir, &env)?;
        }
        Err(BaseSkip::NoCrate) => println!(
            "bench-regress: the base has no {} package, so every benchmark is new",
            crate::REGRESS_PACKAGE
        ),
        Err(BaseSkip::VersionChanged { base, head }) => {
            println!("bench-regress: not compared: gungraun changed from {base} to {head}");
        }
    }

    // 5. Head run.
    for stale in summary_files(&home) {
        let _ = fs::remove_file(stale);
    }
    let exes = build(&repo, None, false)?;
    let exits = run_targets(&repo.join(REGRESS_DIR), &exes, &head_args(), &env);
    let verdict = report::classify(&exits);

    // 6. Verdict and summary.
    let rows = read_rows(&home);
    if !rows.is_empty() {
        println!("\n{}", report::render_table(&rows));
    }
    let line = verdict_line(&verdict, options.report_only, plan.as_ref().err());
    append_step_summary(&report::step_summary(
        short_base,
        short_head,
        &line,
        &rows,
        options.report_only,
    ));
    let closing = report::finish(&verdict, options.report_only, &rows)?;
    println!("{closing}");
    Ok(())
}

/// The base run: a worktree of `base`, its own build, and the base arguments, with the targets run
/// from `package_dir` (where the base has the regression package, see `base_regress_dir`).
fn run_base(
    repo: &Path,
    work: &Path,
    base: &str,
    package_dir: &str,
    env: &[(&str, String)],
) -> Result<(), String> {
    let worktree = Worktree::add(repo, &work.join("base-worktree"), base)?;
    // The base's rust-toolchain.toml may name another toolchain than the head's.
    let install = ["toolchain", "install"];
    println!(
        "$ (in {}) rustup {}",
        worktree.path.display(),
        install.join(" ")
    );
    let status = base_command("rustup", &worktree.path)
        .args(install)
        .status()
        .map_err(|e| format!("failed to start rustup: {e}"))?;
    if !status.success() {
        return Err(format!(
            "`rustup toolchain install` failed for the base ({status})"
        ));
    }
    let exes = build(&worktree.path, Some(&work.join("base-target")), true)?;
    let exits = run_targets(&worktree.path.join(package_dir), &exes, &base_args(), env);
    match exits.iter().find(|(_, code)| *code != Some(0)) {
        Some((name, code)) => Err(format!(
            "the base run failed: `{name}` ended with {}",
            code.map_or("a signal".to_string(), |c| format!("code {c}"))
        )),
        None => Ok(()),
    }
}

/// The job summary of a skipped run.
fn step_summary_skipped(base: &str, head: &str, message: &str) -> String {
    report::step_summary(base, head, message, &[], false)
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
            "bench/regress/benches/prng.rs",
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
        let in_package = format!("{REGRESS_DIR}/benches/new.rs");
        assert!(is_relevant(&in_package), "RELEVANT must cover REGRESS_DIR");
        for path in [
            "docs/benchmarks.md",
            "README.md",
            "xtask/src/mutants.rs",
            "bench/compare/benches/prng.rs",
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
    fn the_base_package_is_found_at_the_current_or_a_legacy_location() {
        assert_eq!(
            base_regress_dir("bench/regress/Cargo.toml\n"),
            Some("bench/regress")
        );
        // A base from before #28 moved the package, such as the `m2` tag.
        assert_eq!(
            base_regress_dir("bench-regress/Cargo.toml\n"),
            Some("bench-regress")
        );
        // The current location wins over a stale copy at an old one.
        assert_eq!(
            base_regress_dir("bench-regress/Cargo.toml\nbench/regress/Cargo.toml\n"),
            Some("bench/regress")
        );
        // Before M2.4 there was no package at all.
        assert_eq!(base_regress_dir(""), None);
        // Only a whole manifest path counts.
        assert_eq!(base_regress_dir("bench/regress/Cargo.toml.orig\n"), None);
    }

    #[test]
    fn base_plan_cases() {
        let v20 = "[[package]]\nname = \"gungraun\"\nversion = \"0.20.0\"\n";
        let v21 = "[[package]]\nname = \"gungraun\"\nversion = \"0.21.0\"\n";
        let none = "[[package]]\nname = \"other\"\nversion = \"1.0.0\"\n";
        assert_eq!(base_plan(None, v20, v20), Err(BaseSkip::NoCrate));
        assert_eq!(base_plan(Some(REGRESS_DIR), v20, v20), Ok(REGRESS_DIR));
        assert_eq!(
            base_plan(Some("bench-regress"), v20, v20),
            Ok("bench-regress")
        );
        assert_eq!(
            base_plan(Some(REGRESS_DIR), v20, v21),
            Err(BaseSkip::VersionChanged {
                base: "0.20.0".to_string(),
                head: "0.21.0".to_string()
            })
        );
        assert_eq!(
            base_plan(Some(REGRESS_DIR), none, v20),
            Err(BaseSkip::VersionChanged {
                base: "none".to_string(),
                head: "0.20.0".to_string()
            })
        );
    }

    #[test]
    fn base_and_head_arguments() {
        assert_eq!(
            base_args(),
            [
                "--bench",
                "--save-baseline=base",
                "--callgrind-args=--cache-sim=yes"
            ]
        );
        assert_eq!(
            head_args(),
            [
                "--bench",
                "--baseline=base",
                "--callgrind-limits=ir=5%",
                "--save-summary=json",
                "--callgrind-args=--cache-sim=yes"
            ]
        );
        // Every regressed benchmark must be reported, so the run is never cut short.
        for args in [base_args(), head_args()] {
            assert!(!args.iter().any(|a| a.contains("fail-fast")), "{args:?}");
        }
    }

    #[test]
    fn build_arguments() {
        assert_eq!(
            build_args(),
            [
                "bench",
                "-p",
                "engine_bench_regress",
                "--no-run",
                "--message-format=json"
            ]
        );
    }

    #[test]
    fn any_relevant_reads_git_name_only_output() {
        assert!(any_relevant("README.md\nlibs/engine_random/src/prng.rs\n"));
        assert!(!any_relevant("README.md\ndocs/benchmarks.md\n"));
        assert!(!any_relevant(""));
    }

    #[test]
    fn verdict_lines_say_what_happened() {
        let none = None;
        assert_eq!(verdict_line(&Verdict::Passed, false, none), "passed");
        assert!(verdict_line(&Verdict::Regressed, false, none).contains("regressed"));
        assert!(verdict_line(&Verdict::Regressed, true, none).contains("accepted"));
        assert!(verdict_line(&Verdict::Failed("boom".into()), true, none).contains("boom"));
        let skip = Some(BaseSkip::NoCrate);
        assert!(verdict_line(&Verdict::Passed, false, skip.as_ref()).contains("new"));
        let skip = Some(BaseSkip::VersionChanged {
            base: "0.20.0".into(),
            head: "0.21.0".into(),
        });
        let line = verdict_line(&Verdict::Passed, false, skip.as_ref());
        assert!(
            line.contains("not compared") && line.contains("0.20.0") && line.contains("0.21.0")
        );
    }

    #[test]
    fn base_commands_do_not_inherit_the_head_toolchain() {
        // `cargo xtask` runs under rustup's proxy, which exports RUSTUP_TOOLCHAIN; left in place it
        // would outrank the base's rust-toolchain.toml and build the base with the head's compiler.
        let command = base_command("cargo", Path::new("/w"));
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        assert!(
            removed.contains(&"RUSTUP_TOOLCHAIN".to_string()),
            "{removed:?}"
        );
        assert!(
            removed.contains(&"RUSTUP_TOOLCHAIN_SOURCE".to_string()),
            "{removed:?}"
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/w")));
    }
}
