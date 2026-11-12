//! `bench-doc`: regenerates the data sections of `docs/benchmarks.md` from saved criterion
//! baselines, so the published numbers can be reproduced.
//!
//! ```text
//! cargo xtask bench-doc [--baseline NAME] [--env FILE]... [--doc FILE]
//! ```
//!
//! Only the text between marker comments is replaced:
//!
//! - `<!-- bench:env -->` ... `<!-- /bench:env -->`: the methodology table, from `--env` files
//!   (`key: value` lines recorded when the run started);
//! - `<!-- bench:ratios -->` ... `<!-- /bench:ratios -->`: our time divided by each competitor's,
//!   per group, as a median with the range across sizes;
//! - `<!-- bench:tables -->` ... `<!-- /bench:tables -->`: the median time of every benchmark.
//!
//! The analysis around the markers is written by hand and is not touched.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::bench_report::CRITERION_DIR;
use crate::bench_report::load::{self, Point};
use crate::bench_report::model::group;
use crate::bench_report::svg::{format_count, format_ns};

/// The parsed `bench-doc` flags.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// The saved criterion baseline to read.
    pub(crate) baseline: String,
    /// Files of `key: value` lines describing the run (machine, power, load), one per run.
    pub(crate) env_files: Vec<PathBuf>,
    /// The document to update.
    pub(crate) doc: PathBuf,
}

/// A ratio within this distance of 1 is a tie: the measured run-to-run noise.
pub(crate) const TIE: f64 = 0.05;

/// Parses the flags after `bench-doc`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let mut options = Options {
        baseline: "final".to_string(),
        env_files: Vec::new(),
        doc: PathBuf::from("docs/benchmarks.md"),
    };
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| -> Result<String, String> {
            it.next()
                .map(|v| (*v).to_string())
                .ok_or_else(|| format!("`{name}` needs a value"))
        };
        match *flag {
            "--baseline" => options.baseline = value("--baseline")?,
            "--env" => options.env_files.push(PathBuf::from(value("--env")?)),
            "--doc" => options.doc = PathBuf::from(value("--doc")?),
            other => return Err(format!("unknown flag `{other}` for `bench-doc`")),
        }
    }
    Ok(options)
}

/// Replaces the text between `<!-- bench:NAME -->` and `<!-- /bench:NAME -->` with `body`.
pub(crate) fn replace_section(doc: &str, name: &str, body: &str) -> Result<String, String> {
    let start = format!("<!-- bench:{name} -->");
    let end = format!("<!-- /bench:{name} -->");
    let mut starts = doc.match_indices(&start).map(|(i, _)| i);
    let Some(start_at) = starts.next() else {
        return Err(format!("the document has no `{start}` marker"));
    };
    if starts.next().is_some() {
        return Err(format!("`{start}` appears more than once"));
    }
    let after_start = start_at + start.len();
    let Some(end_offset) = doc[after_start..].find(&end) else {
        return Err(format!("`{end}` is missing after `{start}`"));
    };
    Ok(format!(
        "{}\n{}\n{}",
        &doc[..after_start],
        body.trim(),
        &doc[after_start + end_offset..]
    ))
}

/// Recognised `key: value` lines of an environment file, with the label shown in the table.
const ENV_KEYS: [(&str, &str); 9] = [
    ("date", "Date"),
    ("cpu", "Machine"),
    ("memory_gb", "Memory (GB)"),
    ("cores", "Cores"),
    ("os", "OS"),
    ("rustc", "Toolchain"),
    ("power", "Power"),
    ("load_before", "System load at start"),
    ("git", "Source commit"),
];

/// The methodology table: one row per recognised `key: value` line, in a fixed order, with one
/// column per environment file.
pub(crate) fn env_table(env_files: &[String]) -> String {
    let parsed: Vec<BTreeMap<&str, &str>> = env_files
        .iter()
        .map(|text| {
            text.lines()
                .filter_map(|line| line.split_once(": "))
                .map(|(k, v)| (k.trim(), v.trim()))
                .collect()
        })
        .collect();
    let columns = parsed.len().max(1);
    let header = if columns == 1 {
        "| | |".to_string()
    } else {
        format!(
            "| | {} |",
            (1..=columns)
                .map(|i| format!("run {i}"))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let mut out = format!("{header}\n|{}\n", "---|".repeat(columns + 1));
    for (key, label) in ENV_KEYS {
        if parsed.iter().all(|env| !env.contains_key(key)) {
            continue;
        }
        let cells: Vec<&str> = parsed
            .iter()
            .map(|env| env.get(key).copied().unwrap_or(""))
            .collect();
        let _ = writeln!(out, "| {label} | {} |", cells.join(" | "));
    }
    out.trim_end().to_string()
}

/// `(group, variant, x)`: what makes two benchmarks of different implementations comparable.
type CaseKey = (String, Option<String>, u64);

fn case_key(p: &Point) -> CaseKey {
    (
        p.group.clone(),
        p.variant.clone(),
        p.x.map_or(0, f64::to_bits),
    )
}

/// Subject-over-competitor time ratios for every case both implementations ran:
/// `group -> subject -> competitor -> ratios`. Subjects are the `engine*` implementations.
fn pair_ratios(points: &[Point]) -> BTreeMap<String, BTreeMap<String, BTreeMap<String, Vec<f64>>>> {
    let mut by_case: BTreeMap<CaseKey, BTreeMap<&str, f64>> = BTreeMap::new();
    for p in points {
        by_case
            .entry(case_key(p))
            .or_default()
            .insert(&p.implementation, p.median_ns);
    }
    let mut out: BTreeMap<String, BTreeMap<String, BTreeMap<String, Vec<f64>>>> = BTreeMap::new();
    for ((group, _, _), impls) in &by_case {
        for (subject, ours) in impls.iter().filter(|(name, _)| name.starts_with("engine")) {
            for (competitor, theirs) in impls.iter().filter(|(name, _)| !name.starts_with("engine"))
            {
                out.entry(group.clone())
                    .or_default()
                    .entry((*subject).to_string())
                    .or_default()
                    .entry((*competitor).to_string())
                    .or_default()
                    .push(ours / theirs);
            }
        }
    }
    out
}

/// For each group, subject and competitor: the median of `subject / competitor` over the matching
/// sizes and variants, with the range, as a markdown table.
pub(crate) fn ratio_table(points: &[Point]) -> String {
    let mut out = String::from(
        "| group | ours | competitor | median ratio (range) | cases |\n|---|---|---|---|---|",
    );
    for (group, subjects) in pair_ratios(points) {
        for (subject, competitors) in subjects {
            for (competitor, mut ratios) in competitors {
                ratios.sort_by(f64::total_cmp);
                let median = ratios[ratios.len() / 2];
                let median = if ratios.len() % 2 == 0 {
                    f64::midpoint(ratios[ratios.len() / 2 - 1], median)
                } else {
                    median
                };
                let _ = write!(
                    out,
                    "\n| `{group}` | `{subject}` | `{competitor}` | {median:.2} ({:.2}\u{2013}{:.2}) | {} |",
                    ratios[0],
                    ratios[ratios.len() - 1],
                    ratios.len()
                );
            }
        }
    }
    out
}

/// How many comparable cases `subject` (an exact implementation name) is more than 5% slower
/// than, within 5% of, and more than 5% faster than `competitor`, over every group:
/// `(slower, tied, faster)`.
pub(crate) fn tally(points: &[Point], subject: &str, competitor: &str) -> (usize, usize, usize) {
    const EPS: f64 = 1e-9;
    let mut counts = (0, 0, 0);
    for competitors in pair_ratios(points).values().filter_map(|s| s.get(subject)) {
        for ratio in competitors.get(competitor).into_iter().flatten() {
            if *ratio > 1.0 + TIE + EPS {
                counts.0 += 1;
            } else if *ratio < 1.0 - TIE - EPS {
                counts.2 += 1;
            } else {
                counts.1 += 1;
            }
        }
    }
    counts
}

/// One markdown table per group with the median time of every benchmark.
pub(crate) fn results_tables(points: &[Point]) -> String {
    let mut sections = Vec::new();
    for group in group(points) {
        let mut xs: Vec<f64> = group
            .series
            .iter()
            .flat_map(|s| s.points.iter().filter_map(|p| p.x))
            .collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let columns: Vec<String> = if xs.is_empty() {
            vec!["\u{2014}".to_string()]
        } else {
            xs.iter().map(|x| format_count(*x)).collect()
        };
        let mut table = format!(
            "#### `{}`\n\n| subject | {} |\n|{}",
            group.name,
            columns.join(" | "),
            "---|".repeat(columns.len() + 1)
        );
        for series in &group.series {
            let cells: Vec<String> = if xs.is_empty() {
                vec![
                    series
                        .points
                        .first()
                        .map_or(String::new(), |p| format_ns(p.median_ns)),
                ]
            } else {
                xs.iter()
                    .map(|x| {
                        series
                            .points
                            .iter()
                            .find(|p| p.x.is_some_and(|px| px.to_bits() == x.to_bits()))
                            .map_or(String::new(), |p| format_ns(p.median_ns))
                    })
                    .collect()
            };
            let _ = write!(table, "\n| `{}` | {} |", series.name, cells.join(" | "));
        }
        sections.push(table);
    }
    sections.join("\n\n")
}

/// The ratio table plus the tallies the analysis quotes.
fn ratios_section(points: &[Point]) -> String {
    let mut lines = vec![ratio_table(points)];
    let mut tallies = Vec::new();
    for competitor in ["slotmap", "slotmap_dense", "thunderdome"] {
        let (slower, tied, faster) = tally(points, "engine_handle", competitor);
        let total = slower + tied + faster;
        if total > 0 {
            tallies.push(format!(
                "`{competitor}`: {slower} of {total} more than 5% slower, {tied} within 5%, {faster} more than 5% faster"
            ));
        }
    }
    if !tallies.is_empty() {
        lines.push(format!(
            "Across all comparable cases, `engine_handle` against {}.",
            tallies.join("; ")
        ));
    }
    lines.join("\n\n")
}

/// Updates the marked sections of `options.doc` from the saved baseline.
pub(crate) fn run(options: &Options) -> Result<(), String> {
    let points = load::load(Path::new(CRITERION_DIR), &options.baseline)?;
    let envs = options
        .env_files
        .iter()
        .map(|path| fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display())))
        .collect::<Result<Vec<_>, _>>()?;
    let doc =
        fs::read_to_string(&options.doc).map_err(|e| format!("{}: {e}", options.doc.display()))?;
    let doc = replace_section(&doc, "env", &env_table(&envs))?;
    let doc = replace_section(&doc, "ratios", &ratios_section(&points))?;
    let doc = replace_section(&doc, "tables", &results_tables(&points))?;
    fs::write(&options.doc, doc).map_err(|e| format!("{}: {e}", options.doc.display()))?;
    println!(
        "bench-doc: updated {} from baseline `{}` ({} benchmarks)",
        options.doc.display(),
        options.baseline,
        points.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(
        group: &str,
        implementation: &str,
        variant: Option<&str>,
        x: Option<f64>,
        median: f64,
    ) -> Point {
        Point {
            group: group.to_string(),
            implementation: implementation.to_string(),
            variant: variant.map(str::to_string),
            x,
            elements: None,
            median_ns: median,
            lower_ns: median,
            upper_ns: median,
        }
    }

    #[test]
    fn flags_default_to_the_documented_run() {
        let o = parse_flags(&[]).unwrap();
        assert_eq!(o.baseline, "final");
        assert!(o.env_files.is_empty());
        assert_eq!(o.doc, PathBuf::from("docs/benchmarks.md"));
    }

    #[test]
    fn flags_accept_several_env_files() {
        let o = parse_flags(&[
            "--baseline",
            "m2",
            "--env",
            "a.txt",
            "--env",
            "b.txt",
            "--doc",
            "d.md",
        ])
        .unwrap();
        assert_eq!(o.baseline, "m2");
        assert_eq!(
            o.env_files,
            [PathBuf::from("a.txt"), PathBuf::from("b.txt")]
        );
        assert_eq!(o.doc, PathBuf::from("d.md"));
        assert!(
            parse_flags(&["--env"])
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(
            parse_flags(&["--nope"])
                .unwrap_err()
                .contains("unknown flag")
        );
    }

    #[test]
    fn a_section_is_replaced_between_its_markers_and_the_rest_is_untouched() {
        let doc = "intro\n<!-- bench:ratios -->\nOLD\n<!-- /bench:ratios -->\noutro\n";
        let out = replace_section(doc, "ratios", "NEW TABLE").unwrap();
        assert_eq!(
            out,
            "intro\n<!-- bench:ratios -->\nNEW TABLE\n<!-- /bench:ratios -->\noutro\n"
        );
        // Running it again changes nothing.
        assert_eq!(replace_section(&out, "ratios", "NEW TABLE").unwrap(), out);
    }

    #[test]
    fn missing_duplicate_or_misordered_markers_are_errors() {
        assert!(
            replace_section("no markers", "ratios", "x")
                .unwrap_err()
                .contains("bench:ratios")
        );
        let twice = "<!-- bench:a -->\n<!-- /bench:a -->\n<!-- bench:a -->\n<!-- /bench:a -->\n";
        assert!(
            replace_section(twice, "a", "x")
                .unwrap_err()
                .contains("more than once")
        );
        let reversed = "<!-- /bench:a -->\n<!-- bench:a -->\n";
        assert!(replace_section(reversed, "a", "x").is_err());
    }

    #[test]
    fn ratios_are_subject_over_competitor_with_a_range_across_sizes() {
        let points = [
            point("get", "engine_handle", Some("u64"), Some(1000.0), 20.0),
            point("get", "slotmap", Some("u64"), Some(1000.0), 10.0),
            point("get", "engine_handle", Some("u64"), Some(100_000.0), 15.0),
            point("get", "slotmap", Some("u64"), Some(100_000.0), 10.0),
            point("get", "engine_handle", Some("u64"), Some(1_000_000.0), 9.0),
            point("get", "slotmap", Some("u64"), Some(1_000_000.0), 10.0),
        ];
        let table = ratio_table(&points);
        // ratios 2.00, 1.50, 0.90: median 1.50, range 0.90 to 2.00, three cases
        assert!(
            table.contains("| `get` | `engine_handle` | `slotmap` | 1.50 (0.90\u{2013}2.00) | 3 |"),
            "{table}"
        );
    }

    #[test]
    fn only_implementations_with_a_competitor_in_the_same_case_are_compared() {
        let points = [
            point("g", "engine_handle", None, Some(1.0), 5.0),
            point("g", "slotmap", None, Some(2.0), 5.0), // different size: no pair
        ];
        assert!(!ratio_table(&points).contains("slotmap"));
    }

    #[test]
    fn the_tally_counts_cases_by_the_five_percent_band() {
        // ratios: 1.20 (slower), 1.05 (tie, boundary), 1.00 (tie), 0.95 (tie, boundary), 0.80 (faster)
        let xs = [
            (1.20, 1.0),
            (1.05, 2.0),
            (1.00, 3.0),
            (0.95, 4.0),
            (0.80, 5.0),
        ];
        let mut points = Vec::new();
        for (ratio, x) in xs {
            points.push(point("g", "engine_handle", None, Some(x), ratio * 100.0));
            points.push(point("g", "slotmap", None, Some(x), 100.0));
        }
        assert_eq!(tally(&points, "engine_handle", "slotmap"), (1, 3, 1));
    }

    #[test]
    fn results_tables_have_a_row_per_series_and_a_column_per_size() {
        let points = [
            point("insert", "engine_handle", Some("u64"), Some(1000.0), 2500.0),
            point(
                "insert",
                "engine_handle",
                Some("u64"),
                Some(100_000.0),
                3_000_000.0,
            ),
            point("insert", "slotmap", Some("u64"), Some(1000.0), 950.0),
            point("prng/next_u64", "fastrand", None, None, 230.0),
        ];
        let t = results_tables(&points);
        assert!(t.contains("#### `insert`"), "{t}");
        assert!(t.contains("| subject | 1K | 100K |"), "{t}");
        assert!(
            t.contains("| `engine_handle \u{b7} u64` | 2.50 \u{b5}s | 3.00 ms |"),
            "{t}"
        );
        assert!(
            t.contains("| `slotmap \u{b7} u64` | 950 ns |  |"),
            "missing cells stay empty: {t}"
        );
        assert!(
            t.contains("#### `prng/next_u64`") && t.contains("| `fastrand` | 230 ns |"),
            "{t}"
        );
    }

    #[test]
    fn the_env_table_labels_known_keys_and_skips_unknown_ones() {
        let env = "date: 2026-11-12T17:00:01Z\ncpu: Apple M4 Pro\npower: Now drawing from 'AC Power'\nsecret_thing: x\n".to_string();
        let t = env_table(&[env]);
        assert!(t.contains("| Date | 2026-11-12T17:00:01Z |"), "{t}");
        assert!(t.contains("| Machine | Apple M4 Pro |"), "{t}");
        assert!(t.contains("| Power | Now drawing from 'AC Power' |"), "{t}");
        assert!(!t.contains("secret_thing"), "{t}");
    }
}
