//! Finding the bench executables in the JSON messages of `cargo bench --no-run
//! --message-format=json` (spec M2.4 §4.4).

use std::path::PathBuf;

use serde_json::Value;

/// A compiled bench target.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct BenchExe {
    /// The target name, such as `prng`.
    pub(super) name: String,
    /// The executable.
    pub(super) path: PathBuf,
}

/// The bench executables among Cargo's messages, sorted by target name.
///
/// Each line is a JSON message; Cargo may interleave plain text, which is skipped. Only
/// `compiler-artifact` messages of a `bench` target count: the same build also reports the
/// library and build-script artifacts. It is an error when the build did not succeed or produced
/// no bench executable.
pub(super) fn bench_executables(messages: &str) -> Result<Vec<BenchExe>, String> {
    let mut exes = Vec::new();
    for message in messages
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        match message["reason"].as_str() {
            Some("build-finished") if message["success"] == Value::Bool(false) => {
                return Err("the build failed (see the compiler output above)".to_string());
            }
            Some("compiler-artifact") => {
                let is_bench = message["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|k| k == "bench"));
                if let (true, Some(name), Some(path)) = (
                    is_bench,
                    message["target"]["name"].as_str(),
                    message["executable"].as_str(),
                ) {
                    exes.push(BenchExe {
                        name: name.to_string(),
                        path: PathBuf::from(path),
                    });
                }
            }
            _ => {}
        }
    }
    if exes.is_empty() {
        return Err("cargo built no bench executable".to_string());
    }
    exes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(exes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = r#"{"reason":"compiler-artifact","package_id":"p","target":{"kind":["lib"],"crate_types":["lib"],"name":"engine_bench_regress"},"profile":{"test":true},"executable":"/t/release/deps/engine_bench_regress-1","fresh":true}"#;
    const BUILD_SCRIPT: &str = r#"{"reason":"compiler-artifact","package_id":"q","target":{"kind":["custom-build"],"crate_types":["bin"],"name":"build-script-build"},"executable":"/t/release/build/q/build_script_build-2","fresh":true}"#;
    const PRNG: &str = r#"{"reason":"compiler-artifact","package_id":"p","target":{"kind":["bench"],"crate_types":["bin"],"name":"prng"},"executable":"/t/release/deps/prng-34248b49","fresh":true}"#;
    const HANDLE: &str = r#"{"reason":"compiler-artifact","package_id":"p","target":{"kind":["bench"],"crate_types":["bin"],"name":"handle"},"executable":"/t/release/deps/handle-99e28a33","fresh":false}"#;
    const FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn bench_executables_picks_bench_artifacts() {
        let messages = [LIB, BUILD_SCRIPT, PRNG, HANDLE, FINISHED].join("\n");
        let exes = bench_executables(&messages).unwrap();
        let found: Vec<_> = exes
            .iter()
            .map(|e| (e.name.as_str(), e.path.to_str().unwrap()))
            .collect();
        assert_eq!(
            found,
            [
                ("handle", "/t/release/deps/handle-99e28a33"),
                ("prng", "/t/release/deps/prng-34248b49")
            ]
        );
    }

    #[test]
    fn no_bench_executable_is_an_error() {
        let messages = [LIB, BUILD_SCRIPT, FINISHED].join("\n");
        assert!(bench_executables(&messages).is_err());
        assert!(bench_executables("").is_err());
    }

    #[test]
    fn a_non_json_line_is_skipped() {
        let messages = [
            "   Compiling engine_bench_regress v0.1.0",
            PRNG,
            "",
            FINISHED,
        ]
        .join("\n");
        assert_eq!(bench_executables(&messages).unwrap().len(), 1);
    }

    #[test]
    fn a_failed_build_is_an_error() {
        let messages = [PRNG, r#"{"reason":"build-finished","success":false}"#].join("\n");
        let error = bench_executables(&messages).unwrap_err();
        assert!(error.contains("build"), "{error}");
    }
}
