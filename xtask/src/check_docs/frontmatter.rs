//! Lesson files (spec M3.16 §4.3) and rule files (§4.4): the restricted frontmatter, and the
//! lesson format.
//!
//! The frontmatter is read line by line, not as YAML: one key, then one `  - "<glob>"` line per
//! glob, so the form Claude Code and `check-docs` agree on is exactly this.

use std::collections::BTreeSet;

use super::markdown;
use super::{Problem, glob};

/// Checks a `docs/lessons/*.md` file: frontmatter with `applies_to:`, then the heading, status
/// line and lessons of `docs/lessons/README.md`.
pub(super) fn check_lesson(path: &str, text: &str, files: &BTreeSet<String>) -> Vec<Problem> {
    let mut problems = Vec::new();
    if !check_frontmatter(path, text, "applies_to", files, &mut problems) {
        return problems;
    }
    let stem = path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".md"))
        .unwrap_or_default();
    let doc = markdown::parse(text);
    match doc.headings.iter().find(|h| h.level == 1) {
        None => problems.push(Problem::new(
            path,
            None,
            "missing level-1 heading `# Lessons: <topic>`; add it after the frontmatter",
        )),
        Some(h) if !h.text.starts_with("Lessons: ") => problems.push(Problem::new(
            path,
            Some(h.line),
            &format!(
                "first heading `{}`: must start with `Lessons: `; rename it `Lessons: <topic>`",
                h.text
            ),
        )),
        Some(_) => {}
    }
    let mut items = doc.items.iter().peekable();
    match items.next_if(|i| i.source.starts_with("Status:")) {
        None => problems.push(Problem::new(
            path,
            None,
            "missing status line `- Status: Living (<date>)`; add it right after the heading",
        )),
        Some(status)
            if !(status.source.starts_with("Status: Living (") && status.source.ends_with(')')) =>
        {
            problems.push(Problem::new(
                path,
                Some(status.line),
                "status line must read `- Status: Living (<date>)`; fix it",
            ));
        }
        Some(_) => {}
    }
    let mut seen = BTreeSet::new();
    for item in items {
        let id = lesson_id(&item.source, stem);
        match &id {
            None => problems.push(Problem::new(
                path,
                Some(item.line),
                &format!(
                    "lesson ID must be `{stem}-<n>` (n a positive integer): start the bullet with `**{stem}-<n>.**`"
                ),
            )),
            Some(id) if !seen.insert(id.clone()) => problems.push(Problem::new(
                path,
                Some(item.line),
                &format!("duplicate lesson ID `{id}`; IDs are never reused, so give this lesson a new number"),
            )),
            Some(_) => {}
        }
        let name = id.map_or_else(|| "this lesson".to_string(), |id| format!("`{id}`"));
        for field in ["*Why:*", "*From:*", "*Confirmed:*"] {
            if !item.source.contains(field) {
                problems.push(Problem::new(
                    path,
                    Some(item.line),
                    &format!("lesson {name} lacks `{field}`; add it"),
                ));
            }
        }
    }
    problems
}

/// Checks a `.claude/rules/*.md` file: frontmatter with `paths:`.
pub(super) fn check_rule(path: &str, text: &str, files: &BTreeSet<String>) -> Vec<Problem> {
    let mut problems = Vec::new();
    check_frontmatter(path, text, "paths", files, &mut problems);
    problems
}

/// The ID of a lesson bullet (`**<stem>-<n>.**`), if it is well formed.
fn lesson_id(source: &str, stem: &str) -> Option<String> {
    let id = source.strip_prefix("**")?.split_once(".**")?.0;
    let number = id.strip_prefix(stem)?.strip_prefix('-')?;
    let positive =
        number.bytes().all(|b| b.is_ascii_digit()) && number.parse::<u32>().is_ok_and(|n| n > 0);
    positive.then(|| id.to_string())
}

/// Which key the list items being read belong to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Under {
    Nothing,
    Expected,
    Other,
}

/// Checks the frontmatter of `text` with the single key `key`, pushing problems. Returns false
/// when the frontmatter is not closed, so nothing after it can be trusted.
fn check_frontmatter(
    path: &str,
    text: &str,
    key: &str,
    files: &BTreeSet<String>,
    problems: &mut Vec<Problem>,
) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first() != Some(&"---") {
        problems.push(Problem::new(
            path,
            None,
            &format!(
                "missing frontmatter; start the file with a `---` block holding `{key}:` and one quoted glob per line, closed by `---`"
            ),
        ));
        return true;
    }
    let Some(end) = lines
        .iter()
        .skip(1)
        .position(|l| *l == "---")
        .map(|i| i + 1)
    else {
        problems.push(Problem::new(
            path,
            None,
            "frontmatter is not closed; add a closing `---` line",
        ));
        return false;
    };
    let (mut under, mut key_line, mut globs) = (Under::Nothing, None, 0_usize);
    let mut other_key = false;
    for (index, line) in lines.iter().enumerate().take(end).skip(1) {
        let number = index + 1;
        if *line == format!("{key}:") {
            if key_line.is_some() {
                problems.push(Problem::new(
                    path,
                    Some(number),
                    &format!("duplicate `{key}:`; merge the lists"),
                ));
            }
            (under, key_line) = (Under::Expected, key_line.or(Some(number)));
        } else if let Some(item) = line.strip_prefix("  - ") {
            if under == Under::Expected {
                globs += 1;
                check_glob(path, number, item, files, problems);
            } else if under == Under::Nothing {
                problems.push(Problem::new(
                    path,
                    Some(number),
                    &format!("glob line before `{key}:`; put it under the key"),
                ));
            }
        } else {
            other_key = true;
            under = Under::Other;
            let name = line.split(':').next().unwrap_or(line);
            problems.push(Problem::new(
                path,
                Some(number),
                &format!(
                    "frontmatter key `{name}`: only `{key}:` is allowed; remove it or rename it"
                ),
            ));
        }
    }
    match key_line {
        None if !other_key => problems.push(Problem::new(
            path,
            Some(1),
            &format!("frontmatter lacks `{key}:`; add it with one quoted glob per line"),
        )),
        Some(number) if globs == 0 => problems.push(Problem::new(
            path,
            Some(number),
            &format!("`{key}:` needs at least one glob line, as in `  - \"docs/**\"`"),
        )),
        _ => {}
    }
    true
}

/// Checks one `  - <glob>` line: quoted, in the supported subset, matching a tracked file.
fn check_glob(
    path: &str,
    number: usize,
    item: &str,
    files: &BTreeSet<String>,
    problems: &mut Vec<Problem>,
) {
    let item = item.trim();
    let Some(glob) = item
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        problems.push(Problem::new(
            path,
            Some(number),
            &format!("glob `{item}` must be in double quotes: write `  - \"{item}\"`"),
        ));
        return;
    };
    if let Err(reason) = glob::validate(glob) {
        problems.push(Problem::new(
            path,
            Some(number),
            &format!("glob `{glob}`: {reason}"),
        ));
    } else if !files.iter().any(|f| glob::matches(glob, f)) {
        problems.push(Problem::new(
            path,
            Some(number),
            &format!("glob `{glob}` matches no tracked file; fix the glob or remove it"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check_docs::fixture;

    const PATH: &str = "docs/lessons/demo.md";
    const LESSON: &str = "---\napplies_to:\n  - \"docs/**\"\n---\n# Lessons: demo\n\n- Status: Living (2026-11-15)\n\n- **demo-1.** Do x. *Why:* y. *From:* M1 · *Confirmed:* 2026-11-15\n";
    const RULE_PATH: &str = ".claude/rules/docs.md";
    const RULE: &str = "---\npaths:\n  - \"docs/**\"\n---\nRead docs/README.md.\n";

    fn lesson(text: &str) -> Vec<String> {
        let (files, _) = fixture(&[(PATH, text), ("docs/a.md", "")]);
        check_lesson(PATH, text, &files)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn rule(text: &str) -> Vec<String> {
        let (files, _) = fixture(&[(RULE_PATH, text), ("docs/a.md", "")]);
        check_rule(RULE_PATH, text, &files)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Each problem starts with `location: ` and contains `fragment`.
    fn expect(actual: &[String], expected: &[(&str, &str)]) {
        assert_eq!(actual.len(), expected.len(), "{actual:#?}");
        for (a, (location, fragment)) in actual.iter().zip(expected) {
            assert!(a.starts_with(&format!("{location}: ")), "{a}");
            assert!(a.contains(fragment), "{a}");
        }
    }

    #[test]
    fn a_well_formed_lesson_and_rule_report_nothing() {
        assert_eq!(lesson(LESSON), Vec::<String>::new());
        assert_eq!(rule(RULE), Vec::<String>::new());
    }

    #[test]
    fn a_lesson_without_frontmatter() {
        let text = LESSON
            .split_once("# Lessons")
            .map(|(_, rest)| format!("# Lessons{rest}"))
            .unwrap();
        expect(&lesson(&text), &[(PATH, "missing frontmatter")]);
    }

    #[test]
    fn an_unclosed_frontmatter() {
        expect(
            &lesson("---\napplies_to:\n  - \"docs/**\"\n# Lessons: demo\n"),
            &[(PATH, "not closed")],
        );
    }

    #[test]
    fn a_repeated_key_is_one_problem() {
        let text =
            "---\npaths:\n  - \"docs/**\"\npaths:\n  - \"xtask/**\"\n---\nRead docs/README.md.\n";
        let (files, _) = fixture(&[(RULE_PATH, text), ("docs/a.md", ""), ("xtask/a.rs", "")]);
        let got: Vec<String> = check_rule(RULE_PATH, text, &files)
            .iter()
            .map(ToString::to_string)
            .collect();
        expect(
            &got,
            &[(
                ".claude/rules/docs.md:4",
                "duplicate `paths:`; merge the lists",
            )],
        );
        let text = LESSON.replace(
            "---\n# Lessons",
            "applies_to:\n  - \"docs/**\"\n---\n# Lessons",
        );
        expect(
            &lesson(&text),
            &[(
                "docs/lessons/demo.md:4",
                "duplicate `applies_to:`; merge the lists",
            )],
        );
    }

    #[test]
    fn an_extra_key() {
        let text = LESSON.replace("---\n# Lessons", "title: x\n---\n# Lessons");
        expect(
            &lesson(&text),
            &[("docs/lessons/demo.md:4", "only `applies_to:`")],
        );
    }

    #[test]
    fn an_unquoted_glob() {
        let text = LESSON.replace("\"docs/**\"", "docs/**");
        expect(
            &lesson(&text),
            &[("docs/lessons/demo.md:3", "double quotes")],
        );
    }

    #[test]
    fn a_glob_matching_nothing() {
        let text = LESSON.replace("docs/**", "nowhere/**");
        expect(
            &lesson(&text),
            &[("docs/lessons/demo.md:3", "matches no tracked file")],
        );
    }

    #[test]
    fn an_unsupported_glob() {
        let text = LESSON.replace("docs/**", "docs/{a,b}");
        expect(&lesson(&text), &[("docs/lessons/demo.md:3", "braces")]);
    }

    #[test]
    fn a_key_without_globs() {
        let text = LESSON.replace("  - \"docs/**\"\n", "");
        expect(
            &lesson(&text),
            &[("docs/lessons/demo.md:2", "at least one glob")],
        );
    }

    #[test]
    fn a_bad_id_a_wrong_stem_and_a_missing_id() {
        for id in ["**docs-x.**", "**other-1.**", "**demo-0.**", "no id"] {
            let text = LESSON.replace("**demo-1.**", id);
            expect(
                &lesson(&text),
                &[("docs/lessons/demo.md:9", "must be `demo-<n>`")],
            );
        }
    }

    #[test]
    fn a_duplicate_id() {
        let text = format!(
            "{LESSON}- **demo-1.** Again. *Why:* y. *From:* M1 · *Confirmed:* 2026-11-15\n"
        );
        expect(
            &lesson(&text),
            &[("docs/lessons/demo.md:10", "duplicate lesson ID `demo-1`")],
        );
    }

    #[test]
    fn a_lesson_missing_a_field() {
        for (field, name) in [
            ("*Why:*", "*Why:*"),
            ("*From:*", "*From:*"),
            ("*Confirmed:*", "*Confirmed:*"),
        ] {
            let text = LESSON.replace(field, "");
            expect(
                &lesson(&text),
                &[("docs/lessons/demo.md:9", &format!("lacks `{name}`"))],
            );
        }
    }

    #[test]
    fn a_wrong_or_missing_heading() {
        expect(
            &lesson(&LESSON.replace("# Lessons: demo", "# Demo")),
            &[("docs/lessons/demo.md:5", "must start with `Lessons: `")],
        );
        let text = LESSON.replace("# Lessons: demo\n", "");
        let problems = lesson(&text);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("missing level-1 heading")),
            "{problems:#?}"
        );
    }

    #[test]
    fn a_missing_status_line() {
        let text = LESSON.replace("- Status: Living (2026-11-15)\n\n", "");
        expect(&lesson(&text), &[(PATH, "missing status line")]);
    }

    #[test]
    fn a_rule_with_applies_to_instead_of_paths() {
        let text = RULE.replace("paths:", "applies_to:");
        expect(
            &rule(&text),
            &[(".claude/rules/docs.md:2", "only `paths:`")],
        );
    }

    #[test]
    fn a_glob_line_before_any_key_is_a_problem() {
        let text = "---\n  - \"**\"\npaths:\n  - \"docs/**\"\n---\nx\n";
        expect(
            &rule(text),
            &[(".claude/rules/docs.md:2", "before `paths:`")],
        );
    }

    #[test]
    fn a_rule_without_frontmatter() {
        expect(
            &rule("Read docs/README.md.\n"),
            &[(RULE_PATH, "missing frontmatter")],
        );
    }
}
