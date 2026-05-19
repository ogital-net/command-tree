//! Heap allocation profile per library, measured with `dhat`.
//!
//! Run as:
//!
//! ```text
//! cargo run --release --example alloc_profile -- command_trie
//! cargo run --release --example alloc_profile -- radix_trie
//! cargo run --release --example alloc_profile -- trie_rs
//! cargo run --release --example alloc_profile -- fst_map
//! cargo run --release --example alloc_profile -- btreemap
//! cargo run --release --example alloc_profile -- all
//! cargo run --release --example alloc_profile -- cap_32k
//! ```
//!
//! The `cap_32k` variant profiles `command_trie` only with a dense corpus
//! of ~32,000 realistic command-name-style keys — close to the documented
//! cap (~32,767 entries, see `CommandTrieBuilder::build`).
//!
//! Each variant builds the same 65-entry git-style command corpus and prints
//! `dhat::HeapStats` snapshots taken (1) just before build, and (2) just
//! after build (but before any structure is dropped), so the delta reflects
//! the cost of the populated data structure itself.

use std::collections::BTreeMap;
use std::env;

use command_trie::CommandTrieBuilder;
use fst::{Map as FstMap, MapBuilder};
use radix_trie::Trie as RadixTrie;
use trie_rs::map::TrieBuilder as TrieRsBuilder;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn corpus() -> Vec<(&'static str, u64)> {
    let keys = [
        "add",
        "alias",
        "annotate",
        "apply",
        "archive",
        "bisect",
        "blame",
        "branch",
        "bundle",
        "checkout",
        "cherry-pick",
        "clean",
        "clone",
        "command",
        "commit",
        "config",
        "credential",
        "describe",
        "diff",
        "difftool",
        "fetch",
        "filter-branch",
        "format-patch",
        "fsck",
        "gc",
        "grep",
        "gui",
        "help",
        "init",
        "instaweb",
        "log",
        "merge",
        "mergetool",
        "mv",
        "notes",
        "pull",
        "push",
        "range-diff",
        "rebase",
        "reflog",
        "remote",
        "repack",
        "replace",
        "request-pull",
        "reset",
        "restore",
        "revert",
        "rev-parse",
        "rm",
        "send-email",
        "shortlog",
        "show",
        "show-branch",
        "stage",
        "stash",
        "status",
        "submodule",
        "switch",
        "tag",
        "verify-commit",
        "verify-tag",
        "version",
        "whatchanged",
        "worktree",
    ];
    keys.iter()
        .enumerate()
        .map(|(i, k)| (*k, i as u64))
        .collect()
}

/// Build a corpus of ~32,000 realistic command-name-style keys
/// (think `print -rl -- ${(k)commands}` on a busy `$PATH`) — close to the
/// documented `~32,767` entry cap. Uses the same prefix/stem/bucket
/// generator as the `insert_32k_entries_no_panic` unit test.
fn corpus_cap_32k() -> Vec<(String, u32)> {
    const PREFIXES: &[&str] = &[
        "git-",
        "cargo-",
        "docker-",
        "kubectl-",
        "npm-",
        "pip-",
        "rustup-",
        "systemctl-",
        "journalctl-",
        "ip-",
        "nmcli-",
        "brew-",
    ];
    const STEMS: &[&str] = &[
        "list", "get", "set", "show", "describe", "create", "delete", "update", "apply", "watch",
        "rollout", "exec", "logs", "status", "info", "config", "scale", "patch", "expose",
        "annotate",
    ];
    // 12 prefixes × 20 stems × 134 buckets = 32,160 unique keys, avg ~16 bytes.
    const BUCKETS: u32 = 134;
    const N: u32 = PREFIXES.len() as u32 * STEMS.len() as u32 * BUCKETS;
    let mut out = Vec::with_capacity(N as usize);
    for i in 0..N {
        use std::fmt::Write;
        let p = (i as usize) % PREFIXES.len();
        let s = (i as usize / PREFIXES.len()) % STEMS.len();
        let bucket = i as usize / (PREFIXES.len() * STEMS.len());
        let mut buf = String::new();
        buf.push_str(PREFIXES[p]);
        buf.push_str(STEMS[s]);
        buf.push('-');
        write!(buf, "{bucket:03}").unwrap();
        out.push((buf, i));
    }
    out
}

fn run_command_trie_cap_32k(items: &[(String, u32)]) {
    let payload: usize = items.iter().map(|(k, _)| k.len()).sum::<usize>()
        + items.len() * std::mem::size_of::<u32>();
    println!(
        "32k corpus: {} entries, payload lower bound = {} bytes (keys + u32 values)\n",
        items.len(),
        payload
    );
    let trie = measure("command_trie::CommandTrie<u32> (~32k entries)", || {
        let mut b = CommandTrieBuilder::new();
        for (k, v) in items {
            b.insert(k, *v);
        }
        b.build()
    });
    // Report the {:?} summary so we can see node / label / edge counts.
    println!("  shape: {trie:?}");
    std::hint::black_box(&trie);
    drop(trie);
}

fn payload_bytes(items: &[(&'static str, u64)]) -> usize {
    // Raw bytes of all keys + values, ignoring per-entry overhead. Useful as a
    // lower bound on what any non-compressing structure can hope to use.
    items.iter().map(|(k, _)| k.len()).sum::<usize>() + items.len() * std::mem::size_of::<u64>()
}

/// Run a build closure inside a dhat profiling region and report the heap
/// delta attributable to it.
fn measure<T, F: FnOnce() -> T>(label: &str, build: F) -> T {
    let before = dhat::HeapStats::get();
    let value = build();
    let after = dhat::HeapStats::get();

    let curr_delta = after.curr_bytes as i64 - before.curr_bytes as i64;
    let blocks_delta = after.curr_blocks as i64 - before.curr_blocks as i64;
    let total_bytes = after.total_bytes - before.total_bytes;
    let total_blocks = after.total_blocks - before.total_blocks;

    println!("{label}");
    println!("  live after build : {curr_delta:>8} bytes in {blocks_delta:>5} blocks (resident)");
    println!(
        "  total during build: {total_bytes:>8} bytes in {total_blocks:>5} blocks (incl. transient)"
    );
    println!(
        "  max during build  : {:>8} bytes in {:>5} blocks (peak)",
        after.max_bytes.saturating_sub(before.curr_bytes),
        after.max_blocks.saturating_sub(before.curr_blocks),
    );
    println!();
    value
}

fn run_command_trie(items: &[(&'static str, u64)]) {
    let trie = measure("command_trie::CommandTrie<u64>", || {
        let mut b = CommandTrieBuilder::new();
        for (k, v) in items {
            b.insert(k, *v);
        }
        b.build()
    });
    std::hint::black_box(&trie);
    drop(trie);
}

fn run_radix_trie(items: &[(&'static str, u64)]) {
    let trie = measure("radix_trie::Trie<&'static str, u64>", || {
        let mut t: RadixTrie<&'static str, u64> = RadixTrie::new();
        for (k, v) in items {
            t.insert(*k, *v);
        }
        t
    });
    std::hint::black_box(&trie);
    drop(trie);
}

fn run_trie_rs(items: &[(&'static str, u64)]) {
    let trie = measure("trie_rs::map::Trie<u8, u64>", || {
        let mut b: TrieRsBuilder<u8, u64> = TrieRsBuilder::new();
        for (k, v) in items {
            b.push(k, *v);
        }
        b.build()
    });
    std::hint::black_box(&trie);
    drop(trie);
}

fn run_fst(items: &[(&'static str, u64)]) {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|(k, _)| *k);
    let map = measure("fst::Map<Vec<u8>>", || {
        let mut b = MapBuilder::memory();
        for (k, v) in &sorted {
            b.insert(k, *v).unwrap();
        }
        FstMap::new(b.into_inner().unwrap()).unwrap()
    });
    std::hint::black_box(&map);
    drop(map);
}

fn run_btreemap(items: &[(&'static str, u64)]) {
    let map = measure("BTreeMap<&'static str, u64>", || {
        let mut m: BTreeMap<&'static str, u64> = BTreeMap::new();
        for (k, v) in items {
            m.insert(*k, *v);
        }
        m
    });
    std::hint::black_box(&map);
    drop(map);
}

fn main() {
    let _profiler = dhat::Profiler::builder().testing().build();

    let items = corpus();
    let which = env::args().nth(1).unwrap_or_else(|| "all".into());

    if which != "cap_32k" {
        println!(
            "corpus: {} entries, payload lower bound = {} bytes (keys + u64 values)\n",
            items.len(),
            payload_bytes(&items)
        );
    }

    match which.as_str() {
        "command_trie" => run_command_trie(&items),
        "radix_trie" => run_radix_trie(&items),
        "trie_rs" => run_trie_rs(&items),
        "fst_map" => run_fst(&items),
        "btreemap" => run_btreemap(&items),
        "all" => {
            run_command_trie(&items);
            run_radix_trie(&items);
            run_trie_rs(&items);
            run_fst(&items);
            run_btreemap(&items);
        }
        "cap_32k" => {
            let big = corpus_cap_32k();
            run_command_trie_cap_32k(&big);
        }
        other => {
            eprintln!("unknown variant: {other}");
            eprintln!(
                "expected one of: command_trie | radix_trie | trie_rs | fst_map | btreemap | all | cap_32k"
            );
            std::process::exit(2);
        }
    }
}
