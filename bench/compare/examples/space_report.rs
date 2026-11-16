// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::print_stdout
)]
//! Heap space of the bitsets after building, at their peak, and after `shrink_to_fit`.
//!
//! `cargo run -p engine_benches --release --example space_report` prints a Markdown table.
//! The cases mirror `benches/collections.rs` (a 1,048,576-index range; occupancies of 0.1%, 1%,
//! 50% and 99%; `uniform` and `clustered` runs of 256), plus one member at 1,000,000.
//!
//! `dhat`'s peak is per process, so the parent spawns itself once per case with
//! `--case <structure> <case>`. A child lists its members first, then builds the structure empty
//! (not presized), inserting them one at a time in a seeded random order, and reads
//! `dhat::HeapStats`. The member list stays alive while the structure is built, so its size is
//! subtracted from every figure. `dhat` runs in testing mode: no `dhat-heap.json` is written.

use std::process::Command;

use dhat::{HeapStats, Profiler};
use engine::collections::{BitSet, HierarchicalBitSet};
use engine::random::Prng;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const RANGE: u32 = 1_048_576;
const RUN: u32 = 256;
const SEED: u64 = 0x5EED;
const ONE_MEMBER: &str = "one member at 1,000,000";
/// Occupancy in parts per thousand, with its label.
const DENSITIES: [(u32, &str); 4] = [(1, "0.1%"), (10, "1%"), (500, "50%"), (990, "99%")];
const SPREADS: [&str; 2] = ["uniform", "clustered"];

const STRUCTURES: [&str; 4] = [
    "engine::HierarchicalBitSet",
    "engine::BitSet",
    "hibitset::BitSet",
    "hi_sparse_bitset::BitSet<_128bit>",
];

/// Every case label, in table order.
fn cases() -> Vec<String> {
    let mut cases = vec![ONE_MEMBER.to_string()];
    for spread in SPREADS {
        for (_, density) in DENSITIES {
            cases.push(format!("{density} {spread}"));
        }
    }
    cases
}

/// The members of a case in a random insertion order, in a vector of exactly their count.
fn members(case: &str) -> Vec<u32> {
    if case == ONE_MEMBER {
        return vec![1_000_000];
    }
    let (density, spread) = case.split_once(' ').expect("case is `<density> <spread>`");
    let permille = DENSITIES
        .iter()
        .find(|d| d.1 == density)
        .expect("known density")
        .0;
    let target = (RANGE * permille / 1_000) as usize;
    let mut prng = Prng::from_seed(SEED);
    let mut members = Vec::with_capacity(target.next_multiple_of(RUN as usize));
    match spread {
        "uniform" => {
            // Rejection sampling yields a random order already, with a 128 KiB bitmap and no
            // range-sized vector whose peak would count.
            let mut taken = vec![0_u64; (RANGE / 64) as usize];
            while members.len() < target {
                let i = prng.range(0..RANGE);
                let (word, bit) = ((i / 64) as usize, i % 64);
                if taken[word] >> bit & 1 == 0 {
                    taken[word] |= 1 << bit;
                    members.push(i);
                }
            }
        }
        "clustered" => {
            let mut runs: Vec<u32> = (0..RANGE / RUN).collect();
            prng.shuffle(&mut runs);
            runs.truncate((((target as u32 + RUN / 2) / RUN).max(1)) as usize);
            for run in runs {
                members.extend(run * RUN..(run + 1) * RUN);
            }
            prng.shuffle(&mut members);
        }
        _ => panic!("unknown spread {spread}"),
    }
    members.shrink_to_fit();
    members
}

/// A structure under test: built empty, grown by `insert`, optionally shrunk.
trait Space: Sized {
    fn empty() -> Self;
    fn insert(&mut self, i: u32);
    /// Releases spare capacity; `false` where the structure has no such method.
    fn shrink(&mut self) -> bool;
}

impl Space for HierarchicalBitSet {
    fn empty() -> Self {
        Self::default()
    }
    fn insert(&mut self, i: u32) {
        HierarchicalBitSet::insert(self, i as usize);
    }
    fn shrink(&mut self) -> bool {
        self.shrink_to_fit();
        true
    }
}

impl Space for BitSet {
    fn empty() -> Self {
        Self::default()
    }
    fn insert(&mut self, i: u32) {
        BitSet::insert(self, i as usize);
    }
    fn shrink(&mut self) -> bool {
        self.shrink_to_fit();
        true
    }
}

impl Space for hibitset::BitSet {
    fn empty() -> Self {
        // It has a fixed capacity of its maximum index; `new` is its only empty constructor.
        hibitset::BitSet::new()
    }
    fn insert(&mut self, i: u32) {
        self.add(i);
    }
    fn shrink(&mut self) -> bool {
        false
    }
}

impl Space for hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit> {
    fn empty() -> Self {
        Self::new()
    }
    fn insert(&mut self, i: u32) {
        Self::insert(self, i as usize);
    }
    fn shrink(&mut self) -> bool {
        false
    }
}

/// One measured row.
#[derive(Debug, PartialEq, Eq)]
struct Row {
    structure: String,
    case: String,
    members: usize,
    built: usize,
    peak: usize,
    shrunk: Option<usize>,
}

/// Builds `case` in `S` and reads the heap. The caller holds a `dhat` profiler in testing mode.
fn measure<S: Space>(structure: &str, case: &str) -> Row {
    let members = members(case);
    let baseline = HeapStats::get().curr_bytes;
    let mut set = S::empty();
    for &i in &members {
        set.insert(i);
    }
    let stats = HeapStats::get();
    let built = stats.curr_bytes - baseline;
    let peak = stats.max_bytes - baseline;
    let shrunk = set.shrink().then(|| HeapStats::get().curr_bytes - baseline);
    std::hint::black_box(&set);
    Row {
        structure: structure.to_string(),
        case: case.to_string(),
        members: members.len(),
        built,
        peak,
        shrunk,
    }
}

fn run_case(structure: &str, case: &str) -> Row {
    match structure {
        "engine::HierarchicalBitSet" => measure::<HierarchicalBitSet>(structure, case),
        "engine::BitSet" => measure::<BitSet>(structure, case),
        "hibitset::BitSet" => measure::<hibitset::BitSet>(structure, case),
        "hi_sparse_bitset::BitSet<_128bit>" => {
            measure::<hi_sparse_bitset::BitSet<hi_sparse_bitset::config::_128bit>>(structure, case)
        }
        _ => panic!("unknown structure {structure}"),
    }
}

fn child(structure: &str, case: &str) {
    let _profiler = Profiler::builder().testing().build();
    let row = run_case(structure, case);
    let shrunk = row.shrunk.map_or("-".to_string(), |b| b.to_string());
    println!("{}\t{}\t{}\t{}", row.members, row.built, row.peak, shrunk);
}

fn spawn(structure: &str, case: &str) -> String {
    let exe = std::env::current_exe().expect("own path");
    let out = Command::new(exe)
        .args(["--case", structure, case])
        .output()
        .expect("spawn child");
    assert!(
        out.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .trim()
        .to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, structure, case] = args.as_slice() {
        assert_eq!(
            flag, "--case",
            "usage: space_report [--case <structure> <case>]"
        );
        child(structure, case);
        return;
    }
    println!(
        "| structure | case | members | bytes after building | peak bytes | bytes after shrink_to_fit | bytes per member |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|");
    for structure in STRUCTURES {
        for case in cases() {
            let row = spawn(structure, &case);
            let f: Vec<&str> = row.split('\t').collect();
            let members: u64 = f[0].parse().unwrap();
            let built: u64 = f[1].parse().unwrap();
            let shrunk = if f[3] == "-" { "—" } else { f[3] };
            let per = if members == 0 {
                "—".to_string()
            } else {
                format!("{:.2}", built as f64 / members as f64)
            };
            println!(
                "| {structure} | {case} | {members} | {built} | {} | {shrunk} | {per} |",
                f[2]
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Layer 0 alone is 15,625 words of 8 bytes for index 1,000,000, so a structure that grows
    /// to hold it has well over 125,000 bytes. (In-process: the test harness cannot take
    /// `--case`, and this is the only test that starts a profiler.)
    #[test]
    fn one_member_at_1_000_000_reports_layer_zero() {
        let _profiler = Profiler::builder().testing().build();
        let row = run_case("engine::HierarchicalBitSet", ONE_MEMBER);
        assert_eq!(row.members, 1);
        assert!(row.built >= 125_000, "built only {} bytes", row.built);
        assert!(row.peak >= row.built);
    }
}
