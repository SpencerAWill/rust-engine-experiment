//! Reading Gungraun's `summary.json` files, classifying the head run, and rendering the results
//! (spec M2.4 §4.5, §4.6). Only the JSON summaries are read, never Gungraun's text output.

use std::fmt::Write as _;

use serde_json::Value;

/// One benchmark of the head run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    /// The bench target (the file under `benches/`), such as `prng`.
    pub(super) target: String,
    /// The benchmark function, such as `next_u64_10k`.
    pub(super) id: String,
    /// The instruction count of the base run; `None` for a benchmark with no baseline.
    pub(super) base_ir: Option<u64>,
    /// The instruction count of the head run.
    pub(super) head_ir: u64,
    /// Whether the count rose past the limit.
    pub(super) regressed: bool,
}

impl Row {
    /// `target::id`, as the messages name a benchmark.
    fn name(&self) -> String {
        format!("{}::{}", self.target, self.id)
    }
}

/// Reads one `summary.json` (Gungraun 0.20.0, summary version 7).
///
/// The fields used: `module_path` (`<target>::<group>::<function>`), `function_name`, `id` (the
/// case, `fixed` for ours), and for the first profile that has instruction counts,
/// `data.total.metrics.Ir.values.{new,old}` and `data.total.regressions`.
pub(super) fn read_summary(json: &str) -> Result<Row, String> {
    let summary: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let missing = |field: &str| format!("summary has no `{field}`");
    let module_path = summary["module_path"]
        .as_str()
        .ok_or_else(|| missing("module_path"))?;
    let target = module_path.split("::").next().unwrap_or(module_path);
    let function = summary["function_name"]
        .as_str()
        .ok_or_else(|| missing("function_name"))?;
    let id = match summary["id"].as_str() {
        Some(case) if case != "fixed" => format!("{function}.{case}"),
        _ => function.to_string(),
    };
    let total = summary["profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|profile| &profile["data"]["total"])
        .find(|total| total["metrics"]["Ir"].is_object())
        .ok_or_else(|| missing("profiles[].data.total.metrics.Ir"))?;
    let values = &total["metrics"]["Ir"]["values"];
    let head_ir = values["new"]
        .as_u64()
        .ok_or_else(|| missing("...Ir.values.new"))?;
    Ok(Row {
        target: target.to_string(),
        id,
        base_ir: values["old"].as_u64(),
        head_ir,
        regressed: total["regressions"]
            .as_array()
            .is_some_and(|regressions| !regressions.is_empty()),
    })
}

/// How the head run went, from its targets' exit codes.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// Every target exited 0.
    Passed,
    /// Every target exited 0 or 3 (Gungraun's "regressed"), and at least one exited 3.
    Regressed,
    /// A target exited with another code or a signal, or there was no target; the text says which.
    Failed(String),
}

/// Classifies the head run from `(target name, exit code)` pairs; a code of `None` means the
/// process was killed by a signal.
pub(super) fn classify(exits: &[(String, Option<i32>)]) -> Verdict {
    if exits.is_empty() {
        return Verdict::Failed("no bench target ran".to_string());
    }
    let failures: Vec<String> = exits
        .iter()
        .filter_map(|(name, code)| match code {
            Some(0 | 3) => None,
            Some(code) => Some(format!("`{name}` exited with code {code}")),
            None => Some(format!("`{name}` was killed by a signal")),
        })
        .collect();
    if !failures.is_empty() {
        return Verdict::Failed(failures.join("; "));
    }
    if exits.iter().any(|(_, code)| *code == Some(3)) {
        Verdict::Regressed
    } else {
        Verdict::Passed
    }
}

/// The names of the regressed rows, as `target::id`.
fn regressed_names(rows: &[Row]) -> Vec<String> {
    rows.iter().filter(|r| r.regressed).map(Row::name).collect()
}

/// The ending of the command: `Ok` carries the closing message, `Err` the failure message
/// (spec §4.5). `report_only` changes only the outcome of a regression.
pub(super) fn finish(verdict: &Verdict, report_only: bool, rows: &[Row]) -> Result<String, String> {
    match verdict {
        Verdict::Passed => Ok("bench-regress: no regression".to_string()),
        Verdict::Failed(reason) => Err(format!("bench-regress failed: {reason}")),
        Verdict::Regressed => {
            let names = regressed_names(rows);
            let what = match names.len() {
                0 => "One or more benchmarks regressed by more than 5% in instructions \
                      (the names are unavailable; read the output above)"
                    .to_string(),
                1 => format!(
                    "1 benchmark regressed by more than 5% in instructions ({})",
                    names[0]
                ),
                n => format!(
                    "{n} benchmarks regressed by more than 5% in instructions ({})",
                    names.join(", ")
                ),
            };
            if report_only {
                Ok(format!("REGRESSION ACCEPTED BY LABEL: {what}."))
            } else {
                Err(format!(
                    "{what}.\nIf the regression is intended, add the `perf-regression-accepted` \
                     label to the pull request\nand say why in its description."
                ))
            }
        }
    }
}

/// The change from base to head in percent, for display: `None` when there is nothing to compare.
fn change(row: &Row) -> Option<f64> {
    let base = row.base_ir.filter(|base| *base != 0)?;
    #[allow(
        clippy::cast_precision_loss,
        reason = "for display; counts stay far below 2^52"
    )]
    Some((row.head_ir as f64 - base as f64) / base as f64 * 100.0)
}

/// The cells of one table row: name, base, head and change.
fn cells(row: &Row) -> [String; 4] {
    [
        row.name(),
        row.base_ir
            .map_or_else(|| "-".to_string(), |b| b.to_string()),
        row.head_ir.to_string(),
        match (row.base_ir, change(row)) {
            (None, _) => "new".to_string(),
            (Some(_), None) => "n/a".to_string(),
            (Some(_), Some(pct)) => format!("{:+.1}%", pct + 0.0),
        },
    ]
}

/// The rows ordered by target, then id.
fn sorted(rows: &[Row]) -> Vec<&Row> {
    let mut sorted: Vec<&Row> = rows.iter().collect();
    sorted.sort_by(|a, b| (&a.target, &a.id).cmp(&(&b.target, &b.id)));
    sorted
}

/// The plain-text table printed after the head run, one line per benchmark.
pub(super) fn render_table(rows: &[Row]) -> String {
    let header = [
        "benchmark".to_string(),
        "base".to_string(),
        "head".to_string(),
        "change".to_string(),
    ];
    let body: Vec<([String; 4], bool)> = sorted(rows)
        .iter()
        .map(|r| (cells(r), r.regressed))
        .collect();
    let width = |column: usize| {
        body.iter()
            .map(|(cells, _)| cells[column].len())
            .chain([header[column].len()])
            .max()
            .unwrap_or(0)
    };
    let widths = [width(0), width(1), width(2), width(3)];
    let line = |cells: &[String; 4], marker: &str| {
        format!(
            "{:<w0$}  {:>w1$}  {:>w2$}  {:>w3$}{marker}\n",
            cells[0],
            cells[1],
            cells[2],
            cells[3],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
            w3 = widths[3],
        )
    };
    let mut table = line(&header, "");
    for (cells, regressed) in &body {
        table.push_str(&line(cells, if *regressed { "  REGRESSED" } else { "" }));
    }
    table
}

/// The Markdown appended to `$GITHUB_STEP_SUMMARY`: the two commits, the verdict, the regressed
/// benchmarks (as "Accepted by label" with `--report-only`) and the table.
pub(super) fn step_summary(
    base: &str,
    head: &str,
    verdict_line: &str,
    rows: &[Row],
    report_only: bool,
) -> String {
    let mut out =
        format!("## bench-regress\n\nBase `{base}`, head `{head}`.\n\n{verdict_line}\n\n");
    let names = regressed_names(rows);
    if !names.is_empty() {
        let heading = if report_only {
            "Accepted by label"
        } else {
            "Regressed benchmarks"
        };
        let _ = writeln!(out, "### {heading}\n");
        for name in &names {
            let _ = writeln!(out, "- `{name}`");
        }
        out.push('\n');
    }
    if !rows.is_empty() {
        out.push_str("| benchmark | base | head | change |\n| --- | ---: | ---: | ---: |\n");
        for row in sorted(rows) {
            let [name, base, head, change] = cells(row);
            let flag = if row.regressed { " **regressed**" } else { "" };
            let _ = writeln!(out, "| {name} | {base} | {head} | {change}{flag} |");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGRESSED: &str = include_str!("fixtures/regressed.json");
    const UNCHANGED: &str = include_str!("fixtures/unchanged.json");
    const NEW: &str = include_str!("fixtures/new.json");

    fn row(target: &str, id: &str, base: Option<u64>, head: u64, regressed: bool) -> Row {
        Row {
            target: target.to_string(),
            id: id.to_string(),
            base_ir: base,
            head_ir: head,
            regressed,
        }
    }

    fn exits(codes: &[Option<i32>]) -> Vec<(String, Option<i32>)> {
        codes
            .iter()
            .enumerate()
            .map(|(i, code)| (format!("target{i}"), *code))
            .collect()
    }

    #[test]
    fn reads_the_fixtures() {
        let regressed = read_summary(REGRESSED).unwrap();
        assert_eq!(
            regressed,
            row("prng", "next_u64_10k", Some(120_010), 132_010, true)
        );
        let unchanged = read_summary(UNCHANGED).unwrap();
        assert_eq!(
            unchanged,
            row("prng", "next_u64_10k", Some(120_010), 120_010, false)
        );
        let new = read_summary(NEW).unwrap();
        assert_eq!(new, row("prng", "next_u64_10k", None, 120_010, false));
    }

    #[test]
    fn malformed_summaries_are_errors() {
        assert!(read_summary("not json").is_err());
        assert!(read_summary("{}").is_err());
        assert!(read_summary(r#"{"module_path":"a::b::c","profiles":[]}"#).is_err());
    }

    #[test]
    fn classify_cases() {
        assert_eq!(classify(&exits(&[Some(0), Some(0)])), Verdict::Passed);
        assert_eq!(classify(&exits(&[Some(3), Some(0)])), Verdict::Regressed);
        assert_eq!(classify(&exits(&[Some(3), Some(3)])), Verdict::Regressed);
        for codes in [[Some(3), Some(101)], [Some(3), None], [Some(1), Some(0)]] {
            assert!(
                matches!(classify(&exits(&codes)), Verdict::Failed(_)),
                "{codes:?}"
            );
        }
        assert!(matches!(classify(&[]), Verdict::Failed(_)));
    }

    #[test]
    fn a_failed_target_names_itself_and_its_code() {
        let Verdict::Failed(message) = classify(&exits(&[Some(0), Some(101)])) else {
            panic!("expected a failure");
        };
        assert!(
            message.contains("target1") && message.contains("101"),
            "{message}"
        );
        let Verdict::Failed(message) = classify(&exits(&[None])) else {
            panic!("expected a failure");
        };
        assert!(message.contains("signal"), "{message}");
    }

    #[test]
    fn a_crash_beside_a_regression_is_a_failure() {
        let verdict = classify(&exits(&[Some(3), None]));
        let rows = [row("prng", "next_u64_10k", Some(100), 200, true)];
        assert!(finish(&verdict, true, &rows).is_err());
        assert!(finish(&verdict, false, &rows).is_err());
    }

    #[test]
    fn finish_cases() {
        let regressed = [
            row("prng", "next_u64_10k", Some(100), 200, true),
            row("handle", "pool_insert_1k", Some(100), 100, false),
            row("handle", "dense_free_1k", Some(100), 130, true),
        ];
        // Passed
        assert!(finish(&Verdict::Passed, false, &regressed).is_ok());
        assert!(finish(&Verdict::Passed, true, &regressed).is_ok());
        // Regressed, no label: an error naming each regressed benchmark and the label.
        let error = finish(&Verdict::Regressed, false, &regressed).unwrap_err();
        assert!(error.contains("prng::next_u64_10k"), "{error}");
        assert!(error.contains("handle::dense_free_1k"), "{error}");
        assert!(!error.contains("pool_insert_1k"), "{error}");
        assert!(error.contains("perf-regression-accepted"), "{error}");
        // Regressed, with the label: success, still naming them.
        let message = finish(&Verdict::Regressed, true, &regressed).unwrap();
        assert!(
            message.contains("REGRESSION ACCEPTED BY LABEL"),
            "{message}"
        );
        assert!(message.contains("prng::next_u64_10k"), "{message}");
        assert!(message.contains("handle::dense_free_1k"), "{message}");
        // Regressed, but no regressed row could be read.
        let unnamed = finish(&Verdict::Regressed, false, &[]).unwrap_err();
        assert!(unnamed.contains("unavailable"), "{unnamed}");
        assert!(unnamed.contains("perf-regression-accepted"), "{unnamed}");
        let unnamed = finish(&Verdict::Regressed, true, &[]).unwrap();
        assert!(unnamed.contains("unavailable"), "{unnamed}");
        // A failure stays a failure.
        let failed = Verdict::Failed("target prng exited with code 101".to_string());
        assert!(finish(&failed, false, &regressed).is_err());
        assert!(finish(&failed, true, &regressed).is_err());
    }

    #[test]
    fn the_guide_message_is_exact_for_one_regression() {
        let rows = [row("prng", "next_u64_10k", Some(100), 200, true)];
        let error = finish(&Verdict::Regressed, false, &rows).unwrap_err();
        assert_eq!(
            error,
            "1 benchmark regressed by more than 5% in instructions (prng::next_u64_10k).\n\
             If the regression is intended, add the `perf-regression-accepted` label to the pull request\n\
             and say why in its description."
        );
    }

    #[test]
    fn table_orders_by_target_then_id_and_marks_regressed_rows() {
        let rows = [
            row("prng", "shuffle_1k", Some(1000), 1053, true),
            row("handle", "pool_insert_1k", None, 55_040, false),
            row("handle", "dense_free_1k", Some(200), 200, false),
        ];
        let table = render_table(&rows);
        let find = |needle: &str| {
            table
                .find(needle)
                .unwrap_or_else(|| panic!("{needle}\n{table}"))
        };
        assert!(find("handle::dense_free_1k") < find("handle::pool_insert_1k"));
        assert!(find("handle::pool_insert_1k") < find("prng::shuffle_1k"));
        assert!(table.contains("+5.3%"), "{table}");
        assert!(table.contains("+0.0%"), "{table}");
        let line_of = |needle: &str| table.lines().find(|l| l.contains(needle)).unwrap();
        assert!(line_of("pool_insert_1k").contains("new"), "{table}");
        assert!(line_of("shuffle_1k").contains("REGRESSED"), "{table}");
        assert!(!line_of("dense_free_1k").contains("REGRESSED"), "{table}");
    }

    #[test]
    fn a_zero_base_count_does_not_divide_by_zero() {
        let table = render_table(&[row("prng", "x", Some(0), 10, false)]);
        assert!(!table.contains("inf") && !table.contains("NaN"), "{table}");
    }

    #[test]
    fn step_summary_has_shas_verdict_and_accepted_list() {
        let rows = [
            row("prng", "next_u64_10k", Some(100), 200, true),
            row("prng", "shuffle_1k", Some(100), 100, false),
        ];
        let accepted = step_summary("abc1234", "def5678", "regression accepted", &rows, true);
        assert!(
            accepted.contains("abc1234") && accepted.contains("def5678"),
            "{accepted}"
        );
        assert!(accepted.contains("regression accepted"), "{accepted}");
        assert!(accepted.contains("Accepted by label"), "{accepted}");
        assert!(accepted.contains("prng::next_u64_10k"), "{accepted}");
        assert!(accepted.contains("| prng::shuffle_1k"), "{accepted}");
        let failed = step_summary("abc1234", "def5678", "regressed", &rows, false);
        assert!(failed.contains("Regressed benchmarks"), "{failed}");
        assert!(!failed.contains("Accepted by label"), "{failed}");
        let clean = step_summary("abc1234", "def5678", "passed", &[rows[1].clone()], false);
        assert!(
            !clean.contains("Regressed benchmarks") && !clean.contains("Accepted"),
            "{clean}"
        );
    }
}
