//! The per-crate coverage summary, computed from `lcov.info` and the source files it names.
//!
//! Everything here is a pure function over strings, so it is tested without `cargo-llvm-cov`.

use std::ops::RangeInclusive;

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
}
