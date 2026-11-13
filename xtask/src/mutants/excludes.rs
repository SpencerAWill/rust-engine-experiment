//! Reading the `exclude_re` list of `.cargo/mutants.toml`.
//!
//! The file is read by hand, in a restricted form, so the command can require a reason comment
//! above every entry (a TOML parser would discard comments) without a TOML dependency. The form
//! is described in the M2.3 spec, section 4.4.

/// The file's path as shown in error messages.
const FILE: &str = ".cargo/mutants.toml";

/// One exclusion entry: a regex matched against the names `cargo mutants --list` prints.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entry {
    /// The 1-based line of the entry in the file.
    pub(super) line: usize,
    /// The regex, between the quotes of its TOML literal string.
    pub(super) regex: String,
}

/// Whether `regex` contains a `:` directly followed by an ASCII digit. That only occurs in the
/// `file:line:col:` prefix of a mutant name, so such an entry depends on a source position and
/// would stop matching when the file changes.
fn has_position(regex: &str) -> bool {
    regex
        .as_bytes()
        .windows(2)
        .any(|pair| pair[0] == b':' && pair[1].is_ascii_digit())
}

/// Whether `line` starts the `exclude_re` key.
fn is_key(line: &str) -> bool {
    line.strip_prefix("exclude_re")
        .is_some_and(|rest| rest.starts_with([' ', '=', '\t']))
}

/// Reads one entry line: a TOML literal string, then `,`, then nothing.
fn parse_entry(line: &str) -> Result<String, &'static str> {
    let body = line
        .strip_prefix('\'')
        .ok_or("expected an entry as a single-quoted TOML literal string, then `,`")?;
    let (regex, rest) = body
        .split_once('\'')
        .ok_or("the entry's string is not closed with `'`")?;
    if rest != "," {
        return Err("expected exactly one entry followed by `,` and nothing else on the line");
    }
    if regex.is_empty() {
        return Err("the entry is empty, so it would match every mutant");
    }
    if has_position(regex) {
        return Err(
            "the entry contains a line or column number (`:` then a digit); name the function instead",
        );
    }
    Ok(regex.to_string())
}

/// Parses the `exclude_re` list in `text` (the contents of `.cargo/mutants.toml`).
///
/// Returns every entry with its line, or every problem found, each starting with
/// `.cargo/mutants.toml:<line>:`. Other keys in the file are ignored.
pub(super) fn parse(text: &str) -> Result<Vec<Entry>, Vec<String>> {
    let mut entries = Vec::new();
    let mut errors = Vec::new();
    let mut seen_key = false;
    let mut inside = false;
    let mut has_reason = false;
    let mut error = |line: usize, message: &str| errors.push(format!("{FILE}:{line}: {message}"));
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if !inside {
            if !is_key(line) {
                continue;
            }
            if seen_key {
                error(line_no, "`exclude_re` appears more than once");
            }
            seen_key = true;
            match line {
                "exclude_re = []" => {}
                "exclude_re = [" => {
                    inside = true;
                    has_reason = false; // the first entry needs a comment too
                }
                _ => error(
                    line_no,
                    "the list must open with a line that is exactly `exclude_re = [` (or be `exclude_re = []`)",
                ),
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            has_reason = true;
            continue;
        }
        if line == "]" {
            inside = false;
            continue;
        }
        let reason_before = std::mem::replace(&mut has_reason, false);
        match parse_entry(line) {
            Err(message) => error(line_no, message),
            Ok(_) if !reason_before => error(
                line_no,
                "the entry has no reason: put a comment line above it",
            ),
            Ok(regex) => entries.push(Entry {
                line: line_no,
                regex,
            }),
        }
    }
    if inside {
        errors.push(format!(
            "{FILE}: the `exclude_re` list is not closed with a line that is exactly `]`"
        ));
    }
    if !seen_key {
        errors.push(format!(
            "{FILE}: no `exclude_re` list found (write `exclude_re = []` for none)"
        ));
    }
    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(body: &str) -> String {
        format!("test_tool = \"nextest\"\nexclude_re = [\n{body}]\n")
    }

    #[test]
    fn parses_entries_with_reasons() {
        let text = list(
            "    # Equivalent: never equal.\n    '^libs/a/src/b\\.rs:.*replace < with <= in f',\n\n    # Not worth a test.\n\n    '^libs/a/src/c\\.rs:.*replace g with ()',\n",
        );
        let entries = parse(&text).unwrap();
        assert_eq!(
            entries,
            vec![
                Entry {
                    line: 4,
                    regex: r"^libs/a/src/b\.rs:.*replace < with <= in f".into()
                },
                Entry {
                    line: 8,
                    regex: r"^libs/a/src/c\.rs:.*replace g with ()".into()
                },
            ]
        );
    }

    #[test]
    fn empty_list_on_one_line() {
        assert_eq!(parse("exclude_re = []\n"), Ok(vec![]));
    }

    #[test]
    fn entry_without_a_reason_is_an_error() {
        let text = list("    # why\n    'a',\n    'b',\n");
        let errors = parse(&text).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(
            errors[0].starts_with(".cargo/mutants.toml:5:"),
            "{errors:?}"
        );
        assert!(errors[0].contains("reason"), "{errors:?}");
    }

    #[test]
    fn a_first_entry_without_a_reason_is_an_error() {
        let errors = parse(&list("    'a',\n")).unwrap_err();
        assert!(
            errors[0].starts_with(".cargo/mutants.toml:3:"),
            "{errors:?}"
        );
    }

    #[test]
    fn malformed_lines_are_errors() {
        for (body, what) in [
            ("# r\n'a', 'b',\n", "two entries on one line"),
            ("# r\n\"a\",\n", "a basic string"),
            ("# r\n'a', # trailing\n", "text after the comma"),
            ("# r\n'a'\n", "a missing comma"),
            ("# r\n'a\n", "an unclosed string"),
        ] {
            let errors = parse(&list(body)).unwrap_err();
            assert_eq!(errors.len(), 1, "{what}: {errors:?}");
            assert!(
                errors[0].starts_with(".cargo/mutants.toml:4:"),
                "{what}: {errors:?}"
            );
        }
    }

    #[test]
    fn list_must_appear_once_and_close() {
        let twice = "exclude_re = []\nexclude_re = []\n";
        let errors = parse(twice).unwrap_err();
        assert!(
            errors[0].starts_with(".cargo/mutants.toml:2:"),
            "{errors:?}"
        );

        let unclosed = "exclude_re = [\n    # r\n    'a',\n";
        let errors = parse(unclosed).unwrap_err();
        assert!(errors[0].contains("`]`"), "{errors:?}");

        let errors = parse("test_tool = \"nextest\"\n").unwrap_err();
        assert!(errors[0].contains("exclude_re"), "{errors:?}");
    }

    #[test]
    fn positions_are_rejected() {
        let errors = parse(&list("# r\n'^libs/a/src/b\\.rs:12:',\n")).unwrap_err();
        assert!(
            errors[0].starts_with(".cargo/mutants.toml:4:"),
            "{errors:?}"
        );
        assert!(errors[0].contains("line or column"), "{errors:?}");
        assert!(has_position(r"^libs/a/src/b\.rs:12:"));
        assert!(!has_position("replace u64 with 0"));
        assert!(!has_position("[u64; 4]"));
        assert!(!has_position("a: b"));
    }

    #[test]
    fn all_failures_are_reported_together() {
        let text = list("# r\n\"x\",\n'y'\n'z:1',\n");
        let errors = parse(&text).unwrap_err();
        assert_eq!(errors.len(), 3, "{errors:?}");
    }
}
