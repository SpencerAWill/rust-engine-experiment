//! Markdown parsing for `check-docs`, on `pulldown-cmark`.
//!
//! One pass gives the headings, the relative-link candidates (outside code and HTML) and the
//! top-level bullets, each with a 1-based line computed from the byte offset of its event.

use std::collections::BTreeMap;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

/// A heading: its level, its plain text (code spans included) and its line.
pub(super) struct Heading {
    pub(super) level: u8,
    pub(super) text: String,
    pub(super) line: usize,
}

/// A link, image or reference-definition target as written, and its line.
pub(super) struct Link {
    pub(super) target: String,
    pub(super) line: usize,
}

/// A top-level bullet (not nested in another list or a block quote): its line and its source
/// text without the bullet marker.
pub(super) struct Item {
    pub(super) line: usize,
    pub(super) source: String,
}

/// A Contents candidate: a top-level bullet or a table body row, its line, and its name: the
/// first inline code span or link in it (a link's name is its target), if any.
pub(super) struct Entry {
    pub(super) line: usize,
    pub(super) name: Option<String>,
}

/// What `check-docs` needs from one Markdown file.
pub(super) struct Doc {
    pub(super) entries: Vec<Entry>,
    pub(super) headings: Vec<Heading>,
    pub(super) links: Vec<Link>,
    pub(super) items: Vec<Item>,
}

impl Doc {
    /// The GitHub anchor of each heading, in order: the second heading with the same anchor
    /// gets `-1`, the third `-2`.
    pub(super) fn anchors(&self) -> Vec<String> {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        self.headings
            .iter()
            .map(|h| {
                let base = slug(&h.text);
                let count = seen.entry(base.clone()).or_insert(0);
                let anchor = if *count == 0 {
                    base
                } else {
                    format!("{base}-{count}")
                };
                *count += 1;
                anchor
            })
            .collect()
    }
}

/// Parses `text`. `---` frontmatter is a metadata block (not a thematic break and a setext
/// heading), so it never shows up as a heading.
pub(super) fn parse(text: &str) -> Doc {
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let line_of = |offset: usize| line_starts.partition_point(|&start| start <= offset);

    let options = Options::ENABLE_TABLES | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let mut parser = Parser::new_ext(text, options).into_offset_iter();
    let mut doc = Doc {
        entries: Vec::new(),
        headings: Vec::new(),
        links: Vec::new(),
        items: Vec::new(),
    };
    let mut heading: Option<Heading> = None;
    let mut entry: Option<Entry> = None;
    let (mut list_depth, mut quote_depth) = (0_usize, 0_usize);
    for (event, range) in &mut parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(Heading {
                    level: level as u8,
                    text: String::new(),
                    line: line_of(range.start),
                });
            }
            Event::End(TagEnd::Heading(_)) => doc.headings.extend(heading.take()),
            Event::Text(t) => {
                if let Some(h) = heading.as_mut() {
                    h.text.push_str(&t);
                }
            }
            Event::Code(t) => {
                if let Some(h) = heading.as_mut() {
                    h.text.push_str(&t);
                }
                if let Some(e) = entry.as_mut().filter(|e| e.name.is_none()) {
                    e.name = Some(t.to_string());
                }
            }
            Event::Start(Tag::TableRow) if entry.is_none() && list_depth == 0 => {
                entry = Some(Entry {
                    line: line_of(range.start),
                    name: None,
                });
            }
            Event::End(TagEnd::TableRow) => doc.entries.extend(entry.take()),
            Event::End(TagEnd::Item) if list_depth == 1 && quote_depth == 0 => {
                doc.entries.extend(entry.take());
            }
            Event::Start(Tag::List(_)) => list_depth += 1,
            Event::End(TagEnd::List(_)) => list_depth -= 1,
            Event::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => quote_depth -= 1,
            Event::Start(Tag::Item) if list_depth == 1 && quote_depth == 0 => {
                entry = Some(Entry {
                    line: line_of(range.start),
                    name: None,
                });
                doc.items.push(Item {
                    line: line_of(range.start),
                    source: strip_marker(text[range.clone()].trim_end()).to_string(),
                });
            }
            // Reference-style uses resolve to a definition, which is checked where it is written.
            Event::Start(
                Tag::Link {
                    link_type: LinkType::Inline | LinkType::Autolink,
                    dest_url,
                    ..
                }
                | Tag::Image {
                    link_type: LinkType::Inline | LinkType::Autolink,
                    dest_url,
                    ..
                },
            ) => {
                if let Some(e) = entry.as_mut().filter(|e| e.name.is_none()) {
                    e.name = Some(dest_url.to_string());
                }
                doc.links.push(Link {
                    target: dest_url.to_string(),
                    line: line_of(range.start),
                });
            }
            _ => {}
        }
    }
    for (_, definition) in parser.reference_definitions().iter() {
        doc.links.push(Link {
            target: definition.dest.to_string(),
            line: line_of(definition.span.start),
        });
    }
    doc.links
        .sort_by(|a, b| (a.line, &a.target).cmp(&(b.line, &b.target)));
    doc
}

/// `source` without its leading bullet marker (`-`, `*` or `+`) and the spaces after it.
fn strip_marker(source: &str) -> &str {
    source
        .strip_prefix(['-', '*', '+'])
        .map_or(source, |rest| rest.trim_start_matches(' '))
}

/// The GitHub anchor of heading text: lowercase, every character that is not a letter, digit,
/// space, `-` or `_` dropped, spaces turned into `-`.
pub(super) fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_follow_the_github_rule() {
        assert_eq!(slug("What CI enforces"), "what-ci-enforces");
        assert_eq!(slug("`foo::bar` and baz!"), "foobar-and-baz");
        assert_eq!(slug("a_b - c"), "a_b---c");
    }

    #[test]
    fn duplicate_headings_get_numbered_anchors() {
        let doc = parse("# Same\n\n## Same\n\n## Same\n\n## Other\n");
        assert_eq!(doc.anchors(), ["same", "same-1", "same-2", "other"]);
    }

    #[test]
    fn headings_carry_level_text_and_line() {
        let doc = parse("# One\n\ntext\n\n## `Two` x\n");
        assert_eq!(doc.headings.len(), 2);
        assert_eq!(
            (
                doc.headings[0].level,
                doc.headings[0].text.as_str(),
                doc.headings[0].line
            ),
            (1, "One", 1)
        );
        assert_eq!(
            (
                doc.headings[1].level,
                doc.headings[1].text.as_str(),
                doc.headings[1].line
            ),
            (2, "Two x", 5)
        );
    }

    #[test]
    fn frontmatter_is_not_a_heading_or_a_thematic_break() {
        let doc = parse("---\napplies_to:\n  - \"x\"\n---\n# Title\n");
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].line, 5);
    }

    #[test]
    fn links_in_code_and_html_are_ignored() {
        let text = "[a](a.md) and `[b](b.md)` and <!-- [c](c.md) -->\n\n```\n[d](d.md)\n```\n\n    [e](e.md)\n\n[f](f.md)\n";
        let doc = parse(text);
        let targets: Vec<(&str, usize)> = doc
            .links
            .iter()
            .map(|l| (l.target.as_str(), l.line))
            .collect();
        assert_eq!(targets, [("a.md", 1), ("f.md", 9)]);
    }

    #[test]
    fn images_and_reference_definitions_are_links_and_uses_are_not_counted_twice() {
        let text = "![i](img.png)\n\nsee [x][r] and [r]\n\n[r]: target.md\n";
        let doc = parse(text);
        let targets: Vec<(&str, usize)> = doc
            .links
            .iter()
            .map(|l| (l.target.as_str(), l.line))
            .collect();
        assert_eq!(targets, [("img.png", 1), ("target.md", 5)]);
    }

    #[test]
    fn entries_are_top_level_bullets_and_table_body_rows_named_by_their_first_span_or_link() {
        let text = "- `a` and `b`\n- [`l`](l.md) x\n- prose\n  - `nested`\n- see [t](t.md) then `c`\n\n| H | I |\n|---|---|\n| `r` | `s` |\n| [`q`](q/) | x |\n";
        let doc = parse(text);
        let got: Vec<(Option<&str>, usize)> = doc
            .entries
            .iter()
            .map(|e| (e.name.as_deref(), e.line))
            .collect();
        assert_eq!(
            got,
            [
                (Some("a"), 1),
                (Some("l.md"), 2),
                (Some("nested"), 3),
                (Some("t.md"), 5),
                (Some("r"), 9),
                (Some("q/"), 10),
            ]
        );
    }

    #[test]
    fn top_level_bullets_carry_their_source_and_line() {
        let doc = parse("- one\n  - nested\n- **two** *x*\n\n> - quoted\n");
        let items: Vec<(&str, usize)> = doc
            .items
            .iter()
            .map(|i| (i.source.as_str(), i.line))
            .collect();
        assert_eq!(items, [("one\n  - nested", 1), ("**two** *x*", 3)]);
    }
}
