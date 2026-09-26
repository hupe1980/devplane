//! Source-scan guards for the crate's structural rules. The first: `src/core/`
//! may not reach the outside world (no async, runtime, database, HTTP or
//! spawning), so run state replays, the inbox is derived, and the policy cannot
//! fail open. Nor may it read the clock or the disk: `now` and whatever a
//! file said arrive as values, so the same inputs always derive the same
//! board. Test modules (`#[cfg(test)]` onward) may do both.

use std::path::Path;

/// Tokens banned in `src/core/`, each with the reason a failure prints.
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
    (
        "Timestamp::now",
        "the clock is read at the edge (`src/stamp.rs`) and passed in as `now`, so a replay derives the same inbox",
    ),
    ("Zoned::now", "as above"),
    ("SystemTime::now", "as above"),
    ("Instant::now", "as above"),
    (
        "std::fs::",
        "file I/O belongs outside the pure half (`src/repo.rs`, `src/spec.rs`, `src/config.rs`); pass what the file said in as a value",
    ),
];

#[test]
fn the_pure_half_cannot_reach_the_outside_world() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core");
    let mut files = 0;
    let mut broken = Vec::new();

    // Walked recursively: nested modules are the pure half too.
    for path in walk(&dir.to_string_lossy()) {
        let path = Path::new(&path);
        files += 1;
        let name = path
            .strip_prefix(&dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(path).expect("readable");

        for (line_no, line) in source.lines().enumerate() {
            // Tests may use the clock and the disk to build their fixtures.
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            // Comments may quote what the rule forbids.
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
    // Proves the matcher itself can fail.
    let sample = "pub async fn evaluate(&self) { self.fetch().await }";
    let hits: Vec<&str> = BANNED
        .iter()
        .map(|(t, _)| *t)
        .filter(|t| sample.contains(t))
        .collect();
    assert_eq!(hits, ["async fn", ".await"]);
}

/// No string literal carries a run of spaces between words, which is what a
/// `\` line continuation leaves behind when the backslash is dropped.
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
            // A lost `\` leaves six or more spaces between two words inside a
            // literal; column layouts and `\n`-indented lines don't.
            let bytes = line.as_bytes();
            for (i, _) in line.match_indices("      ") {
                let before = &line[..i];
                let inside_literal = before.matches('"').count() % 2 == 1;
                let prev = bytes[..i].last().copied().unwrap_or(b' ');
                let next = line[i..].bytes().find(|b| *b != b' ').unwrap_or(b'"');
                let word_then_word = (prev.is_ascii_alphanumeric() || b",.;:-".contains(&prev))
                    && next.is_ascii_lowercase();
                // No exemption after `\n`: deliberate indents are two or four
                // spaces, below the threshold.
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

/// The policy engine cannot express an approval (no `Allow` in `Verdict`), and
/// the only accept is built in the function that waits for a person's answer.
/// An `Allow` variant would break exhaustive matches before this runs, so the
/// session assertions are the load-bearing ones.
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

    // Exactly one accept, inside the answer channel: a second would be a
    // second way to say yes.
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

/// Nothing in the question path writes a file or edits settings: an agent runs
/// as the same user and could reach any such route.
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

    // The answer route composes the reply, never a rule.
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

/// A permission answer naming neither a decision nor an option is refused, not
/// read as `Deny`: fail-closed suits a gate, not a person's answer.
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

    // `parse_answer` is the one parser every surface's answer goes through.
    let driven = std::fs::read_to_string("src/driven.rs").expect("driven.rs");
    let parser = driven
        .split("pub fn parse_answer(")
        .nth(1)
        .and_then(|s| s.split("\npub ").next())
        .expect("parse_answer");
    let permission = parser
        .split("Kind::Permission")
        .nth(1)
        .and_then(|s| s.split("Kind::Question").next())
        .expect("the permission arm");
    let refuses = permission.find("return Err(");
    let parses = permission.find("Decision::parse(decision");
    assert!(
        matches!((refuses, parses), (Some(r), Some(p)) if r < p),
        "the permission arm of `parse_answer` reaches `Decision::parse` without first refusing an \
         answer that named neither a decision nor an option. `parse` maps nothing to `Deny`, so \
         that path records a refusal nobody chose"
    );

    // The refusal tells a mis-wired caller what to send.
    assert!(
        permission.contains("say what the answer is"),
        "the refusal does not tell the caller what to send"
    );
}

/// Each way a question ends (answered, run ended, unshowable form) has its own
/// non-empty sentence, so the log shows which one happened.
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

    // An answer records what was chosen, not only that somebody answered.
    assert!(
        driven.contains(r#""a person answered: {chose}""#),
        "the answer's own content left the reason, so the log records that somebody answered \
         and not what they said"
    );
}

/// The answer channel itself refuses a run Devplane did not start, so a caller
/// with `curl` and the token gets the same refusal as the page.
#[test]
fn only_a_run_devplane_drives_can_have_a_question_answered() {
    let driven = std::fs::read_to_string("src/driven.rs").expect("driven.rs");

    // `deliver` must check the run before taking either route.
    let deliver = driven
        .split("async fn deliver(")
        .nth(1)
        .and_then(|s| s.split("\n/// ").next())
        .expect("driven::deliver");

    let lookup = deliver
        // `state.sessions`, however rustfmt breaks the chain.
        .find(".sessions")
        .expect("delivery asks the driven-session table whether this run is one of ours");
    let live_send = deliver
        .find("Answer::Permission(d) => decide(")
        .expect("the live route goes through `decide`");
    assert!(
        lookup < live_send,
        "an answer is sent before the run is checked to be one Devplane drives"
    );

    // The resumed route goes through `resume_run`, which refuses the same way.
    assert!(
        deliver.contains("resume_run(state, &ask.run"),
        "the route for a run whose agent has gone must go through `resume_run`"
    );
    let resume_fn = driven
        .split("async fn resume_run(")
        .nth(1)
        .and_then(|s| s.split("\n/// ").next())
        .expect("driven::resume_run");
    assert!(
        resume_fn.contains("that is a session Devplane watches, not one it drives"),
        "resume stopped refusing a session Devplane merely watches"
    );

    // The shared refusal a direct caller meets.
    assert!(
        driven.contains("that run is not one Devplane drives"),
        "the refusal lost its sentence; a caller is told nothing about why"
    );
}

/// Every public function in the crate has a non-test caller. An unread rule
/// tends to get rewritten elsewhere and the copies drift; a helper only tests
/// need counts as dead.
#[test]
fn nothing_in_this_crate_is_a_rule_with_no_reader() {
    let mut defined: Vec<(String, String)> = Vec::new();
    let mut bodies: Vec<(String, String)> = Vec::new();

    for entry in walk("src") {
        let text = std::fs::read_to_string(&entry).expect("source");
        let prod = production(&text);
        {
            // Skip `#[cfg(test)]` items: their callers are the stripped test modules.
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
        // Reached in ways this scan can't see: traits, constructors, serde.
        if matches!(
            name.as_str(),
            "new" | "default" | "fmt" | "from" | "parse" | "serialize" | "deserialize"
                // The binary's entry point.
                | "main"
        ) {
            continue;
        }
        // Whole-word, so a function passed by reference counts as used.
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

/// Source before the file's column-zero `#[cfg(test)] mod`, comments blanked:
/// a doc comment naming a function is a mention, not a reader.
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

/// How many times this file defines `name`, so its signature is not a use.
fn defs(body: &str, name: &str) -> usize {
    let mut n = 0;
    for prefix in ["pub fn ", "pub const fn ", "pub async fn "] {
        let needle = format!("{prefix}{name}");
        let mut from = 0;
        while let Some(i) = body[from..].find(&needle) {
            let end = from + i + needle.len();
            // Require a boundary so `draft_links` doesn't define `draft_link`.
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

/// The Spec Kit gate reports an exit code and never names permission
/// vocabulary, so it cannot become a second decider.
#[test]
fn the_speckit_gate_never_becomes_a_second_decider() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    // The two command bodies, cut from the file they share.
    let cli = std::fs::read_to_string(root.join("src/cli/change.rs")).expect("the CLI");
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
    let spec = std::fs::read_to_string(root.join("src/spec.rs")).expect("the hook contract");
    bodies.push(("src/spec.rs", spec));
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
        // Comments may name the refusal to explain it; only code is checked.
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

/// Every `AcpEvent` variant is constructed somewhere. A `pub` variant is never
/// dead to the compiler, and a match arm is a reader, not a constructor.
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

    // Where an event may be built, including the fixture agent in `examples/`.
    let sources: String = walk("src")
        .into_iter()
        .chain(walk("examples"))
        .map(|f| production(&std::fs::read_to_string(&f).expect("source")))
        .collect::<Vec<_>>()
        .join("\n");

    let mut unbuildable = Vec::new();
    for v in &variants {
        // A `=>` before `;`/`)` means a match arm; otherwise it was built.
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

/// `CLAUDE_AFK_TIMEOUT_MS` is read but never set: setting it would put a clock
/// on a person's own questions.
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

    // `CLAUDE_AFK_COUNTDOWN_MS` is cosmetic and belongs to the person's
    // terminal; it is never read.
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

/// The CLI reads the modes response through shared types, never `.get("…")`
/// on JSON, so a renamed field is a compile error rather than a blank line.
#[test]
fn the_clock_is_never_read_out_of_json_by_hand() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let whole = std::fs::read_to_string(root.join("src/cli/inbox.rs")).expect("src/cli/inbox.rs");

    // Comments stripped: they may describe the defect this guards against.
    let text: String = whole
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    // Every `ClockLine` field.
    for field in [
        "after",
        "immediate",
        "source",
        "where_set",
        "says",
        "chosen_by_the_person",
        "answers_for_you",
        // A former field name, so that exact regression stays caught.
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

    // Positive half: it deserialises `world::Modes`, which carries the clock.
    assert!(
        text.contains("world::Modes"),
        "the modes rendering must go through the shared type, which carries the clock"
    );

    // Every `Modes` field too.
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

/// Vendor-specific behaviour lives in `core::vendors`, never in a surface
/// comparing an agent id.
#[test]
fn no_surface_branches_on_which_vendor_it_is() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Surfaces only; observation adapters are vendor-specific by definition.
    let surfaces = [
        "src/api.rs",
        "src/cli/board.rs",
        "src/cli/inbox.rs",
        "src/cli/change.rs",
        "src/cli/admin.rs",
        "src/render.rs",
    ];

    // The shape of a comparison, not the word: `"claude"` as a value is fine.
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

/// `unreported`, which guards the reassuring modes line, exists on the type and
/// is filled by the handler.
#[test]
fn every_figure_the_modes_surface_guards_on_is_one_the_api_sends() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let world = std::fs::read_to_string(root.join("src/core/world.rs")).expect("world");
    let api = std::fs::read_to_string(root.join("src/view.rs")).expect("view");

    // The type carries it…
    assert!(
        world.contains("pub unreported: usize"),
        "core::world::Modes has no `unreported`, which the CLI guards the reassuring line on"
    );
    // …and the handler fills it; an unset field serialises as zero.
    assert!(
        api.contains("unreported: projects.iter().map(|p| p.unreported).sum()"),
        "/api/modes does not sum `unreported`, so the guard on the reassuring line reads zero"
    );
}

/// A `never` question clock renders as not answering, distinct from both an
/// answering clock and no clock.
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

/// The inbox ordering (level, decorrelation, age) never reads a model's score.
#[test]
fn the_inbox_never_orders_by_what_a_model_thinks() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let whole = std::fs::read_to_string(root.join("src/core/attention.rs")).expect("attention.rs");

    // Comments stripped: this file discusses the refusal.
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

/// Every read of a stored gate report either checks it against the current
/// tree or names the commit it ran against; none reports passed with neither.
#[test]
fn every_read_of_a_stored_gate_report_is_honest_about_its_tree() {
    /// `(file, what reads it, how it is honest)`
    const SITES: &[(&str, &str, &str)] = &[
        (
            "src/core/change.rs",
            "Completion::of_at",
            "checks currency via facts::still_current",
        ),
        (
            "src/view.rs",
            "ChangeView::of",
            "names the commit as GateView::ran_at",
        ),
        (
            "src/change.rs",
            "the pull-request body",
            "names the commit in ## Verification",
        ),
        (
            "src/core/certificate.rs",
            "the certificate",
            "names the commit; a historical claim",
        ),
        (
            "src/core/attention.rs",
            "the inbox",
            "reads only a failure, and a stale failure still happened",
        ),
        (
            "src/driven.rs",
            "the prompt context",
            "reads only a failure, and a stale failure still happened",
        ),
    ];

    let mut readers = 0;
    for (file, _, _) in SITES {
        let src = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        readers += src.matches("check_report()").count();
    }
    assert!(
        readers >= SITES.len(),
        "the declared sites no longer read a stored gate report — this guard would pass vacuously"
    );

    // Every file that reads one must be declared here.
    for path in walk("src") {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !src.contains("check_report()") {
            continue;
        }
        let path = path.replace('\\', "/");
        assert!(
            SITES.iter().any(|(f, _, _)| *f == path),
            "{path} reads a stored gate report and is not declared in this guard. \
             Say how it is honest about the tree — check currency, or name the commit."
        );
    }

    // And the two surfaces that name a commit must actually carry it.
    let api = std::fs::read_to_string("src/view.rs").unwrap();
    assert!(
        api.contains("ran_at"),
        "GateView stopped carrying the commit it ran against"
    );
    let work = std::fs::read_to_string("src/change.rs").unwrap();
    assert!(
        work.contains("Against commit"),
        "the pull-request body stopped naming the tree its verification describes"
    );
}

/// A cost never received renders as unknown, never as `$0.00` or the `-` of a
/// free session.
#[test]
fn a_cost_that_was_never_received_is_not_rendered_as_an_amount() {
    let board = std::fs::read_to_string("src/cli/board.rs").expect("board.rs");
    assert!(
        board.contains("cost_unknown"),
        "the board stopped distinguishing a missing cost from a zero one"
    );

    // No formatted zero cost anywhere; comments stripped since they quote it.
    let mut checked = 0;
    for path in walk("src") {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let code: String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        checked += 1;
        assert!(
            !code.contains("$0.00"),
            "{path} renders a literal $0.00 — a cost nobody received is unknown, not nothing"
        );
    }
    assert!(
        checked > 0,
        "no sources read — this guard would pass vacuously"
    );
}

/// `devplane quit` reads and says what it will end before stopping the host.
/// Checked in source because stdout order cannot show it.
#[test]
fn what_a_quit_ends_is_said_before_it_ends_it() {
    let src = std::fs::read_to_string("src/cli/mod.rs").expect("cli/mod.rs");
    let quit = src
        .find("async fn cmd_quit")
        .expect("`cmd_quit` is gone; update this guard or delete it");
    let body = &src[quit..];
    let stops = body
        .find("stop_and_wait()")
        .expect("`cmd_quit` no longer stops anything; update this guard");
    // The sentence is `Quitting::says`.
    let told = body[..stops].find(".says()");
    assert!(
        told.is_some(),
        "`cmd_quit` asks the host to stop before it has said what that ends. \
         A person cannot consent to what they were not told."
    );
    // The inventory must be read before the stop too.
    let read = body[..stops].find("/api/quitting");
    assert!(
        read.is_some(),
        "`cmd_quit` reads what it would stop only after stopping it"
    );
}

/// State words and glyphs are spelled once in `ChangeState` and mirrored by the
/// interface's state table; no other file spells one.
#[test]
fn the_six_state_words_are_spelled_once_and_the_table_carries_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core = std::fs::read_to_string(root.join("src/core/change.rs")).expect("core::change");
    let arms = |body: &str| -> Vec<String> {
        core.split(body)
            .nth(1)
            .and_then(|s| s.split('}').next())
            .unwrap_or("")
            .lines()
            .filter_map(|l| l.split("=> \"").nth(1))
            .filter_map(|r| r.split('"').next())
            .map(str::to_string)
            .collect()
    };
    let words = arms("pub fn as_str(self) -> &'static str {");
    let glyphs = arms("pub fn glyph(self) -> &'static str {");
    assert_eq!(
        words,
        [
            "drafted",
            "isolated",
            "in flight",
            "verified",
            "offered",
            "archived"
        ],
        "the six states changed under this guard"
    );
    assert_eq!(glyphs.len(), 6, "every state has a glyph");

    // The interface's one table pairs each word with the host's glyph.
    let table = std::fs::read_to_string(root.join("ui/src/lib/State.svelte")).expect("the table");
    for (word, glyph) in words.iter().zip(&glyphs) {
        assert!(
            table.contains(&format!("glyph: \"{glyph}\", word: \"{word}\"")),
            "the state table does not pair `{word}` with `{glyph}`"
        );
    }

    // Nowhere else spells one; `verified` is also a gate word, so it is skipped.
    let mut offenders = Vec::new();
    let mut stack = vec![root.join("ui/src"), root.join("src/cli")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.ends_with("wire") {
                    continue;
                }
                stack.push(p);
                continue;
            }
            let is_source = p
                .extension()
                .is_some_and(|x| x == "ts" || x == "svelte" || x == "rs");
            if !is_source || p.ends_with("State.svelte") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            for word in ["drafted", "isolated", "in flight", "offered", "archived"] {
                if text.contains(&format!("\"{word}\"")) {
                    offenders.push(format!("{}: \"{word}\"", p.display()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a state word is spelled outside the one table: {offenders:?}"
    );
}

/// A report's finding and evidence are read only through `Report::quoted`, so
/// they always render attributed and quoted. The store is exempt.
#[test]
fn a_reports_finding_is_read_only_where_it_is_quoted() {
    let allowed = ["src/core/report.rs", "src/store.rs"];
    let mut scanned = 0;
    let mut offenders = Vec::new();
    for file in walk("src") {
        let text = std::fs::read_to_string(&file).expect("source");
        if !text.contains("report::") && !text.contains("mod report") {
            continue;
        }
        scanned += 1;
        if allowed.iter().any(|a| file.ends_with(a)) {
            continue;
        }
        let prod = production(&text);
        for (n, line) in prod.lines().enumerate() {
            for field in [".finding", ".evidence"] {
                let hit = line.match_indices(field).any(|(i, _)| {
                    !line[i + field.len()..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                });
                if hit {
                    offenders.push(format!("{file}:{}: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        scanned >= 6,
        "the scan found {scanned} files that use reports, so it is not reading the crate"
    );
    assert!(
        offenders.is_empty(),
        "a report's finding or evidence is read outside `Report::quoted` — render it through \
         that, or it can reach somebody unquoted: {offenders:?}"
    );
}

/// The two forge writes — `github::create_issue` and `github::create_pr` —
/// are each called once: the issue from `reports::open`, the pull request from
/// `change::offer`, both reached only from a person's command or button. No
/// program is run to write to a forge.
#[test]
fn nothing_writes_to_a_forge_but_a_persons_open() {
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    for (write, home, within) in [
        ("github::create_issue(", "src/reports.rs", "pubasyncfnopen("),
        ("github::create_pr(", "src/change.rs", "pubasyncfnoffer("),
    ] {
        let mut sites = Vec::new();
        for file in walk("src") {
            let prod = squash(&production(
                &std::fs::read_to_string(&file).expect("source"),
            ));
            let n = prod.matches(write).count();
            if n > 0 {
                sites.push((file, n, prod));
            }
        }
        assert_eq!(
            sites
                .iter()
                .map(|(f, n, _)| (f.as_str(), *n))
                .collect::<Vec<_>>(),
            [(home, 1)],
            "`{write}…)` must be called exactly once, in {home}"
        );
        let (_, _, body) = &sites[0];
        let open_at = body
            .find(within)
            .unwrap_or_else(|| panic!("{within} exists in {home}"));
        let create_at = body.find(write).expect("the call");
        let next_fn = body[open_at + 1..]
            .find("pubasyncfn")
            .map(|i| open_at + 1 + i)
            .unwrap_or(body.len());
        assert!(
            (open_at..next_fn).contains(&create_at),
            "the forge write `{write}…)` is not inside {within}…)"
        );
    }
    // Nothing starts a forge's command-line tool, for any purpose.
    for file in walk("src") {
        let prod = squash(&production(
            &std::fs::read_to_string(&file).expect("source"),
        ));
        assert!(
            !prod.contains("Command::new(\"gh\")") && !prod.contains("\"DEVPLANE_GH\""),
            "{file} runs `gh`"
        );
    }

    // Its callers: the route, and nothing else.
    let mut callers = Vec::new();
    for file in walk("src") {
        let prod = production(&std::fs::read_to_string(&file).expect("source"));
        if prod.contains("reports::open(") {
            callers.push(file.clone());
        }
    }
    assert_eq!(
        callers,
        ["src/api.rs"],
        "reports::open is reached from somewhere but its route"
    );
    let api = production(&std::fs::read_to_string("src/api.rs").expect("api"));
    assert_eq!(
        api.matches("reports::open(").count(),
        1,
        "reports::open is called from more than one place in the API"
    );
    // And the route is named by the person's command and nothing else in src.
    let mut named = Vec::new();
    for file in walk("src") {
        let prod = production(&std::fs::read_to_string(&file).expect("source"));
        if prod.contains("/open\"") && prod.contains("reports/") {
            named.push(file);
        }
    }
    named.sort();
    assert_eq!(
        named,
        ["src/api.rs", "src/cli/report.rs"],
        "the open route is called from somewhere but `devplane report open`"
    );
}
