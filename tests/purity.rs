//! The one architectural rule in this crate, enforced rather than written down.
//!
//! `src/core/` may not reach the outside world: no `async fn`, no `.await`, no
//! async runtime, no database, no HTTP. Three of the product's claims rest on
//! that and on nothing else:
//!
//! * the board is rebuildable, because run state is a pure
//!   `(Run, Event) -> Run` that can be replayed;
//! * the inbox is correct after a restart, because it is derived and never
//!   stored;
//! * the permission policy cannot fail open, because it cannot wait on
//!   anything — it runs on the synchronous hook a Claude Code session is
//!   blocked on.
//!
//! This was a separate crate, and the manifest enforced it by simply not
//! linking those dependencies: stronger in principle, because it covers names
//! nobody thought to list. In practice it was a second manifest and a second
//! crates.io publish for one binary, which is the same objection that deleted
//! the other six library crates, and consistency is worth more here than the
//! last few percent of rigour. The check below covers every way the rule breaks
//! that anybody has actually written.
//!
//! The one thing deliberately allowed is a **synchronous** read of a small
//! local file, in `config` and `policy_cache`. That is bounded in a way a
//! network call is not, and it is how a project's rules are read on the hook.

use std::path::Path;

/// What may not appear in `src/core/`, and the sentence explaining why — because
/// a failing test that only names a token teaches nobody the rule.
const BANNED: &[(&str, &str)] = &[
    (
        "async fn",
        "the reducer and the policy are synchronous; a policy that can await is a policy that can fail open",
    ),
    (
        ".await",
        "same: this code runs on a hook Claude Code is blocked on",
    ),
    (
        "tokio::",
        "an async runtime belongs on the other side of this line",
    ),
    (
        "sqlx::",
        "the store is a caller of this code, never the other way round",
    ),
    ("rusqlite::", "as above"),
    ("reqwest::", "nothing here may touch a network"),
    ("hyper::", "nothing here may touch a network"),
    ("axum::", "the API is a caller of this code"),
    (
        "std::process::Command",
        "spawning is unbounded; gates and git are the other side of this line",
    ),
    (
        "tokio::process",
        "spawning is unbounded; gates and git are the other side of this line",
    ),
];

#[test]
fn the_pure_half_cannot_reach_the_outside_world() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core");
    let mut files = 0;
    let mut broken = Vec::new();

    for entry in std::fs::read_dir(&dir).expect("src/core exists") {
        let path = entry.expect("readable").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        files += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&path).expect("readable");

        for (line_no, line) in source.lines().enumerate() {
            // Comments say what the rule is and quote what it forbids; they are
            // not the code the rule is about.
            let code = line.split("//").next().unwrap_or("");
            for (token, why) in BANNED {
                if code.contains(token) {
                    broken.push(format!("{name}:{}: `{token}` — {why}", line_no + 1));
                }
            }
        }
    }

    assert!(
        files >= 13,
        "the check read {files} files from {}; it is looking in the wrong place",
        dir.display()
    );
    assert!(
        broken.is_empty(),
        "src/core/ must not reach the outside world:\n  {}\n\n\
         If this code genuinely needs to wait on something, it belongs in a \
         module outside src/core/ and the pure half should call into it through \
         a value, not a future.",
        broken.join("\n  ")
    );
}

#[test]
fn the_check_would_notice() {
    // A guard that cannot fail is a guard nobody should trust. This asserts the
    // matcher itself, so the test above means something.
    let sample = "pub async fn evaluate(&self) { self.fetch().await }";
    let hits: Vec<&str> = BANNED
        .iter()
        .map(|(t, _)| *t)
        .filter(|t| sample.contains(t))
        .collect();
    assert_eq!(hits, ["async fn", ".await"]);
}

/// No user-facing string carries a collapsed line continuation.
///
/// Rust's `\` at the end of a line eats the newline *and* the indentation after
/// it, which is how every long message here is written. Lose the backslash and
/// the string keeps the indentation: it compiles, it passes every test that
/// checks what it *says*, and it reaches a person as
/// `…a prefix rule would                  approve whatever…`. Only visible by
/// running the command, which is why it is worth a check.
#[test]
fn no_message_carries_a_collapsed_line_continuation() {
    let mut bad = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no sources found under {}",
        root.display()
    );

    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            // Comments and doc comments are prose and may be indented freely.
            if trimmed.starts_with("//") {
                continue;
            }
            // The signature is a gap of six or more spaces **between two
            // words**, inside a string literal. That is what a lost `\` leaves
            // behind and it is not what anything else looks like: a column
            // layout puts its spaces before a `{}` or at the start of the
            // string, and a deliberately indented second line follows a `\n`.
            let bytes = line.as_bytes();
            for (i, _) in line.match_indices("      ") {
                let before = &line[..i];
                let inside_literal = before.matches('"').count() % 2 == 1;
                let prev = bytes[..i].last().copied().unwrap_or(b' ');
                let next = line[i..].bytes().find(|b| *b != b' ').unwrap_or(b'"');
                let word_then_word = (prev.is_ascii_alphanumeric() || b",.;:-".contains(&prev))
                    && next.is_ascii_lowercase();
                if inside_literal && word_then_word && !before.ends_with("\\n") {
                    bad.push(format!("{}:{}", path.display(), n + 1));
                    break;
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "these string literals have a run of spaces inside them, which is what a \
         `\\` line continuation looks like after somebody drops the backslash — \
         the message reaches a person with the gap in it:\n  {}",
        bad.join("\n  ")
    );
}
