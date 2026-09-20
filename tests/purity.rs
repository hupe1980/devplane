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
                // **A run after `\n` is exempt only while it is small enough to
                // have been chosen.** Indenting a continuation line by two or
                // four spaces is deliberate; seventeen is where the literal
                // happened to sit in the source file, which is what `cargo fmt`
                // leaves behind when it folds a `\` continuation onto one line.
                // That is how `devplane agents` shipped a ragged message past
                // this check.
                let run = line[i..].bytes().take_while(|b| *b == b' ').count();
                let deliberate_indent = before.ends_with("\\n") && run < 10;
                if inside_literal && word_then_word && !deliberate_indent {
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

/// **Principle II, enforced where it now actually lives.**
///
/// The constitution says Devplane originates no approval and may carry one:
/// nothing it can produce on its own — from a rule, a clock, a default or a
/// model — may approve anything, and the single exception is a person's answer
/// to a question an agent asked, which cannot exist without that person having
/// answered.
///
/// That used to be enforced by a sentence with no mechanism: *"the hook never
/// answers allow"*. It was true, and it became insufficient the day a second
/// channel could say yes. This asserts the shape that makes the principle
/// structural rather than careful:
///
/// * **the policy engine still cannot express an approval** — no `Allow`
///   variant, so no rule, cache or matcher can produce one however it is
///   called; and
/// * **the one place that can is downstream of a delivered answer** — the
///   accept is constructed in exactly one function, and that function's only
///   source of content is the channel a person's answer arrives on.
///
/// A source scan rather than a behavioural test on purpose, for the reason the
/// rest of this file is one: a behavioural test proves the paths somebody
/// thought to exercise, and the rule has to hold for the ones nobody did.
///
/// **Three of the four assertions below were watched failing; the `Verdict` one
/// cannot be, and that is worth saying rather than implying otherwise.** Adding
/// an `Allow` variant to try it makes every `match` on `Verdict` non-exhaustive,
/// so the crate stops compiling before any test runs — the compiler enforces
/// that half more strongly than this ever could. The assertion stays because it
/// costs nothing and names the rule where somebody reading the enum will look,
/// but the load-bearing guards here are the other three.
#[test]
fn nothing_can_approve_except_a_person_who_answered() {
    let policy = std::fs::read_to_string("src/core/policy.rs").expect("policy.rs");
    let verdict = policy
        .split("pub enum Verdict")
        .nth(1)
        .and_then(|s| s.split('}').next())
        .expect("Verdict is an enum in policy.rs");
    assert!(
        !verdict.contains("Allow"),
        "`Verdict` grew an `Allow` variant. The policy engine may refuse and defer and \
         nothing else — this is the constitution's Principle II, and it cost \
         $1,756-$3,511 a month to learn"
    );

    // The accept exists in exactly one place, and that place reads the answer
    // channel. Two would mean a second way to say yes, which is how the first
    // version of this rule was lost.
    let session = std::fs::read_to_string("src/acp/session.rs").expect("session.rs");
    let accepts = session.matches("ElicitationAction::Accept").count();
    assert_eq!(
        accepts, 1,
        "a person's answer may be turned into an approval in exactly one place; \
         found {accepts}"
    );
    let waiter = session
        .split("async fn ask_person")
        .nth(1)
        .expect("ask_person is where a question waits");
    assert!(
        waiter.contains("ElicitationAction::Accept"),
        "the accept moved out of the function that waits for the answer, so it is no \
         longer provably downstream of a person having answered"
    );
    assert!(
        !session.contains("ElicitationAction::Decline"),
        "`decline` is folded by the adapter into *answered, with no answers* — the agent \
         proceeds having asked and heard nothing, which is the failure this product exists \
         to prevent. The refusal that stops the call is `cancel`"
    );
}

/// **Nothing in the question path writes to anybody's settings.**
///
/// A question arrives, is shown, and is answered — and none of that may touch a
/// permission file, a settings file or a rule table. The reason is the one
/// behind every refusal in this product: an agent on this machine runs as the
/// same user and can read the token the page uses, so a route that edits the
/// file governing agents is reachable by the party the file exists to bound.
///
/// Asserted over the source rather than by running, because the failure it
/// guards against is a *new* write appearing on a path nobody thought to
/// exercise. The behavioural half — that a whole answered round trip leaves
/// `git status` clean — is the quickstart's, and a person runs it.
#[test]
fn the_question_path_writes_no_settings() {
    let question = std::fs::read_to_string("src/core/question.rs").expect("question.rs");
    for banned in [
        "settings.json",
        "devplane.toml",
        "fs::write",
        "File::create",
    ] {
        assert!(
            !question.contains(banned),
            "`core::question` reached for `{banned}`. A question is read, shown and answered; \
             nothing in that path may write a file, least of all one that governs agents"
        );
    }

    // And the answer route composes the reply, never a rule.
    let driven = std::fs::read_to_string("src/driven.rs").expect("driven.rs");
    let answer = driven
        .split("pub async fn answer")
        .nth(1)
        .and_then(|s| s.split("\npub ").next())
        .expect("driven::answer");
    for banned in ["fs::write", "settings.json", "auto_allow", "permissions"] {
        assert!(
            !answer.contains(banned),
            "`driven::answer` mentions `{banned}`. Delivering somebody's answer is not an \
             occasion to edit their configuration"
        );
    }
}

/// **Every way a question can end reads differently, and none of them reads as
/// nothing having happened.**
///
/// The specification asked for six distinguishable outcomes. Three of the six
/// described the `defer` mechanism — a paused session swept off disk, a call
/// deferred alongside other work, a pending tool gone on resume — and cannot
/// occur on the channel a question actually takes. What is left is three, and
/// they are the ones a person meets:
///
/// * a person answered, and what they chose;
/// * the run ended before anybody answered;
/// * the agent asked in a form Devplane could not show.
///
/// The property that matters is not that there are three. It is that **no two
/// share a sentence** and none is empty — because the failure this guards
/// against is somebody reusing one reason for two outcomes, which makes them
/// indistinguishable in the one place a person goes to find out what happened,
/// while every test still passes.
///
/// Asserted over the source because the alternative needs a signed-in agent and
/// real money for a property that is a fact about four string literals.
#[test]
fn a_question_ends_in_a_sentence_that_says_which_ending_it_was() {
    let driven = std::fs::read_to_string("src/driven.rs").expect("driven.rs");
    let reasons: Vec<&str> = [
        "a person answered:",
        "the run ended before anybody answered",
        "the agent asked in a form Devplane cannot show",
    ]
    .into_iter()
    .inspect(|r| {
        assert!(
            driven.contains(r),
            "the reason {r:?} is gone. An outcome with no sentence is an outcome that reads \
             as nothing having happened, which is what this whole feature is about"
        );
    })
    .collect();

    for (i, a) in reasons.iter().enumerate() {
        for (j, b) in reasons.iter().enumerate() {
            if i != j {
                assert!(
                    !a.contains(b) && !b.contains(a),
                    "two endings share a sentence ({a:?} and {b:?}), so a person reading the \
                     log cannot tell which happened"
                );
            }
        }
    }

    // An answer says *what* was chosen. "Somebody answered" without the answer
    // is a row nobody can check, which is the one property the decision log has.
    assert!(
        driven.contains(r#""a person answered: {chose}""#),
        "the answer's own content left the reason, so the log records that somebody answered \
         and not what they said"
    );
}

/// **A question from a session Devplane did not start cannot be answered,
/// asserted at the channel rather than at the surface.**
///
/// The inbox already declines to offer a control for one
/// (`a_question_devplane_cannot_answer_says_where_it_can_be`), and that is a
/// property of a *rendering*. This is the property underneath it: there is no
/// route to the answer path at all for a run Devplane does not drive, so a
/// caller who constructs the request by hand — a script, a future surface,
/// somebody with `curl` and the token — gets the same refusal as the page.
///
/// It matters because the two guards fail in opposite directions. A surface
/// that forgets the rule offers a button that does nothing; a *channel* that
/// forgets it would deliver an answer into a session whose dialog belongs to
/// somebody else's window.
#[test]
fn only_a_run_devplane_drives_can_have_a_question_answered() {
    let driven = std::fs::read_to_string("src/driven.rs").expect("driven.rs");

    // Delivery has exactly two routes and each has to refuse a run Devplane did
    // not start. `deliver` chooses between them, so the check lives there —
    // before either is taken — and the two functions underneath check again.
    let deliver = driven
        .split("async fn deliver(")
        .nth(1)
        .and_then(|s| s.split("\n/// ").next())
        .expect("driven::deliver");

    let lookup = deliver
        .find("state.sessions")
        .expect("delivery asks the driven-session table whether this run is one of ours");
    let live_send = deliver
        .find("Answer::Permission(d) => decide(")
        .expect("the live route goes through `decide`");
    assert!(
        lookup < live_send,
        "an answer is sent before the run is checked to be one Devplane drives"
    );

    // And the resumed route, which is the one that exists because the session
    // may be gone. It goes through `resume`, whose own refusal is the same one.
    assert!(
        deliver.contains("resume(state, &ask.run)"),
        "the route for a run whose agent has gone must go through `resume`"
    );
    let resume_fn = driven
        .split("pub async fn resume(")
        .nth(1)
        .and_then(|s| s.split("\n/// ").next())
        .expect("driven::resume");
    assert!(
        resume_fn.contains("that is a session Devplane watches, not one it drives"),
        "resume stopped refusing a session Devplane merely watches"
    );

    // The refusal underneath both, which is what a caller with `curl` and the
    // token meets.
    assert!(
        driven.contains("that run is not one Devplane drives"),
        "the refusal lost its sentence; a caller is told nothing about why"
    );
}

/// **Every public function in the pure core has a caller outside it.**
///
/// The inverse of every other guard in this repository, and the only one that
/// looks for *code with no claim* rather than a claim with no code. The rule it
/// mirrors — a documented trigger with no code is a lie the documentation tells
/// for you — has a twin nobody had written down: a function with no caller is a
/// rule stated where nothing reads it, and the danger is not the dead bytes. It is
/// that somebody writes the rule again, somewhere impure, and the two copies
/// drift.
///
/// That is exactly what happened to the fan-out. `core::batch::order` and
/// `members_of` held the rule *what needs you first, then what failed* and had
/// no production caller at all, while `api::render_batch` inlined the same
/// filter and the same `sort_by_key` — so the tested copy decided nothing and
/// the deciding copy was untested.
///
/// A helper only tests need is not exempt; it is the case this exists to find.
/// Something that genuinely belongs to the core alone is called by the core,
/// and this counts those.
#[test]
fn nothing_in_the_core_is_a_rule_with_no_reader() {
    let mut defined: Vec<(String, String)> = Vec::new();
    let mut bodies: Vec<(String, String)> = Vec::new();

    for entry in walk("src") {
        let text = std::fs::read_to_string(&entry).expect("source");
        let prod = production(&text);
        if entry.starts_with("src/core/") {
            // **A `#[cfg(test)]` block is not production code, and its readers
            // are the test modules this guard strips.** Counting them would
            // report every test-only helper as dead while its callers sat two
            // lines away — the guard being loudly wrong about the one case it
            // cannot see, which is how a guard gets switched off.
            let mut in_test_only = false;
            let mut depth = 0i32;
            for line in prod.lines() {
                let t = line.trim_start();
                if in_test_only {
                    depth += line.matches('{').count() as i32;
                    depth -= line.matches('}').count() as i32;
                    if depth <= 0 {
                        in_test_only = false;
                    }
                    continue;
                }
                if t.starts_with("#[cfg(test)]") {
                    in_test_only = true;
                    depth = 0;
                    continue;
                }
                if let Some(rest) = t
                    .strip_prefix("pub fn ")
                    .or_else(|| t.strip_prefix("pub const fn "))
                {
                    let name = rest
                        .split(['(', '<'])
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    if !name.is_empty() {
                        defined.push((name, entry.clone()));
                    }
                }
            }
        }
        bodies.push((entry, prod));
    }
    assert!(
        defined.len() > 50,
        "the scan found {} public core functions, which means it is not reading the core",
        defined.len()
    );

    let mut orphans: Vec<String> = Vec::new();
    for (name, home) in &defined {
        // Reached by name shapes this cannot see: trait methods, operator
        // impls, constructors.
        // Reached by name shapes this cannot see: trait methods, operator
        // impls, constructors, and serde's own contract — a `#[serde(with =
        // "…")]` attribute names the *module*, never `serialize`.
        if matches!(
            name.as_str(),
            "new" | "default" | "fmt" | "from" | "parse" | "serialize" | "deserialize"
        ) {
            continue;
        }
        // Whole-word, so a function passed by reference — `map(gate_down_item)`
        // — counts as the reader it is. The first version of this guard looked
        // for `name(` and reported four such uses as dead.
        let uses: usize = bodies
            .iter()
            .map(|(file, body)| {
                let mut n = words(body, name);
                if file == home {
                    n = n.saturating_sub(defs(body, name));
                }
                n
            })
            .sum();
        if uses == 0 {
            orphans.push(format!("{home}::{name}"));
        }
    }

    assert!(
        orphans.is_empty(),
        "these core functions state a rule nothing reads — either call them from \
         the surface that needs the rule, or delete them: {orphans:?}"
    );
}

/// Everything above the file's own `#[cfg(test)] mod`, with comments removed.
///
/// **The module marker at column zero, not the first `#[cfg(test)]` anywhere.**
/// A `#[cfg(test)]` on a struct field sits at an indent in the middle of a file,
/// and splitting on the first one truncated `policy_cache.rs` at line 60 —
/// which made this guard report four live functions as dead the first time it
/// ran. Comments go because a doc comment naming a function is a mention and
/// not a reader.
fn production(text: &str) -> String {
    let cut = text.find("\n#[cfg(test)]\nmod ").unwrap_or(text.len());
    text[..cut]
        .lines()
        .map(|l| match l.trim_start().starts_with("//") {
            true => "",
            false => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many times `name` appears as a whole identifier.
fn words(body: &str, name: &str) -> usize {
    let mut n = 0;
    let bytes = body.as_bytes();
    let mut from = 0;
    while let Some(i) = body[from..].find(name) {
        let at = from + i;
        let before = at.checked_sub(1).map(|j| bytes[j] as char);
        let after = body[at + name.len()..].chars().next();
        let boundary = |c: Option<char>| !c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if boundary(before) && boundary(after) {
            n += 1;
        }
        from = at + name.len();
    }
    n
}

/// The declaration lines for `name`, which are not uses of it.
fn defs(body: &str, name: &str) -> usize {
    body.matches(&format!("pub fn {name}")).count()
        + body.matches(&format!("pub const fn {name}")).count()
}

/// Every `.rs` under a directory.
fn walk(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p.to_string_lossy()));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p.to_string_lossy().to_string());
        }
    }
    out
}

/// **The Spec Kit gate adds no second permission surface, and this is how that
/// stays true.**
///
/// The set of things that can approve, deny or defer a tool call is the most
/// important short list in this product, and the pressure to extend it comes
/// from features exactly like this one: a hook that already knows something
/// about a repository, fires at a useful moment, and would be *so easy* to let
/// answer a permission on somebody's behalf. It reports an exit code. If these
/// names ever appear in its path, the reason will have been a good one, and the
/// feature will still have become a decider.
#[test]
fn the_speckit_gate_never_becomes_a_second_decider() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // The two command bodies, cut out of the file they share with everything
    // else — the rule is about this feature, not about `src/cli/work.rs`.
    let cli = std::fs::read_to_string(root.join("src/cli/work.rs")).expect("the CLI");
    let mut bodies = Vec::new();
    for name in ["pub async fn cmd_gate_run", "pub fn cmd_speckit_install"] {
        let start = cli
            .find(name)
            .unwrap_or_else(|| panic!("{name} has been renamed; this guard now checks nothing"));
        // To the start of the next top-level item, which is where the body ends.
        let rest = &cli[start..];
        let end = rest[1..].find("\n}\n").map(|i| i + 4).unwrap_or(rest.len());
        bodies.push((name, rest[..end].to_string()));
    }
    let spec = std::fs::read_to_string(root.join("src/core/spec.rs")).expect("the hook contract");
    bodies.push(("src/core/spec.rs", spec));
    let skill = std::fs::read_to_string(root.join("plugin/skills/devplane-gate/SKILL.md"))
        .expect("the gate skill");
    bodies.push(("plugin/skills/devplane-gate/SKILL.md", skill));

    const FORBIDDEN: &[(&str, &str)] = &[
        (
            "Verdict",
            "the permission verdict type — a gate reports an exit code and \
             never decides a tool call",
        ),
        (
            "Policy",
            "the permission policy — this feature must not consult it, because \
             a gate that reads policy is one refactor from applying it",
        ),
        (
            "permission",
            "a gate result is not a permission, and calling it one is how the \
             two become one surface",
        ),
        (
            "defer",
            "the hook must not block on a person: a gate waiting for a human \
             inside somebody's workflow is a stalled workflow, not supervision",
        ),
    ];

    let mut found = Vec::new();
    for (where_, body) in &bodies {
        // Prose in this repository explains the refusal by naming it, and a
        // guard that cannot tell an explanation from a call would forbid saying
        // why. Comments and doc comments are the explanation; code is not.
        let code: String = body
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with('*') && !t.starts_with('#')
            })
            .collect::<Vec<_>>()
            .join("\n");
        for (token, why) in FORBIDDEN {
            if code.contains(token) {
                found.push(format!("{where_} names `{token}`: {why}"));
            }
        }
    }

    assert!(
        found.is_empty(),
        "this feature has grown a decision surface:\n  {}",
        found.join("\n  ")
    );
}
