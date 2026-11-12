//! Assembles the report page: summary, per-group charts and tables, all inline.

use std::fmt::Write as _;

use super::load::Point;
use super::model::{self, Comparison, Delta, Group, NOISE_PCT, Verdict};
use super::svg::{self, Colors, escape, format_count, format_ns};

/// What the page shows.
pub(crate) struct Input<'a> {
    pub(crate) title: &'a str,
    pub(crate) baseline: &'a str,
    pub(crate) points: &'a [Point],
    /// The baseline compared against, and the comparison, when `--against` was given.
    pub(crate) comparison: Option<(&'a str, &'a Comparison)>,
    /// Free text shown verbatim at the top (machine, load, conditions).
    pub(crate) notes: Option<&'a str>,
}

const STYLE: &str = r"
:root{--bg:#fff;--fg:#1b1f24;--muted:#5b6670;--grid:#d8dee4;--card:#f6f8fa;--band:rgba(154,164,175,.2);
--c0:#0072b2;--c1:#e69f00;--c2:#009e73;--c3:#cc79a7;--c4:#56b4e9;--c5:#d55e00;--c6:#8a7f00;--c7:#6b7280;
--faster:#0072b2;--slower:#d55e00;--noise:#9aa4af}
@media (prefers-color-scheme: dark){:root{--bg:#0d1117;--fg:#e6edf3;--muted:#9aa4af;--grid:#2d333b;--card:#161b22;--band:rgba(154,164,175,.18);
--c0:#4aa3df;--c1:#f0b429;--c2:#34c79a;--c3:#e08fc0;--c4:#7cc4f0;--c5:#ff8a4c;--c6:#d4c84a;--c7:#9aa4af;
--faster:#4aa3df;--slower:#ff8a4c;--noise:#6b7280}}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,sans-serif}
main{max-width:1100px;margin:0 auto;padding:24px 16px 64px}
h1{font-size:1.6rem;margin:0 0 4px}h2{margin:36px 0 8px;font-size:1.25rem}h3{margin:28px 0 6px;font-size:1.05rem}
p,li{max-width:70ch}.muted{color:var(--muted)}code{font-family:ui-monospace,monospace;font-size:.92em}
nav ul{columns:3 220px;list-style:none;padding:0;margin:8px 0}nav a{color:inherit}
pre.notes{background:var(--card);padding:10px 12px;overflow:auto;border-radius:6px}
.summary{display:flex;gap:12px;flex-wrap:wrap;margin:8px 0}
.summary span{background:var(--card);padding:4px 10px;border-radius:6px}
.legend{display:flex;gap:14px;flex-wrap:wrap;margin:4px 0 6px}
.swatch{display:inline-block;width:12px;height:12px;border-radius:2px;margin-right:6px;vertical-align:-1px;background:var(--c)}
.s0{--c:var(--c0)}.s1{--c:var(--c1)}.s2{--c:var(--c2)}.s3{--c:var(--c3)}.s4{--c:var(--c4)}.s5{--c:var(--c5)}.s6{--c:var(--c6)}.s7{--c:var(--c7)}
svg.chart{width:100%;height:auto;max-width:720px;display:block}
.line{fill:none;stroke:var(--c);stroke-width:2}.d1{stroke-dasharray:6 3}.d2{stroke-dasharray:2 3}.d3{stroke-dasharray:8 3 2 3}
.band{fill:var(--c);opacity:.14;stroke:none}.dot{fill:var(--c)}.bar{fill:var(--c)}
.grid{stroke:var(--grid);stroke-width:1}.axis{stroke:var(--muted);stroke-width:1}.whisker{stroke:var(--fg);stroke-width:1.5}
.tick,.label,.value,.axis-label{fill:var(--muted);font:11px system-ui,sans-serif}
.noise-band{fill:var(--band)}.delta.faster{fill:var(--faster)}.delta.slower{fill:var(--slower)}.delta.noise{fill:var(--noise)}
details{margin:6px 0}summary{cursor:pointer;color:var(--muted)}
table{border-collapse:collapse;margin:6px 0;font-variant-numeric:tabular-nums}
th,td{padding:3px 10px;text-align:right;border-bottom:1px solid var(--grid)}th:first-child,td:first-child{text-align:left}
td.faster{color:var(--faster)}td.slower{color:var(--slower)}
";

/// A URL-fragment-safe id for a group name.
fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn legend(group: &Group, colors: &Colors) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::from("<div class=\"legend\">");
    for series in &group.series {
        if seen.contains(&series.implementation.as_str()) {
            continue;
        }
        seen.push(&series.implementation);
        let _ = write!(
            out,
            "<span><i class=\"swatch {}\"></i>{}</span>",
            colors.class(&series.implementation),
            escape(&series.implementation)
        );
    }
    let variants = svg::distinct_variants(group);
    if variants.len() > 1 {
        let styles = ["solid", "dashed", "dotted", "dash-dot"];
        let described: Vec<String> = variants
            .iter()
            .enumerate()
            .map(|(i, v)| {
                format!(
                    "{} = {}",
                    styles[i % styles.len()],
                    escape(v.as_deref().unwrap_or("default"))
                )
            })
            .collect();
        let _ = write!(
            out,
            "<span class=\"muted\">line style: {}</span>",
            described.join(", ")
        );
    }
    out.push_str("</div>");
    out
}

fn results_section(group: &Group, colors: &Colors) -> String {
    let sized = group
        .series
        .iter()
        .any(|s| s.points.iter().any(|p| p.x.is_some()));
    let chart = if sized {
        svg::scaling_chart(group, colors)
    } else {
        svg::bar_chart(group, colors)
    };
    let mut out = format!(
        "<section><h3 id=\"{}\">{}</h3>{}{}<details><summary>numbers</summary><table><tr><th>series</th><th>size</th><th>per element</th><th>95% interval</th><th>per iteration</th></tr>",
        slug(&group.name),
        escape(&group.name),
        legend(group, colors),
        chart
    );
    for series in &group.series {
        for p in &series.points {
            let (median, lo, hi) = p.per_element();
            let _ = write!(
                out,
                "<tr class=\"row\"><td>{}</td><td>{}</td><td>{}</td><td>{} to {}</td><td>{}</td></tr>",
                escape(&series.name),
                p.x.map_or_else(|| "-".to_string(), format_count),
                format_ns(median),
                format_ns(lo),
                format_ns(hi),
                format_ns(p.median_ns)
            );
        }
    }
    out.push_str("</table></details></section>");
    out
}

fn changes_section(baseline: &str, against: &str, comparison: &Comparison) -> String {
    let count = |v: Verdict| comparison.deltas.iter().filter(|d| d.verdict == v).count();
    let mut out = format!(
        "<section id=\"changes\"><h2>Changes from {} to {}</h2><div class=\"summary\"><span>{} faster</span><span>{} slower</span><span>{} within noise</span></div><p class=\"muted\">A change counts only if it exceeds {NOISE_PCT}% and the 95% intervals do not overlap; anything else is within noise. Blue is faster, orange is slower, grey is noise; the shaded band is &plusmn;{NOISE_PCT}%. If implementations you did not change also moved, the machine drifted between runs: treat the comparison with suspicion and re-run.</p>",
        escape(against),
        escape(baseline),
        count(Verdict::Faster),
        count(Verdict::Slower),
        count(Verdict::WithinNoise)
    );
    let mut groups: Vec<&str> = comparison.deltas.iter().map(|d| d.group.as_str()).collect();
    groups.dedup();
    let mut unchanged = 0;
    for name in groups {
        let mut deltas: Vec<&Delta> = comparison
            .deltas
            .iter()
            .filter(|d| d.group == name)
            .collect();
        if deltas.iter().all(|d| d.verdict == Verdict::WithinNoise) {
            unchanged += 1;
            continue;
        }
        deltas.sort_by(|a, b| a.change_pct.total_cmp(&b.change_pct));
        let _ = write!(
            out,
            "<h3>{}</h3>{}",
            escape(name),
            svg::delta_chart(&deltas)
        );
    }
    if unchanged > 0 {
        let _ = write!(
            out,
            "<p class=\"muted\">{unchanged} group(s) had no change beyond noise.</p>"
        );
    }
    out.push_str("<details><summary>every comparison</summary><table><tr><th>benchmark</th><th>old</th><th>new</th><th>change</th><th>verdict</th></tr>");
    for d in &comparison.deltas {
        let (class, word) = match d.verdict {
            Verdict::Faster => ("faster", "faster"),
            Verdict::Slower => ("slower", "slower"),
            Verdict::WithinNoise => ("", "within noise"),
        };
        let label = match d.x {
            Some(x) => format!("{}/{} at {}", d.group, d.series, format_count(x)),
            None => format!("{}/{}", d.group, d.series),
        };
        let _ = write!(
            out,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"{class}\">{:+.1}%</td><td class=\"{class}\">{word}</td></tr>",
            escape(&label),
            format_ns(d.old_ns),
            format_ns(d.new_ns),
            d.change_pct
        );
    }
    out.push_str("</table></details>");
    for (name, ids) in [
        (against, &comparison.only_old),
        (baseline, &comparison.only_new),
    ] {
        if !ids.is_empty() {
            let _ = write!(
                out,
                "<p class=\"muted\">only in {} ({}): {}</p>",
                escape(name),
                ids.len(),
                escape(&ids.join(", "))
            );
        }
    }
    out.push_str("</section>");
    out
}

/// Renders the whole page.
pub(crate) fn render(input: &Input<'_>) -> String {
    let groups = model::group(input.points);
    let colors = Colors::for_points(input.points);
    let mut out = format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} \u{b7} {}</title><style>{STYLE}</style></head><body><main><h1>{}</h1><p class=\"muted\">Baseline <code>{}</code>. Times are medians per element with 95% confidence intervals.</p>",
        escape(input.title),
        escape(input.baseline),
        escape(input.title),
        escape(input.baseline)
    );
    if let Some(notes) = input.notes {
        let _ = write!(out, "<pre class=\"notes\">{}</pre>", escape(notes));
    }
    out.push_str("<nav><ul>");
    if input.comparison.is_some() {
        out.push_str("<li><a href=\"#changes\">Changes</a></li>");
    }
    for group in &groups {
        let _ = write!(
            out,
            "<li><a href=\"#{}\">{}</a></li>",
            slug(&group.name),
            escape(&group.name)
        );
    }
    out.push_str("</ul></nav>");
    if let Some((against, comparison)) = input.comparison {
        out.push_str(&changes_section(input.baseline, against, comparison));
    }
    out.push_str("<h2>Results</h2>");
    for group in &groups {
        out.push_str(&results_section(group, &colors));
    }
    out.push_str("</main></body></html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench_report::model;

    fn point(group: &str, implementation: &str, x: Option<f64>, median: f64) -> Point {
        Point {
            group: group.to_string(),
            implementation: implementation.to_string(),
            variant: None,
            x,
            elements: Some(10),
            median_ns: median,
            lower_ns: median * 0.9,
            upper_ns: median * 1.1,
        }
    }

    fn sample() -> Vec<Point> {
        vec![
            point("get_random", "engine_handle", Some(1_000.0), 500.0),
            point("get_random", "engine_handle", Some(100_000.0), 900.0),
            point("get_random", "slotmap", Some(1_000.0), 400.0),
            point("prng/next_u64", "fastrand", None, 10.0),
            point("prng/next_u64", "rand_xoshiro", None, 20.0),
        ]
    }

    fn page(points: &[Point]) -> String {
        render(&Input {
            title: "Benchmarks",
            baseline: "m1",
            points,
            comparison: None,
            notes: None,
        })
    }

    #[test]
    fn page_is_a_complete_document() {
        let html = page(&sample());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("name=\"viewport\""));
        assert!(html.contains("<title>Benchmarks"));
        assert!(html.contains("prefers-color-scheme: dark"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn every_group_gets_a_heading_a_chart_and_a_table_row_per_measurement() {
        let html = page(&sample());
        assert!(html.contains("id=\"get_random\"") && html.contains("id=\"prng-next_u64\""));
        assert_eq!(html.matches("<svg").count(), 2);
        assert_eq!(html.matches("<polyline").count(), 2); // sized group: a line per series
        assert_eq!(html.matches("<rect class=\"bar").count(), 2); // unsized group: a bar per series
        assert_eq!(html.matches("<tr class=\"row\"").count(), 5);
    }

    #[test]
    fn legend_names_each_implementation_with_its_colour_class() {
        let html = page(&sample());
        assert!(html.contains("class=\"swatch s0\"") || html.contains("class=\"swatch s1\""));
        assert!(html.contains(">engine_handle<") && html.contains(">slotmap<"));
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(page(&sample()), page(&sample()));
    }

    #[test]
    fn title_and_notes_are_escaped() {
        let html = render(&Input {
            title: "<b>x</b>",
            baseline: "m1",
            points: &sample(),
            comparison: None,
            notes: Some("machine <M2> & 'load'"),
        });
        assert!(html.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(html.contains("machine &lt;M2&gt; &amp;"));
        assert!(!html.contains("<b>x</b>"));
    }

    #[test]
    fn without_a_comparison_there_is_no_changes_section() {
        assert!(!page(&sample()).contains("id=\"changes\""));
    }

    #[test]
    fn a_comparison_adds_a_summary_and_a_delta_chart_for_changed_groups() {
        let old = vec![
            point("get_random", "engine_handle", Some(1_000.0), 1000.0),
            point("get_random", "slotmap", Some(1_000.0), 400.0),
            point("gone", "x", None, 1.0),
        ];
        let new = vec![
            point("get_random", "engine_handle", Some(1_000.0), 500.0), // 50% faster
            point("get_random", "slotmap", Some(1_000.0), 401.0),       // noise
            point("added", "y", None, 1.0),
        ];
        let comparison = model::compare(&old, &new);
        let html = render(&Input {
            title: "T",
            baseline: "new",
            points: &new,
            comparison: Some(("old", &comparison)),
            notes: None,
        });
        assert!(html.contains("id=\"changes\""));
        assert!(
            html.contains("1 faster")
                && html.contains("0 slower")
                && html.contains("1 within noise")
        );
        assert!(html.contains("class=\"delta faster\""));
        assert!(html.contains("only in old") && html.contains("only in new"));
        assert!(html.contains("gone") && html.contains("added"));
    }
}
