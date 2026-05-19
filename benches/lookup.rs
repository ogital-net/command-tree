//! Microbenchmarks for the read-side hot paths.
//!
//! Run with `cargo bench` (release profile). The corpus is a synthetic set
//! of git-style commands plus some long-tail keys to keep the branching
//! factor near typical CLI workloads.

use std::hint::black_box;

use command_trie::{CommandTrie, CommandTrieBuilder};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn corpus() -> Vec<(&'static str, u32)> {
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
        .map(|(i, k)| (*k, i as u32))
        .collect()
}

fn build_trie() -> CommandTrie<u32> {
    let mut b = CommandTrieBuilder::new();
    for (k, v) in corpus() {
        b.insert(k, v);
    }
    b.build()
}

fn bench_build(c: &mut Criterion) {
    let items = corpus();
    c.bench_function("build", |b| {
        b.iter(|| {
            let mut builder = CommandTrieBuilder::new();
            for (k, v) in &items {
                builder.insert(black_box(k), black_box(*v));
            }
            black_box(builder.build())
        });
    });
}

fn bench_get(c: &mut Criterion) {
    let trie = build_trie();
    let mut group = c.benchmark_group("get");
    let cases: &[(&str, &str)] = &[
        ("hit_short", "rm"),
        ("hit_medium", "commit"),
        ("hit_long", "verify-commit"),
        ("miss_short", "zz"),
        ("miss_diverge", "comx"),
    ];
    for (name, key) in cases {
        group.throughput(Throughput::Bytes(key.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), key, |b, &k| {
            b.iter(|| black_box(trie.get(black_box(k))));
        });
    }
    group.finish();
}

fn bench_longest_prefix_match(c: &mut Criterion) {
    let trie = build_trie();
    let mut group = c.benchmark_group("longest_prefix_match");
    let cases: &[(&str, &str)] = &[
        ("hit_short", "rm -rf ."),
        ("hit_medium", "commit -a -m wip"),
        ("hit_long", "verify-commit HEAD"),
        ("miss", "zzz unknown command"),
    ];
    for (name, line) in cases {
        group.throughput(Throughput::Bytes(line.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(name), line, |b, &l| {
            b.iter(|| black_box(trie.longest_prefix_match(black_box(l))));
        });
    }
    group.finish();
}

fn bench_completion(c: &mut Criterion) {
    let trie = build_trie();
    let mut group = c.benchmark_group("completion");
    let prefixes: &[(&str, &str)] = &[
        ("empty", ""),
        ("single_char", "r"),
        ("branch_point", "co"),
        ("passthrough", "comm"),
        ("unique", "verify-c"),
        ("none", "zz"),
    ];

    for (name, pfx) in prefixes {
        group.bench_with_input(BenchmarkId::new("completion_prefix", name), pfx, |b, &p| {
            b.iter(|| black_box(trie.completion_prefix(black_box(p))));
        });
        group.bench_with_input(BenchmarkId::new("count_completions", name), pfx, |b, &p| {
            b.iter(|| black_box(trie.count_completions(black_box(p))));
        });
        group.bench_with_input(BenchmarkId::new("subtrie", name), pfx, |b, &p| {
            b.iter(|| black_box(trie.subtrie(black_box(p))));
        });
    }
    group.finish();
}

fn bench_for_each(c: &mut Criterion) {
    let trie = build_trie();
    c.bench_function("for_each_all", |b| {
        b.iter(|| {
            let mut n = 0u32;
            trie.for_each(|_, v| n = n.wrapping_add(*v));
            black_box(n)
        });
    });
    c.bench_function("for_each_completion_comm", |b| {
        b.iter(|| {
            let mut n = 0u32;
            trie.for_each_completion("comm", |_, v| n = n.wrapping_add(*v));
            black_box(n)
        });
    });
}

criterion_group!(
    benches,
    bench_build,
    bench_get,
    bench_longest_prefix_match,
    bench_completion,
    bench_for_each
);
criterion_main!(benches);
