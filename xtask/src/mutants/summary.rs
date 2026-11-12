//! The summary table built from `cargo-mutants`' outcome files.
//!
//! Each outcome file lists mutant names in the form `cargo mutants --list` prints:
//! `<path>:<line>:<column>: <description>`. [`render`] groups them by crate and writes the
//! Markdown that `cargo xtask mutants` prints and CI appends to the job summary.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Component, Path};

/// The contents of the four outcome files in `mutants.out/`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Outcomes<'a> {
    /// `caught.txt`: a test failed with the mutant in place.
    pub(super) caught: &'a str,
    /// `missed.txt`: every test passed with the mutant in place.
    pub(super) missed: &'a str,
    /// `timeout.txt`: the tests did not finish in time.
    pub(super) timeout: &'a str,
    /// `unviable.txt`: the mutant did not compile.
    pub(super) unviable: &'a str,
}

/// The facts the heading line reports.
#[derive(Debug, Clone, Copy)]
pub(super) struct Header<'a> {
    /// The `cargo mutants --version` output.
    pub(super) version: &'a str,
    /// The packages that were run.
    pub(super) packages: &'a [String],
    /// How many mutants the exclusion list removes from those packages.
    pub(super) excluded_mutants: usize,
    /// How many entries the exclusion list has.
    pub(super) exclude_entries: usize,
}

/// Outcome counts of one crate.
#[derive(Debug, Default, Clone, Copy)]
struct Row {
    caught: usize,
    missed: usize,
    timeout: usize,
    unviable: usize,
}

impl Row {
    fn tested(self) -> usize {
        self.caught + self.missed + self.timeout + self.unviable
    }

    fn line(self, name: &str) -> String {
        format!(
            "| {name} | {} | {} | {} | {} | {} |\n",
            self.tested(),
            self.caught,
            self.missed,
            self.timeout,
            self.unviable
        )
    }
}

/// A mutant name split into the parts the summary needs.
struct Name<'a> {
    krate: String,
    path: &'a str,
    line: u32,
    column: u32,
}

/// Splits a mutant name into its crate, path, line and column.
fn parse_name(name: &str) -> Result<Name<'_>, String> {
    let bad = || {
        format!("cannot read `{name}` as a mutant name (`<path>:<line>:<column>: <description>`)")
    };
    let (location, _description) = name.split_once(": ").ok_or_else(bad)?;
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next().and_then(|c| c.parse().ok()).ok_or_else(bad)?;
    let line = parts.next().and_then(|l| l.parse().ok()).ok_or_else(bad)?;
    let path = parts.next().filter(|p| !p.is_empty()).ok_or_else(bad)?;
    let mut components = Path::new(path).components().map(Component::as_os_str);
    let krate = match (components.next(), components.next(), components.next()) {
        (Some(root), Some(krate), Some(src))
            if (root == "libs" || root == "examples") && src == "src" =>
        {
            krate.to_string_lossy().into_owned()
        }
        _ => {
            return Err(format!(
                "`{name}` is not under `libs/<crate>/src/` or `examples/<crate>/src/`"
            ));
        }
    };
    Ok(Name {
        krate,
        path,
        line,
        column,
    })
}

/// The non-blank lines of an outcome file.
fn names(file: &str) -> impl Iterator<Item = &str> {
    file.lines().map(str::trim).filter(|l| !l.is_empty())
}

/// Renders the summary: a heading line, the per-crate table, and the missed mutants.
///
/// # Errors
///
/// A name that is not of the form `<path>:<line>:<column>: <description>`, or whose path is not
/// under `libs/<crate>/src/` or `examples/<crate>/src/`, is an error naming it.
pub(super) fn render<'a>(header: &Header<'_>, outcomes: &Outcomes<'a>) -> Result<String, String> {
    let mut rows: BTreeMap<String, Row> = header
        .packages
        .iter()
        .map(|p| (p.clone(), Row::default()))
        .collect();
    let mut missed = Vec::new();
    let mut tally = |file: &'a str, count: fn(&mut Row), is_missed: bool| -> Result<(), String> {
        for name in names(file) {
            let parsed = parse_name(name)?;
            count(rows.entry(parsed.krate).or_default());
            if is_missed {
                missed.push((parsed.path, parsed.line, parsed.column, name));
            }
        }
        Ok(())
    };
    tally(outcomes.caught, |r| r.caught += 1, false)?;
    tally(outcomes.missed, |r| r.missed += 1, true)?;
    tally(outcomes.timeout, |r| r.timeout += 1, false)?;
    tally(outcomes.unviable, |r| r.unviable += 1, false)?;
    missed.sort_unstable();

    let mut out = String::from("## Mutation testing\n\n");
    let _ = writeln!(
        out,
        "{}; packages: {}; {} mutants excluded by {} entries in .cargo/mutants.toml.\n",
        header.version,
        header.packages.join(", "),
        header.excluded_mutants,
        header.exclude_entries
    );
    out.push_str("| Crate | Tested | Caught | Missed | Timeout | Unviable |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    let mut total = Row::default();
    for (name, row) in &rows {
        out.push_str(&row.line(name));
        total.caught += row.caught;
        total.missed += row.missed;
        total.timeout += row.timeout;
        total.unviable += row.unviable;
    }
    out.push_str(&total.line("Total"));

    let _ = write!(out, "\n### Missed ({})\n\n", missed.len());
    if missed.is_empty() {
        out.push_str("None.\n");
    } else {
        out.push_str("```text\n");
        for (_, _, _, name) in &missed {
            let _ = writeln!(out, "{name}");
        }
        out.push_str("```\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packages(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_string()).collect()
    }

    fn header(packages: &[String]) -> Header<'_> {
        Header {
            version: "cargo-mutants 27.1.0",
            packages,
            excluded_mutants: 0,
            exclude_entries: 0,
        }
    }

    const EMPTY: Outcomes<'static> = Outcomes {
        caught: "",
        missed: "",
        timeout: "",
        unviable: "",
    };

    #[test]
    fn rows_are_ordered_by_crate_with_total_last() {
        let pkgs = packages(&["engine_random", "engine_handle"]);
        let outcomes = Outcomes {
            caught: "libs/engine_random/src/a.rs:1:1: x\nlibs/engine_handle/src/a.rs:1:1: y\nlibs/engine_handle/src/a.rs:2:1: z\n",
            missed: "libs/engine_random/src/a.rs:3:1: m\n",
            timeout: "libs/engine_handle/src/a.rs:5:1: t\n",
            unviable: "libs/engine_random/src/a.rs:4:1: u\nlibs/engine_random/src/a.rs:9:1: v\n",
        };
        let out = render(&header(&pkgs), &outcomes).unwrap();
        let rows: Vec<&str> = out.lines().filter(|l| l.starts_with("| ")).collect();
        assert_eq!(
            rows,
            [
                "| Crate | Tested | Caught | Missed | Timeout | Unviable |",
                "| engine_handle | 3 | 2 | 0 | 1 | 0 |",
                "| engine_random | 4 | 1 | 1 | 0 | 2 |",
                "| Total | 7 | 3 | 1 | 1 | 2 |",
            ]
        );
    }

    #[test]
    fn selected_crate_without_outcomes_shows_zeros() {
        let pkgs = packages(&["engine_handle", "engine_time"]);
        let outcomes = Outcomes {
            caught: "libs/engine_handle/src/a.rs:1:1: x\n",
            ..EMPTY
        };
        let out = render(&header(&pkgs), &outcomes).unwrap();
        assert!(out.contains("| engine_time | 0 | 0 | 0 | 0 | 0 |"), "{out}");
    }

    #[test]
    fn missed_list_is_in_file_and_line_order() {
        let pkgs = packages(&["engine_handle"]);
        let outcomes = Outcomes {
            missed: "libs/engine_handle/src/b.rs:9:3: r1\nlibs/engine_handle/src/a.rs:30:1: r2\nlibs/engine_handle/src/a.rs:4:1: r3\n",
            ..EMPTY
        };
        let out = render(&header(&pkgs), &outcomes).unwrap();
        let (_, missed) = out.split_once("### Missed (3)\n").expect(&out);
        let listed: Vec<&str> = missed.lines().filter(|l| l.starts_with("libs/")).collect();
        assert_eq!(
            listed,
            [
                "libs/engine_handle/src/a.rs:4:1: r3",
                "libs/engine_handle/src/a.rs:30:1: r2",
                "libs/engine_handle/src/b.rs:9:3: r1",
            ]
        );
        assert!(missed.contains("```text\n"), "{missed}");
    }

    #[test]
    fn no_missed_says_none() {
        let pkgs = packages(&["engine_handle"]);
        let out = render(&header(&pkgs), &EMPTY).unwrap();
        assert!(out.contains("### Missed (0)\n\nNone.\n"), "{out}");
        assert!(!out.contains("```"), "{out}");
    }

    #[test]
    fn heading_line_names_version_packages_and_exclusions() {
        let pkgs = packages(&["engine_handle", "engine_random"]);
        let head = Header {
            excluded_mutants: 12,
            exclude_entries: 5,
            ..header(&pkgs)
        };
        let out = render(&head, &EMPTY).unwrap();
        assert!(out.starts_with("## Mutation testing\n\n"), "{out}");
        assert!(
            out.contains("cargo-mutants 27.1.0; packages: engine_handle, engine_random; 12 mutants excluded by 5 entries in .cargo/mutants.toml.\n"),
            "{out}"
        );
    }

    #[test]
    fn examples_paths_map_to_their_crate() {
        let pkgs = packages(&["fixed_step_sim"]);
        let outcomes = Outcomes {
            caught: "examples/fixed_step_sim/src/main.rs:1:1: x\n",
            ..EMPTY
        };
        let out = render(&header(&pkgs), &outcomes).unwrap();
        assert!(
            out.contains("| fixed_step_sim | 1 | 1 | 0 | 0 | 0 |"),
            "{out}"
        );
    }

    #[test]
    fn name_outside_the_patterns_is_an_error() {
        let pkgs = packages(&["engine_handle"]);
        let outcomes = Outcomes {
            caught: "xtask/src/main.rs:1:1: x\n",
            ..EMPTY
        };
        let err = render(&header(&pkgs), &outcomes).unwrap_err();
        assert!(err.contains("xtask/src/main.rs:1:1: x"), "{err}");
    }

    #[test]
    fn line_that_is_not_a_name_is_an_error() {
        let pkgs = packages(&["engine_handle"]);
        let outcomes = Outcomes {
            missed: "garbage\n",
            ..EMPTY
        };
        let err = render(&header(&pkgs), &outcomes).unwrap_err();
        assert!(err.contains("garbage"), "{err}");
    }

    #[test]
    fn blank_lines_are_ignored() {
        let pkgs = packages(&["engine_handle"]);
        let outcomes = Outcomes {
            caught: "\nlibs/engine_handle/src/a.rs:1:1: x\n\n   \n",
            ..EMPTY
        };
        let out = render(&header(&pkgs), &outcomes).unwrap();
        assert!(
            out.contains("| engine_handle | 1 | 1 | 0 | 0 | 0 |"),
            "{out}"
        );
    }

    #[test]
    fn descriptions_may_contain_colons() {
        let pkgs = packages(&["engine_handle"]);
        let outcomes = Outcomes {
            missed: "libs/engine_handle/src/a.rs:7:5: replace <impl fmt::Debug for Handle<T>>::fmt with Ok(Default::default())\n",
            ..EMPTY
        };
        assert!(render(&header(&pkgs), &outcomes).is_ok());
    }
}
