//! Required READMEs, their shape and their Contents (spec M3.16 §4.1 and §4.2).

use std::collections::{BTreeMap, BTreeSet};

use super::markdown::{self, Doc};
use super::{Problem, REQUIRED_READMES, glob, links};

const FIX_MISSING: &str = "add one in the shape of docs/README.md Conventions";

/// Every README that must exist (`REQUIRED_READMES` and one per `spikes/<topic>/`), checked for
/// shape and for a Contents that matches the directory.
pub(super) fn check(files: &BTreeSet<String>, texts: &BTreeMap<String, String>) -> Vec<Problem> {
    let mut dirs: Vec<String> = REQUIRED_READMES.iter().map(|d| (*d).to_string()).collect();
    for file in files {
        if let Some((topic, _)) = file
            .strip_prefix("spikes/")
            .and_then(|rest| rest.split_once('/'))
        {
            let dir = format!("spikes/{topic}");
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    let mut problems = Vec::new();
    for dir in &dirs {
        let path = format!("{dir}/README.md");
        if !files.contains(&path) {
            problems.push(Problem::new(
                &path,
                None,
                &format!("`{path}` is missing; {FIX_MISSING}"),
            ));
        } else if let Some(text) = texts.get(&path) {
            problems.extend(check_readme(dir, &path, text, files));
        }
    }
    problems
}

/// The shape of one README and, when it has a `Contents` section, that section against `dir`.
fn check_readme(dir: &str, path: &str, text: &str, files: &BTreeSet<String>) -> Vec<Problem> {
    let doc = markdown::parse(text);
    let mut problems = check_shape(path, text, &doc);
    let level2: Vec<&markdown::Heading> = doc.headings.iter().filter(|h| h.level == 2).collect();
    if let Some(index) = level2.iter().position(|h| h.text.trim() == "Contents") {
        let start = level2[index].line;
        let end = level2.get(index + 1).map_or(usize::MAX, |h| h.line);
        problems.extend(check_contents(dir, path, &doc, start, end, files));
    }
    problems
}

fn check_shape(path: &str, text: &str, doc: &Doc) -> Vec<Problem> {
    let mut problems = Vec::new();
    if doc.headings.first().is_none_or(|h| h.level != 1) {
        problems.push(Problem::new(
            path,
            None,
            "the README must start with a level-1 heading; add `# <title>` first",
        ));
    }
    let level2: Vec<&markdown::Heading> = doc.headings.iter().filter(|h| h.level == 2).collect();
    let first_level2 = level2.first().map_or(usize::MAX, |h| h.line);
    let status = text
        .lines()
        .enumerate()
        .take(first_level2.saturating_sub(1))
        .find_map(|(i, l)| {
            l.strip_prefix("Status:")
                .or_else(|| l.strip_prefix("- **Status:**"))
                .map(|value| (i + 1, value.trim_start()))
        });
    match status {
        None => problems.push(Problem::new(
            path,
            None,
            "no status line before the first `##` heading; add `Status: Living (<date>)`",
        )),
        Some((line, value)) if !value.starts_with("Living (") => problems.push(Problem::new(
            path,
            Some(line),
            &format!(
                "status must be `Living (<date>)` for a README; found `{}`",
                value.trim()
            ),
        )),
        Some(_) => {}
    }
    let position = |name: &str| level2.iter().position(|h| h.text.trim() == name);
    let (when, contents) = (position("When this applies"), position("Contents"));
    if when.is_none() {
        problems.push(Problem::new(
            path,
            None,
            "no `When this applies` section; add a `## When this applies` heading before `## Contents`",
        ));
    }
    if contents.is_none() {
        problems.push(Problem::new(
            path,
            None,
            "no `Contents` section; add a `## Contents` heading after `## When this applies`",
        ));
    }
    if let (Some(w), Some(c)) = (when, contents)
        && c < w
    {
        problems.push(Problem::new(
            path,
            Some(level2[c].line),
            "`Contents` comes before `When this applies`; put `When this applies` first",
        ));
    }
    problems
}

/// A Contents name reduced to a path relative to the README: no `#anchor`, no trailing `/`.
/// `None` for a cross-reference (`../`, `/`, a URL scheme) or a name with no path.
fn normalize(name: &str) -> Option<String> {
    let name = name.split('#').next().unwrap_or("");
    let name = name.trim_end_matches('/');
    let name = name.strip_prefix("./").unwrap_or(name);
    if name.is_empty()
        || name.starts_with("../")
        || name.starts_with('/')
        || links::has_scheme(name)
    {
        None
    } else {
        Some(name.to_string())
    }
}

/// Every tracked file and directory under `dir`, relative to it (`a`, `a/b`, `a/b/c.rs`).
fn tracked_paths(dir: &str, files: &BTreeSet<String>) -> BTreeSet<String> {
    let prefix = format!("{dir}/");
    let mut paths = BTreeSet::new();
    for rest in files.iter().filter_map(|f| f.strip_prefix(&prefix)) {
        let mut end = 0;
        for segment in rest.split('/') {
            end += segment.len();
            paths.insert(rest[..end].to_string());
            end += 1;
        }
    }
    paths
}

fn check_contents(
    dir: &str,
    path: &str,
    doc: &Doc,
    start: usize,
    end: usize,
    files: &BTreeSet<String>,
) -> Vec<Problem> {
    let paths = tracked_paths(dir, files);
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    let mut problems = Vec::new();
    let entries = doc
        .entries
        .iter()
        .filter(|e| e.line > start && e.line < end);
    for entry in entries {
        let Some(name) = entry.name.as_deref().and_then(normalize) else {
            continue;
        };
        let matched: Vec<&String> = if name.contains('*') {
            paths.iter().filter(|p| glob::matches(&name, p)).collect()
        } else {
            paths.get(&name).into_iter().collect()
        };
        if matched.is_empty() {
            let message = if name.contains('*') {
                format!(
                    "`Contents` wildcard `{name}` matches nothing; fix the pattern or remove the entry"
                )
            } else {
                format!(
                    "`Contents` entry `{name}` is not a tracked file or directory; fix the name or remove the entry"
                )
            };
            problems.push(Problem::new(path, Some(entry.line), &message));
        }
        covered.extend(
            matched
                .iter()
                .map(|p| p.split('/').next().unwrap_or(p.as_str())),
        );
    }
    for child in paths.iter().filter(|p| !p.contains('/')) {
        if child != "README.md" && !covered.contains(child.as_str()) {
            let shown = if paths.iter().any(|p| p.starts_with(&format!("{child}/"))) {
                format!("{child}/")
            } else {
                child.clone()
            };
            problems.push(Problem::new(
                path,
                None,
                &format!("`Contents` does not list `{shown}`; add an entry or a wildcard"),
            ));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    /// A well-formed README for `dir` whose Contents section is `contents`.
    fn readme(contents: &str) -> String {
        format!(
            "# Title\n\nStatus: Living (2026-11-15)\n\n## When this applies\n\nx\n\n## Contents\n\n{contents}\n"
        )
    }

    /// Problems of `check` on a tree, as strings.
    fn run(entries: &[(&str, &str)]) -> Vec<String> {
        let (files, texts) = fixture(entries);
        check(&files, &texts)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Problems for the README of `docs/` only (the other required ones are missing).
    fn docs(entries: &[(&str, &str)]) -> Vec<String> {
        run(entries)
            .into_iter()
            .filter(|p| p.starts_with("docs/README.md"))
            .collect()
    }

    #[test]
    fn a_missing_required_readme_is_reported() {
        let problems = run(&[("bench/x.rs", "")]);
        assert!(
            problems.contains(
                &"bench/README.md: `bench/README.md` is missing; add one in the shape of docs/README.md Conventions"
                    .to_string()
            ),
            "{problems:#?}"
        );
        assert_eq!(problems.len(), 12, "{problems:#?}");
    }

    #[test]
    fn every_spikes_topic_with_tracked_files_needs_a_readme() {
        let problems = run(&[
            ("spikes/a/src/lib.rs", ""),
            ("spikes/b/README.md", &readme("")),
            ("spikes/loose.txt", ""),
        ]);
        assert!(
            problems
                .iter()
                .any(|p| p.starts_with("spikes/a/README.md: `spikes/a/README.md` is missing")),
            "{problems:#?}"
        );
        assert!(
            !problems
                .iter()
                .any(|p| p.starts_with("spikes/b/README.md: `spikes/b/README.md` is missing"))
        );
        assert!(
            !problems
                .iter()
                .any(|p| p.contains("spikes/loose") && p.contains("missing"))
        );
    }

    #[test]
    fn a_well_formed_readme_reports_nothing() {
        let r = readme("- `a.md`\n- [`sub/`](sub/)\n");
        assert_eq!(
            docs(&[
                ("docs/README.md", &r),
                ("docs/a.md", ""),
                ("docs/sub/x.md", "")
            ]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_missing_status_line_is_reported() {
        let r = "# T\n\n## When this applies\n\nx\n\n## Contents\n\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].contains("status line"), "{}", p[0]);
    }

    #[test]
    fn a_bold_status_bullet_counts_as_a_status_line() {
        let r = "# T\n\n- **Status:** Living (2026-11-15)\n\n## When this applies\n\nx\n\n## Contents\n\n";
        assert_eq!(docs(&[("docs/README.md", r)]), Vec::<String>::new());
    }

    #[test]
    fn a_status_other_than_living_is_reported() {
        let r =
            "# T\n\nStatus: Accepted (2026-11-14)\n\n## When this applies\n\nx\n\n## Contents\n\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(
            p[0].contains("status must be `Living (<date>)`")
                && p[0].contains("found `Accepted (2026-11-14)`"),
            "{}",
            p[0]
        );
    }

    #[test]
    fn any_spacing_after_the_status_colon_passes() {
        for status in [
            "Status:Living (2026-11-15)",
            "Status:  Living (2026-11-15)",
            "- **Status:**Living (2026-11-15)",
        ] {
            let r = format!("# T\n\n{status}\n\n## When this applies\n\nx\n\n## Contents\n\n");
            assert_eq!(
                docs(&[("docs/README.md", &r)]),
                Vec::<String>::new(),
                "{status}"
            );
        }
    }

    #[test]
    fn a_wrong_status_without_a_space_is_quoted_as_written() {
        let r = "# T\n\nStatus:Accepted (x)\n\n## When this applies\n\nx\n\n## Contents\n\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].contains("found `Accepted (x)`"), "{}", p[0]);
    }

    #[test]
    fn a_wrong_status_is_reported_at_its_line() {
        let r = "# T\n\nStatus: Accepted (x)\n\n## When this applies\n\nx\n\n## Contents\n\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].starts_with("docs/README.md:3: "), "{}", p[0]);
    }

    #[test]
    fn a_living_status_with_trailing_text_passes() {
        let r = "# T\n\nStatus: Living (2026-11-15). Text\n\n## When this applies\n\nx\n\n## Contents\n\n";
        assert_eq!(docs(&[("docs/README.md", r)]), Vec::<String>::new());
    }

    #[test]
    fn contents_before_when_this_applies_is_reported() {
        let r = "# T\n\nStatus: Living (2026-11-15)\n\n## Contents\n\n## When this applies\n\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(
            p[0].contains("`When this applies`") && p[0].contains("before"),
            "{}",
            p[0]
        );
    }

    #[test]
    fn a_missing_contents_is_reported() {
        let r = "# T\n\nStatus: Living (2026-11-15)\n\n## When this applies\n\nx\n";
        let p = docs(&[("docs/README.md", r)]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].contains("no `Contents` section"), "{}", p[0]);
    }

    #[test]
    fn an_uncovered_child_is_reported() {
        let p = docs(&[
            ("docs/README.md", &readme("- `a.md`\n")),
            ("docs/a.md", ""),
            ("docs/foo.rs", ""),
            ("docs/d/x", ""),
        ]);
        assert_eq!(
            p,
            [
                "docs/README.md: `Contents` does not list `d/`; add an entry or a wildcard",
                "docs/README.md: `Contents` does not list `foo.rs`; add an entry or a wildcard",
            ]
        );
    }

    #[test]
    fn an_entry_naming_nothing_is_reported_at_its_line() {
        let p = docs(&[("docs/README.md", &readme("- `gone.md`\n"))]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].starts_with("docs/README.md:11: "), "{}", p[0]);
        assert!(
            p[0].contains("`gone.md`") && p[0].contains("remove the entry"),
            "{}",
            p[0]
        );
    }

    #[test]
    fn a_wildcard_matching_nothing_is_reported() {
        let p = docs(&[("docs/README.md", &readme("- `m*/`\n"))]);
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(
            p[0].contains("`m*`") && p[0].contains("matches nothing"),
            "{}",
            p[0]
        );
    }

    #[test]
    fn a_wildcard_covers_every_match_and_stays_within_a_segment() {
        let r = readme("- `_template/`\n- `m*/`\n");
        let ok = docs(&[
            ("docs/README.md", &r),
            ("docs/_template/a", ""),
            ("docs/m0/a", ""),
            ("docs/m1/a", ""),
            ("docs/m2/a", ""),
        ]);
        assert_eq!(ok, Vec::<String>::new());
        let p = docs(&[
            ("docs/README.md", &r),
            ("docs/_template/a", ""),
            ("docs/m0/a", ""),
            ("docs/other/m1", ""),
        ]);
        assert_eq!(
            p,
            ["docs/README.md: `Contents` does not list `other/`; add an entry or a wildcard"]
        );
    }

    #[test]
    fn a_table_row_with_a_link_around_a_code_span_names_the_link_target() {
        let r = readme("| Path | Holds |\n|---|---|\n| [`a.md`](a.md) | x |\n| `sub/` | y |\n");
        assert_eq!(
            docs(&[
                ("docs/README.md", &r),
                ("docs/a.md", ""),
                ("docs/sub/x", "")
            ]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_table_header_row_is_not_an_entry() {
        let r = readme("| `Path` | Holds |\n|---|---|\n| `a.md` | x |\n");
        assert_eq!(
            docs(&[("docs/README.md", &r), ("docs/a.md", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_multi_segment_entry_covers_its_first_segment() {
        let r = readme("- `skills/chunk/`\n");
        assert_eq!(
            docs(&[("docs/README.md", &r), ("docs/skills/chunk/SKILL.md", "")]),
            Vec::<String>::new()
        );
        let p = docs(&[("docs/README.md", &r), ("docs/skills/other/SKILL.md", "")]);
        assert_eq!(p.len(), 2, "{p:#?}");
    }

    #[test]
    fn cross_references_and_prose_bullets_are_ignored() {
        let r = readme(
            "- `a.md`\n- see `../x.md`\n- [site](https://example.com/x)\n- `/abs.md`\n- plain prose with no name\n",
        );
        assert_eq!(
            docs(&[("docs/README.md", &r), ("docs/a.md", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn sections_after_contents_are_allowed_and_their_bullets_are_not_entries() {
        let r = format!("{}\n## How to\n\n- `nothing.md`\n", readme("- `a.md`\n"));
        assert_eq!(
            docs(&[("docs/README.md", &r), ("docs/a.md", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_anchor_and_a_trailing_slash_are_stripped_from_a_name() {
        let r = readme("- [`a.md`](a.md#part)\n- `d/`\n");
        assert_eq!(
            docs(&[("docs/README.md", &r), ("docs/a.md", ""), ("docs/d/x", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_root_readme_is_not_checked_for_shape() {
        let p = run(&[("README.md", "# Root\n\nno status, no sections\n")]);
        assert!(!p.iter().any(|m| m.starts_with("README.md")), "{p:#?}");
    }

    #[test]
    fn the_readme_itself_needs_no_entry() {
        assert_eq!(
            docs(&[("docs/README.md", &readme(""))]),
            Vec::<String>::new()
        );
    }
}
