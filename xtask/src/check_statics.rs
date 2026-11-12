//! `check-statics`: scan library sources for global mutable state (ADR 0008).

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// Type names that make a `static` interior-mutable.
const INTERIOR_MUTABLE: &[&str] = &[
    "Cell",
    "RefCell",
    "UnsafeCell",
    "Mutex",
    "RwLock",
    "OnceCell",
    "OnceLock",
    "LazyLock",
    "LazyCell",
    "Lazy",
    "LazyLock",
    "OnceBox",
    "Once",
    "Condvar",
    "Barrier",
];

/// Marker comment that sanctions a finding when placed on the line above it.
const ALLOW_MARKER: &str = "// engine:allow-global-state";

/// One global-state finding.
#[derive(Debug)]
pub(crate) struct Finding {
    pub(crate) path: PathBuf,
    pub(crate) line: usize,
    pub(crate) what: String,
    pub(crate) allowed_by_comment: bool,
}

struct TypeIdents(Vec<String>);

impl<'ast> Visit<'ast> for TypeIdents {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.0.push(ident.to_string());
    }
}

struct Scanner {
    hits: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for Scanner {
    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        let mut idents = TypeIdents(Vec::new());
        syn::visit::visit_type(&mut idents, &item.ty);
        let bad = idents
            .0
            .iter()
            .any(|i| INTERIOR_MUTABLE.contains(&i.as_str()) || i.starts_with("Atomic"));
        let line = item.static_token.span.start().line;
        if matches!(item.mutability, syn::StaticMutability::Mut(_)) {
            self.hits.push((
                line,
                format!("`static mut {}` is global mutable state", item.ident),
            ));
        } else if bad {
            self.hits.push((
                line,
                format!("static `{}` has interior mutability", item.ident),
            ));
        }
        syn::visit::visit_item_static(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "thread_local")
        {
            let line = mac.path.segments[0].ident.span().start().line;
            self.hits
                .push((line, "thread_local! declares global state".to_string()));
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// Finds interior-mutable statics and `thread_local!` uses in `src`.
pub(crate) fn scan_statics(path: &Path, src: &str) -> Vec<Finding> {
    let file = match syn::parse_file(src) {
        Ok(f) => f,
        Err(e) => {
            return vec![Finding {
                path: path.to_path_buf(),
                line: 0,
                what: format!("failed to parse: {e}"),
                allowed_by_comment: false,
            }];
        }
    };
    let mut scanner = Scanner { hits: Vec::new() };
    scanner.visit_file(&file);
    let lines: Vec<&str> = src.lines().collect();
    scanner
        .hits
        .into_iter()
        .map(|(line, what)| Finding {
            path: path.to_path_buf(),
            line,
            what,
            allowed_by_comment: line >= 2
                && lines
                    .get(line - 2)
                    .is_some_and(|l| l.trim().starts_with(ALLOW_MARKER)),
        })
        .collect()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Scans `libs/*/src/**/*.rs`; fails on findings without an allow comment.
pub(crate) fn run() -> Result<(), String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir("libs") {
        for e in entries.filter_map(Result::ok) {
            rs_files(&e.path().join("src"), &mut files);
        }
    }
    files.sort();
    let mut failures = Vec::new();
    for file in &files {
        let src = fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        for f in scan_statics(file, &src) {
            let msg = format!("{}:{}: {}", f.path.display(), f.line, f.what);
            if f.allowed_by_comment {
                println!("allowed: {msg}");
            } else {
                failures.push(msg);
            }
        }
    }
    if failures.is_empty() {
        println!("check-statics: ok ({} files)", files.len());
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn flags_interior_mutable_statics_and_thread_local() {
        let src = "static A: std::sync::Mutex<u8> = std::sync::Mutex::new(0);\n\
                   static B: AtomicU32 = AtomicU32::new(0);\n\
                   static C: [u8; 2] = [0, 1];\n\
                   thread_local! { static D: u8 = 0; }\n";
        let f = scan_statics(Path::new("x.rs"), src);
        assert_eq!(f.iter().map(|f| f.line).collect::<Vec<_>>(), vec![1, 2, 4]);
    }

    #[test]
    fn flags_static_mut_and_more_lazy_types() {
        let src = "static mut A: u32 = 0;\n\
                   static B: Lazy<u8> = Lazy::new(|| 0);\n\
                   static C: OnceBox<u8> = OnceBox::new();\n\
                   static D: Once = Once::new();\n\
                   static E: Condvar = Condvar::new();\n\
                   static F: Barrier = Barrier::new(1);\n\
                   static G: [u8; 2] = [0, 1];\n";
        let f = scan_statics(Path::new("x.rs"), src);
        assert_eq!(
            f.iter().map(|f| f.line).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5, 6]
        );
    }

    #[test]
    fn honors_allow_comment() {
        let src = "// engine:allow-global-state: diagnostics facade\nstatic A: OnceLock<u8> = OnceLock::new();\n";
        assert!(scan_statics(Path::new("x.rs"), src)[0].allowed_by_comment);
    }
}
