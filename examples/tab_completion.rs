//! Fish-style tab completion against a `CommandTrie`.
//!
//! Run with: `cargo run --example tab_completion`
//!
//! For each simulated TAB press the example asks the trie what to splice
//! into the buffer and whether the choice is now unique. The same flow a
//! line editor would use.

use command_trie::{CommandTrie, CommandTrieBuilder, SubTrie};

/// Payload attached to each command. Anything cheaply cloneable works; the
/// trie is generic over `T`.
#[derive(Clone)]
struct CommandInfo {
    help: &'static str,
}

fn build() -> CommandTrie<CommandInfo> {
    let commands = [
        ("add", "stage file contents"),
        ("alias", "create a command alias"),
        ("branch", "list, create, or delete branches"),
        ("checkout", "switch branches or restore files"),
        ("cherry-pick", "apply changes from existing commits"),
        ("clean", "remove untracked files"),
        ("clone", "clone a repository"),
        ("command", "run a raw shell command"),
        ("commit", "record changes to the repository"),
        ("config", "get and set configuration"),
        ("diff", "show changes between commits"),
        ("fetch", "download objects and refs"),
        ("merge", "join two histories together"),
        ("push", "update remote refs"),
        ("rebase", "reapply commits on top of another base"),
        // Non-ASCII keys: arbitrary UTF-8 is accepted. Multi-byte chars
        // never get bisected by a radix split, so completions splice in
        // whole code points.
        ("café", "open the team café board"),
        ("cafétéria", "daily menu"),
        ("naïve-merge", "merge without conflict resolution"),
        ("🚀-deploy", "ship it"),
        ("🚀-rollback", "unship it"),
    ];

    let mut b = CommandTrieBuilder::new();
    for (name, help) in commands {
        b.insert(name, CommandInfo { help });
    }
    b.build()
}

/// The complete TAB handler: produces an updated buffer plus optional
/// menu/commit feedback for the UI.
enum TabOutcome<'a> {
    /// Nothing matched what the user typed.
    NoMatch,
    /// Splice `extension` into the buffer; the result is uniquely resolved
    /// and the caller can commit `info`.
    Commit {
        extension: String,
        info: &'a CommandInfo,
    },
    /// Splice `extension` into the buffer, then display `menu` (the per-
    /// candidate suffix beyond the new buffer contents).
    Menu {
        extension: String,
        menu: Vec<String>,
    },
}

fn handle_tab<'a>(trie: &'a CommandTrie<CommandInfo>, typed: &str) -> TabOutcome<'a> {
    let Some(sub) = trie.subtrie(typed) else {
        return TabOutcome::NoMatch;
    };

    // unique_value() == Some(_) means the caller can stop prompting.
    if let Some(info) = sub.unique_value() {
        return TabOutcome::Commit {
            extension: sub.extension().to_string(),
            info,
        };
    }

    // Ambiguous: still splice in any unambiguous extension (may be empty),
    // and show the per-candidate suffix beyond the new buffer contents.
    let menu = candidates_after_common_prefix(&sub);
    TabOutcome::Menu {
        extension: sub.extension().to_string(),
        menu,
    }
}

/// Pull every key in the view and strip the shared prefix off the front so
/// the displayed menu only shows what actually distinguishes the choices.
fn candidates_after_common_prefix(sub: &SubTrie<'_, CommandInfo>) -> Vec<String> {
    let cp_len = sub.common_prefix().len();
    let mut out = Vec::with_capacity(sub.len());
    sub.for_each(|key, _| out.push(key[cp_len..].to_string()));
    out
}

fn simulate(trie: &CommandTrie<CommandInfo>, typed: &str) {
    print!("typed {typed:>10?} -> ");
    match handle_tab(trie, typed) {
        TabOutcome::NoMatch => {
            println!("no match");
        }
        TabOutcome::Commit { extension, info } => {
            println!(
                "splice {extension:?}, COMMIT  ({}{}: {})",
                typed, extension, info.help
            );
        }
        TabOutcome::Menu { extension, menu } => {
            print!("splice {extension:?}, menu of {} -> ", menu.len());
            // Cap to a few items for output readability.
            let preview: Vec<_> = menu.iter().take(6).collect();
            println!("{preview:?}");
        }
    }
}

fn main() {
    let trie = build();

    println!("== fish-style TAB simulation ==\n");
    // Many roots: empty input shows the full menu.
    simulate(&trie, "");
    // Single root letter: still ambiguous, no unambiguous extension yet.
    simulate(&trie, "c");
    // "co" lies inside the "comm" / "config" branch but still ambiguous.
    simulate(&trie, "co");
    // "comm" branch point between "command" and "commit".
    simulate(&trie, "comm");
    // "comma" forces "command" -- unique, extension "nd".
    simulate(&trie, "comma");
    // Full key already typed -- unique, no extension.
    simulate(&trie, "clone");
    // No completions.
    simulate(&trie, "xyz");

    println!("\n== UTF-8 keys ==\n");
    // "caf" splices the multi-byte 'é' as a unit -- the LCP across
    // {café, cafétéria} is exactly "café".
    simulate(&trie, "caf");
    // Past the branch point: "café" itself is a stored key but also a
    // prefix of "cafétéria", so the view is not unique.
    simulate(&trie, "café");
    // Unique inside the cafétéria branch -- splices the remainder.
    simulate(&trie, "cafét");
    // Single emoji prefix between two 🚀-* commands.
    simulate(&trie, "🚀");
    simulate(&trie, "🚀-d");

    println!("\n== count_completions / completion_prefix ==\n");
    for typed in ["c", "co", "comm", "comma", "z", "caf", "🚀"] {
        println!(
            "{typed:>8?}: {} match(es), LCP = {:?}",
            trie.count_completions(typed),
            trie.completion_prefix(typed),
        );
    }
}
