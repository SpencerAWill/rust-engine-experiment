//! `check-unsafe`: every crate root must forbid `unsafe` unless allowlisted.

use std::fs;
use std::path::{Path, PathBuf};

/// True iff some line of `src`, once trimmed, is exactly `#![forbid(unsafe_code)]`.
pub(crate) fn crate_root_forbids_unsafe(src: &str) -> bool {
    src.lines().any(|l| l.trim() == "#![forbid(unsafe_code)]")
}

/// Extracts the `name = "..."` values from an allowlist file (hand parse, no TOML dependency).
pub(crate) fn parse_allowlist(toml: &str) -> Vec<String> {
    toml.lines()
        .filter_map(|l| {
            let rest = l
                .trim()
                .strip_prefix("name")?
                .trim_start()
                .strip_prefix('=')?;
            let value = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some(value.to_string())
        })
        .collect()
}

/// The package name declared in a crate's `Cargo.toml`.
fn package_name(crate_dir: &Path) -> Option<String> {
    let manifest = fs::read_to_string(crate_dir.join("Cargo.toml")).ok()?;
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package && let Some(name) = parse_allowlist(line).pop() {
            return Some(name);
        }
    }
    None
}

/// Directories of every workspace crate: `libs/*`, `examples/*`, `benches`, `xtask`.
pub(crate) fn workspace_crates() -> Vec<PathBuf> {
    let mut crates = Vec::new();
    for group in ["libs", "examples"] {
        if let Ok(entries) = fs::read_dir(group) {
            crates.extend(
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| p.join("Cargo.toml").is_file()),
            );
        }
    }
    for single in ["benches", "xtask"] {
        let p = PathBuf::from(single);
        if p.join("Cargo.toml").is_file() {
            crates.push(p);
        }
    }
    crates.sort();
    crates
}

/// Every crate-root source file of the crate in `dir`: the library and binary roots, `build.rs`,
/// and each file (or `<name>/main.rs`) under `src/bin`, `examples`, `benches` and `tests`.
pub(crate) fn crate_roots(dir: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = ["src/lib.rs", "src/main.rs", "build.rs"]
        .iter()
        .map(|r| dir.join(r))
        .filter(|p| p.is_file())
        .collect();
    for sub in ["src/bin", "examples", "benches", "tests"] {
        let Ok(entries) = fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for path in entries.filter_map(Result::ok).map(|e| e.path()) {
            if path.extension().is_some_and(|e| e == "rs") {
                roots.push(path);
            } else if path.join("main.rs").is_file() {
                roots.push(path.join("main.rs"));
            }
        }
    }
    roots.sort();
    roots
}

/// Runs the check over the whole workspace.
pub(crate) fn run() -> Result<(), String> {
    let allow = parse_allowlist(&fs::read_to_string("unsafe-allowlist.toml").unwrap_or_default());
    let mut failures = Vec::new();
    for dir in workspace_crates() {
        if package_name(&dir).is_some_and(|n| allow.contains(&n)) {
            continue;
        }
        for path in crate_roots(&dir) {
            if let Ok(src) = fs::read_to_string(&path)
                && !crate_root_forbids_unsafe(&src)
            {
                failures.push(format!(
                        "{}: missing #![forbid(unsafe_code)] (add it, or list the crate in unsafe-allowlist.toml with an ADR)",
                        path.display()
                    ));
            }
        }
    }
    if failures.is_empty() {
        println!("check-unsafe: ok");
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_forbid_attribute() {
        assert!(crate_root_forbids_unsafe(
            "//! docs\n#![forbid(unsafe_code)]\n"
        ));
        assert!(!crate_root_forbids_unsafe("#![deny(unsafe_code)]\n"));
        assert!(!crate_root_forbids_unsafe("// #![forbid(unsafe_code)]\n"));
    }

    #[test]
    fn finds_every_crate_root_kind() {
        let dir = std::env::temp_dir().join(format!("xtask-roots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for f in [
            "src/lib.rs",
            "src/bin/tool.rs",
            "src/bin/multi/main.rs",
            "build.rs",
            "examples/ex.rs",
            "benches/b.rs",
            "tests/t.rs",
        ] {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "").unwrap();
        }
        let found: Vec<String> = crate_roots(&dir)
            .iter()
            .map(|p| {
                // Compare with `/` separators: on Windows the same path comes back as `src\bin\tool.rs`.
                p.strip_prefix(&dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        fs::remove_dir_all(&dir).unwrap();
        for want in [
            "src/lib.rs",
            "src/bin/tool.rs",
            "src/bin/multi/main.rs",
            "build.rs",
            "examples/ex.rs",
            "benches/b.rs",
            "tests/t.rs",
        ] {
            assert!(found.iter().any(|f| f == want), "missing {want}: {found:?}");
        }
    }

    #[test]
    fn parses_allowlist_names() {
        let toml = "# c\n[[crate]]\nname = \"engine_ffi\"\nadr = \"docs/adr/0010-x.md\"\n";
        assert_eq!(parse_allowlist(toml), vec!["engine_ffi".to_string()]);
        assert_eq!(parse_allowlist("# empty\n"), Vec::<String>::new());
    }
}
