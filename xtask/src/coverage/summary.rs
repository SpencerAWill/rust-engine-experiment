//! The per-crate coverage summary, computed from `lcov.info` and the source files it names.
//!
//! Everything here is a pure function over strings, so it is tested without `cargo-llvm-cov`.

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::{Component, Path, PathBuf};

use syn::spanned::Spanned;
use syn::visit::Visit;

/// Line ranges (1-based, inclusive, first attribute to closing brace) of the test items in
/// `source`, in source order.
///
/// A test item carries `#[test]`, or `#[cfg(P)]` where `P` can only be true when `test` is set.
/// Items are visited at every depth (modules, `impl` blocks, traits); a test item is not descended
/// into.
///
/// # Errors
///
/// Returns a message if `source` does not parse, or declares an out-of-line `#[cfg(test)] mod`
/// (the filter does not follow module files).
pub(super) fn test_item_ranges(source: &str) -> Result<Vec<RangeInclusive<u32>>, String> {
    let file = syn::parse_file(source).map_err(|e| format!("does not parse: {e}"))?;
    let mut finder = Finder::default();
    finder.visit_file(&file);
    match finder.error {
        Some(error) => Err(error),
        None => Ok(finder.ranges),
    }
}

#[derive(Default)]
struct Finder {
    ranges: Vec<RangeInclusive<u32>>,
    error: Option<String>,
}

impl Finder {
    /// Records `node` and returns true if its attributes mark it as a test item.
    fn record(&mut self, attrs: &[syn::Attribute], node: &impl Spanned) -> bool {
        if !attrs.iter().any(is_test_attribute) {
            return false;
        }
        let span = node.span();
        let to_line = |line: usize| u32::try_from(line).unwrap_or(u32::MAX);
        self.ranges
            .push(to_line(span.start().line)..=to_line(span.end().line));
        true
    }
}

impl<'ast> Visit<'ast> for Finder {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if let syn::Item::Mod(module) = item
            && module.content.is_none()
            && module.attrs.iter().any(is_test_attribute)
        {
            self.error.get_or_insert_with(|| {
                format!(
                    "out-of-line #[cfg(test)] module `mod {};`: keep test modules inline",
                    module.ident
                )
            });
            return;
        }
        if !self.record(item_attrs(item), item) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !self.record(impl_item_attrs(item), item) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !self.record(trait_item_attrs(item), item) {
            syn::visit::visit_trait_item(self, item);
        }
    }
}

/// The outer attributes of any item kind.
fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    use syn::Item as I;
    match item {
        I::Const(i) => &i.attrs,
        I::Enum(i) => &i.attrs,
        I::ExternCrate(i) => &i.attrs,
        I::Fn(i) => &i.attrs,
        I::ForeignMod(i) => &i.attrs,
        I::Impl(i) => &i.attrs,
        I::Macro(i) => &i.attrs,
        I::Mod(i) => &i.attrs,
        I::Static(i) => &i.attrs,
        I::Struct(i) => &i.attrs,
        I::Trait(i) => &i.attrs,
        I::TraitAlias(i) => &i.attrs,
        I::Type(i) => &i.attrs,
        I::Union(i) => &i.attrs,
        I::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    use syn::ImplItem as I;
    match item {
        I::Const(i) => &i.attrs,
        I::Fn(i) => &i.attrs,
        I::Type(i) => &i.attrs,
        I::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    use syn::TraitItem as I;
    match item {
        I::Const(i) => &i.attrs,
        I::Fn(i) => &i.attrs,
        I::Type(i) => &i.attrs,
        I::Macro(i) => &i.attrs,
        _ => &[],
    }
}

/// Whether `attr` is `#[test]` or `#[cfg(P)]` with a `P` that requires `test`.
fn is_test_attribute(attr: &syn::Attribute) -> bool {
    if attr.path().is_ident("test") {
        return true;
    }
    attr.path().is_ident("cfg")
        && attr
            .parse_args::<syn::Meta>()
            .is_ok_and(|meta| requires_test(&meta))
}

/// Whether a `cfg` predicate can only be true when `test` is set: `test`, or `all(...)` with at
/// least one such argument. `any(...)` and `not(...)` never qualify.
fn requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|args| args.iter().any(requires_test)),
        _ => false,
    }
}

/// One source file's instrumented lines and functions, as read from `lcov.info`.
struct LcovFile {
    path: PathBuf,
    /// `(line, hits)` for each `DA` record.
    lines: Vec<(u32, u64)>,
    /// `(start line, hits)` for each distinct function start line. Hits are the sum over the names
    /// declared there, so a line is covered when any of its functions ran.
    functions: Vec<(u32, u64)>,
}

/// A file being read: its `DA` records, its `FN` declarations and the `FNDA` hits by name.
struct Pending {
    path: PathBuf,
    lines: Vec<(u32, u64)>,
    declared: Vec<(u32, String)>,
    hits: BTreeMap<String, u64>,
}

impl Pending {
    fn finish(self) -> LcovFile {
        let mut by_line: BTreeMap<u32, u64> = BTreeMap::new();
        for (line, name) in &self.declared {
            let hits = self.hits.get(name).copied().unwrap_or(0);
            let total = by_line.entry(*line).or_insert(0);
            *total = total.saturating_add(hits);
        }
        LcovFile {
            path: self.path,
            lines: self.lines,
            functions: by_line.into_iter().collect(),
        }
    }
}

/// Reads the `SF`, `DA`, `FN` and `FNDA` records of `text` and ignores every other record.
fn parse_lcov(text: &str) -> Result<Vec<LcovFile>, String> {
    let mut files = Vec::new();
    let mut current: Option<Pending> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let bad = |why: &str| format!("lcov.info line {number}: malformed record `{line}`: {why}");
        let number_field = |field: &str, what: &str| {
            field
                .parse::<u64>()
                .map_err(|_| bad(&format!("{what} `{field}` is not a number")))
        };
        let line_field = |field: &str| {
            field
                .parse::<u32>()
                .map_err(|_| bad(&format!("line `{field}` is not a line number")))
        };
        if let Some(path) = line.strip_prefix("SF:") {
            files.extend(current.take().map(Pending::finish));
            current = Some(Pending {
                path: PathBuf::from(path),
                lines: Vec::new(),
                declared: Vec::new(),
                hits: BTreeMap::new(),
            });
        } else if line == "end_of_record" {
            files.extend(current.take().map(Pending::finish));
        } else if let Some(rest) = line.strip_prefix("DA:") {
            let file = current
                .as_mut()
                .ok_or_else(|| bad("DA record before any SF record"))?;
            // `DA:<line>,<hits>[,<checksum>]`
            let mut fields = rest.splitn(3, ',');
            let (Some(at), Some(hits)) = (fields.next(), fields.next()) else {
                return Err(bad("expected `DA:<line>,<hits>`"));
            };
            file.lines
                .push((line_field(at)?, number_field(hits, "hit count")?));
        } else if let Some(rest) = line.strip_prefix("FN:") {
            let file = current
                .as_mut()
                .ok_or_else(|| bad("FN record before any SF record"))?;
            // `FN:<line>,<name>` or `FN:<line>,<end line>,<name>`. Numbers come from the left;
            // the rest is the name, which may itself contain commas.
            let Some((at, rest)) = rest.split_once(',') else {
                return Err(bad("expected `FN:<line>[,<end line>],<name>`"));
            };
            let name = match rest.split_once(',') {
                Some((end, name)) if end.parse::<u32>().is_ok() => name,
                _ => rest,
            };
            file.declared.push((line_field(at)?, name.to_string()));
        } else if let Some(rest) = line.strip_prefix("FNDA:") {
            let file = current
                .as_mut()
                .ok_or_else(|| bad("FNDA record before any SF record"))?;
            let Some((hits, name)) = rest.split_once(',') else {
                return Err(bad("expected `FNDA:<hits>,<name>`"));
            };
            let hits = number_field(hits, "hit count")?;
            let total = file.hits.entry(name.to_string()).or_insert(0);
            *total = total.saturating_add(hits);
        }
    }
    files.extend(current.take().map(Pending::finish));
    Ok(files)
}

/// The crate directory name of a source file: `libs/<name>/src/...` or `examples/<name>/src/...`
/// under `root`. Any other path is an error, because dropping it silently would skew the totals.
fn crate_of(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        format!(
            "{} is outside the workspace root {}",
            path.display(),
            root.display()
        )
    })?;
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        ["libs" | "examples", name, "src", _, ..] => Ok((*name).to_string()),
        _ => Err(format!(
            "{} is outside the counted patterns (libs/*/src and examples/*/src)",
            path.display()
        )),
    }
}

/// Covered and total lines and functions.
#[derive(Clone, Copy, Default)]
struct Counts {
    lines: u64,
    lines_hit: u64,
    functions: u64,
    functions_hit: u64,
}

impl Counts {
    fn add(&mut self, other: Self) {
        self.lines += other.lines;
        self.lines_hit += other.lines_hit;
        self.functions += other.functions;
        self.functions_hit += other.functions_hit;
    }
}

/// Counts `file` outside `test_ranges`, and the number of `DA` lines inside them.
fn count(file: &LcovFile, test_ranges: &[RangeInclusive<u32>]) -> (Counts, u64) {
    let in_test = |line: &u32| test_ranges.iter().any(|r| r.contains(line));
    let mut counts = Counts::default();
    let mut excluded = 0;
    for (line, hits) in &file.lines {
        if in_test(line) {
            excluded += 1;
        } else {
            counts.lines += 1;
            counts.lines_hit += u64::from(*hits > 0);
        }
    }
    for (line, hits) in &file.functions {
        if !in_test(line) {
            counts.functions += 1;
            counts.functions_hit += u64::from(*hits > 0);
        }
    }
    (counts, excluded)
}

/// `covered/total (p.p%)`, the percentage in per-mille half up with integers only, or a dash when
/// there is nothing to cover.
fn cell(hit: u64, total: u64) -> String {
    if total == 0 {
        return "\u{2014}".to_string();
    }
    let per_mille = (hit * 1000 * 2 + total) / (total * 2);
    format!("{hit}/{total} ({}.{}%)", per_mille / 10, per_mille % 10)
}

/// The Markdown table, one row per crate (in name order) and a `Total` row.
fn render(rows: &BTreeMap<String, Counts>, excluded_lines: u64) -> String {
    let row = |name: &str, c: &Counts| {
        format!(
            "| {name} | {} | {} |\n",
            cell(c.lines_hit, c.lines),
            cell(c.functions_hit, c.functions)
        )
    };
    let mut total = Counts::default();
    let mut out = format!(
        "## Coverage\n\nExcluded: doctests (not run); tests/, benches/ and tooling sources; \
         {excluded_lines} instrumented lines in #[cfg(test)] items.\n\n\
         | Crate | Lines | Functions |\n|---|---|---|\n"
    );
    for (name, counts) in rows {
        total.add(*counts);
        out.push_str(&row(name, counts));
    }
    out.push_str(&row("Total", &total));
    out
}

/// The whole summary, as Markdown, from the text of `lcov.info`. `read_source` returns the text of
/// a source file named by an `SF` record.
///
/// # Errors
///
/// Returns a message naming the cause for a malformed record, a path outside the counted patterns,
/// a source that cannot be read or parsed, an out-of-line test module, or an `lcov.info` with no
/// counted file.
pub(super) fn summarize(
    root: &Path,
    lcov: &str,
    read_source: impl Fn(&Path) -> Result<String, String>,
) -> Result<String, String> {
    let files = parse_lcov(lcov)?;
    if files.is_empty() {
        return Err(
            "no counted file in lcov.info: the ignore regex or the workspace root is wrong"
                .to_string(),
        );
    }
    let mut rows: BTreeMap<String, Counts> = BTreeMap::new();
    let mut excluded = 0;
    for file in &files {
        let name = crate_of(root, &file.path)?;
        let source =
            read_source(&file.path).map_err(|e| format!("{}: {e}", file.path.display()))?;
        let ranges =
            test_item_ranges(&source).map_err(|e| format!("{}: {e}", file.path.display()))?;
        let (counts, removed) = count(file, &ranges);
        rows.entry(name).or_default().add(counts);
        excluded += removed;
    }
    Ok(render(&rows, excluded))
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    #[test]
    fn excludes_cfg_test_module() {
        let source = "fn a() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
        assert_eq!(test_item_ranges(source).unwrap(), [2..=6]);
    }

    #[test]
    fn excludes_a_test_fn_and_cfg_all_test() {
        let source = "\
#[test]
fn top() {}

struct S;
impl S {
    #[cfg(all(test, feature = \"x\"))]
    fn helper() {
        let _ = 1;
    }
}
";
        assert_eq!(test_item_ranges(source).unwrap(), [1..=2, 6..=9]);
    }

    #[test]
    fn counts_cfg_any_test_and_not_test() {
        let source = "\
#[cfg(any(test, feature = \"test-invariants\"))]
fn a() {}
#[cfg(not(test))]
fn b() {}
";
        assert_eq!(test_item_ranges(source).unwrap(), []);
    }

    #[test]
    fn finds_test_items_at_any_depth() {
        let source = "\
mod outer {
    #[cfg(test)]
    use std::fmt;
    #[cfg(test)]
    const C: u32 = 1;
    #[cfg(test)]
    mod inner {
        fn f() {}
    }
    fn live() {}
}
";
        assert_eq!(test_item_ranges(source).unwrap(), [2..=3, 4..=5, 6..=9]);
    }

    #[test]
    fn out_of_line_test_module_is_an_error() {
        let err = test_item_ranges("#[cfg(test)]\nmod tests;\n").unwrap_err();
        assert!(err.contains("out-of-line"), "{err}");
        assert!(err.contains("mod tests"), "{err}");
    }

    #[test]
    fn unparsable_source_is_an_error() {
        assert!(test_item_ranges("fn (").is_err());
    }

    // ---- summarize ----

    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    const ROOT: &str = "/w";

    /// Runs `summarize` over `lcov`, with `sources` as (path under the root, text) pairs.
    fn run(lcov: &str, sources: &[(&str, &str)]) -> Result<String, String> {
        let files: BTreeMap<PathBuf, String> = sources
            .iter()
            .map(|(p, text)| (Path::new(ROOT).join(p), (*text).to_string()))
            .collect();
        summarize(Path::new(ROOT), lcov, |p| {
            files
                .get(p)
                .cloned()
                .ok_or_else(|| format!("no source for {}", p.display()))
        })
    }

    #[test]
    fn cell_rounds_half_up_per_mille() {
        assert_eq!(cell(1, 3), "1/3 (33.3%)");
        assert_eq!(cell(2, 3), "2/3 (66.7%)");
        assert_eq!(cell(1, 8), "1/8 (12.5%)");
        assert_eq!(cell(5, 5), "5/5 (100.0%)");
        assert_eq!(cell(0, 0), "\u{2014}");
    }

    #[test]
    fn rows_are_ordered_by_crate_with_total_last() {
        let lcov = "SF:/w/libs/engine_time/src/a.rs\nDA:1,1\nDA:2,0\nend_of_record\n\
                    SF:/w/libs/engine/src/b.rs\nDA:1,3\nend_of_record\n";
        let out = run(
            lcov,
            &[
                ("libs/engine_time/src/a.rs", "fn a() {}\n"),
                ("libs/engine/src/b.rs", "fn b() {}\n"),
            ],
        )
        .unwrap();
        assert!(out.starts_with("## Coverage\n"), "{out}");
        let rows: Vec<&str> = out.lines().filter(|l| l.starts_with("| ")).collect();
        assert!(rows[1].starts_with("| engine "), "{out}");
        assert!(rows[2].starts_with("| engine_time "), "{out}");
        assert!(rows[3].starts_with("| Total "), "{out}");
        assert!(rows[3].contains("2/3 (66.7%)"), "{out}");
        assert!(
            out.lines()
                .nth(2)
                .unwrap()
                .contains(" 0 instrumented lines"),
            "{out}"
        );
    }

    #[test]
    fn test_ranges_remove_lines_and_functions() {
        let source = "fn a() {}\n\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\n";
        let lcov = "SF:/w/libs/engine/src/a.rs\nFN:1,live\nFN:4,test_fn\nFNDA:1,live\nFNDA:1,test_fn\n\
                    DA:1,1\nDA:4,1\nDA:5,1\nend_of_record\n";
        let out = run(lcov, &[("libs/engine/src/a.rs", source)]).unwrap();
        let row = out.lines().find(|l| l.starts_with("| engine ")).unwrap();
        assert!(row.contains("1/1 (100.0%) | 1/1 (100.0%)"), "{out}");
        assert!(
            out.lines()
                .nth(2)
                .unwrap()
                .contains(" 2 instrumented lines"),
            "{out}"
        );
    }

    #[test]
    fn several_fn_records_on_one_line_are_one_function() {
        let lcov = "SF:/w/libs/engine/src/a.rs\nFN:10,first\nFN:10,second\nFNDA:0,first\nFNDA:4,second\n\
                    DA:10,4\nend_of_record\n";
        let out = run(lcov, &[("libs/engine/src/a.rs", "fn a() {}\n")]).unwrap();
        let row = out.lines().find(|l| l.starts_with("| engine ")).unwrap();
        assert!(row.ends_with("| 1/1 (100.0%) |"), "{out}");
    }

    #[test]
    fn fn_record_accepts_an_optional_end_line() {
        let lcov = "SF:/w/libs/engine/src/a.rs\nFN:3,9,with_end\nFN:12,without_end\n\
                    FNDA:1,with_end\nFNDA:0,without_end\nDA:3,1\nend_of_record\n";
        let out = run(lcov, &[("libs/engine/src/a.rs", "fn a() {}\n")]).unwrap();
        let row = out.lines().find(|l| l.starts_with("| engine ")).unwrap();
        assert!(row.ends_with("| 1/2 (50.0%) |"), "{out}");
    }

    #[test]
    fn fn_record_name_may_contain_commas() {
        let lcov = "SF:/w/libs/engine/src/a.rs\nFN:5,<Foo<A, B> as Bar>::baz\n\
                    FNDA:2,<Foo<A, B> as Bar>::baz\nDA:5,2\nend_of_record\n";
        let out = run(lcov, &[("libs/engine/src/a.rs", "fn a() {}\n")]).unwrap();
        let row = out.lines().find(|l| l.starts_with("| engine ")).unwrap();
        assert!(row.ends_with("| 1/1 (100.0%) |"), "{out}");
    }

    #[test]
    fn other_records_are_ignored() {
        let lcov = "VER:2\nSF:/w/libs/engine/src/a.rs\nDA:1,1\nBRDA:1,0,0,1\nLF:1\nLH:1\nFNF:0\nFNH:0\nend_of_record\n";
        let out = run(lcov, &[("libs/engine/src/a.rs", "fn a() {}\n")]).unwrap();
        let row = out.lines().find(|l| l.starts_with("| engine ")).unwrap();
        assert!(row.contains("1/1 (100.0%)"), "{out}");
    }

    #[test]
    fn path_outside_the_patterns_is_an_error() {
        let lcov = "SF:/w/xtask/src/main.rs\nDA:1,1\nend_of_record\n";
        let err = run(lcov, &[("xtask/src/main.rs", "")]).unwrap_err();
        assert!(err.contains("/w/xtask/src/main.rs"), "{err}");
        assert!(err.contains("outside the counted patterns"), "{err}");
    }

    #[test]
    fn path_outside_the_root_is_an_error() {
        let lcov = "SF:/elsewhere/libs/x/src/a.rs\nDA:1,1\nend_of_record\n";
        let err = run(lcov, &[]).unwrap_err();
        assert!(err.contains("outside the workspace root"), "{err}");
    }

    #[test]
    fn no_counted_file_is_an_error() {
        let err = run("", &[]).unwrap_err();
        assert!(err.contains("no counted file"), "{err}");
    }

    #[test]
    fn da_before_sf_is_an_error() {
        let err = run("DA:1,1\n", &[]).unwrap_err();
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("before any SF"), "{err}");
    }

    #[test]
    fn non_numeric_da_is_an_error() {
        let lcov = "SF:/w/libs/engine/src/a.rs\nDA:1,x\nend_of_record\n";
        let err = run(lcov, &[("libs/engine/src/a.rs", "")]).unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("DA:1,x"), "{err}");
    }

    #[test]
    fn unparsable_source_names_the_file() {
        let lcov = "SF:/w/libs/engine/src/a.rs\nDA:1,1\nend_of_record\n";
        let err = run(lcov, &[("libs/engine/src/a.rs", "fn (")]).unwrap_err();
        assert!(err.contains("/w/libs/engine/src/a.rs"), "{err}");
    }
}
