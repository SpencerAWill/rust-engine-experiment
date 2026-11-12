//! Groups points into charts and compares two baselines.

use std::collections::BTreeMap;

use super::load::Point;

/// A change smaller than this (in percent) is treated as noise. It is the drift measured on the
/// untouched control benchmarks between runs on the same quiet machine.
pub(crate) const NOISE_PCT: f64 = 5.0;

/// One line (or bar) of a chart: an implementation, and variant, across the group's sizes.
#[derive(Debug, Clone)]
pub(crate) struct Series {
    pub(crate) name: String,
    pub(crate) implementation: String,
    pub(crate) variant: Option<String>,
    /// Sorted by size.
    pub(crate) points: Vec<Point>,
}

/// All the series of one criterion group, which share a chart.
#[derive(Debug, Clone)]
pub(crate) struct Group {
    pub(crate) name: String,
    pub(crate) series: Vec<Series>,
}

/// Groups points by criterion group and series, sorted by name and then numerically by size.
pub(crate) fn group(points: &[Point]) -> Vec<Group> {
    let mut by_group: BTreeMap<&str, BTreeMap<String, Series>> = BTreeMap::new();
    for point in points {
        let series = by_group
            .entry(&point.group)
            .or_default()
            .entry(point.series())
            .or_insert_with(|| Series {
                name: point.series(),
                implementation: point.implementation.clone(),
                variant: point.variant.clone(),
                points: Vec::new(),
            });
        series.points.push(point.clone());
    }
    by_group
        .into_iter()
        .map(|(name, series)| Group {
            name: name.to_string(),
            series: series
                .into_values()
                .map(|mut s| {
                    s.points
                        .sort_by(|a, b| a.x.unwrap_or(0.0).total_cmp(&b.x.unwrap_or(0.0)));
                    s
                })
                .collect(),
        })
        .collect()
}

/// Whether a benchmark got measurably faster or slower between two baselines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Faster,
    Slower,
    WithinNoise,
}

/// The change in one benchmark between two baselines.
#[derive(Debug, Clone)]
pub(crate) struct Delta {
    pub(crate) group: String,
    pub(crate) series: String,
    pub(crate) x: Option<f64>,
    pub(crate) old_ns: f64,
    pub(crate) new_ns: f64,
    /// Positive means slower.
    pub(crate) change_pct: f64,
    pub(crate) verdict: Verdict,
}

/// The result of [`compare`].
#[derive(Debug, Clone, Default)]
pub(crate) struct Comparison {
    pub(crate) deltas: Vec<Delta>,
    /// Benchmarks only the old baseline has.
    pub(crate) only_old: Vec<String>,
    /// Benchmarks only the new baseline has.
    pub(crate) only_new: Vec<String>,
}

/// Compares `new` against `old`, benchmark by benchmark.
///
/// A change counts as real only when it exceeds [`NOISE_PCT`] **and** the two confidence
/// intervals do not overlap: a large shift between overlapping intervals cannot be told apart
/// from measurement noise.
pub(crate) fn compare(old: &[Point], new: &[Point]) -> Comparison {
    let old_by_id: BTreeMap<String, &Point> = old.iter().map(|p| (p.id(), p)).collect();
    let new_by_id: BTreeMap<String, &Point> = new.iter().map(|p| (p.id(), p)).collect();
    let mut result = Comparison::default();
    for (id, n) in &new_by_id {
        let Some(o) = old_by_id.get(id) else {
            result.only_new.push(id.clone());
            continue;
        };
        let change_pct = (n.median_ns - o.median_ns) / o.median_ns * 100.0;
        let separated = n.lower_ns > o.upper_ns || n.upper_ns < o.lower_ns;
        let verdict = if change_pct.abs() <= NOISE_PCT || !separated {
            Verdict::WithinNoise
        } else if change_pct > 0.0 {
            Verdict::Slower
        } else {
            Verdict::Faster
        };
        result.deltas.push(Delta {
            group: n.group.clone(),
            series: n.series(),
            x: n.x,
            old_ns: o.median_ns,
            new_ns: n.median_ns,
            change_pct,
            verdict,
        });
    }
    result.only_old = old_by_id
        .keys()
        .filter(|id| !new_by_id.contains_key(*id))
        .cloned()
        .collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(
        group: &str,
        implementation: &str,
        x: Option<f64>,
        median: f64,
        half_width: f64,
    ) -> Point {
        Point {
            group: group.to_string(),
            implementation: implementation.to_string(),
            variant: None,
            x,
            elements: None,
            median_ns: median,
            lower_ns: median - half_width,
            upper_ns: median + half_width,
        }
    }

    #[test]
    fn groups_sort_numerically_by_group_series_and_size() {
        let points = [
            point("b", "z", Some(1_000.0), 1.0, 0.0),
            point("a", "z", Some(100_000.0), 1.0, 0.0),
            point("a", "z", Some(2_000.0), 1.0, 0.0),
            point("a", "m", Some(9.0), 1.0, 0.0),
        ];
        let groups = group(&points);
        let names: Vec<_> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        let series: Vec<_> = groups[0].series.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(series, ["m", "z"]);
        let xs: Vec<_> = groups[0].series[1]
            .points
            .iter()
            .map(|p| p.x.unwrap())
            .collect();
        assert_eq!(xs, [2_000.0, 100_000.0]); // numeric order, not text order
    }

    #[test]
    fn identical_runs_are_within_noise() {
        let a = [point("g", "f", Some(10.0), 100.0, 1.0)];
        let c = compare(&a, &a);
        assert_eq!(c.deltas.len(), 1);
        assert_eq!(c.deltas[0].verdict, Verdict::WithinNoise);
        assert!(c.deltas[0].change_pct.abs() < f64::EPSILON);
    }

    #[test]
    fn a_big_change_with_separated_intervals_is_real() {
        let old = [point("g", "f", Some(10.0), 100.0, 1.0)];
        let slower = [point("g", "f", Some(10.0), 120.0, 1.0)];
        let faster = [point("g", "f", Some(10.0), 80.0, 1.0)];
        let d = &compare(&old, &slower).deltas[0];
        assert_eq!(d.verdict, Verdict::Slower);
        assert!((d.change_pct - 20.0).abs() < 1e-9);
        let d = &compare(&old, &faster).deltas[0];
        assert_eq!(d.verdict, Verdict::Faster);
        assert!((d.change_pct + 20.0).abs() < 1e-9);
    }

    #[test]
    fn a_big_change_with_overlapping_intervals_is_not_trusted() {
        let old = [point("g", "f", Some(10.0), 100.0, 15.0)];
        let new = [point("g", "f", Some(10.0), 120.0, 15.0)];
        assert_eq!(compare(&old, &new).deltas[0].verdict, Verdict::WithinNoise);
    }

    #[test]
    fn a_small_change_is_noise_even_with_tight_intervals() {
        let old = [point("g", "f", Some(10.0), 100.0, 0.1)];
        let new = [point("g", "f", Some(10.0), 103.0, 0.1)];
        assert_eq!(compare(&old, &new).deltas[0].verdict, Verdict::WithinNoise);
    }

    #[test]
    fn benchmarks_present_in_only_one_baseline_are_reported() {
        let old = [
            point("g", "gone", None, 1.0, 0.0),
            point("g", "kept", None, 1.0, 0.0),
        ];
        let new = [
            point("g", "kept", None, 1.0, 0.0),
            point("g", "added", None, 1.0, 0.0),
        ];
        let c = compare(&old, &new);
        assert_eq!(c.deltas.len(), 1);
        assert_eq!(c.only_old, ["g/gone"]);
        assert_eq!(c.only_new, ["g/added"]);
    }
}
