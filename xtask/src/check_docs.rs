//! `check-docs`: the documentation checks of spec M3.16 (links, lesson files, rule files and
//! the length of `AGENTS.md`), run on the tracked files.
//!
//! [`check`] is the testable core: it takes the set of tracked paths and a reader, so tests build
//! fixture trees from string literals. [`run`] lists the files with git, reads them from disk and
//! prints the sorted problems. The required READMEs and their Contents (spec §4.1, §4.2) are
//! checked in the `readme` submodule.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::process::Command;

mod frontmatter;
mod glob;
mod links;
mod markdown;
mod readme;

/// Where lesson files live (every `*.md` directly inside, except `README.md`).
const LESSON_DIR: &str = "docs/lessons/";

/// Where rule files live (every `*.md` directly inside).
const RULE_DIR: &str = ".claude/rules/";

/// Directories that must have a `README.md` (spec §4.1), besides every `spikes/<topic>/` that
/// holds tracked files.
const REQUIRED_READMES: [&str; 12] = [
    "docs",
    "docs/adr",
    "docs/chunks",
    "docs/lessons",
    ".claude",
    "infra/github",
    "libs/engine",
    "bench",
    "fuzz",
    "xtask",
    "spikes",
    "examples",
];

/// `AGENTS.md` must have fewer lines than this.
const AGENTS_MAX_LINES: usize = 80;

/// One problem: the file, the 1-based line when there is one, and what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Problem {
    path: String,
    line: Option<usize>,
    message: String,
}

impl Problem {
    /// A problem in `path`, at `line` when known.
    fn new(path: &str, line: Option<usize>, message: &str) -> Self {
        Self {
            path: path.to_string(),
            line,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Problem {
    /// `<path>:<line>: <message>`, or `<path>: <message>` without a line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.path, self.message),
            None => write!(f, "{}: {}", self.path, self.message),
        }
    }
}

/// Runs every check on the tree `files` (repository-relative paths), reading contents through
/// `read`. Returns every problem, sorted by path, line and message.
pub(crate) fn check(
    files: &BTreeSet<String>,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut texts = BTreeMap::new();
    for path in files.iter().filter(|p| is_markdown(p)) {
        match read(path) {
            Ok(text) => {
                texts.insert(path.clone(), text);
            }
            Err(reason) => problems.push(Problem::new(
                path,
                None,
                &format!("cannot read the file ({reason}); check that it exists and is readable"),
            )),
        }
    }
    for (path, text) in &texts {
        if is_direct_child(path, LESSON_DIR) && path != "docs/lessons/README.md" {
            problems.extend(frontmatter::check_lesson(path, text, files));
        } else if is_direct_child(path, RULE_DIR) {
            problems.extend(frontmatter::check_rule(path, text, files));
        }
    }
    if let Some(text) = texts.get("AGENTS.md") {
        let lines = text.lines().count();
        if lines >= AGENTS_MAX_LINES {
            problems.push(Problem::new(
                "AGENTS.md",
                None,
                &format!(
                    "{lines} lines; it must stay under {AGENTS_MAX_LINES}: move detail into the README it points to"
                ),
            ));
        }
    }
    problems.extend(readme::check(files, &texts));
    problems.extend(links::check(files, &texts));
    problems.sort();
    problems
}

/// Whether `path` is a file directly inside the directory `dir` (which ends with `/`).
fn is_direct_child(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| !rest.contains('/') && is_markdown(rest))
}

/// Whether `path` names a Markdown file (extension `.md`, any case).
pub(crate) fn is_markdown(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// The tracked files: `git ls-files --cached --others --exclude-standard`, without paths that
/// are not on disk (`--cached` still lists a deleted file).
fn tracked_files() -> Result<BTreeSet<String>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`git ls-files` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|p| !p.is_empty() && fs::symlink_metadata(p).is_ok())
        .map(ToString::to_string)
        .collect())
}

/// `cargo xtask check-docs`: prints each problem and a count line, and fails if there is any.
pub(crate) fn run() -> Result<(), String> {
    let files = tracked_files()?;
    let problems = check(&files, &|path| {
        fs::read_to_string(path).map_err(|e| e.to_string())
    });
    for problem in &problems {
        println!("{problem}");
    }
    match problems.len() {
        0 => {
            println!("check-docs: 0 problems");
            Ok(())
        }
        1 => Err("check-docs: 1 problem".to_string()),
        n => Err(format!("check-docs: {n} problems")),
    }
}

/// An in-memory tree for tests: every path, and the contents of the Markdown ones.
#[cfg(test)]
fn fixture(entries: &[(&str, &str)]) -> (BTreeSet<String>, BTreeMap<String, String>) {
    let files = entries.iter().map(|(p, _)| (*p).to_string()).collect();
    let texts = entries
        .iter()
        .filter(|(p, _)| is_markdown(p))
        .map(|(p, t)| ((*p).to_string(), (*t).to_string()))
        .collect();
    (files, texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(entries: &[(&str, &str)]) -> Vec<String> {
        let (files, texts) = fixture(entries);
        let read = |path: &str| {
            texts
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{path}: not found"))
        };
        // These fixtures hold no READMEs; the README checks have tests of their own.
        check(&files, &read)
            .iter()
            .map(ToString::to_string)
            .filter(|p| !p.contains("README.md"))
            .collect()
    }

    fn lines(n: usize) -> String {
        "x\n".repeat(n)
    }

    #[test]
    fn agents_md_under_80_lines_passes_and_80_fails() {
        assert_eq!(run(&[("AGENTS.md", &lines(79))]), Vec::<String>::new());
        let problems = run(&[("AGENTS.md", &lines(80))]);
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(problems[0].starts_with("AGENTS.md: "), "{}", problems[0]);
        assert!(problems[0].contains("80 lines"), "{}", problems[0]);
    }

    #[test]
    fn problems_from_two_files_come_back_sorted() {
        let problems = run(&[("z.md", "[a](nope.md)\n"), ("a.md", "x\n[a](nope.md)\n")]);
        assert_eq!(
            problems,
            [
                "a.md:2: link `nope.md`: no such file; fix the path or remove the link",
                "z.md:1: link `nope.md`: no such file; fix the path or remove the link",
            ]
        );
    }

    #[test]
    fn a_problem_without_a_line_prints_without_one() {
        let p = Problem::new("a.md", None, "msg");
        assert_eq!(p.to_string(), "a.md: msg");
        let p = Problem::new("a.md", Some(3), "msg");
        assert_eq!(p.to_string(), "a.md:3: msg");
    }

    #[test]
    fn problems_sort_by_path_then_line_then_message() {
        let mut v = [
            Problem::new("b.md", Some(1), "x"),
            Problem::new("a.md", Some(2), "x"),
            Problem::new("a.md", None, "z"),
            Problem::new("a.md", Some(2), "a"),
        ];
        v.sort();
        let shown: Vec<String> = v.iter().map(ToString::to_string).collect();
        assert_eq!(shown, ["a.md: z", "a.md:2: a", "a.md:2: x", "b.md:1: x"]);
    }

    #[test]
    fn lesson_and_rule_files_are_found_by_directory() {
        let lesson = "---\napplies_to:\n  - \"nowhere/**\"\n---\n# Lessons: t\n\n- Status: Living (2026-11-15)\n";
        let rule = "---\npaths:\n  - \"nowhere/**\"\n---\nx\n";
        let problems = run(&[
            ("docs/lessons/t.md", lesson),
            ("docs/lessons/README.md", "# README\n"),
            (".claude/rules/r.md", rule),
            ("docs/other.md", "# not a lesson\n"),
        ]);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(
            problems[0].starts_with(".claude/rules/r.md:3: "),
            "{}",
            problems[0]
        );
        assert!(
            problems[1].starts_with("docs/lessons/t.md:3: "),
            "{}",
            problems[1]
        );
    }

    #[test]
    fn an_unreadable_file_is_a_problem() {
        let (files, _) = fixture(&[("a.md", "x\n")]);
        let read = |path: &str| -> Result<String, String> { Err(format!("{path}: denied")) };
        let problems: Vec<String> = check(&files, &read)
            .iter()
            .map(ToString::to_string)
            .filter(|p| !p.contains("README.md"))
            .collect();
        assert_eq!(problems.len(), 1, "{problems:#?}");
        assert!(problems[0].starts_with("a.md: "), "{}", problems[0]);
    }
}
