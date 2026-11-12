//! Reads criterion's saved results (`target/criterion/**/<baseline>/`) into [`Point`]s.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// One benchmark measurement: a median with its confidence interval.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Point {
    /// Criterion's group id, such as `get_random` or `scenario/steady_churn_frame`.
    pub(crate) group: String,
    /// The compared implementation (criterion's function id), such as `engine_handle`.
    pub(crate) implementation: String,
    /// What distinguishes runs of one implementation, such as `u64` in `u64/1000`.
    pub(crate) variant: Option<String>,
    /// The size or count the benchmark was run at, when it has one.
    pub(crate) x: Option<f64>,
    /// Elements processed per iteration (criterion's throughput), when declared.
    pub(crate) elements: Option<u64>,
    pub(crate) median_ns: f64,
    pub(crate) lower_ns: f64,
    pub(crate) upper_ns: f64,
}

impl Point {
    /// The series this point belongs to within its group: the implementation plus its variant.
    pub(crate) fn series(&self) -> String {
        match &self.variant {
            Some(variant) => format!("{} \u{b7} {variant}", self.implementation),
            None => self.implementation.clone(),
        }
    }

    /// A stable id for matching the same benchmark across two baselines.
    pub(crate) fn id(&self) -> String {
        match self.x {
            Some(x) => format!("{}/{}@{x}", self.group, self.series()),
            None => format!("{}/{}", self.group, self.series()),
        }
    }

    /// Median and interval in nanoseconds per element (or per iteration without a throughput).
    pub(crate) fn per_element(&self) -> (f64, f64, f64) {
        #[allow(
            clippy::cast_precision_loss,
            reason = "element counts are far below 2^53"
        )]
        let divisor = self.elements.map_or(1.0, |e| e.max(1) as f64);
        (
            self.median_ns / divisor,
            self.lower_ns / divisor,
            self.upper_ns / divisor,
        )
    }
}

fn number(value: &Value, path: &[&str]) -> Result<f64, String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor
            .get(key)
            .ok_or_else(|| format!("missing `{}`", path.join(".")))?;
    }
    cursor
        .as_f64()
        .ok_or_else(|| format!("`{}` is not a number", path.join(".")))
}

/// Builds a [`Point`] from the text of a `benchmark.json` and its sibling `estimates.json`.
pub(crate) fn parse_point(benchmark: &str, estimates: &str) -> Result<Point, String> {
    let bench: Value =
        serde_json::from_str(benchmark).map_err(|e| format!("benchmark.json: {e}"))?;
    let est: Value = serde_json::from_str(estimates).map_err(|e| format!("estimates.json: {e}"))?;
    let text = |key: &str| {
        bench
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("benchmark.json: missing `{key}`"))
    };
    let group = text("group_id")?;
    let implementation = text("function_id")?;
    // `value_str` is the benchmark parameter: `1000`, or `u64/1000` when a variant is prepended.
    let (variant, x) = match bench.get("value_str").and_then(Value::as_str) {
        None => (None, None),
        Some(value) => match value.rsplit_once('/') {
            Some((variant, last)) => (Some(variant.to_string()), last.parse::<f64>().ok()),
            None => match value.parse::<f64>() {
                Ok(x) => (None, Some(x)),
                Err(_) => (Some(value.to_string()), None),
            },
        },
    };
    let elements = bench
        .get("throughput")
        .and_then(|t| t.get("Elements"))
        .and_then(Value::as_u64);
    Ok(Point {
        group,
        implementation,
        variant,
        x,
        elements,
        median_ns: number(&est, &["median", "point_estimate"])?,
        lower_ns: number(&est, &["median", "confidence_interval", "lower_bound"])?,
        upper_ns: number(&est, &["median", "confidence_interval", "upper_bound"])?,
    })
}

fn collect_baseline_dirs(dir: &Path, baseline: &str, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|n| n == baseline) && path.join("estimates.json").is_file()
        {
            out.push(path);
        } else if path.file_name().is_some_and(|n| n != "report") {
            collect_baseline_dirs(&path, baseline, out)?;
        }
    }
    Ok(())
}

/// Loads every benchmark saved under `baseline` in `criterion_dir`, in a stable order.
///
/// # Errors
///
/// Fails if the directory cannot be read, a result file is malformed, or no benchmark has that
/// baseline (the message lists the baselines that do exist).
pub(crate) fn load(criterion_dir: &Path, baseline: &str) -> Result<Vec<Point>, String> {
    let mut dirs = Vec::new();
    collect_baseline_dirs(criterion_dir, baseline, &mut dirs)?;
    if dirs.is_empty() {
        return Err(format!(
            "no benchmark in {} has a baseline named `{baseline}` (run `cargo xtask bench --save {baseline}` first)",
            criterion_dir.display()
        ));
    }
    let mut points = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let read = |name: &str| {
            fs::read_to_string(dir.join(name))
                .map_err(|e| format!("{}: {e}", dir.join(name).display()))
        };
        let point = parse_point(&read("benchmark.json")?, &read("estimates.json")?)
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        points.push(point);
    }
    points.sort_by(|a, b| {
        (&a.group, a.series(), a.x.unwrap_or(0.0).to_bits()).cmp(&(
            &b.group,
            b.series(),
            b.x.unwrap_or(0.0).to_bits(),
        ))
    });
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BENCH_SIZED: &str = r#"{"group_id":"get_random","function_id":"engine_handle","value_str":"u64/1000000","throughput":{"Elements":1000000},"full_id":"get_random/engine_handle/u64/1000000","directory_name":"get_random/engine_handle/u64_1000000","title":"x"}"#;
    const BENCH_PLAIN_SIZE: &str = r#"{"group_id":"allocator/batch_fresh","function_id":"batch","value_str":"1000","throughput":{"Elements":1000},"full_id":"a","directory_name":"a","title":"a"}"#;
    const BENCH_UNSIZED: &str = r#"{"group_id":"prng/next_u64","function_id":"fastrand","value_str":null,"throughput":{"Elements":1000000},"full_id":"a","directory_name":"a","title":"a"}"#;
    const ESTIMATES: &str = r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":3.0},"point_estimate":2.0,"standard_error":0.1},"median":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1500000.0,"upper_bound":2500000.0},"point_estimate":2000000.0,"standard_error":0.1}}"#;

    #[test]
    fn parses_a_sized_benchmark_with_a_variant() {
        let p = parse_point(BENCH_SIZED, ESTIMATES).unwrap();
        assert_eq!(p.group, "get_random");
        assert_eq!(p.implementation, "engine_handle");
        assert_eq!(p.variant.as_deref(), Some("u64"));
        assert_eq!(p.x, Some(1_000_000.0));
        assert_eq!(p.elements, Some(1_000_000));
        assert_eq!(
            (p.median_ns, p.lower_ns, p.upper_ns),
            (2_000_000.0, 1_500_000.0, 2_500_000.0)
        );
        assert_eq!(p.series(), "engine_handle \u{b7} u64");
        assert_eq!(p.id(), "get_random/engine_handle \u{b7} u64@1000000");
        assert_eq!(p.per_element(), (2.0, 1.5, 2.5));
    }

    #[test]
    fn parses_a_bare_size_and_a_benchmark_without_one() {
        let p = parse_point(BENCH_PLAIN_SIZE, ESTIMATES).unwrap();
        assert_eq!((p.variant.as_deref(), p.x), (None, Some(1000.0)));
        let q = parse_point(BENCH_UNSIZED, ESTIMATES).unwrap();
        assert_eq!((q.variant.as_deref(), q.x), (None, None));
        assert_eq!(q.id(), "prng/next_u64/fastrand");
    }

    #[test]
    fn reports_which_field_is_missing() {
        let err = parse_point(BENCH_SIZED, r#"{"mean":{}}"#).unwrap_err();
        assert!(err.contains("median.point_estimate"), "{err}");
        let err = parse_point("{}", ESTIMATES).unwrap_err();
        assert!(err.contains("group_id"), "{err}");
        assert!(parse_point("not json", ESTIMATES).is_err());
    }

    #[test]
    fn loads_only_the_named_baseline_in_a_stable_order() {
        let root = std::env::temp_dir().join(format!("xtask-load-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (dir, bench) in [
            ("b_group/f/1000", BENCH_PLAIN_SIZE),
            ("a_group/f", BENCH_UNSIZED),
        ] {
            for baseline in ["m1", "new"] {
                let d = root.join(dir).join(baseline);
                fs::create_dir_all(&d).unwrap();
                fs::write(d.join("benchmark.json"), bench).unwrap();
                fs::write(d.join("estimates.json"), ESTIMATES).unwrap();
            }
        }
        fs::create_dir_all(root.join("report")).unwrap();
        let points = load(&root, "m1").unwrap();
        let groups: Vec<_> = points.iter().map(|p| p.group.as_str()).collect();
        assert_eq!(groups, ["allocator/batch_fresh", "prng/next_u64"]);
        let err = load(&root, "nope").unwrap_err();
        assert!(err.contains("`nope`"), "{err}");
        fs::remove_dir_all(&root).unwrap();
    }
}
