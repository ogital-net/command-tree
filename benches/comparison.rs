//! Comparison benchmarks against peer libraries.
//!
//! Apples-to-apples coverage of three operations across four libraries:
//!
//! | crate              | build | get | count completions |
//! |--------------------|:-----:|:---:|:-----------------:|
//! | `command-trie`     |   x   |  x  |         x         |
//! | `trie-rs` (map)    |   x   |  x  |         x         |
//! | `radix_trie`       |   x   |  x  |         x         |
//! | `fst::Map`         |   x   |  x  |         x         |
//! | `BTreeMap` (std)   |   x   |  x  |         x         |
//!
//! Same git-style corpus as `benches/lookup.rs`. `fst` requires sorted
//! input, so we sort once up front; the build bench reflects the cost of
//! the build step itself, not the sort.
//!
//! Run with `cargo bench --bench comparison`.

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use command_trie::CommandTrieBuilder;
use fst::{IntoStreamer, Map as FstMap, MapBuilder, Streamer};
use radix_trie::{Trie as RadixTrie, TrieCommon};
use trie_rs::map::{Trie as TrieRsTrie, TrieBuilder as TrieRsBuilder};

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

fn sorted_corpus() -> Vec<(&'static str, u64)> {
    let mut c = corpus();
    c.sort_by_key(|(k, _)| *k);
    c
}

// ---------- Pre-built trees ----------

fn build_command_trie() -> command_trie::CommandTrie<u64> {
    let mut b = CommandTrieBuilder::new();
    for (k, v) in corpus() {
        b.insert(k, v);
    }
    b.build()
}

fn build_trie_rs() -> TrieRsTrie<u8, u64> {
    let mut b = TrieRsBuilder::new();
    for (k, v) in corpus() {
        b.push(k, v);
    }
    b.build()
}

fn build_radix_trie() -> RadixTrie<&'static str, u64> {
    let mut t = RadixTrie::new();
    for (k, v) in corpus() {
        t.insert(k, v);
    }
    t
}

fn build_fst() -> FstMap<Vec<u8>> {
    let mut b = MapBuilder::memory();
    for (k, v) in sorted_corpus() {
        b.insert(k, v).unwrap();
    }
    b.into_map()
}

fn build_btreemap() -> BTreeMap<&'static str, u64> {
    let mut m = BTreeMap::new();
    for (k, v) in corpus() {
        m.insert(k, v);
    }
    m
}

// ---------- build ----------

fn bench_build(c: &mut Criterion) {
    let items = corpus();
    let sorted = sorted_corpus();
    let mut group = c.benchmark_group("build");

    group.bench_function("command_trie", |b| {
        b.iter(|| {
            let mut builder = CommandTrieBuilder::new();
            for (k, v) in &items {
                builder.insert(black_box(k), black_box(*v));
            }
            black_box(builder.build())
        });
    });

    group.bench_function("trie_rs", |b| {
        b.iter(|| {
            let mut builder: TrieRsBuilder<u8, u64> = TrieRsBuilder::new();
            for (k, v) in &items {
                builder.push(black_box(k), black_box(*v));
            }
            black_box(builder.build())
        });
    });

    group.bench_function("radix_trie", |b| {
        b.iter(|| {
            let mut t: RadixTrie<&'static str, u64> = RadixTrie::new();
            for (k, v) in &items {
                t.insert(black_box(k), black_box(*v));
            }
            black_box(t)
        });
    });

    group.bench_function("fst_map", |b| {
        b.iter(|| {
            let mut builder = MapBuilder::memory();
            for (k, v) in &sorted {
                builder.insert(black_box(k), black_box(*v)).unwrap();
            }
            black_box(builder.into_map())
        });
    });

    group.bench_function("btreemap", |b| {
        b.iter(|| {
            let mut m: BTreeMap<&'static str, u64> = BTreeMap::new();
            for (k, v) in &items {
                m.insert(black_box(k), black_box(*v));
            }
            black_box(m)
        });
    });

    group.finish();
}

// ---------- get ----------

fn bench_get(c: &mut Criterion) {
    let ct = build_command_trie();
    let tr = build_trie_rs();
    let rt = build_radix_trie();
    let fst_map = build_fst();
    let bt = build_btreemap();

    let cases: &[(&str, &str)] = &[
        ("hit_short", "rm"),
        ("hit_medium", "commit"),
        ("hit_long", "verify-commit"),
        ("miss", "zzz_unknown"),
    ];

    for (name, key) in cases {
        let mut group = c.benchmark_group(format!("get/{name}"));

        group.bench_with_input(BenchmarkId::new("command_trie", name), key, |b, &k| {
            b.iter(|| black_box(ct.get(black_box(k))));
        });
        group.bench_with_input(BenchmarkId::new("trie_rs", name), key, |b, &k| {
            // trie-rs map: exact_match returns Option<&V>. Confirmed fastest
            // exact-lookup API on this corpus (inc_search is ~1.5x slower in
            // 0.4.2).
            b.iter(|| black_box(tr.exact_match(black_box(k))));
        });
        group.bench_with_input(BenchmarkId::new("radix_trie", name), key, |b, &k| {
            b.iter(|| black_box(rt.get(black_box(k))));
        });
        group.bench_with_input(BenchmarkId::new("fst_map", name), key, |b, &k| {
            b.iter(|| black_box(fst_map.get(black_box(k))));
        });
        group.bench_with_input(BenchmarkId::new("btreemap", name), key, |b, &k| {
            b.iter(|| black_box(bt.get(black_box(k))));
        });
        group.finish();
    }
}

// ---------- count completions for a prefix ----------

fn bench_count_completions(c: &mut Criterion) {
    let ct = build_command_trie();
    let tr = build_trie_rs();
    let rt = build_radix_trie();
    let fst_map = build_fst();
    let bt = build_btreemap();

    let prefixes: &[(&str, &str)] = &[
        ("single_char", "r"),
        ("branch_point", "co"),
        ("passthrough", "comm"),
        ("unique", "verify-c"),
        ("none", "zz"),
    ];

    for (name, pfx) in prefixes {
        let mut group = c.benchmark_group(format!("count_completions/{name}"));

        group.bench_with_input(BenchmarkId::new("command_trie", name), pfx, |b, &p| {
            b.iter(|| black_box(ct.count_completions(black_box(p))));
        });

        group.bench_with_input(BenchmarkId::new("trie_rs", name), pfx, |b, &p| {
            // postfix_search yields only the suffix per match (smaller alloc
            // than predictive_search which materializes prefix+suffix).
            b.iter(|| {
                let n = tr
                    .postfix_search(black_box(p))
                    .map(|(_k, _v): (Vec<u8>, &u64)| ())
                    .count();
                black_box(n)
            });
        });

        group.bench_with_input(BenchmarkId::new("radix_trie", name), pfx, |b, &p| {
            // Subtrie covers everything reachable below p; count terminal nodes.
            b.iter(|| {
                let n = match rt.get_raw_descendant(black_box(p)) {
                    Some(sub) => sub.iter().count(),
                    None => 0,
                };
                black_box(n)
            });
        });

        group.bench_with_input(BenchmarkId::new("fst_map", name), pfx, |b, &p| {
            // Search for all keys starting with `p` via the Str automaton.
            use fst::automaton::{Automaton, Str};
            let auto = Str::new(p).starts_with();
            b.iter(|| {
                let mut stream = fst_map.search(black_box(&auto)).into_stream();
                let mut n = 0usize;
                while stream.next().is_some() {
                    n += 1;
                }
                black_box(n)
            });
        });

        group.bench_with_input(BenchmarkId::new("btreemap", name), pfx, |b, &p| {
            b.iter(|| {
                let n = bt.range(p..).take_while(|(k, _)| k.starts_with(p)).count();
                black_box(n)
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_build, bench_get, bench_count_completions);
criterion_main!(benches);
