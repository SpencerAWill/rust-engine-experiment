//! Inline-SVG charts. Colours come from CSS classes (`s0`..`s7` per implementation, `d0`..`d3`
//! dash styles per variant, `faster`/`slower`/`noise` for deltas) so the page can theme them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use engine_math::scalar;

use super::load::Point;
use super::model::{Delta, Group, NOISE_PCT, Series, Verdict};

/// Number of distinct implementation colours in the page's stylesheet.
const COLOR_CLASSES: usize = 8;
/// Number of distinct dash styles, one per variant of an implementation.
const DASH_CLASSES: usize = 4;
/// Smallest time plotted, so a zero measurement cannot reach `ln(0)`.
const FLOOR_NS: f64 = 1e-3;

/// Assigns each implementation one colour class, shared by every chart on the page.
#[derive(Debug, Clone)]
pub(crate) struct Colors {
    /// `(implementation, colour index)`.
    assigned: Vec<(String, usize)>,
}

impl Colors {
    /// Colours `implementations` in the order given, one index each.
    #[cfg(test)]
    pub(crate) fn new(implementations: Vec<String>) -> Self {
        Self {
            assigned: implementations
                .into_iter()
                .enumerate()
                .map(|(i, name)| (name, i % COLOR_CLASSES))
                .collect(),
        }
    }

    /// Colours every implementation in `points` so that two implementations that appear in the
    /// same group never share a colour.
    ///
    /// Greedy colouring, most widely compared implementations first (so the main competitors
    /// always get the first colours), each taking the lowest colour not used by an already
    /// coloured implementation it shares a group with.
    pub(crate) fn for_points(points: &[Point]) -> Self {
        let mut groups_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut members: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for p in points {
            groups_of
                .entry(&p.implementation)
                .or_default()
                .insert(&p.group);
            members
                .entry(&p.group)
                .or_default()
                .insert(&p.implementation);
        }
        let mut order: Vec<&str> = groups_of.keys().copied().collect();
        order.sort_by(|a, b| groups_of[b].len().cmp(&groups_of[a].len()).then(a.cmp(b)));
        let mut assigned: Vec<(String, usize)> = Vec::new();
        for name in order {
            let taken: BTreeSet<usize> = assigned
                .iter()
                .filter(|(other, _)| {
                    groups_of[name]
                        .iter()
                        .any(|g| members[g].contains(other.as_str()))
                })
                .map(|(_, index)| *index)
                .collect();
            let index = (0..COLOR_CLASSES)
                .find(|i| !taken.contains(i))
                .unwrap_or(assigned.len() % COLOR_CLASSES);
            assigned.push((name.to_string(), index));
        }
        Self { assigned }
    }

    /// The colour class (`s0`..`s7`) for `implementation`; unknown names get `s7`.
    pub(crate) fn class(&self, implementation: &str) -> String {
        let index = self
            .assigned
            .iter()
            .find(|(name, _)| name == implementation)
            .map_or(COLOR_CLASSES - 1, |(_, index)| *index);
        format!("s{index}")
    }
}

/// Three significant digits: `950`, `12.5`, `1.50`.
fn sig3(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}")
    } else if value >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

/// Formats nanoseconds with a unit and three significant digits: `950 ns`, `1.50 µs`, `2.50 ms`.
pub(crate) fn format_ns(ns: f64) -> String {
    if !ns.is_finite() {
        return "n/a".to_string();
    }
    let (value, unit) = if ns >= 1e9 {
        (ns / 1e9, "s")
    } else if ns >= 1e6 {
        (ns / 1e6, "ms")
    } else if ns >= 1e3 {
        (ns / 1e3, "\u{b5}s")
    } else {
        (ns, "ns")
    };
    format!("{} {unit}", sig3(value))
}

/// Formats a size compactly: `1000` → `1K`, `1000000` → `1M`.
pub(crate) fn format_count(n: f64) -> String {
    if n >= 1e6 {
        format!("{}M", n / 1e6)
    } else if n >= 1e3 {
        format!("{}K", n / 1e3)
    } else {
        format!("{n}")
    }
}

/// Escapes text for use inside SVG or HTML.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

const WIDTH: f64 = 640.0;
/// Width reserved for row labels in the delta chart.
const DELTA_LABEL: f64 = 230.0;
/// Room kept free beside the longest delta bar for its percentage text.
const DELTA_VALUE_ROOM: f64 = 56.0;
const MARGIN_LEFT: f64 = 76.0;
const MARGIN_RIGHT: f64 = 20.0;
const MARGIN_TOP: f64 = 16.0;
const MARGIN_BOTTOM: f64 = 48.0;
const HEIGHT: f64 = 340.0;

/// Maps `value` from the log-spaced domain `lo..hi` onto `a..b` (the midpoint if `lo == hi`).
fn log_map(value: f64, lo: f64, hi: f64, a: f64, b: f64) -> f64 {
    if hi <= lo {
        return f64::midpoint(a, b);
    }
    let t = (scalar::ln(value.max(lo)) - scalar::ln(lo)) / (scalar::ln(hi) - scalar::ln(lo));
    a + t * (b - a)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "decade exponents are small integers"
)]
fn decade_floor(value: f64) -> i32 {
    scalar::log10(value).floor() as i32
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "decade exponents are small integers"
)]
fn decade_ceil(value: f64) -> i32 {
    scalar::log10(value).ceil() as i32
}

/// The mapping from data to pixels for one scaling chart: log axes with decade ticks.
struct Plot {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    x_lo: f64,
    x_hi: f64,
    y_lo: f64,
    y_hi: f64,
    lo_decade: i32,
    hi_decade: i32,
}

impl Plot {
    /// Fits the axes to the sizes `xs` (sorted) and to the intervals of `points`.
    fn fit(xs: &[f64], points: &[&Point]) -> Self {
        let mut y_min = f64::INFINITY;
        let mut y_max = 0.0_f64;
        for p in points {
            let (_, lo, hi) = p.per_element();
            y_min = y_min.min(lo.max(FLOOR_NS));
            y_max = y_max.max(hi.max(FLOOR_NS));
        }
        if !y_min.is_finite() {
            (y_min, y_max) = (1.0, 10.0);
        }
        let lo_decade = decade_floor(y_min);
        let hi_decade = decade_ceil(y_max).max(lo_decade + 1);
        Self {
            left: MARGIN_LEFT,
            right: WIDTH - MARGIN_RIGHT,
            top: MARGIN_TOP,
            bottom: HEIGHT - MARGIN_BOTTOM,
            x_lo: xs.first().copied().unwrap_or(1.0),
            x_hi: xs.last().copied().unwrap_or(1.0),
            y_lo: scalar::powi(10.0, lo_decade),
            y_hi: scalar::powi(10.0, hi_decade),
            lo_decade,
            hi_decade,
        }
    }

    fn px(&self, x: f64) -> f64 {
        log_map(x, self.x_lo, self.x_hi, self.left + 12.0, self.right - 12.0)
    }

    fn py(&self, y: f64) -> f64 {
        log_map(y.max(FLOOR_NS), self.y_lo, self.y_hi, self.bottom, self.top)
    }

    /// Grid lines, tick labels and axis titles.
    fn axes(&self, svg: &mut String, xs: &[f64]) {
        for decade in self.lo_decade..=self.hi_decade {
            let value = scalar::powi(10.0, decade);
            let y = self.py(value);
            let _ = write!(
                svg,
                "<line class=\"grid\" x1=\"{:.1}\" x2=\"{:.1}\" y1=\"{y:.1}\" y2=\"{y:.1}\"/><text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>",
                self.left,
                self.right,
                self.left - 6.0,
                y + 4.0,
                format_ns(value)
            );
        }
        for x in xs {
            let _ = write!(
                svg,
                "<text class=\"tick\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
                self.px(*x),
                self.bottom + 18.0,
                format_count(*x)
            );
        }
        let _ = write!(
            svg,
            "<text class=\"axis-label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">size (elements)</text>\
             <text class=\"axis-label\" transform=\"translate(14 {:.1}) rotate(-90)\" text-anchor=\"middle\">time per element (log scale)</text>",
            f64::midpoint(self.left, self.right),
            HEIGHT - 8.0,
            f64::midpoint(self.top, self.bottom)
        );
    }

    /// One series: its confidence band, its line, and a marker with a tooltip per point.
    fn series(
        &self,
        svg: &mut String,
        series: &Series,
        points: &[&Point],
        color: &str,
        dash: usize,
    ) {
        let mut line = String::new();
        let mut lower = String::new();
        let mut upper = Vec::new();
        for p in points {
            let (median, lo, hi) = p.per_element();
            let x = self.px(p.x.unwrap_or(self.x_lo));
            let _ = write!(line, "{x:.1},{:.1} ", self.py(median));
            let _ = write!(lower, "{x:.1},{:.1} ", self.py(lo));
            upper.push(format!("{x:.1},{:.1}", self.py(hi)));
        }
        upper.reverse();
        let _ = write!(
            svg,
            "<polygon class=\"band {color}\" points=\"{}{}\"/><polyline class=\"line {color} d{dash}\" points=\"{}\"/>",
            lower,
            upper.join(" "),
            line.trim_end()
        );
        for p in points {
            let (median, lo, hi) = p.per_element();
            let _ = write!(
                svg,
                "<circle class=\"dot {color}\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.5\"><title>{} at {}: {} per element ({} to {})</title></circle>",
                self.px(p.x.unwrap_or(self.x_lo)),
                self.py(median),
                escape(&series.name),
                format_count(p.x.unwrap_or(0.0)),
                format_ns(median),
                format_ns(lo),
                format_ns(hi)
            );
        }
    }
}

/// Time per element against size, on log axes, one line per series with its confidence band.
pub(crate) fn scaling_chart(group: &Group, colors: &Colors) -> String {
    let sized: Vec<(&Series, Vec<&Point>)> = group
        .series
        .iter()
        .map(|s| {
            (
                s,
                s.points
                    .iter()
                    .filter(|p| p.x.is_some())
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(_, points)| !points.is_empty())
        .collect();
    let mut xs: Vec<f64> = sized
        .iter()
        .flat_map(|(_, points)| points.iter().filter_map(|p| p.x))
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup();
    let all: Vec<&Point> = sized
        .iter()
        .flat_map(|(_, points)| points.iter().copied())
        .collect();
    let plot = Plot::fit(&xs, &all);

    let mut svg = format!(
        "<svg viewBox=\"0 0 {WIDTH} {HEIGHT}\" role=\"img\" aria-label=\"{}: time per element against size\" class=\"chart\">",
        escape(&group.name)
    );
    plot.axes(&mut svg, &xs);
    let variants = distinct_variants(group);
    for (series, points) in &sized {
        let dash = variants
            .iter()
            .position(|v| *v == series.variant)
            .map_or(0, |i| i % DASH_CLASSES);
        plot.series(
            &mut svg,
            series,
            points,
            &colors.class(&series.implementation),
            dash,
        );
    }
    svg.push_str("</svg>");
    svg
}

/// The distinct variants of a group's series, in first-seen order (`None` is a variant too).
pub(crate) fn distinct_variants(group: &Group) -> Vec<Option<String>> {
    let mut variants: Vec<Option<String>> = Vec::new();
    for series in &group.series {
        if !variants.contains(&series.variant) {
            variants.push(series.variant.clone());
        }
    }
    variants
}

/// Horizontal bars of time per element for a group whose benchmarks have no size.
pub(crate) fn bar_chart(group: &Group, colors: &Colors) -> String {
    const ROW: f64 = 30.0;
    const LABEL: f64 = 190.0;
    let rows: Vec<_> = group
        .series
        .iter()
        .filter_map(|s| s.points.first().map(|p| (s, p.per_element())))
        .collect();
    let max = rows
        .iter()
        .map(|(_, (_, _, hi))| *hi)
        .fold(FLOOR_NS, f64::max);
    #[allow(clippy::cast_precision_loss, reason = "a handful of rows")]
    let height = rows.len() as f64 * ROW + 16.0;
    let track = WIDTH - LABEL - 90.0;
    let mut svg = format!(
        "<svg viewBox=\"0 0 {WIDTH} {height}\" role=\"img\" aria-label=\"{}: time per element\" class=\"chart\">",
        escape(&group.name)
    );
    for (i, (series, (median, lo, hi))) in rows.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "a handful of rows")]
        let y = 8.0 + i as f64 * ROW;
        let bar = (median / max * track).max(1.0);
        let (lo_x, hi_x) = (lo / max * track, hi / max * track);
        let _ = write!(
            svg,
            "<text class=\"label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>\
             <rect class=\"bar {}\" x=\"{LABEL}\" y=\"{y:.1}\" width=\"{bar:.1}\" height=\"18\"><title>{}: {} per element</title></rect>\
             <line class=\"whisker\" x1=\"{:.1}\" x2=\"{:.1}\" y1=\"{:.1}\" y2=\"{:.1}\"/>\
             <text class=\"value\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
            LABEL - 8.0,
            y + 13.0,
            escape(&series.name),
            colors.class(&series.implementation),
            escape(&series.name),
            format_ns(*median),
            LABEL + lo_x,
            LABEL + hi_x,
            y + 9.0,
            y + 9.0,
            LABEL + hi_x.max(bar) + 8.0,
            y + 13.0,
            format_ns(*median)
        );
    }
    svg.push_str("</svg>");
    svg
}

/// Horizontal bars of the percentage change per benchmark, with the noise band shaded.
pub(crate) fn delta_chart(deltas: &[&Delta]) -> String {
    const ROW: f64 = 22.0;
    const LABEL: f64 = DELTA_LABEL;
    let span = deltas
        .iter()
        .map(|d| d.change_pct.abs())
        .fold(NOISE_PCT * 4.0, f64::max)
        .ceil();
    let track = WIDTH - LABEL - 24.0;
    let zero = LABEL + track / 2.0;
    let scale = (track / 2.0 - DELTA_VALUE_ROOM) / span;
    #[allow(clippy::cast_precision_loss, reason = "a handful of rows")]
    let height = deltas.len() as f64 * ROW + 12.0;
    let mut svg = format!(
        "<svg viewBox=\"0 0 {WIDTH} {height}\" role=\"img\" aria-label=\"change between baselines\" class=\"chart\">\
         <rect class=\"noise-band\" x=\"{:.1}\" y=\"0\" width=\"{:.1}\" height=\"{height}\"/>\
         <line class=\"axis\" x1=\"{zero:.1}\" x2=\"{zero:.1}\" y1=\"0\" y2=\"{height}\"/>",
        zero - NOISE_PCT * scale,
        2.0 * NOISE_PCT * scale
    );
    for (i, d) in deltas.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "a handful of rows")]
        let y = 6.0 + i as f64 * ROW;
        let width = (d.change_pct.abs() * scale).max(1.0);
        let x = if d.change_pct >= 0.0 {
            zero
        } else {
            zero - width
        };
        let class = match d.verdict {
            Verdict::Faster => "faster",
            Verdict::Slower => "slower",
            Verdict::WithinNoise => "noise",
        };
        let label = match d.x {
            Some(x) => format!("{} at {}", d.series, format_count(x)),
            None => d.series.clone(),
        };
        let text_x = if d.change_pct >= 0.0 {
            x + width + 6.0
        } else {
            x - 6.0
        };
        let anchor = if d.change_pct >= 0.0 { "start" } else { "end" };
        let _ = write!(
            svg,
            "<text class=\"label\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\">{}</text>\
             <rect class=\"delta {class}\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{width:.1}\" height=\"14\"><title>{}: {} to {}</title></rect>\
             <text class=\"value\" x=\"{text_x:.1}\" y=\"{:.1}\" text-anchor=\"{anchor}\">{:+.1}%</text>",
            LABEL - 8.0,
            y + 11.0,
            escape(&label),
            escape(&label),
            format_ns(d.old_ns),
            format_ns(d.new_ns),
            y + 11.0,
            d.change_pct
        );
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench_report::load::Point;
    use crate::bench_report::model::{self, Verdict};

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
            elements: Some(1),
            median_ns: median,
            lower_ns: median * 0.95,
            upper_ns: median * 1.05,
        }
    }

    fn scaling_group() -> Group {
        let points = [
            point("g", "engine_handle", Some("u64"), Some(1_000.0), 2.0),
            point("g", "engine_handle", Some("u64"), Some(100_000.0), 3.0),
            point("g", "engine_handle", Some("u64"), Some(1_000_000.0), 20.0),
            point("g", "slotmap", Some("u64"), Some(1_000.0), 1.0),
            point("g", "slotmap", Some("u64"), Some(1_000_000.0), 9.0),
        ];
        model::group(&points).remove(0)
    }

    fn colors() -> Colors {
        Colors::new(vec!["engine_handle".into(), "slotmap".into()])
    }

    #[test]
    fn formats_times_and_counts() {
        assert_eq!(format_ns(0.123), "0.12 ns");
        assert_eq!(format_ns(950.0), "950 ns");
        assert_eq!(format_ns(1_500.0), "1.50 \u{b5}s");
        assert_eq!(format_ns(2_500_000.0), "2.50 ms");
        assert_eq!(format_ns(3_000_000_000.0), "3.00 s");
        assert_eq!(format_count(1_000.0), "1K");
        assert_eq!(format_count(100_000.0), "100K");
        assert_eq!(format_count(1_000_000.0), "1M");
        assert_eq!(format_count(750.0), "750");
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(escape("a<b&\"c\">"), "a&lt;b&amp;&quot;c&quot;&gt;");
    }

    #[test]
    fn implementations_that_share_a_chart_never_share_a_colour() {
        // 12 implementations, far more than the 8 colours, but never more than 5 per group.
        let mut points = Vec::new();
        for (group, members) in [
            ("g1", ["a", "b", "c", "d", "e"]),
            ("g2", ["c", "d", "e", "f", "g"]),
            ("g3", ["e", "f", "g", "h", "i"]),
            ("g4", ["g", "h", "i", "j", "k"]),
            ("g5", ["i", "j", "k", "l", "a"]),
        ] {
            for m in members {
                points.push(point(group, m, None, None, 1.0));
            }
        }
        let colors = Colors::for_points(&points);
        for group in model::group(&points) {
            let mut classes: Vec<String> = group
                .series
                .iter()
                .map(|s| colors.class(&s.implementation))
                .collect();
            let total = classes.len();
            classes.sort();
            classes.dedup();
            assert_eq!(classes.len(), total, "colour clash in {}", group.name);
        }
    }

    #[test]
    fn the_most_widely_compared_implementations_get_the_first_colours() {
        let points = [
            point("g1", "rare", None, None, 1.0),
            point("g1", "common", None, None, 1.0),
            point("g2", "common", None, None, 1.0),
            point("g3", "common", None, None, 1.0),
            point("g2", "mid", None, None, 1.0),
            point("g3", "mid", None, None, 1.0),
        ];
        let colors = Colors::for_points(&points);
        assert_eq!(colors.class("common"), "s0");
        assert_eq!(colors.class("mid"), "s1");
        assert_eq!(colors.class("rare"), "s1"); // never shares a chart with `mid`, so it may reuse s1
        // Deterministic.
        assert_eq!(Colors::for_points(&points).class("mid"), "s1");
    }

    #[test]
    fn colours_are_stable_per_implementation() {
        let c = colors();
        assert_eq!(c.class("engine_handle"), "s0");
        assert_eq!(c.class("slotmap"), "s1");
        assert_eq!(c.class("unknown"), "s7");
    }

    #[test]
    fn scaling_chart_draws_one_line_per_series_and_a_marker_per_point() {
        let svg = scaling_chart(&scaling_group(), &colors());
        assert!(svg.starts_with("<svg"), "{svg}");
        assert!(svg.contains("role=\"img\""));
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert_eq!(svg.matches("<circle").count(), 5);
        assert_eq!(svg.matches("<polygon").count(), 2); // one confidence band per series
        assert!(svg.contains(">1K<") && svg.contains(">1M<"), "x ticks");
        assert!(svg.contains("class=\"line s0 d0\"") && svg.contains("class=\"line s1 d0\""));
    }

    #[test]
    fn scaling_chart_is_finite_and_deterministic() {
        let a = scaling_chart(&scaling_group(), &colors());
        let b = scaling_chart(&scaling_group(), &colors());
        assert_eq!(a, b);
        for bad in ["NaN", "inf"] {
            assert!(!a.contains(bad), "found {bad}");
        }
    }

    #[test]
    fn scaling_chart_survives_degenerate_data() {
        // A single point, and a zero-time measurement, must not produce NaN or infinity.
        let points = [point("g", "f", None, Some(10.0), 0.0)];
        let svg = scaling_chart(&model::group(&points).remove(0), &colors());
        for bad in ["NaN", "inf"] {
            assert!(!svg.contains(bad), "found {bad} in {svg}");
        }
        assert_eq!(svg.matches("<circle").count(), 1);
    }

    #[test]
    fn names_are_escaped_in_tooltips() {
        let points = [
            point("g", "a<b&c", None, Some(1.0), 1.0),
            point("g", "a<b&c", None, Some(2.0), 1.0),
        ];
        let svg = scaling_chart(&model::group(&points).remove(0), &colors());
        assert!(svg.contains("a&lt;b&amp;c"));
        assert!(!svg.contains("a<b&c"));
    }

    #[test]
    fn bar_chart_has_one_bar_per_series() {
        let points = [
            point("g", "engine_handle", None, None, 5.0),
            point("g", "slotmap", None, None, 10.0),
        ];
        let svg = bar_chart(&model::group(&points).remove(0), &colors());
        assert_eq!(svg.matches("<rect class=\"bar").count(), 2);
        assert!(svg.contains("5.00 ns") && svg.contains("10.0 ns"));
        assert!(!svg.contains("NaN"));
    }

    #[test]
    fn the_longest_delta_bar_leaves_room_for_its_value_label() {
        let mk = |change_pct: f64, verdict| Delta {
            group: "g".into(),
            series: "f".into(),
            x: None,
            old_ns: 100.0,
            new_ns: 100.0 + change_pct,
            change_pct,
            verdict,
        };
        let svg = delta_chart(&[&mk(-80.0, Verdict::Faster), &mk(80.0, Verdict::Slower)]);
        let x_of = |class: &str| -> f64 {
            let at = svg.find(&format!("class=\"delta {class}\" x=\"")).unwrap();
            let rest = &svg[at..];
            let start = rest.find("x=\"").unwrap() + 3;
            rest[start..start + rest[start..].find('"').unwrap()]
                .parse()
                .unwrap()
        };
        let width_of = |class: &str| -> f64 {
            let at = svg.find(&format!("class=\"delta {class}\"")).unwrap();
            let rest = &svg[at..];
            let start = rest.find("width=\"").unwrap() + 7;
            rest[start..start + rest[start..].find('"').unwrap()]
                .parse()
                .unwrap()
        };
        // The value text (about 50px) must fit between the row label and the faster bar, and
        // between the slower bar and the right edge.
        assert!(
            x_of("faster") >= DELTA_LABEL + 50.0,
            "left bar starts at {}",
            x_of("faster")
        );
        assert!(
            x_of("slower") + width_of("slower") + 50.0 <= WIDTH,
            "right bar ends at {}",
            x_of("slower") + width_of("slower")
        );
    }

    #[test]
    fn delta_chart_shades_the_noise_band_and_labels_changes() {
        let slower = Delta {
            group: "g".into(),
            series: "f".into(),
            x: Some(1000.0),
            old_ns: 100.0,
            new_ns: 120.0,
            change_pct: 20.0,
            verdict: Verdict::Slower,
        };
        let noise = Delta {
            change_pct: 1.0,
            verdict: Verdict::WithinNoise,
            ..slower.clone()
        };
        let svg = delta_chart(&[&slower, &noise]);
        assert!(svg.contains("class=\"noise-band\""));
        assert!(svg.contains("class=\"delta slower\"") && svg.contains("class=\"delta noise\""));
        assert!(svg.contains("+20.0%") && svg.contains("+1.0%"));
        assert!(!svg.contains("NaN"));
    }
}
