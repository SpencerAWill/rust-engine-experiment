//! `bench-report`: turns saved criterion results into one self-contained HTML page.
//!
//! ```text
//! cargo xtask bench-report [--baseline NAME] [--against NAME] [--notes FILE] [--title TEXT] [--out FILE]
//! ```
//!
//! Results come from `target/criterion` (save them with `cargo xtask bench --save NAME`). With
//! `--against`, the page leads with what changed between the two baselines.

mod html;
pub(crate) mod load;
pub(crate) mod model;
pub(crate) mod svg;

use std::fs;
use std::path::{Path, PathBuf};

/// Where criterion keeps its results.
pub(crate) const CRITERION_DIR: &str = "target/criterion";

/// The parsed `bench-report` flags.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// The baseline to report on (criterion's `new` is the most recent run).
    pub(crate) baseline: String,
    /// An older baseline to compare against.
    pub(crate) against: Option<String>,
    pub(crate) title: String,
    /// A text file (machine, load, conditions) shown at the top of the page.
    pub(crate) notes: Option<PathBuf>,
    pub(crate) out: PathBuf,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            baseline: "new".to_string(),
            against: None,
            title: "Benchmark report".to_string(),
            notes: None,
            out: PathBuf::from("target/bench-report/index.html"),
        }
    }
}

/// Parses the flags after `bench-report`.
pub(crate) fn parse_flags(flags: &[&str]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut it = flags.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| -> Result<String, String> {
            it.next()
                .map(|v| (*v).to_string())
                .ok_or_else(|| format!("`{name}` needs a value"))
        };
        match *flag {
            "--baseline" => options.baseline = value("--baseline")?,
            "--against" => options.against = Some(value("--against")?),
            "--title" => options.title = value("--title")?,
            "--notes" => options.notes = Some(PathBuf::from(value("--notes")?)),
            "--out" => options.out = PathBuf::from(value("--out")?),
            other => return Err(format!("unknown flag `{other}` for `bench-report`")),
        }
    }
    if options.against.as_deref() == Some(options.baseline.as_str()) {
        return Err("`--against` must name a different baseline than `--baseline`".to_string());
    }
    Ok(options)
}

/// Builds the page for `options` from the results in `criterion_dir`.
///
/// # Errors
///
/// Fails if a baseline is missing or unreadable, or the notes file cannot be read.
pub(crate) fn build_page(criterion_dir: &Path, options: &Options) -> Result<String, String> {
    let points = load::load(criterion_dir, &options.baseline)?;
    let comparison = match &options.against {
        Some(against) => {
            let old = load::load(criterion_dir, against)?;
            Some((against.as_str(), model::compare(&old, &points)))
        }
        None => None,
    };
    let notes = match &options.notes {
        Some(path) => {
            Some(fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)
        }
        None => None,
    };
    Ok(html::render(&html::Input {
        title: &options.title,
        baseline: &options.baseline,
        points: &points,
        comparison: comparison.as_ref().map(|(name, c)| (*name, c)),
        notes: notes.as_deref(),
    }))
}

/// Runs the command: builds the page and writes it to `options.out`.
pub(crate) fn run(options: &Options) -> Result<(), String> {
    let page = build_page(Path::new(CRITERION_DIR), options)?;
    if let Some(parent) = options.out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(&options.out, page).map_err(|e| format!("{}: {e}", options.out.display()))?;
    println!("bench-report: wrote {}", options.out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_report_the_latest_run() {
        let o = parse_flags(&[]).unwrap();
        assert_eq!(o, Options::default());
        assert_eq!(o.baseline, "new");
        assert!(o.against.is_none());
    }

    #[test]
    fn parses_every_flag() {
        let o = parse_flags(&[
            "--baseline",
            "m2",
            "--against",
            "m1",
            "--title",
            "T",
            "--notes",
            "n.txt",
            "--out",
            "o.html",
        ])
        .unwrap();
        assert_eq!(o.baseline, "m2");
        assert_eq!(o.against.as_deref(), Some("m1"));
        assert_eq!(o.title, "T");
        assert_eq!(o.notes, Some(PathBuf::from("n.txt")));
        assert_eq!(o.out, PathBuf::from("o.html"));
    }

    #[test]
    fn rejects_bad_flags() {
        assert!(
            parse_flags(&["--baseline"])
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(
            parse_flags(&["--nope"])
                .unwrap_err()
                .contains("unknown flag")
        );
        assert!(
            parse_flags(&["--baseline", "a", "--against", "a"])
                .unwrap_err()
                .contains("different baseline")
        );
    }

    fn write_bench(root: &Path, dir: &str, baseline: &str, bench: &str, median: f64) {
        let d = root.join(dir).join(baseline);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("benchmark.json"), bench).unwrap();
        fs::write(
            d.join("estimates.json"),
            format!(
                r#"{{"median":{{"point_estimate":{median},"confidence_interval":{{"lower_bound":{},"upper_bound":{}}}}}}}"#,
                median * 0.99,
                median * 1.01
            ),
        )
        .unwrap();
    }

    #[test]
    fn builds_a_comparison_page_from_two_saved_baselines() {
        let root = std::env::temp_dir().join(format!("xtask-report-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bench = r#"{"group_id":"get_random","function_id":"engine_handle","value_str":"u64/1000","throughput":{"Elements":10},"full_id":"x","directory_name":"x","title":"x"}"#;
        write_bench(
            &root,
            "get_random/engine_handle/u64_1000",
            "old",
            bench,
            2000.0,
        );
        write_bench(
            &root,
            "get_random/engine_handle/u64_1000",
            "new",
            bench,
            1000.0,
        );
        let options = Options {
            baseline: "new".into(),
            against: Some("old".into()),
            ..Options::default()
        };
        let page = build_page(&root, &options).unwrap();
        assert!(page.contains("1 faster"), "{page}");
        assert!(page.contains("id=\"changes\"") && page.contains("id=\"get_random\""));
        let missing = Options {
            baseline: "absent".into(),
            ..Options::default()
        };
        assert!(
            build_page(&root, &missing)
                .unwrap_err()
                .contains("`absent`")
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
