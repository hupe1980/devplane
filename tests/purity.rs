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
                // **There is no exemption for a run after `\n`, and there used
                // to be.**
                //
                // It allowed anything under ten spaces, on the reasoning that a
                // continuation indented by two or four is deliberate. That
                // reasoning is sound and the exemption was still useless: this
                // check only fires on a run of **six or more**, so a deliberate
                // two- or four-space indent never reaches it. All the exemption
                // could ever do was wave through runs of six to nine — and it
                // did, for a message in `devplane ls` that shipped with a
                // newline and nine spaces in the middle of a sentence.
                //
                // Measured before removing it: every `\n`-plus-spaces run in
                // `src/` is either two or four spaces, or a `.join("\n   ")`
                // whose spaces end at the closing quote and are already
                // excluded by `word_then_word`. Nothing legitimate was relying
                // on it.
                if inside_literal && word_then_word {
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

/// **A permission answer nobody made may not become a refusal.**
///
/// `Decision::parse(None, None)` answers `Deny` — *fail closed*, which is the
/// right rule for a **gate** deciding with no person present and the wrong one
/// for a **person's answer**, where the honest reading is that nothing was said.
///
/// The difference was not academic. The board posted `{"choice":"allow"}`;
/// `AnswerBody` has no `choice` field and rejected nothing it did not
/// understand, so **pressing *allow* recorded a deny** — under the person's
/// name, on the product whose entire claim is recording who decided what, while
/// the surface said *"answered — on the record and on its way to the agent"*.
///
/// The CLI had always refused this before sending. Refusing it at the route as
/// well is what turns the next wiring mistake into a 400 instead of a wrong
/// decision, and `deny_unknown_fields` is what makes the mistake itself
/// impossible to post.
#[test]
fn a_permission_answer_that_says_nothing_is_refused_rather_than_read_as_deny() {
    let api = std::fs::read_to_string("src/api.rs").expect("api.rs");

    let body = api
        .split("struct AnswerBody")
        .next()
        .expect("AnswerBody is declared in api.rs");
    assert!(
        body.rsplit("#[derive")
            .next()
            .is_some_and(|d| d.contains("deny_unknown_fields")),
        "`AnswerBody` does not deny unknown fields. A surface posting a key this route never \
         reads then answers as though nobody said anything — which is how pressing `allow` on \
         the board came to record a deny"
    );

    let handler = api
        .split("async fn answer_ask")
        .nth(1)
        .and_then(|s| s.split("\nasync fn ").next())
        .expect("answer_ask");
    let permission = handler
        .split("Kind::Permission")
        .nth(1)
        .and_then(|s| s.split("Kind::Question").next())
        .expect("the permission arm");
    assert!(
        permission.contains("BAD_REQUEST"),
        "the permission arm of `answer_ask` reaches `Decision::parse` without first refusing an \
         answer that named neither a decision nor an option. `parse` maps nothing to `Deny`, so \
         that path records a refusal nobody chose"
    );

    // And the sentence a person meets says what to send, because this is the
    // failure a mis-wired surface produces.
    assert!(
        permission.contains("say what the answer is"),
        "the refusal does not tell the caller what to send"
    );
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

/// **Every public function in this crate has a caller.**
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
/// Something that genuinely belongs to one module is called by that module,
/// and this counts those.
///
/// **It scanned `src/core/` only, for the whole of its life, and the rule it
/// states is not about purity.** The reasoning above is that the danger is a
/// second copy written *somewhere impure* — which is everywhere this guard was
/// not looking. Widened to the crate on 2026-09-20, it found `api::content_type_of`:
/// the function that says what the built interface bundle is served as, beside
/// a bundle that is embedded in the binary and routed by nothing. Six lines
/// stating a rule, in the file that would have to read it, unread.
#[test]
fn nothing_in_this_crate_is_a_rule_with_no_reader() {
    let mut defined: Vec<(String, String)> = Vec::new();
    let mut bodies: Vec<(String, String)> = Vec::new();

    for entry in walk("src") {
        let text = std::fs::read_to_string(&entry).expect("source");
        let prod = production(&text);
        {
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
                    .or_else(|| t.strip_prefix("pub async fn "))
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
        defined.len() > 300,
        "the scan found {} public functions, which means it is not reading the crate",
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
                // Entry points and trait obligations the crate never calls by
                // name: the binary's own, and the ACP client's handlers, which
                // the protocol crate invokes through `dyn Client`.
                | "main"
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
        "these functions state a rule nothing reads — either call them from \
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
/// How many times this file *defines* `name`, so its own signature is not
/// counted as a reader of itself.
///
/// **The boundary after the name is load-bearing and was missing.** This was a
/// substring search, so `pub fn draft_links` counted as a definition of
/// `draft_link` — two definitions, two occurrences, and the live function came
/// out with zero readers. No such prefix pair existed while the guard only read
/// `src/core/`; widening it to the crate produced the false positive within a
/// minute, which is the argument for running a widened check against a known
/// answer before believing its list.
fn defs(body: &str, name: &str) -> usize {
    let mut n = 0;
    for prefix in ["pub fn ", "pub const fn ", "pub async fn "] {
        let needle = format!("{prefix}{name}");
        let mut from = 0;
        while let Some(i) = body[from..].find(&needle) {
            let end = from + i + needle.len();
            // A definition is the name followed by its parameter list or its
            // generics — never by another identifier character.
            if !body[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                n += 1;
            }
            from = end;
        }
    }
    n
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

/// **Every variant of the protocol event enum is constructed somewhere.**
///
/// The no-reader guard above looks at functions. This looks at the other shape
/// the same defect takes, and it is the shape that survived a deletion: when
/// the hard-coded six-hundred-second permission timeout was removed, its event
/// variant was not. `AcpEvent::PermissionExpired` stayed declared, stayed
/// matched in `driven.rs`, and was constructed by nothing — a handler that
/// could never run, writing a decision row reading *"nobody answered within ten
/// minutes"* about a rule this product had just deleted and now indicts other
/// people for having.
///
/// Nothing caught it. `cargo` does not, because the enum is `pub` and a `pub`
/// variant of a library crate is never dead code to the compiler. The function
/// guard does not, because a variant is not a function. And every match on it
/// was exhaustive, so the arm read as coverage.
///
/// **A match arm is a reader, not a constructor**, which is the whole
/// distinction: this counts only the places the variant is *built*. The enum is
/// the boundary between the protocol crate and this one — everything an agent
/// can say arrives through it — so a variant nobody builds is a sentence about
/// agents that no agent can cause.
#[test]
fn every_protocol_event_can_actually_happen() {
    let home = std::fs::read_to_string("src/acp/session.rs").expect("session.rs");
    let decl = home
        .split_once("pub enum AcpEvent")
        .expect("the protocol event enum")
        .1;
    let decl = &decl[..decl.find("\n}").expect("the enum closes")];

    let variants: Vec<String> = decl
        .lines()
        .filter_map(|l| {
            let t = l.strip_prefix("    ")?;
            let first = t.chars().next()?;
            if !first.is_ascii_uppercase() {
                return None;
            }
            Some(t.split([' ', '{', '(', ',']).next()?.to_string())
        })
        .collect();
    assert!(
        variants.len() > 10,
        "the scan found {} variants, which means it is not reading the enum: {variants:?}",
        variants.len()
    );

    // Where an event may be built: the session that translates the protocol,
    // and the fixture agent that stands in for a real one.
    let sources: String = walk("src")
        .into_iter()
        .chain(walk("examples"))
        .map(|f| production(&std::fs::read_to_string(&f).expect("source")))
        .collect::<Vec<_>>()
        .join("\n");

    let mut unbuildable = Vec::new();
    for v in &variants {
        // **The arrow is what tells an arm from a construction**, not the
        // brace: `AcpEvent::V { field } =>` and `AcpEvent::V { field: x }` are
        // the same characters up to the payload. So look forward from each
        // mention and see which comes first — a `=>` means this one is being
        // matched, a `;` or a `)` means the expression ended and it was built.
        let built = sources
            .match_indices(&format!("AcpEvent::{v}"))
            .any(|(i, _)| {
                let rest = &sources[i..];
                let arrow = rest.find("=>").unwrap_or(usize::MAX);
                let ends = rest.find([';', ')']).unwrap_or(usize::MAX);
                ends < arrow
            });
        if !built {
            unbuildable.push(v.clone());
        }
    }

    assert!(
        unbuildable.is_empty(),
        "these protocol events are declared and handled and **constructed by nothing**, so \
         their handlers can never run — delete the variant and its arm, or build it where \
         the protocol produces it: {unbuildable:?}"
    );
}

/// **Nothing in this tree ever sets the vendor's question timer.**
///
/// `CLAUDE_AFK_TIMEOUT_MS` decides how long a person gets to answer their own
/// agent's question, and reading it is the whole of feature `019`. Writing it
/// would be this product putting a clock on somebody's attention — the trade it
/// indicts four vendors for, and the one it has already shipped twice by
/// accident and deleted twice.
///
/// So the read is allowed in exactly the two places that do it, and the
/// vocabulary of writing is banned outright. The same shape as the fan-out's
/// absence check, for the same reason: a default that can be configured away is
/// not a guarantee, and a prohibition retrofitted after somebody has added the
/// convenience is a prohibition that loses.
#[test]
fn nothing_here_ever_sets_the_vendors_question_timer() {
    let mut writers = Vec::new();
    for entry in walk("src").into_iter().chain(walk("examples")) {
        let text = std::fs::read_to_string(&entry).expect("source");
        for (i, line) in text.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with("///") {
                continue;
            }
            if !line.contains("CLAUDE_AFK") {
                continue;
            }
            // Setting it, in any of the shapes that would.
            for bad in ["set_var", "env(\"CLAUDE_AFK", ".env(", "export "] {
                if line.contains(bad) {
                    writers.push(format!("{entry}:{}: {}", i + 1, t.trim()));
                }
            }
        }
    }
    assert!(
        writers.is_empty(),
        "these put a clock on somebody's own questions — this product reads that value \
         and never writes it: {writers:?}"
    );

    // And the countdown variable is cosmetic — when the on-screen countdown
    // appears — in the person's own terminal. Reading it would be this product
    // taking an interest in something that is not its business.
    let all: String = walk("src")
        .into_iter()
        .map(|f| std::fs::read_to_string(&f).expect("source"))
        .collect::<Vec<_>>()
        .join("\n");
    let mentions: usize = all
        .lines()
        .filter(|l| l.contains("CLAUDE_AFK_COUNTDOWN_MS"))
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with("///")
        })
        .count();
    assert_eq!(
        mentions, 0,
        "`CLAUDE_AFK_COUNTDOWN_MS` is cosmetic and belongs to the person's own terminal"
    );
}

// ---------------------------------------------------------------------------
// The question clock, read through its type and never key by key
// ---------------------------------------------------------------------------

/// **The CLI talks to its own API and used to read it with string keys.**
///
/// `devplane modes` reached for `c.get("file")` — a field renamed to
/// `where_set` three passes earlier — and `unwrap_or("")` printed a blank line
/// where the settings path belongs. So a person was told a clock was answering
/// their questions and never told where to change it, and every test passed,
/// because no test asserted on the location and the fallback is a valid string.
///
/// The fix is not a corrected key. Both structs live in **this crate**: the API
/// serialises [`ClockLine`] and the CLI can deserialise the same type, which
/// makes a rename a compile error instead of a blank line. This asserts the
/// clock path stays that way.
///
/// An absence check, because that is the only way an absence stays true — and
/// this one fails the moment somebody adds a *reasonable* convenience: one
/// `.get("says")` to avoid a clone.
#[test]
fn the_clock_is_never_read_out_of_json_by_hand() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let whole = std::fs::read_to_string(root.join("src/cli/inbox.rs")).expect("src/cli/inbox.rs");

    // **Comments are stripped before the check, and the first run is why.**
    // It fired on the comment that *documents* the defect it is guarding
    // against — the same shape as the npm guard in `concepts-check.sh` firing
    // on files that were quoting a stale claim in order to retire it. A guard
    // that cannot tell a description of the bug from the bug reads every
    // post-mortem as a regression.
    let text: String = whole
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Every field of `ClockLine`. Reading any of them by name out of a
    // `serde_json::Value` is the defect, whatever the name happens to be today.
    for field in [
        "after",
        "immediate",
        "source",
        "where_set",
        "says",
        "chosen_by_the_person",
        "answers_for_you",
        // The name the defect was actually spelled with, kept so the exact
        // regression cannot come back under its own name.
        "file",
    ] {
        let by_hand = format!(".get(\"{field}\")");
        assert!(
            !text.contains(&by_hand),
            "src/cli/inbox.rs reads the clock field `{field}` out of JSON by hand ({by_hand}).\n\
             Deserialise `core::clock::ClockLine` instead — it is the type the API serialises, \
             so a rename is a compile error rather than a blank line."
        );
    }

    // And the positive half: it does deserialise a type. An absence check alone
    // passes on a file that dropped the feature entirely.
    //
    // **The clock arrives inside the envelope now**, so naming `ClockLine` here
    // is no longer the evidence — `core::world::Modes` holds it, and that is
    // the one type this surface deserialises. Asserting the old name would fail
    // on the fix that made it unnecessary.
    assert!(
        text.contains("world::Modes"),
        "the modes rendering must go through the shared type, which carries the clock"
    );

    // **The whole answer, not only the clock inside it.** The clock was typed
    // and the envelope around it was not, so thirty-one lookups remained — and
    // one of them asked for `unreported`, a field no version of this endpoint
    // has ever served. It read zero for ever: one of the three sentences never
    // printed, and the guard on the reassuring one became `0 < total`.
    for field in [
        "projects",
        "sessions",
        "unsupervised",
        "unknown",
        "unreported",
        "clock_read",
        "clock_unread",
        "asks_a_person",
        "agent_mode",
        "question_clock",
    ] {
        let by_hand = format!(".get(\"{field}\")");
        assert!(
            !text.contains(&by_hand),
            "src/cli/inbox.rs reads `{field}` out of JSON by hand ({by_hand}).\n\
             Deserialise `core::world::Modes` — a key the type does not have is a compile \
             error, and a key the API does not send is `unwrap_or(0)` for ever."
        );
    }
}

/// **A vendor is a row, not an `if`.**
///
/// The whole cross-vendor claim is that support is a table with a date on each
/// row, re-read against the vendor's own documentation — and `devplane doctor`
/// prints exactly that. A handler that compares an agent id to a string literal
/// is the same fact in a second place, where nothing dates it and nothing
/// reports it: `agent != "claude"` decided which sessions could show an
/// abandoned question, inside an API handler, for the life of the feature.
///
/// So the vendor names live in `core::vendors` and nowhere else. This is an
/// absence check because that is the only way an absence stays true, and it
/// fails the moment somebody adds the *reasonable* convenience — one
/// comparison, to special-case the vendor they happen to be testing.
#[test]
fn no_surface_branches_on_which_vendor_it_is() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Every file that renders or serves, and none of the observation adapters:
    // `observe/copilot.rs` and `observe/hook.rs` are a vendor's own channel by
    // definition, and naming it there is what an adapter is.
    let surfaces = [
        "src/api.rs",
        "src/cli/board.rs",
        "src/cli/inbox.rs",
        "src/cli/work.rs",
        "src/cli/admin.rs",
        "src/render.rs",
    ];

    // The spellings that would be a branch on identity. `"claude"` as a *value*
    // — an agent to launch, a registry id — is not one, so this looks for the
    // shape of a comparison rather than for the word.
    let branches = [
        r#"== "claude""#,
        r#"!= "claude""#,
        r#"eq_ignore_ascii_case("claude")"#,
        r#"== "copilot""#,
        r#"!= "copilot""#,
        r#"eq_ignore_ascii_case("copilot")"#,
        r#"contains("claude")"#,
        r#"starts_with("claude")"#,
    ];

    for rel in surfaces {
        let whole =
            std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        // Comments may discuss the rule; code may not be the rule.
        let text: String = whole
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for bad in branches {
            assert!(
                !text.contains(bad),
                "{rel} branches on which vendor it is ({bad}).\n\
                 A vendor is a row in `core::vendors`, with a date and a reason a person can read. \
                 Add the question to that table — `can_show_an_abandoned_question` is the shape — \
                 so `devplane doctor` reports the answer instead of a handler deciding it silently."
            );
        }
    }

    // And the positive half: the table is what answers it.
    let vendors = std::fs::read_to_string(root.join("src/core/vendors.rs")).expect("vendors");
    assert!(
        vendors.contains("fn can_show_an_abandoned_question"),
        "the capability must be a function over the table, or the absence check above is \
         protecting nothing"
    );
}

/// **A reassuring sentence may not be printed over no evidence.**
///
/// *Every session that has reported asks you* is guarded by `unreported <
/// total`, so that a machine where nothing has reported does not get the green
/// line. The CLI read `unreported` from a key the API never sent, so the guard
/// was `0 < total` — true whenever anything is live — and the green line was
/// reachable with zero sessions having said anything at all.
///
/// This asserts the field exists on both sides, which is what makes the guard
/// mean what it says.
#[test]
fn every_figure_the_modes_surface_guards_on_is_one_the_api_sends() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let world = std::fs::read_to_string(root.join("src/core/world.rs")).expect("world");
    let api = std::fs::read_to_string(root.join("src/api.rs")).expect("api");

    // The type carries it…
    assert!(
        world.contains("pub unreported: usize"),
        "core::world::Modes has no `unreported`, which the CLI guards the reassuring line on"
    );
    // …and the handler fills it. A field on the type that the handler never
    // sets is `Default` on the wire, which is the same zero by another route.
    assert!(
        api.contains("unreported: projects.iter().map(|p| p.unreported).sum()"),
        "/api/modes does not sum `unreported`, so the guard on the reassuring line reads zero"
    );
}

/// **`never` is one of the setting's four documented values and is its
/// default**, so a clock that does not answer has to be tellable from a clock
/// that does — and from no clock at all.
///
/// Until 2026-09-21 every string in the settings file became a duration, so
/// `askUserQuestionTimeout: "never"` rendered as *"a never timer … after that,
/// whatever is selected is submitted"*: false, about the default, in the
/// direction of alarm, and worst when an administrator had deployed it as
/// hardening.
#[test]
fn a_clock_that_does_not_answer_is_not_rendered_as_one_that_does() {
    use devplane::core::clock::{After, ClockLine, QuestionClock, Source};

    let line = |after: After, source: Source| {
        ClockLine::from(&QuestionClock {
            after,
            source,
            where_set: "~/.claude/settings.json".into(),
        })
    };

    for source in [Source::User, Source::Managed] {
        let never = line(After::Never, source);
        assert!(
            !never.answers_for_you,
            "`never` does not answer for anybody ({source:?})"
        );
        assert!(
            never.says.contains("wait until you"),
            "the sentence has to say the questions wait: {}",
            never.says
        );
        assert!(
            !never.says.contains("is submitted"),
            "this is the sentence the defect printed: {}",
            never.says
        );
    }

    // The three states stay three: answers / does not answer / nothing set.
    assert!(line(After::Idle("5m".into()), Source::User).answers_for_you);
    assert!(line(After::Immediately, Source::Environment).answers_for_you);
    assert!(!line(After::Never, Source::User).answers_for_you);
}

/// **Nothing in the inbox's ordering reads a model's opinion.**
///
/// Ranking by how important a model thinks something is would be the thirteenth
/// principle in a new costume: the ordering is level, then decorrelation, then
/// age, and every one of those is a function of trusted inputs. A relevance
/// score is the one change that would make this surface unexplainable — a
/// person cannot ask why a row is where it is if the answer is an embedding.
///
/// An absence check, because that is the only way an absence stays true. It
/// fails the moment somebody adds a *reasonable* improvement: a confidence
/// field on an item, a similarity sort, a "probably urgent" weight.
#[test]
fn the_inbox_never_orders_by_what_a_model_thinks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let whole = std::fs::read_to_string(root.join("src/core/attention.rs")).expect("attention.rs");

    // Comments are stripped: this file *discusses* the refusal at length, and
    // a guard that cannot tell the rule from its violation reads every
    // explanation as a breach.
    let code: String = whole
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && !t.starts_with("///") && !t.starts_with("//!")
        })
        .collect::<Vec<_>>()
        .join("\n");

    for banned in [
        "confidence",
        "relevance",
        "similarity",
        "embedding",
        "importance_score",
        "predicted",
        "llm",
        "prompt(",
        "complete(",
    ] {
        assert!(
            !code.to_lowercase().contains(banned),
            "core::attention names `{banned}` outside a comment. The ordering is level, then \
             decorrelation, then age — all functions of trusted inputs. A score a model produced \
             makes the surface unexplainable, which is the one thing a supervision tool cannot be."
        );
    }

    // The positive half: the ordering is still the one this claims it is.
    assert!(
        code.contains("pub fn rank("),
        "there is no ranking function left to make claims about"
    );
}

/// **The rule-coverage path writes nothing, and cannot grow a way to.**
///
/// This is the whole of the feature rather than a safeguard on it. An agent on
/// this machine runs as the same user as the daemon and can read the bearer
/// token, so a route that edited a permission file would be reachable by the
/// party the file exists to bound — and writing into the *vendor's* settings
/// would be strictly worse, because that is the file its own enforcement reads.
///
/// So the refusal is structural: there is no write, and the absence is proved
/// over the source rather than promised in a comment. The convenience somebody
/// would reasonably add — *it already knows the file and the text, why not put
/// it there* — is exactly what this fails on.
#[test]
fn the_rule_coverage_path_writes_nothing_anywhere() {
    // Every file the feature is made of. The reader lives in the API because it
    // touches a disk; the decision is pure. Both are covered, because the
    // write would most naturally be added next to the read.
    const BANNED: &[(&str, &str)] = &[
        ("fs::write", "writing a file"),
        ("File::create", "creating one"),
        ("fs::rename", "renaming one over another"),
        ("OpenOptions", "opening one for anything but reading"),
        ("create_new", "as above"),
        ("fs::remove", "removing one"),
        ("truncate", "emptying one"),
    ];

    let core = std::fs::read_to_string("src/core/rules.rs").expect("core/rules.rs");
    let cli = std::fs::read_to_string("src/cli/rules.rs").expect("cli/rules.rs");
    for (source, name) in [(&core, "core::rules"), (&cli, "cli::rules")] {
        for (banned, what) in BANNED {
            assert!(
                !source.contains(banned),
                "`{name}` names `{banned}` — {what}. This feature reads six permission files and \
                 prints what is missing, which is the exact shape of a tool that would apply it. \
                 It does not, on purpose. Hand the text over instead."
            );
        }
    }

    // The route, sliced out of the API so the check is about this path rather
    // than about a file that legitimately writes elsewhere.
    let api = std::fs::read_to_string("src/api.rs").expect("api.rs");
    let reader = api
        .split("fn read_rules(")
        .nth(1)
        .expect("`read_rules` is the one function here that touches a disk");
    let reader = &reader[..reader.find("\n}\n").expect("the end of read_rules")];
    for (banned, what) in BANNED {
        assert!(
            !reader.contains(banned),
            "`read_rules` names `{banned}` — {what}. It reads two files per project and returns \
             what they say. A write here would be reachable by an agent holding the token."
        );
    }
    // And it is the only function of the feature that opens anything at all.
    let route = api
        .split("async fn rules(")
        .nth(1)
        .expect("the /api/rules route");
    let route = &route[..route.find("\n}\n").expect("the end of the route")];
    assert!(
        !route.contains("std::fs"),
        "the `/api/rules` route reaches the filesystem directly. Reading belongs in `read_rules`, \
         where one check can cover it."
    );

    // **No apply-to-all, in any surface or flag.** The measured result says the
    // obvious next step is as likely to hurt as help, so offering the button
    // would be taking a side the evidence does not support.
    let cli_mod = std::fs::read_to_string("src/cli/mod.rs").expect("cli/mod.rs");
    for shape in ["apply_all", "apply-to-all", "--all", "everywhere"] {
        let near = cli_mod
            .split("Rules {")
            .nth(1)
            .map(|r| &r[..r.find('}').unwrap_or(r.len())])
            .unwrap_or("");
        assert!(
            !near.contains(shape),
            "`devplane rules` grew a `{shape}` flag. There is no apply-to-all: adding instruction \
             files helped 27.7% of 148 measured projects and hurt 26.35%, and what separated them \
             was their content rather than their presence."
        );
    }
}
