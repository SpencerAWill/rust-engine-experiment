//! Relative links (spec M3.16 §4.6): the target exists inside the repository and a `#anchor` on
//! a Markdown file matches one of its headings.

use std::collections::{BTreeMap, BTreeSet};

use super::markdown::{self, Doc};
use super::{Problem, is_markdown};

/// Checks every relative link of every Markdown file in `texts` (path to contents).
pub(super) fn check(files: &BTreeSet<String>, texts: &BTreeMap<String, String>) -> Vec<Problem> {
    let docs: BTreeMap<&str, Doc> = texts
        .iter()
        .map(|(path, text)| (path.as_str(), markdown::parse(text)))
        .collect();
    let mut problems = Vec::new();
    for (path, doc) in &docs {
        for link in &doc.links {
            if let Some(message) = check_link(path, &link.target, files, &docs) {
                problems.push(Problem::new(path, Some(link.line), &message));
            }
        }
    }
    problems
}

/// The problem with `target`, written in the file `from`, if it has one.
fn check_link(
    from: &str,
    target: &str,
    files: &BTreeSet<String>,
    docs: &BTreeMap<&str, Doc>,
) -> Option<String> {
    if has_scheme(target) || target.starts_with("//") {
        return None;
    }
    let (location, anchor) = target.split_once('#').unwrap_or((target, ""));
    let location = location.split('?').next().unwrap_or_default();
    let decoded = percent_decode(location);
    let resolved = if decoded.is_empty() {
        from.to_string()
    } else {
        let Some(resolved) = resolve(from, &decoded) else {
            return Some(format!(
                "link `{target}`: points outside the repository; fix the path"
            ));
        };
        resolved
    };
    if !(resolved.is_empty() || files.contains(&resolved) || is_directory(files, &resolved)) {
        return Some(format!(
            "link `{target}`: no such file; fix the path or remove the link"
        ));
    }
    if anchor.is_empty() || !is_markdown(&resolved) {
        return None;
    }
    let anchor = percent_decode(anchor);
    let doc = docs.get(resolved.as_str())?;
    if doc.anchors().contains(&anchor) {
        None
    } else {
        Some(format!(
            "link `{target}`: no heading in {resolved} has the anchor `{anchor}`"
        ))
    }
}

/// Whether `target` starts with a URL scheme such as `https:` or `mailto:`.
pub(super) fn has_scheme(target: &str) -> bool {
    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Resolves `target` against the directory of `from`. `None` when it climbs out of the
/// repository. A leading `/` means the repository root, as on GitHub.
fn resolve(from: &str, target: &str) -> Option<String> {
    let mut segments: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        let mut dir: Vec<&str> = from.split('/').collect();
        dir.pop();
        dir
    };
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}

/// Whether some tracked file is inside the directory `path`.
fn is_directory(files: &BTreeSet<String>, path: &str) -> bool {
    let prefix = format!("{path}/");
    files
        .range(prefix.clone()..)
        .next()
        .is_some_and(|f| f.starts_with(&prefix))
}

/// Decodes `%XX` escapes (invalid ones are kept as written).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        if let Some(byte) = escaped {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check_docs::fixture;

    fn run(entries: &[(&str, &str)]) -> Vec<String> {
        let (files, texts) = fixture(entries);
        check(&files, &texts)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_good_link_reports_nothing() {
        assert_eq!(
            run(&[("a.md", "[b](docs/b.md)\n"), ("docs/b.md", "# B\n")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_missing_target_names_the_fix() {
        assert_eq!(
            run(&[("docs/a.md", "x\n\n[b](b.md)\n")]),
            ["docs/a.md:3: link `b.md`: no such file; fix the path or remove the link"]
        );
    }

    #[test]
    fn a_target_outside_the_repository_is_reported() {
        assert_eq!(
            run(&[("a.md", "[b](../b.md)\n")]),
            ["a.md:1: link `../b.md`: points outside the repository; fix the path"]
        );
    }

    #[test]
    fn dot_dot_inside_the_repository_resolves() {
        assert_eq!(
            run(&[("docs/a.md", "[b](../x/b.md)\n"), ("x/b.md", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_percent_encoded_target_is_decoded() {
        assert_eq!(
            run(&[("a.md", "[b](a%20b.md)\n"), ("a b.md", "# B\n")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_directory_target_is_a_tracked_directory() {
        assert_eq!(
            run(&[("a.md", "[d](docs/) [e](docs)\n"), ("docs/x.md", "")]),
            Vec::<String>::new()
        );
        assert_eq!(
            run(&[("a.md", "[d](nodir/)\n")]),
            ["a.md:1: link `nodir/`: no such file; fix the path or remove the link"]
        );
    }

    #[test]
    fn anchors_are_checked_against_headings() {
        let b = "# B\n\n## Same\n\n## Same\n";
        assert_eq!(
            run(&[
                ("a.md", "[x](b.md#same) [y](b.md#same-1) [z](b.md#b)\n"),
                ("b.md", b)
            ]),
            Vec::<String>::new()
        );
        assert_eq!(
            run(&[("a.md", "[x](b.md#nope)\n"), ("b.md", b)]),
            ["a.md:1: link `b.md#nope`: no heading in b.md has the anchor `nope`"]
        );
    }

    #[test]
    fn a_same_file_anchor_is_checked() {
        assert_eq!(
            run(&[("a.md", "# Top\n\n[x](#top)\n")]),
            Vec::<String>::new()
        );
        assert_eq!(
            run(&[("a.md", "# Top\n\n[x](#bottom)\n")]),
            ["a.md:3: link `#bottom`: no heading in a.md has the anchor `bottom`"]
        );
    }

    #[test]
    fn a_heading_with_punctuation_and_code_has_the_github_anchor() {
        assert_eq!(
            run(&[("a.md", "## `foo::bar` and baz!\n\n[x](#foobar-and-baz)\n")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn anchors_on_other_files_are_not_checked() {
        assert_eq!(
            run(&[("a.md", "[x](src/lib.rs#L10)\n"), ("src/lib.rs", "")]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn code_html_and_external_links_are_ignored() {
        let text = "`[a](missing.md)` <!-- [b](missing.md) -->\n\n```\n[c](missing.md)\n```\n\n[d](https://example.com/x) [e](mailto:a@b.c)\n";
        assert_eq!(run(&[("a.md", text)]), Vec::<String>::new());
    }

    #[test]
    fn reference_definitions_and_images_are_checked() {
        assert_eq!(
            run(&[("a.md", "![i](img.png)\n\n[r]: gone.md\n")]),
            [
                "a.md:1: link `img.png`: no such file; fix the path or remove the link",
                "a.md:3: link `gone.md`: no such file; fix the path or remove the link",
            ]
        );
    }
}
