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

/// The workspace as `cargo metadata` reports it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Workspace {
    /// The workspace root, as an absolute path.
    pub(crate) root: PathBuf,
    /// The directory of every member, relative to `root`, sorted.
    pub(crate) members: Vec<PathBuf>,
}

/// The workspace that contains `dir`, from `cargo metadata` run there, so the check does not
/// depend on the directory it is started from. The `members` list of the root `Cargo.toml` is the
/// only list, so a crate that is added or moved cannot silently drop out of the check.
pub(crate) fn workspace(dir: &Path) -> Result<Workspace, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(dir)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("failed to start `cargo metadata`: {e}"))?;
    if !output.status.success() {
        return Err(format!("`cargo metadata` failed ({})", output.status));
    }
    parse_workspace(&String::from_utf8_lossy(&output.stdout))
}

/// The workspace in `cargo metadata --no-deps` output.
fn parse_workspace(metadata: &str) -> Result<Workspace, String> {
    let json: serde_json::Value =
        serde_json::from_str(metadata).map_err(|e| format!("cargo metadata: {e}"))?;
    let missing = |field: &str| format!("cargo metadata: no `{field}`");
    let root = json["workspace_root"]
        .as_str()
        .ok_or_else(|| missing("workspace_root"))?;
    let ids: Vec<&str> = json["workspace_members"]
        .as_array()
        .ok_or_else(|| missing("workspace_members"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    let mut members = Vec::new();
    for package in json["packages"]
        .as_array()
        .ok_or_else(|| missing("packages"))?
    {
        if !package["id"].as_str().is_some_and(|id| ids.contains(&id)) {
            continue;
        }
        let manifest = package["manifest_path"]
            .as_str()
            .ok_or_else(|| missing("manifest_path"))?;
        let dir = Path::new(manifest)
            .parent()
            .and_then(|dir| dir.strip_prefix(root).ok())
            .ok_or_else(|| format!("cargo metadata: {manifest} is not below {root}"))?;
        members.push(dir.to_path_buf());
    }
    members.sort();
    Ok(Workspace {
        root: PathBuf::from(root),
        members,
    })
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
    let workspace = workspace(Path::new("."))?;
    let allowlist = workspace.root.join("unsafe-allowlist.toml");
    let allow = parse_allowlist(&fs::read_to_string(allowlist).unwrap_or_default());
    let mut failures = Vec::new();
    for member in &workspace.members {
        let dir = workspace.root.join(member);
        if package_name(&dir).is_some_and(|n| allow.contains(&n)) {
            continue;
        }
        for path in crate_roots(&dir) {
            if let Ok(src) = fs::read_to_string(&path)
                && !crate_root_forbids_unsafe(&src)
            {
                let shown = path.strip_prefix(&workspace.root).unwrap_or(&path);
                failures.push(format!(
                        "{}: missing #![forbid(unsafe_code)] (add it, or list the crate in unsafe-allowlist.toml with an ADR)",
                        shown.display()
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
        let checked = workspace(root).unwrap().members;
        // One member of each kind the root `Cargo.toml` lists: a glob member and plain members.
        for member in [
            "libs/engine_handle",
            "examples/agents_2d",
            "bench/compare",
            "bench/regress",
            "xtask",
        ] {
            assert!(
                checked.contains(&PathBuf::from(member)),
                "{member} is a workspace member but check-unsafe does not check it: {checked:?}"
            );
        }
    }

    #[test]
    fn the_workspace_is_found_from_a_subdirectory() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let from_root = workspace(root).unwrap();
        let from_libs = workspace(&root.join("libs")).unwrap();
        assert_eq!(from_libs, from_root);
        for member in &from_libs.members {
            let dir = from_libs.root.join(member);
            assert!(
                dir.join("Cargo.toml").is_file(),
                "{} is not a crate",
                dir.display()
            );
        }
    }

    #[test]
    fn parse_workspace_keeps_only_members_relative_to_the_root() {
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
            parse_workspace(metadata).unwrap(),
            Workspace {
                root: PathBuf::from("/repo"),
                members: vec![PathBuf::from("libs/a"), PathBuf::from("xtask")],
            }
        );
        assert!(parse_workspace("not json").is_err());
        assert!(parse_workspace(r#"{"workspace_root": "/repo"}"#).is_err());
    }

    #[test]
    fn parses_allowlist_names() {
        let toml = "# c\n[[crate]]\nname = \"engine_ffi\"\nadr = \"docs/adr/0010-x.md\"\n";
        assert_eq!(parse_allowlist(toml), vec!["engine_ffi".to_string()]);
        assert_eq!(parse_allowlist("# empty\n"), Vec::<String>::new());
    }
}
