//! The glob subset of spec M3.16 §4.5: `*`, `?` and `**` as a whole path segment.
//!
//! Claude Code's matcher and this one agree on that subset; anything else is rejected by
//! [`validate`] so the two cannot disagree silently.

/// Checks that `glob` uses only the supported subset; the error says what to change.
pub(super) fn validate(glob: &str) -> Result<(), String> {
    if glob.is_empty() {
        return Err("the glob is empty; write a path pattern".to_string());
    }
    if glob.starts_with('!') {
        return Err("a leading `!` is not supported; list the paths to include".to_string());
    }
    if glob.contains(['{', '}']) {
        return Err("braces are not supported; write one glob per alternative".to_string());
    }
    if glob.contains(['[', ']']) {
        return Err("brackets are not supported; use `?` or list each path".to_string());
    }
    if glob.split('/').any(|s| s != "**" && s.contains("**")) {
        return Err("`**` must be a whole path segment, as in `docs/**`".to_string());
    }
    Ok(())
}

/// Whether the repository-relative `path` matches `glob`.
///
/// `*` and `?` stay within one path segment; a `**` segment matches zero or more segments.
pub(super) fn matches(glob: &str, path: &str) -> bool {
    let pattern: Vec<&str> = glob.split('/').collect();
    let segments: Vec<&str> = path.split('/').collect();
    match_segments(&pattern, &segments)
}

fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| match_segments(rest, &path[skip..])),
        Some((first, rest)) => path.split_first().is_some_and(|(segment, tail)| {
            match_segment(first, segment) && match_segments(rest, tail)
        }),
    }
}

/// Matches one path segment against a pattern of literals, `?` and `*`.
fn match_segment(pattern: &str, segment: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let segment: Vec<char> = segment.chars().collect();
    let (mut p, mut s) = (0, 0);
    // The last `*` seen: its pattern index and the segment index it currently absorbs up to.
    let mut star: Option<(usize, usize)> = None;
    while s < segment.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, s));
                p += 1;
            }
            Some(&c) if c == '?' || c == segment[s] => {
                p += 1;
                s += 1;
            }
            _ => match star {
                Some((star_p, star_s)) => {
                    p = star_p + 1;
                    s = star_s + 1;
                    star = Some((star_p, star_s + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_stays_in_one_segment() {
        assert!(matches("docs/*.md", "docs/a.md"));
        assert!(!matches("docs/*.md", "docs/x/a.md"));
        assert!(matches("a*c", "abbbc"));
        assert!(matches("a*c", "ac"));
        assert!(!matches("a*c", "abx"));
    }

    #[test]
    fn question_mark_is_one_character() {
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(!matches("a?c", "abbc"));
        assert!(!matches("a?c", "a/c"));
    }

    #[test]
    fn double_star_matches_zero_or_more_segments() {
        assert!(matches("docs/**", "docs/a.md"));
        assert!(matches("docs/**", "docs/x/y/a.md"));
        assert!(matches("**/README.md", "README.md"));
        assert!(matches("**/README.md", "a/b/README.md"));
        assert!(!matches("**/README.md", "a/b/README.txt"));
        assert!(matches("libs/**/src/lib.rs", "libs/src/lib.rs"));
        assert!(matches("libs/**/src/lib.rs", "libs/engine/x/src/lib.rs"));
        assert!(!matches("libs/**/src/lib.rs", "libs/engine/lib.rs"));
        assert!(!matches("docs/**", "other/a.md"));
    }

    #[test]
    fn a_literal_glob_matches_only_itself() {
        assert!(matches("AGENTS.md", "AGENTS.md"));
        assert!(!matches("AGENTS.md", "docs/AGENTS.md"));
    }

    #[test]
    fn the_supported_subset_is_accepted() {
        for glob in ["docs/**", "**/README.md", "libs/*/src", "a?c", "AGENTS.md"] {
            assert_eq!(validate(glob), Ok(()), "{glob}");
        }
    }

    #[test]
    fn braces_brackets_negation_and_partial_double_stars_are_rejected() {
        for glob in [
            "{a,b}",
            "docs/{a,b}.md",
            "[ab]",
            "x/[a-z].md",
            "!x",
            "a**",
            "**b/c",
        ] {
            assert!(validate(glob).is_err(), "{glob}");
        }
        assert!(validate("").is_err());
    }
}
