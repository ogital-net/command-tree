//! Dispatching a parsed command line against a `CommandTrie`.
//!
//! Run with: `cargo run --example command_dispatch`
//!
//! Shows the build-once / query-many pattern with a function-pointer payload:
//! `longest_prefix_match` splits the input into `(command, args)` and the
//! attached handler is invoked directly.

use command_trie::{CommandTrie, CommandTrieBuilder};

/// Each command stores its handler plus a one-line description.
struct Command {
    run: fn(&str),
    help: &'static str,
}

fn cmd_help(_args: &str) {
    println!("  [help]   available: add | commit | config | exit | help");
}
fn cmd_add(args: &str) {
    println!("  [add]    staging: {args:?}");
}
fn cmd_commit(args: &str) {
    println!("  [commit] message: {args:?}");
}
fn cmd_config(args: &str) {
    println!("  [config] key/value: {args:?}");
}
fn cmd_exit(_args: &str) {
    println!("  [exit]   bye!");
}
fn cmd_resume(args: &str) {
    println!("  [résumé] generating with: {args:?}");
}

fn build() -> CommandTrie<Command> {
    let mut b = CommandTrieBuilder::new();
    b.insert(
        "help",
        Command {
            run: cmd_help,
            help: "show available commands",
        },
    );
    b.insert(
        "add",
        Command {
            run: cmd_add,
            help: "stage files",
        },
    );
    b.insert(
        "commit",
        Command {
            run: cmd_commit,
            help: "record changes",
        },
    );
    b.insert(
        "config",
        Command {
            run: cmd_config,
            help: "get/set configuration",
        },
    );
    b.insert(
        "exit",
        Command {
            run: cmd_exit,
            help: "quit the shell",
        },
    );
    // Non-ASCII command: `longest_prefix_match` returns matches at char
    // boundaries, so `&line[cmd_text.len()..]` below is always a valid
    // `&str` regardless of the command being ASCII or multi-byte UTF-8.
    b.insert(
        "résumé",
        Command {
            run: cmd_resume,
            help: "emit a résumé (UTF-8 command name)",
        },
    );
    b.build()
}

/// Resolve and dispatch a single line.
///
/// Strategy: ask the trie for the longest stored key that is a prefix of the
/// input. If the match isn't followed by a word boundary, reject it so e.g.
/// `"config"` doesn't accidentally match a hypothetical `"conf"` entry the
/// user actually meant.
fn dispatch(trie: &CommandTrie<Command>, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }

    let Some((cmd_text, cmd)) = trie.longest_prefix_match(line) else {
        println!("{line:>20} -> unknown command");
        return;
    };

    let rest = &line[cmd_text.len()..];
    // Require a word boundary after the matched command name.
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        println!("{line:>20} -> unknown command (partial match {cmd_text:?})");
        return;
    }

    print!("{line:>20} -> ");
    (cmd.run)(rest.trim_start());
}

fn main() {
    let trie = build();

    println!("== registered commands ==");
    trie.for_each(|name, cmd| println!("  {name:<10} {}", cmd.help));
    println!();

    println!("== dispatch ==");
    for line in [
        "help",
        "add src/lib.rs",
        "commit -m \"first cut\"",
        "config user.name spencer",
        "exit",
        "co",            // partial, not a prefix of anything reachable as a full word
        "configure now", // "config" matches but no word boundary -> rejected
        "nope",          // no match at all
        // UTF-8 command name with args after a whitespace boundary.
        "résumé --format=pdf",
        // No word boundary after the matched key -> rejected, even though
        // the boundary check is char-aware (the trailing 's' is fine to
        // examine because `cmd_text.len()` always lands on a char edge).
        "résumés",
    ] {
        dispatch(&trie, line);
    }
}
