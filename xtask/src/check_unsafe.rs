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

/// Runs the check over the whole workspace.
pub(crate) fn run() -> Result<(), String> {
    let allow = parse_allowlist(&fs::read_to_string("unsafe-allowlist.toml").unwrap_or_default());
    let mut failures = Vec::new();
    for dir in workspace_crates() {
        if package_name(&dir).is_some_and(|n| allow.contains(&n)) {
            continue;
        }
        for root in ["src/lib.rs", "src/main.rs"] {
            let path = dir.join(root);
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
    fn parses_allowlist_names() {
        let toml = "# c\n[[crate]]\nname = \"engine_ffi\"\nadr = \"docs/adr/0010-x.md\"\n";
        assert_eq!(parse_allowlist(toml), vec!["engine_ffi".to_string()]);
        assert!(parse_allowlist("# empty\n").is_empty());
    }
}
