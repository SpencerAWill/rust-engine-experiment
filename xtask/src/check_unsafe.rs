//! `check-unsafe`: every crate root must forbid `unsafe` unless allowlisted.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// Directories of every workspace member, as `root` joined with the member's path, from
/// `cargo metadata` run in `root` (the current directory when `root` is empty). The `members` list
/// of the root `Cargo.toml` is the only list, so a crate that is added or moved cannot silently
/// drop out of the check.
pub(crate) fn workspace_crates(root: &Path) -> Result<Vec<PathBuf>, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .stderr(Stdio::inherit());
    if !root.as_os_str().is_empty() {
        command.current_dir(root);
    }
    let output = command
        .output()
        .map_err(|e| format!("failed to start `cargo metadata`: {e}"))?;
    if !output.status.success() {
        return Err(format!("`cargo metadata` failed ({})", output.status));
    }
    let members = member_dirs(&String::from_utf8_lossy(&output.stdout))?;
    Ok(members.iter().map(|dir| root.join(dir)).collect())
}

/// The directories of the workspace members in `cargo metadata --no-deps` output, relative to the
/// workspace root and sorted.
fn member_dirs(metadata: &str) -> Result<Vec<PathBuf>, String> {
    let json: serde_json::Value =
        serde_json::from_str(metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let missing = |field: &str| format!("cargo metadata: no `{field}`");
    let root = json["workspace_root"]
        .as_str()
        .ok_or_else(|| missing("workspace_root"))?;
    let members: Vec<&str> = json["workspace_members"]
        .as_array()
        .ok_or_else(|| missing("workspace_members"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let mut dirs = Vec::new();
    for package in json["packages"]
        .as_array()
        .ok_or_else(|| missing("packages"))?
    {
        if !package["id"]
            .as_str()
            .is_some_and(|id| members.contains(&id))
        {
            continue;
        }
        let manifest = package["manifest_path"]
            .as_str()
            .ok_or_else(|| missing("manifest_path"))?;
        let dir = Path::new(manifest)
            .parent()
            .and_then(|dir| dir.strip_prefix(root).ok())
            .ok_or_else(|| format!("cargo metadata: {manifest} is not below {root}"))?;
        dirs.push(dir.to_path_buf());
    }
    dirs.sort();
    Ok(dirs)
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
    for dir in workspace_crates(Path::new(""))? {
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
    fn every_workspace_member_is_checked() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let checked: Vec<PathBuf> = workspace_crates(root)
            .unwrap()
            .iter()
            .map(|dir| dir.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        // One member of each kind the root `Cargo.toml` lists: a glob member and plain members.
        for member in [
            "libs/engine_handle",
            "examples/agents_2d",
            "benches",
            "bench-regress",
            "xtask",
        ] {
            assert!(
                checked.contains(&PathBuf::from(member)),
                "{member} is a workspace member but check-unsafe does not check it: {checked:?}"
            );
        }
    }

    #[test]
    fn member_dirs_keeps_only_workspace_members_relative_to_the_root() {
        let metadata = r#"{
            "workspace_root": "/repo",
            "workspace_members": ["path+file:///repo/libs/a#0.1.0", "path+file:///repo/xtask#0.1.0"],
            "packages": [
                {"id": "path+file:///repo/xtask#0.1.0", "manifest_path": "/repo/xtask/Cargo.toml"},
                {"id": "path+file:///repo/libs/a#0.1.0", "manifest_path": "/repo/libs/a/Cargo.toml"},
                {"id": "registry+https://x#dep@1.0.0", "manifest_path": "/home/.cargo/dep/Cargo.toml"}
            ]
        }"#;
        assert_eq!(
            member_dirs(metadata).unwrap(),
            vec![PathBuf::from("libs/a"), PathBuf::from("xtask")]
        );
        assert!(member_dirs("not json").is_err());
        assert!(member_dirs(r#"{"workspace_root": "/repo"}"#).is_err());
    }

    #[test]
    fn parses_allowlist_names() {
        let toml = "# c\n[[crate]]\nname = \"engine_ffi\"\nadr = \"docs/adr/0010-x.md\"\n";
        assert_eq!(parse_allowlist(toml), vec!["engine_ffi".to_string()]);
        assert_eq!(parse_allowlist("# empty\n"), Vec::<String>::new());
    }
}
