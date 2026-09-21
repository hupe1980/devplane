//! Splitting a shell command the way Claude Code splits one.
//!
//! A `Bash(…)` rule is not matched against the string the agent wrote. Claude
//! Code is aware of shell operators, strips a fixed set of wrappers, and then
//! matches **each subcommand independently** — and the two sides are not
//! symmetric:
//!
//! * a **deny** or **ask** rule fires when *any* subcommand matches, including
//!   one nested in a subshell, a command substitution or a control-flow body;
//! * an **allow** rule approves only when *every* subcommand matches.
//!
//! Matching the whole string instead was wrong in both directions at once, and
//! both were silent:
//!
//! * `never_auto = ["Bash(rm -rf *)"]` did not stop `ls && rm -rf /` — the
//!   prohibition read as protection and was none;
//! * `auto_allow = ["Bash(pnpm test *)"]` **auto-approved** `pnpm test && rm
//!   -rf /`, which is the example Claude Code's own documentation uses to
//!   explain why it splits. Devplane answers the permission prompt, so that
//!   was a destructive command approved without anybody being asked.
//!
//! Everything here is a pure function of the command text, because it runs on
//! the synchronous hook a session is blocked on.
//!
//! It is a splitter, not a shell. Claude Code's own documentation is explicit
//! that a Bash rule "isn't a security boundary around the program", and that is
//! true here too: this reads a line, it does not run one.
//!
//! **What changed is which direction it is allowed to be wrong in.** This module
//! was built to *agree* with Claude Code — `/bin/rm`, `sudo rm` and `sh -c 'rm …'`
//! are not covered there by a rule naming `rm`, and they were not covered here
//! either. That was correct while Devplane could **approve**: a matcher broader
//! than the vendor's would have approved calls the user's own settings refuse.
//!
//! Devplane cannot approve. [`crate::core::Verdict`] has no `Allow` and the type
//! cannot express one, so the only thing a broader match can do is **refuse
//! more**, and refusing more costs a prompt. So on the restrictive side:
//!
//! * [`strip_transparent`] looks through `sudo`, `doas`, `exec`, `env` and the
//!   exec wrappers, so `sudo rm -rf /` meets `Bash(rm *)`;
//! * [`basename_program`] reduces `/bin/rm` to `rm`;
//! * and [`undecidable`] names the lines where neither works, so the call can go
//!   to a person instead of past one.
//!
//! None of that reaches the allow-class analysis, which still describes what the
//! vendor's own rules do, because that is what it is for.

/// Shell operators that separate one command from the next.
///
/// Claude Code's list, in the order they must be tested — two-character
/// operators before the one-character ones they start with.
const SEPARATORS: &[&str] = &["&&", "||", "|&", ";", "|", "&", "\n"];

/// Wrappers that run their argument as the real command, so the rule should see
/// the argument. Claude Code's fixed list; it is not configurable there either.
const WRAPPERS: &[&str] = &[
    "timeout", "time", "nice", "nohup", "stdbuf", "command", "builtin", "noglob",
];

/// Wrappers that run their argument but which Claude Code will **not** let a
/// prefix rule approve past: *"Exec wrappers such as `watch`, `setsid`,
/// `ionice`, and `flock` can't be auto-approved by a prefix rule like
/// `Bash(watch *)`, so in Manual mode they always prompt."*
///
/// They are deliberately not in `WRAPPERS`. Stripping them would be worse than
/// ignoring them: `Bash(watch *)` would then be read as a rule about whatever
/// `watch` runs, and `watch rm -rf /` would be approved by a rule whose author
/// was thinking about watching a log file.
const EXEC_WRAPPERS: &[&str] = &["watch", "setsid", "ionice", "flock"];

/// `find` predicates that make it run a program or delete files, which is why
/// *"a `Bash(find *)` rule doesn't cover these forms"*.
const FIND_EXEC_PREDICATES: &[&str] = &["-exec", "-execdir", "-delete", "-ok", "-okdir"];

/// The longest command Claude Code will analyse. Past it, *"commands longer
/// than 10,000 characters always prompt because they exceed what the analysis
/// parses"* — so an allow rule must not answer for one either.
pub const MAX_ANALYSED: usize = 10_000;

/// Whether any part of this line hands its arguments to something else to run.
///
/// The reference's escape hatch for an exec wrapper is *"write an exact-match
/// rule for the full command string"*, and it works there. It measurably does
/// **not** work for these: the running product refuses
/// `env -C . cat .env ; git config …` under an allow rule naming that exact
/// line. So a whole-line rule stops short of them, while the wrappers the
/// reference names keep their escape hatch.
pub fn contains_analysis_barrier(command: &str) -> bool {
    // A substring scan before the parse. This is reached once per allow rule
    // per evaluation, and almost no command contains any of these words.
    if !ANALYSIS_BARRIERS.iter().any(|b| command.contains(b)) {
        return false;
    }
    nested_commands(command).iter().any(|part| {
        let stripped = strip(part, false);
        stripped
            .split_whitespace()
            .next()
            .map(|p| p.rsplit('/').next().unwrap_or(p))
            .is_some_and(|p| ANALYSIS_BARRIERS.contains(&p))
    })
}

/// Why no *prefix* allow rule may answer for this command, if there is a reason.
///
/// These are the forms Claude Code puts in front of a person whatever an allow
/// rule says, and the reason they need naming here is the direction the mistake
/// runs in: Devplane is the thing answering the prompt, so a rule that is
/// broader here than there is a call approved without anybody being asked. The
/// escape hatch is the one Claude Code documents — *"write an exact-match rule
/// for the full command string"* — so a rule with no wildcard in it still
/// works, and only the prefix form is refused.
/// Shell constructs this matcher does not model, and therefore may not approve.
///
/// **This is the allowlist half, and it is the important half.**
/// [`unapprovable_by_prefix`] is a blocklist: allow unless one of the hazards
/// somebody thought of is present. Every widening this gate was found to have
/// was a hazard nobody had thought of yet, which is why that list only ever
/// grows — and why keeping a blocklist honest needed a harness running for
/// ever to find the next one. That harness is gone, with the approval it
/// protected.
///
/// This asks the opposite question: **is every construct here one we claim to
/// understand?** A command carrying anything else cannot be approved, whatever
/// the rules say — not because that construct is known to be dangerous, but
/// because its meaning is not known at all, and approving a call you cannot
/// read is the definition of the widening direction.
///
/// It is also why this needs no oracle. *Narrower than the vendor* is the safe
/// side of Principle II, and refusing to approve what we cannot parse is
/// narrower by construction, on every release the vendor has ever shipped and
/// every one it will. **This is why the approval path could be deleted without
/// deleting the safety it bought**: refusing what cannot be parsed needs no
/// agreement from anybody, so it survives the harness that used to be the only
/// thing between here and a class nobody enumerated.
///
/// Found by asking what the blocklist lets through: `cat $'\x2e\x65nv'` was
/// approved under `Bash(cat *)` while `cat .env` was correctly denied by
/// `Read(.env)` — the same file, spelled in a quoting form [`dequoted`] does
/// not read. That is a project's own `never_auto` defeated by its own
/// `auto_allow`, with no vendor disagreement needed to make it wrong.
pub fn unmodelled_construct(command: &str) -> Option<String> {
    // Ordered most-specific first, so the reason a reader gets is the useful one.
    const UNMODELLED: &[(&str, &str)] = &[
        (
            "$'",
            "ANSI-C quoting, whose escapes name characters this matcher does not decode",
        ),
        (
            "$((",
            "arithmetic expansion, whose result is computed by the shell",
        ),
        (
            "$(",
            "command substitution, whose text is produced by another command",
        ),
        (
            "`",
            "command substitution in backticks, whose text is produced by another command",
        ),
        (
            "<(",
            "process substitution, which becomes a path the shell creates",
        ),
        (
            ">(",
            "process substitution, which becomes a path the shell creates",
        ),
        ("${", "parameter expansion, whose value the shell supplies"),
    ];
    for (needle, why) in UNMODELLED {
        if command.contains(needle) {
            return Some(format!("it contains `{needle}` — {why}"));
        }
    }
    // A bare `$NAME`: the value arrives from the environment and is unknown
    // here. `$` followed by anything else — a literal dollar in a filename, an
    // end-of-string — is left alone rather than guessed at.
    let bytes: Vec<char> = command.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if *c == '$'
            && bytes
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || *n == '_')
        {
            return Some(
                "it contains a `$` variable, whose value the environment supplies".to_string(),
            );
        }
    }
    if unbalanced_quote(command) {
        return Some("it has an unbalanced quote, so the shell reads it differently".to_string());
    }
    None
}

/// Whether a quote opens and never closes.
///
/// The rest of the line is then not what it looks like, and [`dequoted`] reads
/// it differently from the shell — so nothing downstream of it is a fact.
pub fn unbalanced_quote(command: &str) -> bool {
    let (mut single, mut double) = (false, false);
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        if c == '\\' && !single {
            chars.next();
        } else if c == '\'' && !double {
            single = !single;
        } else if c == '"' && !single {
            double = !double;
        }
    }
    single || double
}

pub fn unapprovable_by_prefix(command: &str) -> Option<String> {
    if command.chars().count() > MAX_ANALYSED {
        return Some(format!(
            "it is over {MAX_ANALYSED} characters, past what the command analysis reads"
        ));
    }
    for part in nested_commands(command) {
        let stripped = strip(&part, false);
        let mut words = stripped.split_whitespace();
        let Some(program) = words.next() else {
            continue;
        };
        if EXEC_WRAPPERS.contains(&program) {
            return Some(format!("`{program}` runs the command that follows it"));
        }
        // The same reasoning, one step further: these assemble a command from
        // their own arguments, so a prefix rule naming them grants whatever they
        // turn out to run. The
        // running product refuses `env -C . cat .env` under an allow rule
        // naming the whole line, which is what put `env` here rather than an
        // argument about it.
        if ANALYSIS_BARRIERS.contains(&program) {
            return Some(format!(
                "`{program}` runs a command assembled from its own arguments"
            ));
        }
        if program == "find"
            && let Some(p) = words.find(|w| FIND_EXEC_PREDICATES.contains(w))
        {
            return Some(format!("`find {p}` runs a program or deletes files"));
        }
        // **A loop assigns its variable, once per iteration.** `OPTIND=1/0` is
        // arithmetic rather than a string, and `for OPTIND in 1 2` performs the
        // same assignment with the `=` out of sight — so the check that catches
        // the first has to catch the second, or the loop is the way around it.
        //
        // Measured against Claude Code 2.1.273, three times: it runs
        // `for i in 1; do …; done` and refuses `for OPTIND in 1 2; do …; done`
        // under the same allow rule. The 2.1.274 changelog then named the class
        // — *"commands that loop over or assign certain special shell
        // variables… now ask for permission"* — which is the row that sent
        // somebody looking.
        // `for` and `in` are control words and are stripped before this, so the
        // loop *variable* arrives here as the part's first word — `for OPTIND
        // in 1 2` reaches this as `OPTIND in 1 2`. The `in` is what tells a
        // loop header apart from a bare command that happens to share the name.
        if EVALUATED_VARS.contains(&program) && words.next() == Some("in") {
            return Some(format!(
                "`for {program}` assigns a variable the shell evaluates, once per iteration"
            ));
        }
    }
    None
}

/// Programs that take the code they run **on their own command line**, so a
/// prefix rule naming one grants whatever that code turns out to be.
///
/// `Bash(python:*)` matches `python -c 'import os; os.system("…")'` and answers
/// `allow`, because the command *is* the one the rule names. **Claude Code
/// allows it too** — which is why this list may only ever produce a *report* on
/// the **allow** side. On the restrictive side [`undecidable`] now reads it, and
/// the difference is that a report cannot become a grant here, only a prompt. A scan of 3,171 public agent setups found
/// **3.1 %** pre-approving arbitrary execution through a grant of this shape.
///
/// Deliberately absent: [`EXEC_WRAPPERS`] and [`ANALYSIS_BARRIERS`], which
/// [`unapprovable_by_prefix`] already refuses to approve; and `xargs`,
/// `timeout` and `nohup`, which [`strip`] removes before matching. Reporting
/// either would be warning about a hole that is closed.
const RUNS_GIVEN_CODE: &[(&str, &str)] = &[
    ("sh", "-c"),
    ("bash", "-c"),
    ("zsh", "-c"),
    ("dash", "-c"),
    ("ksh", "-c"),
    ("fish", "-c"),
    ("python", "-c"),
    ("python3", "-c"),
    ("node", "-e"),
    ("deno", "eval"),
    ("bun", "-e"),
    ("ruby", "-e"),
    ("perl", "-e"),
    ("php", "-r"),
    ("Rscript", "-e"),
    ("osascript", "-e"),
];

/// Whether `program` runs code handed to it on the command line, and the flag
/// that does it — for the sentence a report prints.
///
/// This drives `devplane check`, the trust scan and [`undecidable`].
pub fn runs_given_code(program: &str) -> Option<&'static str> {
    RUNS_GIVEN_CODE
        .iter()
        .find(|(p, _)| *p == program)
        .map(|(_, flag)| *flag)
}

/// The programs Claude Code documents by name as read-only.
///
/// The reference's set is *"`ls`, `cat`, `echo`, `pwd`, `head`, `tail`,
/// `grep`, `find`, `wc`, `which`, `diff`, `stat`, `du`, `cd`, and read-only
/// forms of `git`"* — and the last clause is deliberately not represented
/// here. Whether `git` is read-only is a property of the subcommand, and this
/// list only ever drives a *warning*; inventing a table of read-only git
/// subcommands would make that warning wrong for `git commit`, which is worse
/// than not warning at all. The same reasoning excludes the read-only forms of
/// `docker`, and `file`, and every other command the reference qualifies.
const READ_ONLY_PROGRAMS: &[&str] = &[
    "ls", "cat", "echo", "pwd", "head", "tail", "grep", "find", "wc", "which", "diff", "stat",
    "du", "cd",
];

/// Programs in that set whose flags can make an unquoted glob expand into
/// something write- or exec-capable, so Claude Code prompts rather than
/// treating the call as read-only.
const GLOB_UNSAFE: &[&str] = &["find"];

/// Whether a subcommand is a control-flow header rather than a command.
///
/// It must contain a control word — which is what makes it a header rather
/// than a program that happens to be called `i` — and must run nothing of its
/// own: no command substitution, and no redirection.
fn is_control_header(part: &str) -> bool {
    if part.contains("$(") || part.contains('`') || part.contains('>') || part.contains('<') {
        return false;
    }
    let words = words_of(part);
    !words.is_empty() && words.iter().any(|w| CONTROL_WORDS.contains(&w.as_str()))
}

/// Shell no-ops. Not in the reference's list, and included on a stronger
/// argument than a list: they do nothing, there is nothing for them to be
/// permitted to do, and a compound command that begins `true &&` is a shape
/// agents write constantly. Verified against the running product, which runs
/// `true && touch x` under an allow rule naming only `touch`.
const NO_OPS: &[&str] = &["true", "false", ":"];

/// Whether a call is one Claude Code runs with **no permission check at all**,
/// in every mode.
///
/// Not the same question as "is the program in the read-only set", and the
/// difference is the whole reason this exists. A read-only command still
/// prompts when an unquoted glob could expand to a flag like `-delete`, when an
/// argument is a Windows network path, and when the analysis cannot parse the
/// line.
///
/// **It is deliberately not a veto on allow rules.** Those cases describe what
/// happens in Manual mode when *no rule matches*; an allow rule the user wrote
/// still approves the call, there and here. Treating them as a veto would make
/// Devplane refuse calls the user's own settings allow — the mistake this
/// module has made six times in the other direction and must not now make in
/// this one. What this is for is the *warning*: telling somebody that
/// `Bash(find *)` "approves nothing" is false the moment their `find` carries a
/// glob.
pub fn never_asks_about(command: &str) -> bool {
    if command.chars().count() > MAX_ANALYSED {
        return false;
    }
    // A redirection is deliberately not considered. It "adds a check on the
    // target" rather than making the command itself something a person is
    // asked about — `ls` is still read-only when its output goes to a file —
    // and the target is checked separately by `policy::uncovered_targets`.
    let Some(parts) = subcommands(command) else {
        return false;
    };
    parts.iter().all(|part| {
        // An assignment that *runs something* is what stops this being a
        // read-only command, and only that: with the permissive strip
        // `DIRSTACKSIZE=$(id) ls` and `OPTIND=1/0 ls` were looked past and what
        // remained read as read-only, so a rule about `ls` auto-approved them
        // (2.1.251, 2.1.260). Refusing *every* non-safe assignment instead
        // would be the other failure — `SECRET=x ls` is still `ls`, and this
        // question is not about which rule covers the call.
        if !leading_assignments_are_inert(part) {
            return false;
        }
        let stripped = strip(part, true);
        let words = words_of(&stripped);
        let Some(first) = words.first() else {
            return false;
        };
        // `for i in 1`, `then`, `else` — a header, not a command. It runs
        // nothing at all unless a substitution inside it does, and the words
        // that make it a header are the evidence: a part with no control word
        // in it takes the ordinary path below.
        if is_control_header(part) {
            return true;
        }
        let program = first.rsplit('/').next().unwrap_or(first);
        if NO_OPS.contains(&program) {
            return true;
        }
        if !READ_ONLY_PROGRAMS.contains(&program) {
            return false;
        }
        // A UNC path anywhere in the arguments: reaching one can send the
        // user's Windows credentials to the host it names. Tested against the
        // text rather than against the split words, because word splitting
        // treats a backslash as an escape and `\\server\share` does not
        // survive it — the check has to see the spelling the agent wrote.
        if part.contains("\\\\") {
            return false;
        }
        let rest = &words[1..];
        !(GLOB_UNSAFE.contains(&program)
            && rest.iter().any(|w| {
                !w.starts_with('-') && (w.contains('*') || w.contains('?') || w.contains('['))
            }))
    })
}

/// Words that introduce or close a control-flow body. Dropped, so the commands
/// inside are reached: `for i in 1 2; do npm test; done` contains `npm test`.
const CONTROL_WORDS: &[&str] = &[
    "for", "while", "until", "if", "then", "else", "elif", "fi", "do", "done", "case", "esac",
    "select", "function", "in",
];

/// Environment variables Claude Code will look past even for an allow rule.
const SAFE_ASSIGNMENTS: &[&str] = &[
    "NODE_ENV",
    "RUST_LOG",
    "RUST_BACKTRACE",
    "CI",
    "DEBUG",
    "LANG",
    "LC_ALL",
    "TZ",
    "FORCE_COLOR",
    "NO_COLOR",
];

/// The subcommands of one command line, or `None` when it cannot be parsed.
///
/// `None` is not "no subcommands": Claude Code treats an unfinished command
/// like `npm test &&` as unparseable and declines to split it, and an allow
/// rule then approves nothing. Returning an empty list instead would make
/// "every subcommand matches" vacuously true, which is the direction that
/// grants.
pub fn subcommands(text: &str) -> Option<Vec<String>> {
    if unterminated(text) {
        return None;
    }
    let parts = split_top_level(text);
    let out: Vec<String> = parts
        .into_iter()
        .map(|p| strip_control_words(unwrap_group(p.trim())).to_string())
        .filter(|p| !p.is_empty())
        .collect();
    (!out.is_empty()).then_some(out)
}

/// Drops a leading `do`, `then`, `else` and friends, so the command after one
/// is what a rule is matched against — the same reach into a loop body that the
/// deny side has, on the side that grants.
fn strip_control_words(part: &str) -> &str {
    let mut cur = part.trim();
    loop {
        let Some((first, rest)) = cur.split_once(char::is_whitespace) else {
            return if CONTROL_WORDS.contains(&cur) {
                ""
            } else {
                cur
            };
        };
        if CONTROL_WORDS.contains(&first) {
            cur = rest.trim();
        } else {
            return cur;
        }
    }
}

/// Strips a wrapping `( … )` or `{ … }`: `(touch x)` is `touch x`, since the
/// parentheses say *in a subshell* rather than *a different command*.
fn unwrap_group(part: &str) -> &str {
    let mut cur = part.trim();
    loop {
        let inner = match (cur.strip_prefix('('), cur.strip_prefix('{')) {
            (Some(rest), _) => rest.strip_suffix(')'),
            (_, Some(rest)) => rest.strip_suffix('}'),
            _ => None,
        };
        match inner {
            // Only when the bracket that closes it is the *last* character, so
            // `(a) && b` is left alone for the splitter above to handle.
            Some(i) if balanced_group(i) => cur = i.trim().trim_end_matches(';').trim(),
            _ => return cur,
        }
    }
}

/// Whether a group body has no unbalanced bracket of its own, which is what
/// makes stripping the outer pair safe.
fn balanced_group(body: &str) -> bool {
    let mut depth = 0i32;
    for c in body.chars() {
        match c {
            '(' | '{' => depth += 1,
            ')' | '}' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

/// Every command anywhere in the text: subcommands, and the contents of every
/// subshell, command substitution and control-flow body.
///
/// What a deny or ask rule is matched against, one at a time.
pub fn nested_commands(text: &str) -> Vec<String> {
    nested_commands_bounded(text).0
}

/// The same split, with whether it ran out of room.
///
/// A caller matching rule *text* can ignore the flag: an unread command is one
/// more string that might have matched, and missing it costs the same prompt
/// the unread command would have cost. A caller collecting *file targets*
/// cannot, because there the unread remainder is the difference between "names
/// no protected file" and "nobody looked".
pub fn nested_commands_bounded(text: &str) -> (Vec<String>, bool) {
    memo(&SPLITS, text, nested_commands_bounded_uncached)
}

fn nested_commands_bounded_uncached(text: &str) -> (Vec<String>, bool) {
    let mut out = Vec::new();
    let mut truncated = false;
    collect(text, &mut out, 0, &mut truncated);
    (out, truncated)
}

fn collect(text: &str, out: &mut Vec<String>, depth: usize, truncated: &mut bool) {
    // Bounded: a command an agent wrote is untrusted input, and this runs on a
    // hook a session is blocked on. Reaching a bound is reported rather than
    // swallowed — see `Targets`.
    if depth > MAX_DEPTH || out.len() >= MAX_COMMANDS {
        *truncated = true;
        return;
    }
    for part in split_top_level(text) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Words like `do` or `then` are not commands; what follows them is.
        let words = part.split_whitespace().collect::<Vec<_>>();
        let first_real = words
            .iter()
            .position(|w| !CONTROL_WORDS.contains(&w.trim_matches(|c| c == ';')));
        let part = match first_real {
            Some(0) => part.to_string(),
            Some(n) => words[n..].join(" "),
            None => continue,
        };
        if part.is_empty() {
            continue;
        }
        out.push(part.clone());
        // And anything nested inside it.
        for inner in nested_spans(&part) {
            collect(&inner, out, depth + 1, truncated);
        }
    }
}

/// The contents of each `$( … )`, `` ` … ` ``, `( … )` and `{ … }` in the text.
fn nested_spans(text: &str) -> Vec<String> {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            // Single quotes are literal; a substitution inside them is text.
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                }
                i += 1;
                continue;
            }
            Some('"') => {
                if c == '\\' {
                    i += 2;
                    continue;
                }
                if c == '"' {
                    quote = None;
                    i += 1;
                    continue;
                }
                // `$( … )` *is* expanded inside double quotes.
                if c == '$'
                    && b.get(i + 1) == Some(&'(')
                    && let Some((inner, next)) = balanced(&b, i + 2, '(', ')')
                {
                    out.push(inner);
                    i = next;
                    continue;
                }
                i += 1;
                continue;
            }
            _ => {}
        }
        match c {
            '\\' => i += 2,
            '\'' | '"' => {
                quote = Some(c);
                i += 1;
            }
            '$' if b.get(i + 1) == Some(&'(') => match balanced(&b, i + 2, '(', ')') {
                Some((inner, next)) => {
                    out.push(inner);
                    i = next;
                }
                None => i += 1,
            },
            '`' => match until(&b, i + 1, '`') {
                Some((inner, next)) => {
                    out.push(inner);
                    i = next;
                }
                None => i += 1,
            },
            '(' => match balanced(&b, i + 1, '(', ')') {
                Some((inner, next)) => {
                    out.push(inner);
                    i = next;
                }
                None => i += 1,
            },
            '{' => match balanced(&b, i + 1, '{', '}') {
                Some((inner, next)) => {
                    out.push(inner);
                    i = next;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out
}

fn balanced(b: &[char], from: usize, open: char, close: char) -> Option<(String, usize)> {
    let mut depth = 1usize;
    let mut i = from;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                } else if c == open {
                    depth += 1;
                } else if c == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some((b[from..i].iter().collect(), i + 1));
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn until(b: &[char], from: usize, close: char) -> Option<(String, usize)> {
    let mut i = from;
    while i < b.len() {
        if b[i] == '\\' {
            i += 2;
            continue;
        }
        if b[i] == close {
            return Some((b[from..i].iter().collect(), i + 1));
        }
        i += 1;
    }
    None
}

/// Splits on shell operators at the top level, ignoring anything quoted or
/// nested. The nested text is reached by [`nested_spans`] instead.
fn split_top_level(text: &str) -> Vec<String> {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    let mut depth = 0usize;

    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            if c == '\\' && q == '"' {
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match c {
            '\\' => {
                i += 2;
                continue;
            }
            '\'' | '"' => {
                quote = Some(c);
                i += 1;
                continue;
            }
            '(' | '{' => {
                depth += 1;
                i += 1;
                continue;
            }
            ')' | '}' => {
                depth = depth.saturating_sub(1);
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0
            && let Some(sep) = SEPARATORS
                .iter()
                .find(|s| b[i..].starts_with(&s.chars().collect::<Vec<_>>()[..]))
        {
            out.push(b[start..i].iter().collect::<String>());
            i += sep.chars().count();
            start = i;
            continue;
        }
        i += 1;
    }
    out.push(b[start..].iter().collect::<String>());
    out
}

/// The command with shell quoting removed, so a rule is matched against the
/// word the shell will build rather than the spelling the model produced.
///
/// `r''m -rf /home` runs `rm`; a deny written `Bash(rm *)` is matched against
/// the text and therefore does not see it. The published name for the class is
/// **GuardFall** — eleven agents tested, ten with the gap, and the one that
/// closed it did so by evaluating *"the way bash does, before applying security
/// rules"*. Operand matching here has always dequoted, because targets are
/// extracted word by word; the command **text** path had not, and that
/// asymmetry is the whole bug.
///
/// Used for **restrictive rules only**. Removing quotes can only make more
/// text match, so on the allow side it would approve a call whose spelling
/// nobody wrote a rule for — the one direction this module is never allowed to
/// be wrong in.
///
/// Quote removal as the shell does it: a `'` outside double quotes and a `"`
/// outside single quotes are syntax and disappear; a backslash outside single
/// quotes escapes the next character and disappears. What is deliberately *not*
/// done is expansion — `$IFS` and `$(echo rm)` name something known only at
/// runtime, and inventing a value for them would be guessing rather than
/// canonicalising. Those stay unanalysable, and `unapprovable_by_prefix` is
/// what keeps an allow rule off them.
pub fn dequoted(command: &str) -> String {
    if !command.contains(['\'', '"', '\\']) {
        return command.to_string();
    }
    let mut out = String::with_capacity(command.len());
    let (mut single, mut double) = (false, false);
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '\\' if !single => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Whether the text ends in an operator with nothing after it.
///
/// Claude Code calls that unparseable and will not split it for an allow rule,
/// so `Bash(npm *)` does not approve `npm test &&`.
fn unterminated(text: &str) -> bool {
    let t = text.trim_end();
    ["&&", "||", "|", "|&"].iter().any(|s| t.ends_with(s))
}

/// Strips the wrappers and leading assignments Claude Code strips, so a rule
/// sees the command that will actually run.
///
/// `any_assignment` is the allow/deny asymmetry: a deny or ask rule looks past
/// *any* leading assignment, an allow rule only past a known-safe one.
pub fn strip(command: &str, any_assignment: bool) -> String {
    let mut rest = command.trim();
    loop {
        // `FOO=bar cmd …`
        if let Some((head, tail)) = rest.split_once(char::is_whitespace) {
            if let Some((name, value)) = head.split_once('=')
                && is_env_name(name)
                && (any_assignment
                    || (SAFE_ASSIGNMENTS.contains(&name) && inert_assignment(name, value)))
            {
                rest = tail.trim_start();
                continue;
            }
            let word = head;
            // `timeout 30 npm test` → `npm test`, but never `command -v x`.
            if WRAPPERS.contains(&word) {
                let tail = tail.trim_start();
                if word == "command" && tail.starts_with("-v") {
                    break;
                }
                // `timeout` and `stdbuf` take options and an argument of their
                // own before the command; skipping a leading `-flag` and the
                // bare duration is what makes `timeout 30 npm test` work.
                let mut inner = tail;
                if word == "timeout" || word == "stdbuf" || word == "nice" {
                    while let Some((w, t)) = inner.split_once(char::is_whitespace) {
                        if w.starts_with('-') || w.chars().all(|c| c.is_ascii_digit() || c == '.') {
                            inner = t.trim_start();
                        } else {
                            break;
                        }
                    }
                }
                rest = inner;
                continue;
            }
            // Bare `xargs` only: with a flag it is an `xargs` command itself.
            if word == "xargs" && !tail.trim_start().starts_with('-') {
                rest = tail.trim_start();
                continue;
            }
        }
        break;
    }
    rest.to_string()
}

/// Programs that run the command that follows them, with the arguments that
/// follow that, under a different user, environment or process image.
///
/// **Deny side only, and that asymmetry is the whole point.** Claude Code does
/// not look through these — `sudo rm -rf /` is not covered there by a rule
/// naming `rm`, and `Bash(sudo *)` is the spelling its reference offers. This
/// module spent its life agreeing with that, on a premise that no longer holds:
/// agreeing mattered while Devplane could *approve*, because a matcher broader
/// than the vendor's would have approved calls the user's own settings refuse.
/// Devplane cannot approve, so the only thing a broader match can do now is
/// **refuse more**, and refusing more is free.
///
/// Measured before it was changed: with `never_auto = ["Bash(rm *)"]` set,
/// `nohup rm -rf /tmp/x` was denied and `sudo rm -rf /tmp/x` was not — because
/// `nohup` is in [`WRAPPERS`] and `sudo` was in `ANALYSIS_BARRIERS`, a list
/// about what an *allow* rule may reach through.
const TRANSPARENT: &[&str] = &[
    "sudo", "doas", "exec", "env", "watch", "setsid", "ionice", "flock",
];

/// Flags of a transparent wrapper that take a value in the next word.
fn transparent_value_flags(program: &str) -> &'static [&'static str] {
    match program {
        "sudo" | "doas" => &[
            "-u", "-g", "-p", "-C", "-h", "-r", "-t", "-U", "--user", "--group",
        ],
        "env" => &["-u", "-C", "-S", "--unset", "--chdir", "--split-string"],
        "flock" => &["-w", "--timeout", "-E", "--conflict-exit-code"],
        "ionice" => &["-c", "-n", "-p", "-P", "-u"],
        _ => &[],
    }
}

/// [`strip`], plus the wrappers that run something else under another user,
/// environment or process image: `sudo`, `doas`, `exec`, `env`, `watch`,
/// `setsid`, `ionice`, `flock`.
///
/// What a **restrictive** rule is matched against, in addition to everything
/// [`strip`] already produces. `env FOO=1 sudo -u root /bin/rm -rf /` reduces to
/// `/bin/rm -rf /` here, and [`basename_program`] takes it the last step to
/// `rm -rf /`.
///
/// It loops, because these nest: `sudo env FOO=1 watch rm …` is three of them.
/// The bound is the number of words, so the loop cannot spin on a line that
/// reduces to itself.
pub fn strip_transparent(command: &str) -> String {
    let mut rest = strip(command, true);
    for _ in 0..rest.split_whitespace().count().min(32) {
        let Some((head, tail)) = rest.trim().split_once(char::is_whitespace) else {
            break;
        };
        let word = head.rsplit('/').next().unwrap_or(head);
        if !TRANSPARENT.contains(&word) {
            break;
        }
        // Skip this wrapper's own options and any `VAR=value` before the
        // program. A flag that takes a value eats the word after it, or
        // `sudo -u root rm` would reduce to `root rm`.
        let value_flags = transparent_value_flags(word);
        let mut inner = tail.trim_start();
        while let Some((w, t)) = inner.split_once(char::is_whitespace) {
            if w == "--" {
                inner = t.trim_start();
                break;
            }
            if w.starts_with('-') {
                inner = if value_flags.contains(&w) {
                    t.trim_start()
                        .split_once(char::is_whitespace)
                        .map_or("", |(_, r)| r)
                        .trim_start()
                } else {
                    t.trim_start()
                };
                continue;
            }
            if let Some((name, _)) = w.split_once('=')
                && is_env_name(name)
            {
                inner = t.trim_start();
                continue;
            }
            break;
        }
        if inner.trim().is_empty() {
            break;
        }
        let next = strip(inner, true);
        if next == rest {
            break;
        }
        rest = next;
    }
    rest
}

/// The same command line with an absolute or relative program path reduced to
/// its file name, when it has one.
///
/// `/bin/rm -rf /` becomes `rm -rf /`. Claude Code documents that it does *not*
/// do this — a `Bash(rm *)` rule there does not cover `/bin/rm` — and for a
/// prohibition that is a hole anybody can see. Deny side only, for
/// the same reason the transparent wrappers are looked through.
///
/// `None` when the program has no path separator, so a caller can skip a second
/// match that would ask the same question twice.
pub fn basename_program(command: &str) -> Option<String> {
    let trimmed = command.trim_start();
    let (head, tail) = match trimmed.split_once(char::is_whitespace) {
        Some((h, t)) => (h, t),
        None => (trimmed, ""),
    };
    if !head.contains('/') || head.starts_with('-') {
        return None;
    }
    let base = head.rsplit('/').next().filter(|b| !b.is_empty())?;
    Some(if tail.is_empty() {
        base.to_string()
    } else {
        format!("{base} {tail}")
    })
}

/// Why a prohibition cannot be decided by reading this command line.
///
/// `None` means every word that decides **what program runs** is a literal this
/// matcher can compare against a rule, so "no rule matched" is a fact. `Some`
/// means at least one of those words is produced by the shell, by another
/// command, or by an interpreter reading a program this matcher has not seen —
/// so "no rule matched" means *nobody looked*, and the two must not be answered
/// the same way.
///
/// **This is deliberately about command position and nothing else.**
/// `echo "$(date)"` is decidable: the substitution is an argument, and the
/// program is `echo`. `$(echo rm) -rf /` is not: the program is whatever the
/// substitution prints. An earlier version of this idea used
/// [`unmodelled_construct`], which answers a different question — *is every
/// part of this line modelled* — and flags the first of those two as well.
///
/// The list behind [`runs_given_code`] carries a note saying it "may only ever
/// produce a report and must never change a verdict", because Claude Code allows those calls and a
/// matcher that refused them would refuse calls the user's settings allow. That
/// was true while Devplane could approve. It cannot, so the list may now change a
/// verdict — in the one direction that is free, which is towards a
/// person.
pub fn undecidable(text: &str) -> Option<String> {
    if text.chars().count() > MAX_ANALYSED {
        return Some(format!(
            "it is over {MAX_ANALYSED} characters, past what the command analysis reads"
        ));
    }
    if unbalanced_quote(text) {
        return Some("it has an unbalanced quote, so the shell reads it differently".to_string());
    }
    if subcommands(text).is_none() {
        return Some("it ends in an operator, so what follows is not on this line".to_string());
    }
    for part in nested_commands(text) {
        let stripped = strip_transparent(&part);
        let mut words = stripped.split_whitespace();
        let Some(program) = words.next() else {
            continue;
        };
        // The program name is not a name: the shell builds it at run time.
        if program.contains('$') || program.contains('`') {
            return Some(format!(
                "the program in `{program}` is produced by the shell, so no rule can name it"
            ));
        }
        let base = program.rsplit('/').next().unwrap_or(program);
        if ANALYSIS_BARRIERS.contains(&base) && !TRANSPARENT.contains(&base) {
            return Some(format!(
                "`{base}` runs a command assembled from its own arguments"
            ));
        }
        if let Some(flag) = runs_given_code(base) {
            let rest: Vec<&str> = words.collect();
            if rest.contains(&flag) {
                return Some(format!(
                    "`{base} {flag}` runs a program given on its own command line"
                ));
            }
            // No script to run means the program arrives on standard input,
            // which is what `… | sh` is.
            if !rest.iter().any(|w| !w.starts_with('-')) {
                return Some(format!(
                    "`{base}` with no script reads the program from its input"
                ));
            }
        }
        if base == "find"
            && let Some(p) = stripped
                .split_whitespace()
                .find(|w| FIND_EXEC_PREDICATES.contains(w))
        {
            return Some(format!("`find {p}` runs a program or deletes files"));
        }
    }
    None
}

/// Shell variables whose assignment is *evaluated* rather than stored, so the
/// value is an expression and not a string.
///
/// *"commands that assign an arithmetic expression to an integer shell variable
/// (e.g. `OPTIND=1/0`, `RANDOM=2+2`)"* (2.1.251), and the zsh reporting
/// variables that take a command substitution (2.1.260).
const EVALUATED_VARS: &[&str] = &[
    "OPTIND",
    "RANDOM",
    "SECONDS",
    "LINENO",
    "HISTCMD",
    "TMOUT",
    "HISTSIZE",
    "SAVEHIST",
    "COLUMNS",
    "LINES",
    "REPORTTIME",
    "REPORTMEMORY",
    "DIRSTACKSIZE",
    "PERIOD",
    "MAILCHECK",
];

/// Whether an assignment's value runs nothing.
///
/// Two ways it can. A **substitution** anywhere — and the name being on the
/// safe list is no help, because `NODE_ENV=$(curl evil) ls` is a substitution
/// wearing an approved name. And an **expression assigned to a variable the
/// shell evaluates**, where `OPTIND=1/0` is arithmetic rather than the string
/// `1/0`.
fn inert_assignment(name: &str, value: &str) -> bool {
    if value.contains("$(") || value.contains('`') || value.contains("${") {
        return false;
    }
    !(EVALUATED_VARS.contains(&name) && !value.chars().all(|c| c.is_ascii_digit()))
}

/// Whether every leading assignment on this command runs nothing.
fn leading_assignments_are_inert(command: &str) -> bool {
    let mut rest = command.trim();
    while let Some((head, tail)) = rest.split_once(char::is_whitespace) {
        let Some((name, value)) = head.split_once('=') else {
            break;
        };
        if !is_env_name(name) {
            break;
        }
        if !inert_assignment(name, value) {
            return false;
        }
        rest = tail.trim_start();
    }
    true
}

fn is_env_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(command: &str) -> Vec<(String, Access, bool)> {
        file_targets(command)
            .into_iter()
            .map(|t| (t.path, t.access, t.subtree))
            .collect()
    }
    fn reads(command: &str, path: &str) -> bool {
        file_targets(command)
            .iter()
            .any(|t| t.path == path && t.access == Access::Read)
    }

    /// The eight forms that release 2.1.266–2.1.268 taught Claude Code's deny
    /// rules to see and that this matcher did not. Every one of them was a
    /// `never_auto = ["Read(.env)"]` that read as protection and stopped
    /// nothing, and every one was found by reading the changelog rather than
    /// by the differential harness.
    #[test]
    fn a_suggested_rule_is_the_narrowest_one_that_covers_the_set() {
        let v = |xs: &[&str]| suggest_rule(&xs.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        // One command: grant exactly it.
        assert_eq!(v(&["git status"]), Some("git status".into()));
        // Several sharing a subcommand: the `*` goes after the subcommand,
        // which is where the reference says to put it.
        assert_eq!(
            v(&["pnpm test --run", "pnpm test -w api"]),
            Some("pnpm test *".into())
        );
        assert_eq!(
            v(&["cargo test --workspace", "cargo test -p core"]),
            Some("cargo test *".into())
        );
        // Different subcommands of the same program: `Bash(git *)` is a much
        // larger grant than the prompt asked about, so nothing is suggested.
        assert_eq!(v(&["git status", "git push origin main"]), None);
        // A bare program with no subcommand may take the program-wide form.
        assert_eq!(v(&["ls", "ls"]), Some("ls".into()));
        // Nothing is suggested where the rule would not work: a compound, and
        // a form no prefix rule may approve.
        assert_eq!(v(&["pnpm test && rm -rf /"]), None);
        assert_eq!(v(&["watch pnpm test", "watch pnpm build"]), None);
        assert_eq!(v(&[]), None);
    }

    #[test]
    fn a_command_an_agent_wrote_cannot_crash_the_matcher() {
        // The text is attacker-supplied in the sense that matters: an agent
        // chose it, and the matcher runs on the synchronous hook every tool
        // call on the machine waits for. A panic here is not a wrong answer,
        // it is every session blocked. `grep -é.env x` did exactly that —
        // `&w[2..]` is a *byte* index and `é` is two bytes.
        for command in [
            "grep -é.env x",
            "grep -\u{0}x y",
            "cat --π=.env",
            "head -日本語",
            "sed -\u{1F600}x .env",
            "env -C ｜ cat .env",
            "git diff é",
            "(((((",
            "cat \"",
            "$(",
            "cmd > ",
            "-",
            "--",
            "",
        ] {
            // The assertion is that these return at all.
            let _ = file_targets(command);
            let _ = subcommands(command);
            let _ = nested_commands(command);
            let _ = strip(command, true);
            let _ = without_redirections(command);
            let _ = unapprovable_by_prefix(command);
            let _ = rule_family("Bash", command);
            let _ = suggest_rule(&[command.to_string()]);
        }
    }

    #[test]
    fn a_suggestion_is_written_in_the_vocabulary_that_tool_uses() {
        let v = |tool: &str, xs: &[&str]| {
            suggest_rule_for(tool, &xs.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        // A path rule is about a directory. Offering the file gives a person
        // one rule per file, and the next file in the same directory asks
        // again — which is not a suggestion, it is a treadmill.
        assert_eq!(
            v("Read", &["src/main.rs", "src/lib.rs"]),
            Some("src/**".into())
        );
        assert_eq!(v("Edit", &["a/b/x.rs"]), Some("a/b/**".into()));
        // A bare filename at the root is its own rule: `Read(.env)` is a rule
        // somebody would really write.
        assert_eq!(v("Read", &[".env", ".env"]), Some(".env".into()));
        // Two directories share no rule worth guessing at.
        assert_eq!(v("Read", &["src/a.rs", "docs/b.md"]), None);
        // A WebFetch rule takes a domain, never a URL — and the `domain:`
        // prefix is part of the specifier, not prose about it.
        assert_eq!(
            v(
                "WebFetch",
                &["https://docs.rs/x", "https://docs.rs/y/z?q=1"]
            ),
            Some("domain:docs.rs".into())
        );
        assert_eq!(v("WebFetch", &["https://a.com/x", "https://b.com/y"]), None);
        // Port and credentials are not part of the host.
        assert_eq!(
            v("WebFetch", &["https://u:p@Docs.RS:8443/x"]),
            Some("domain:docs.rs".into())
        );
        // Anything else is matched whole, so several distinct values are not
        // one rule.
        assert_eq!(
            v("WebSearch", &["rust async", "rust async"]),
            Some("rust async".into())
        );
        assert_eq!(v("WebSearch", &["a", "b"]), None);
    }

    #[test]
    fn calls_are_grouped_by_the_rule_that_could_cover_them() {
        // One bucket per tool asks `suggest_rule` to find a single rule
        // covering `pnpm test` and `rm -rf node_modules`, which it rightly
        // refuses — so a screen full of interruptions reports that nothing is
        // worth writing. The family is what makes the question answerable.
        assert_eq!(rule_family("Bash", "pnpm test --run"), "pnpm test");
        assert_eq!(rule_family("Bash", "pnpm test -w api"), "pnpm test");
        assert_eq!(rule_family("Bash", "git status"), "git status");
        assert_eq!(rule_family("Bash", "ls -la"), "ls");
        // A wrapper is not the family: `timeout 5 pnpm test` is `pnpm test`.
        assert_eq!(rule_family("Bash", "timeout 5 pnpm test"), "pnpm test");
        // Not a shell: the family is whatever that tool's rules are written
        // about — a directory for a path rule, a host for a URL.
        assert_eq!(rule_family("Read", "src/main.rs"), "src");
        assert_eq!(rule_family("Edit", "a/b/c.rs"), "a/b");
        assert_eq!(rule_family("Read", ".env"), ".env");
        assert_eq!(rule_family("WebFetch", "https://docs.rs/x?q=1"), "docs.rs");
    }

    #[test]
    fn an_option_value_is_a_path_the_way_2_1_266_made_it_one() {
        // "Fixed Bash `Read()` deny rules missing option values
        // (`--ignore-revs-file=.env`, `-f.env`, `@file`)."
        assert!(reads("grep -f.env README.md", ".env"));
        assert!(reads("sed --file=.env x", ".env"));
        assert!(reads("cat @.env", ".env"));
        // …and only for the commands Claude Code recognises by name. Its own
        // example is `git blame --ignore-revs-file=.env`, and the running
        // product runs that under `Read(.env)` — so the fix lives inside the
        // file-command scan and `git` is reached through operands only. The
        // harness's deny axis is what said so.
        assert!(!reads(
            "git blame --ignore-revs-file=.env README.md",
            ".env"
        ));
        // Generosity is free: a value that is not a path costs one target no
        // rule matches, and `-n 5` must still not read `5` as a filename.
        assert!(
            !paths("head -n 5 README.md")
                .iter()
                .any(|(p, _, _)| p == "5")
        );
    }

    #[test]
    fn git_operands_are_paths_the_way_2_1_268_made_them_paths() {
        // "Fixed deny rules not applying to `git diff`/`git grep` file
        // operands."
        assert!(reads("git diff .env", ".env"));
        assert!(reads("git show .env", ".env"));
        assert!(reads("git grep secret -- .env", ".env"));
        // `git grep` spends its first operand on the pattern, `git diff` none.
        assert!(reads("git grep pattern .env", ".env"));
        assert!(!reads("git grep .env", ".env"));
        // A subcommand that names no files reaches nothing but its options.
        assert!(paths("git status").is_empty());
        assert!(paths("git diff").is_empty());
    }

    /// A loop assigns its variable, and the `=` being out of sight changes
    /// nothing about that.
    ///
    /// **Widening thirty-one, and the mechanisms found it in the order they were
    /// built to.** `changelog-rows.sh` refused to pass with 2.1.274's row
    /// unaccounted — *"commands that loop over or assign certain special shell
    /// variables… now ask for permission"* — and the running product settled it:
    /// asked three times under one allow rule, Claude Code 2.1.273 runs
    /// `for i in 1; do …; done` and refuses `for OPTIND in 1 2; do …; done`.
    /// This matcher approved both, because `for` and `in` are control words that
    /// are stripped before the check that catches `OPTIND=1/0` ever runs.
    #[test]
    fn a_loop_over_an_evaluated_variable_is_not_approved_by_a_prefix_rule() {
        for looped in [
            "for OPTIND in 1 2; do ls; done",
            "for RANDOM in 1; do ls; done",
            "for SECONDS in 1; do ls; done",
        ] {
            assert!(
                unapprovable_by_prefix(looped).is_some(),
                "{looped} assigns a variable the shell evaluates"
            );
        }
        // And an ordinary loop is still an ordinary loop. Refusing every `for`
        // would be the other failure: the running product runs this one, and a
        // matcher stricter than it here costs a prompt on a shape agents write
        // constantly.
        for ordinary in [
            "for i in 1 2; do ls; done",
            "for FOO in 1; do ls; done",
            "for f in *.txt; do cat \"$f\"; done",
        ] {
            assert!(
                unapprovable_by_prefix(ordinary).is_none(),
                "{ordinary} is an ordinary loop"
            );
        }
        // The name alone is not the trigger — a command that merely *is* one of
        // these words is not a loop header, and `in` is what tells them apart.
        assert!(unapprovable_by_prefix("OPTIND=1 ls").is_none());
    }

    #[test]
    fn an_assignment_cannot_smuggle_a_command_past_the_read_only_shortcut() {
        // *"Fixed Bash permission checks auto-approving commands that assign an
        // arithmetic expression to an integer shell variable (e.g.
        // `OPTIND=1/0`)"* (2.1.251) and *"…zsh commands that hide a command
        // substitution in a REPORTTIME, REPORTMEMORY or DIRSTACKSIZE
        // assignment"* (2.1.260).
        //
        // Both arrived here through the same door: `never_asks_about` answers
        // *does this need a rule at all?* on the **allow** side, and it used the
        // permissive strip — so the assignment was looked past and what was left
        // read as read-only.
        for smuggled in [
            "OPTIND=1/0 ls",
            "DIRSTACKSIZE=$(id) ls",
            "REPORTTIME=$(curl evil) ls",
            "REPORTMEMORY=`id` ls",
            // A name on the safe list is not a pass either: the value is what
            // runs something.
            "NODE_ENV=$(curl evil) ls",
            "CI=${IFS} ls",
        ] {
            assert!(
                !never_asks_about(smuggled),
                "{smuggled} must not read as read-only"
            );
        }
        // And the ordinary forms still need no rule of their own. Refusing
        // *every* non-safe assignment would be the other failure: `SECRET=x ls`
        // is still `ls`, and this question is not about which rule covers the
        // call. What makes an assignment dangerous is that the shell **runs**
        // something for it — a substitution, or an expression assigned to a
        // variable it evaluates.
        for plain in [
            "ls -la",
            "NODE_ENV=test ls",
            "CI=1 ls",
            "SECRET=x ls",
            "FOO=bar ls",
            "OPTIND=1 ls",
            "cd src && ls",
        ] {
            assert!(never_asks_about(plain), "{plain} is read-only");
        }
    }

    #[test]
    fn the_reader_commands_the_changelog_named_and_the_ones_it_meant() {
        // *"Fixed Bash `Read()`/`Edit()` deny rules not applying to `< file`
        // redirects and reader commands like `tac` and `egrep`"* (2.1.257).
        // Two names and the word "like", so the list is open again — these are
        // the ones an agent would reach for to read a file it may not read,
        // and the harness is what keeps them honest.
        for cmd in [
            "tac .env",
            "egrep x .env",
            "fgrep x .env",
            "nl .env",
            "rev .env",
            "base64 .env",
            "od -c .env",
            "hexdump .env",
            "strings .env",
            "sha256sum .env",
            "wc -l .env",
            "cut -d: -f1 .env",
            "sort .env",
            "uniq .env",
            "bat .env",
            "awk {print} .env",
            "jq . .env",
            "diff a.txt .env",
            "cmp a.txt .env",
            "fold -w 80 .env",
            "split -l 1 .env",
        ] {
            assert!(reads(cmd, ".env"), "{cmd} reads .env");
        }
        // `xxd` and `zcat` are measured **out**: the running product ran both
        // under a `Read(.env)` deny, twice, under two spellings of the same
        // rule, while refusing the eight commands beside them. An entry the
        // product does not recognise refuses a call the user's own settings
        // allow — so a measurement takes an entry out, where no amount of
        // reasoning about hex dumps or gzip would have.
        for absent in [
            "xxd .env",
            "bzcat .env",
            "join .env .env",
            "less .env",
            "more .env",
            "truncate -s 0 .env",
        ] {
            assert!(!reads(absent, ".env"), "{absent} was measured out");
        }
        // `mv` is not `cp`: it removes its source, so an `Edit` deny reaches
        // it. Measured — the product refuses `mv .env .env.bak` under
        // `Edit(.env)`.
        assert!(
            file_targets("mv .env .env.bak")
                .iter()
                .any(|t| t.path == ".env" && t.access == Access::Write),
            "mv removes its source"
        );
        assert!(
            file_targets("cp .env .env.bak")
                .iter()
                .all(|t| t.access == Access::Read),
            "cp leaves its source alone"
        );

        // And the operand that is a script or a pattern is still not a file,
        // for every command that spends one.
        for cmd in [
            "grep .env README.md",
            "awk .env README.md",
            "jq .env README.md",
        ] {
            assert!(!reads(cmd, ".env"), "{cmd} spends .env on the script");
        }
        // A flag value that is not a path costs a target nothing matches, and
        // must not be mistaken for the filename.
        assert!(reads("sort -k2 .env", ".env"));
        assert!(reads("cut -d: -f1 .env", ".env"));
        assert!(reads("head -n 5 .env", ".env"));
    }

    #[test]
    fn a_pattern_supplied_by_a_flag_does_not_eat_the_filename() {
        // `sed` and `grep` spend their first operand on the script or the
        // pattern, so it is skipped — and when a flag already supplied it, the
        // thing in that position is a file, and skipping it loses the path.
        assert!(reads("grep -f pats.txt .env", ".env"));
        assert!(reads("sed -e s/a/b/ .env", ".env"));
        assert!(reads("grep --file=pats.txt .env", ".env"));
        // The ordinary forms are unchanged: the pattern is still not a file.
        assert!(reads("grep TOKEN .env", ".env"));
        assert!(!reads("grep .env README.md", ".env"));
        assert!(reads("sed -n 1p .env", ".env"));
    }

    #[test]
    fn a_recursive_command_carries_its_subtree() {
        // "Fixed `grep -r`/`cp -r` over directories with denied files."
        assert_eq!(
            paths("grep -r pattern secrets"),
            [("secrets".into(), Access::Read, true)]
        );
        // `cp` reads its operands. The destination *looks* like a write
        // `Edit` rules should reach and measurably is not: under an allow rule
        // naming the command, the running product copies outside the working
        // directory with no prompt. Modelling it as a write refused a call the
        // product runs, which the harness's deny axis reported.
        assert_eq!(
            paths("cp -r secrets /tmp/x"),
            [
                ("secrets".into(), Access::Read, true),
                ("/tmp/x".into(), Access::Read, true)
            ]
        );
        // Without the flag there is no subtree to reach.
        assert_eq!(
            paths("grep pattern secrets/key"),
            [("secrets/key".into(), Access::Read, false)]
        );
    }

    #[test]
    fn a_barrier_command_is_looked_through() {
        // A deliberate narrowing, not a vendor behaviour being tracked: the
        // product hands a barrier's argument through opaquely, and 2.1.268's
        // claim to the contrary was reverted in 2.1.273. These stay because a
        // deny a quote can step around is not a deny; the cost is a prompt.
        assert!(reads("env -C . cat .env", ".env"));
        assert!(reads("env FOO=bar cat .env", ".env"));
        assert!(reads("env -u PATH -C /tmp cat .env", ".env"));
        assert!(reads("eval \"cat .env\"", ".env"));
        assert!(reads("sudo -u root cat .env", ".env"));
        // A barrier with nothing behind it names nothing.
        assert!(paths("env").is_empty());
        assert!(paths("env -i").is_empty());
    }

    /// Every case here is an example from Claude Code's own permission
    /// documentation, quoted in the comment that introduces it.
    #[test]
    fn a_compound_command_splits_the_way_the_documentation_says() {
        // "The recognized command separators are `&&`, `||`, `;`, `|`, `|&`,
        // `&`, and newlines."
        assert_eq!(
            subcommands("a && b || c ; d | e |& f & g\nh").unwrap(),
            ["a", "b", "c", "d", "e", "f", "g", "h"]
        );
        // "When `&&` or `||` has nothing after it … Claude Code treats the
        // command as unparseable and doesn't split it."
        assert_eq!(subcommands("npm test &&"), None);
        assert_eq!(subcommands("npm test ||"), None);
        // A separator inside quotes is text, not an operator.
        assert_eq!(subcommands("echo 'a && b'").unwrap(), ["echo 'a && b'"]);
        assert_eq!(subcommands("echo \"a; b\"").unwrap(), ["echo \"a; b\""]);
    }

    #[test]
    fn a_deny_rule_reaches_into_substitutions_and_bodies() {
        // "including a command nested inside a subshell, a command
        // substitution, or a control-flow body such as a `for` loop"
        let has =
            |text: &str, needle: &str| nested_commands(text).iter().any(|c| c.contains(needle));
        assert!(has("cd /tmp && git clean -f", "git clean -f"));
        assert!(has("echo \"$(git clean -f)\"", "git clean -f"));
        assert!(has("for i in 1 2; do npm test; done", "npm test"));
        assert!(has("if x; then rm -rf /; fi", "rm -rf /"));
        assert!(has("( rm -rf / )", "rm -rf /"));
        assert!(has("`rm -rf /`", "rm -rf /"));
        assert!(has("{ rm -rf /; }", "rm -rf /"));
        // Single quotes are literal, so this is not a nested command.
        assert!(
            !nested_commands("echo '$(rm -rf /)'")
                .iter()
                .any(|c| c == "rm -rf /")
        );
    }

    #[test]
    fn wrappers_and_assignments_are_stripped_the_way_they_are_documented() {
        // "a rule like `Bash(npm test *)` also matches `timeout 30 npm test`"
        assert_eq!(strip("timeout 30 npm test", false), "npm test");
        assert_eq!(strip("nohup npm test", false), "npm test");
        assert_eq!(strip("nice -n 10 npm test", false), "npm test");
        // "the query form `command -v`, which looks up a command rather than
        // running one" is not stripped.
        assert_eq!(strip("command -v npm", false), "command -v npm");
        assert_eq!(strip("command npm test", false), "npm test");
        // "Bare `xargs` is also stripped … an invocation like `xargs -n1 grep
        // pattern` is matched as an `xargs` command."
        assert_eq!(strip("xargs grep pattern", false), "grep pattern");
        assert_eq!(
            strip("xargs -n1 grep pattern", false),
            "xargs -n1 grep pattern"
        );
        // "`Bash(npm test *)` matches `NODE_ENV=test npm test`. An allow rule
        // won't match past an assignment of any other variable. A deny or ask
        // rule matches past any leading assignment."
        assert_eq!(strip("NODE_ENV=test npm test", false), "npm test");
        assert_eq!(strip("FOO=bar rm -rf tmp/", false), "FOO=bar rm -rf tmp/");
        assert_eq!(strip("FOO=bar rm -rf tmp/", true), "rm -rf tmp/");
    }

    #[test]
    fn a_command_an_agent_wrote_cannot_make_this_expensive() {
        // The subject is untrusted text on a hook a session is blocked on.
        let deep = "$(".repeat(200) + "rm" + &")".repeat(200);
        let started = std::time::Instant::now();
        let _ = nested_commands(&deep);
        let _ = subcommands(&"a && ".repeat(2000));
        assert!(
            started.elapsed() < std::time::Duration::from_millis(100),
            "took {:?}",
            started.elapsed()
        );
    }
}

// ---------------------------------------------------------------------------
// The files a command touches
// ---------------------------------------------------------------------------

/// Whether a file target is read or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// How the command names the file.
///
/// It decides **one** thing and deliberately not the other, and the two were
/// confused in both directions before they were measured.
///
/// It does **not** decide whether the allow side speaks: that is
/// [`FileTarget::allow_side_applies`], keyed on the *access*, because the
/// asymmetry Claude Code documents is about whether anybody was going to be
/// asked. A recognised file command like `cat` is in the built-in read-only
/// set, so there is no prompt for an allow rule to skip — while `tee` is a
/// recognised file command that *writes*, and its destination is checked
/// against `Edit` rules exactly as a redirect's is (2.1.269). Keying that on
/// `Via` meant `Edit(.env)` stopped `echo x > .env` and not
/// `echo x | tee .env`.
///
/// It **does** decide whether `Read` rules reach a *write*, through
/// [`Via::read_rules_apply`]. Under `Read(.env)` the running product refuses
/// `echo x | tee .env` and runs `echo x > .env` and `touch .env`, so the reach
/// stops at the commands it recognises by name. Keying that on the access
/// instead made this matcher refuse calls the product runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// `> file`, `>> file`, `2> file`, `< file`.
    Redirect,
    /// An operand of a file command Claude Code recognises.
    FileCommand,
    /// The value of an option rather than a positional operand:
    /// `--ignore-revs-file=.env`, `-f.env`, `@file`. Claude Code applies
    /// `Read` and `Edit` deny rules to these (2.1.266); they are never a
    /// prompt anybody was going to see, so the allow side stays out.
    OptionValue,
    /// A file the command writes without being one of the commands Claude Code
    /// recognises by name — `touch f`, the destination of `cp`. Checked
    /// against `Edit` rules exactly like a redirection target, and **not**
    /// against `Read` rules.
    WritePath,
}

impl Via {
    /// Whether `Read` rules reach a target named this way.
    ///
    /// Only the commands Claude Code recognises by name. The reference says
    /// `Read` and `Edit` deny rules apply *"to the targets of Bash
    /// redirections such as `> file`"* as well — and the running product
    /// disagrees: under `Read(.env)` it refuses `echo x | tee .env` and runs
    /// `echo x > .env` and `touch .env`. `tee` is on its list and a
    /// redirection is not, so the reference is wrong about the redirect and
    /// the measurement decides.
    pub fn read_rules_apply(self) -> bool {
        matches!(self, Via::FileCommand | Via::OptionValue)
    }
}

/// One file a shell command names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTarget {
    pub path: String,
    pub access: Access,
    pub via: Via,
    /// The path cannot be resolved to one file here: it starts with `~`, holds
    /// a glob character, or is a variable. Claude Code asks about these
    /// whatever the rules say, so an allow rule must never cover one.
    pub unresolvable: bool,
    /// The command reaches everything *underneath* this path, not only the
    /// path itself — `grep -r`, `cp -r`, `rm -r`. A deny rule naming a file
    /// inside the directory stops the call (2.1.268), which is a question
    /// about the rule's shape rather than about this string, so it is answered
    /// where the pattern is, in `PathPattern::covers_under`.
    pub subtree: bool,
}

impl FileTarget {
    /// Whether an **allow** rule, and the working-directory check behind it,
    /// may speak for this target.
    ///
    /// Everything a command writes, and everything a redirect names. What is
    /// excluded is the case where no prompt was ever coming: an operand of a
    /// recognised file command that only *reads*, inside the working directory.
    ///
    /// **`cwd` is why this takes an argument, and it was a widening.** The
    /// exemption used to be unconditional — a read by a read-only command was
    /// assumed never to prompt — and the differential harness measured
    /// otherwise against Claude Code 2.1.273: under `auto_allow =
    /// ["Edit(ran.txt)"]` the running product blocks `cat /etc/passwd >
    /// ran.txt` and runs `cat README.md > ran.txt`. A read is free *inside* the
    /// working directory and is a prompt outside it, so an unconditional
    /// exemption turned any path grant into permission to pipe any file on the
    /// machine into it.
    ///
    /// A path that cannot be resolved to one file stays exempt on this side.
    /// That is measured too, from the other direction: the running product
    /// honours an exact whole-line rule over a command substitution, so
    /// treating `cat "$(echo a.txt)"` as an uncovered read would refuse a call
    /// it runs. What `within` must not do is mistake a `~` for a path under the
    /// working directory, which it does not — see [`crate::core::policy::within`].
    pub fn allow_side_applies(&self, cwd: &std::path::Path) -> bool {
        if self.access == Access::Write || self.via == Via::Redirect {
            return true;
        }
        !crate::core::policy::within(cwd, std::path::Path::new(&self.path))
    }
}

/// Commands whose file operands Claude Code applies `Read` and `Edit` rules to,
/// with the access each performs.
///
/// **The list is open.** The reference introduces it with *"such as"*, so a
/// release can add a row without anything here failing. These entries are a
/// floor; `scripts/verify-permissions-diff.sh` is what keeps them honest, by
/// asking a running Claude Code about commands nobody wrote down.
const FILE_COMMANDS: &[(&str, Access)] = &[
    // ── named by the vendor ──────────────────────────────────────────────
    // The reference: *"file commands Claude Code recognizes in Bash, such as
    // `cat`, `head`, `tail`, and `sed`"*. The changelog: *"reader commands
    // like `tac` and `egrep`"* (2.1.257). Six names and two open lists.
    ("cat", Access::Read),
    ("head", Access::Read),
    ("tail", Access::Read),
    ("sed", Access::Read),
    ("tac", Access::Read),
    ("egrep", Access::Read),
    // ── measured against a running Claude Code ───────────────────────────
    // Each refused under `never_auto = ["Read(.env)"]` on both spellings of
    // the rule, by `scripts/verify-permissions-diff.sh`.
    ("grep", Access::Read),
    ("nl", Access::Read),
    ("sort", Access::Read),
    ("cut", Access::Read),
    ("awk", Access::Read),
    ("sha256sum", Access::Read),
    ("base64", Access::Read),
    ("od", Access::Read),
    ("hexdump", Access::Read),
    ("strings", Access::Read),
    ("rev", Access::Read),
    ("jq", Access::Read),
    ("comm", Access::Read),
    ("paste", Access::Read),
    ("fold", Access::Read),
    // Named by the vendor's own 2.1.271 row — *"Fixed Bash permission checks
    // missing the file that `fmt`, `column` and similar commands read"* — which
    // is a measurement somebody else paid for. `column` was already here and
    // `fmt` was not, so `fmt .env` walked past `Read(.env)`.
    ("fmt", Access::Read),
    // ── inferred from a measured sibling ─────────────────────────────────
    // The same program under another name, or the same tool at another
    // digest width. Weaker than a measurement and stronger than a guess.
    ("fgrep", Access::Read),
    ("rgrep", Access::Read),
    ("zgrep", Access::Read),
    ("gawk", Access::Read),
    ("mawk", Access::Read),
    ("yq", Access::Read),
    ("md5sum", Access::Read),
    ("sha1sum", Access::Read),
    ("sha512sum", Access::Read),
    ("shasum", Access::Read),
    ("cksum", Access::Read),
    ("b2sum", Access::Read),
    ("base32", Access::Read),
    // ── unmeasured, and kept because the errors are not symmetric ────────
    // A WIDER row fails the release and a narrower one is reported, so an
    // unmeasured candidate stays until a measurement takes it out — and goes
    // into `DENY_SHAPES` so that one eventually does.
    ("uniq", Access::Read),
    ("expand", Access::Read),
    ("unexpand", Access::Read),
    ("column", Access::Read),
    ("csplit", Access::Read),
    ("split", Access::Read),
    // The same family as `fmt` and `fold`: a text reformatter that opens its
    // operands. Unmeasured, so it is here and in `DENY_SHAPES`.
    ("pr", Access::Read),
    ("most", Access::Read),
    ("bat", Access::Read),
    ("xmllint", Access::Read),
    ("uuencode", Access::Read),
    ("diff", Access::Read),
    ("diff3", Access::Read),
    ("cmp", Access::Read),
    ("wc", Access::Read),
    // Not here, and measured rather than overlooked: `xxd`, `zcat`, `join`,
    // `less`, `more` and `truncate` all run under a deny that stops the
    // commands beside them.
    //
    // `cp` and `mv` read their operands, and with `-r`/`-R` everything beneath
    // them (2.1.268). `cp`'s *destination* is not a target: the running product
    // copies outside the working directory under an allow rule naming the
    // command, with no prompt.
    ("cp", Access::Read),
    // `mv` **removes** its source, so every operand is a write. A `Read` deny
    // still reaches them, since it reaches a write by a command on the list.
    ("mv", Access::Write),
    ("rsync", Access::Read),
    ("install", Access::Read),
    // ── writers ──────────────────────────────────────────────────────────
    // Writes every operand. `-a` and `-i` take no value, so no flag here
    // consumes the word after it.
    ("tee", Access::Write),
    // Creates its operands. An `Edit` deny on the path blocks it; an `Edit`
    // allow alone does not run it, so the `Bash` rule is still required.
    // Neither half is in the reference — both are measured.
    ("touch", Access::Write),
];

/// Writers the vendor's own file-command table does not carry, and the operand
/// each of them writes.
///
/// **Restrictive side only**, for the reason the transparent wrappers are:
/// Claude Code does not apply `Read`/`Edit` rules to these, so mirroring it
/// meant `never_auto = ["Edit(secrets/**)"]` stopped `tee secrets/k` and
/// `echo x > secrets/k` and let `cp /tmp/a secrets/k`, `truncate -s 0 secrets/k`
/// and `dd of=secrets/k` through. That asymmetry was correct while a broader
/// reading could have produced a *grant*; it cannot any more, so the only thing
/// it costs is a prompt.
///
/// It is the fifth of the five published shell-guard bypass classes — the one
/// that reaches a protected file through a command a keyword filter did not
/// think of — and the four before it are [`undecidable`]'s subject.
///
/// `chmod`, `chown` and `chgrp` are deliberately absent: they change a file's
/// mode, not its contents, and an `Edit` rule is about what a file says.
const EXTRA_WRITERS: &[(&str, ExtraWrite)] = &[
    ("truncate", ExtraWrite::EveryOperand),
    ("cp", ExtraWrite::LastOperand),
    ("install", ExtraWrite::LastOperand),
    ("rsync", ExtraWrite::LastOperand),
    ("ln", ExtraWrite::LastOperand),
    ("dd", ExtraWrite::OfAssignment),
];

/// Which operand of an [`EXTRA_WRITERS`] command is the one being written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraWrite {
    /// `truncate -s 0 a b` writes both.
    EveryOperand,
    /// `cp a b c dir/` writes only the last — the destination.
    LastOperand,
    /// `dd if=x of=y` writes the value of `of=`.
    OfAssignment,
}

/// Flags of an [`EXTRA_WRITERS`] command that take a value in the next word, so
/// the value is not mistaken for the destination.
fn extra_writer_value_flags(program: &str) -> &'static [&'static str] {
    match program {
        "truncate" => &["-s", "-r", "--size", "--reference"],
        "install" => &[
            "-m", "-o", "-g", "-S", "--mode", "--owner", "--group", "--suffix",
        ],
        "cp" => &["-S", "--suffix", "-t", "--target-directory"],
        "rsync" => &["-e", "--rsh", "--exclude", "--include", "--files-from"],
        _ => &[],
    }
}

/// Paths a restrictive rule should treat as written, beyond the ones
/// [`file_targets`] finds.
///
/// Every command in the line is looked at, so `ls && cp /tmp/a secrets/k` is
/// covered, and the search reaches into substitutions and control-flow bodies
/// exactly as the deny side does everywhere else.
pub fn extra_write_targets(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in nested_commands(text) {
        let stripped = strip_transparent(&part);
        let without = without_redirections(&stripped);
        let mut words = without.split_whitespace();
        let Some(program) = words.next() else {
            continue;
        };
        let base = program.rsplit('/').next().unwrap_or(program);
        let Some((_, kind)) = EXTRA_WRITERS.iter().find(|(p, _)| *p == base) else {
            continue;
        };
        if *kind == ExtraWrite::OfAssignment {
            out.extend(
                words
                    .filter_map(|w| w.strip_prefix("of="))
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
            );
            continue;
        }
        let value_flags = extra_writer_value_flags(base);
        let mut operands: Vec<&str> = Vec::new();
        let mut skip_next = false;
        for w in words {
            if skip_next {
                skip_next = false;
                continue;
            }
            if w.starts_with('-') {
                skip_next = value_flags.contains(&w);
                continue;
            }
            operands.push(w);
        }
        match kind {
            ExtraWrite::EveryOperand => out.extend(operands.iter().map(|o| (*o).to_string())),
            // One operand is a source with no destination — `cp a` — and names
            // nothing this rule speaks for.
            ExtraWrite::LastOperand if operands.len() >= 2 => {
                out.push(operands[operands.len() - 1].to_string());
            }
            _ => {}
        }
    }
    out
}

/// Commands whose first positional operand is a script, a pattern or an
/// expression rather than a file.
///
/// `sed 's/a/b/' f`, `grep TOKEN f`, `awk '{print}' f`, `jq . f`. Skipping it
/// is right until a flag supplies the thing instead, which is what
/// `pattern_supplied_by_flag` is for.
const SCRIPT_FIRST: &[&str] = &[
    "sed", "grep", "egrep", "fgrep", "rgrep", "zgrep", "awk", "gawk", "mawk", "jq", "yq",
];

/// `git` subcommands whose operands name files, with the operand index at
/// which paths begin. `git grep PATTERN -- path` spends its first operand on
/// the pattern; the rest name paths directly. Deny rules reach all of them
/// (2.1.268); no allow rule is affected, because these read.
const GIT_FILE_SUBCOMMANDS: &[(&str, usize)] = &[
    ("diff", 0),
    ("show", 0),
    ("cat-file", 0),
    ("blame", 0),
    ("log", 0),
    ("add", 0),
    ("checkout", 0),
    ("restore", 0),
    ("grep", 1),
];

/// Flags that make a command reach the whole subtree under its operands.
const RECURSIVE_FLAGS: &[&str] = &["-r", "-R", "--recursive", "-rn", "-nr", "-ri", "-ir"];

/// Programs that run a command assembled from their own arguments. Claude Code
/// applies deny rules to what they actually run (2.1.268), so the file
/// extraction looks through them.
///
/// They are deliberately **not** in `WRAPPERS`: the reference's wrapper list is
/// fixed and does not contain them, so stripping them for an *allow* rule
/// would make `Bash(env *)` narrower here than there. Looking through them
/// only ever adds targets, which only ever reaches deny rules and writes.
///
/// **Every one of them is a measured divergence, and looking through them is a
/// choice rather than a reading.** The running product treats what a barrier is
/// handed as opaque: it runs `eval "cat .env"` and `env -C . cat .env` under a
/// `Read(.env)` deny, and this does not. A prohibition that any agent can step
/// around by quoting is not a prohibition, and the cost is a prompt rather than
/// a refusal. All five shapes are declared in
/// `scripts/verify-permissions-diff.sh` so the harness reports them as choices
/// rather than as findings.
///
/// Claude Code 2.1.268 announced that deny rules would reach these lines and
/// 2.1.273 reverted it, so the product's documented behaviour now matches what
/// the deny axis measured against 2.1.270 all along. The list is unchanged by
/// either release: it was never derived from the changelog.
const ANALYSIS_BARRIERS: &[&str] = &["env", "eval", "exec", "sudo", "doas"];

/// Flags that consume the word after them, **per command**, so that the `5` in
/// `head -n 5 f` is not mistaken for a filename.
///
/// One shared list was wrong in the direction this layer is always wrong in.
/// `-n` takes a value for `head` and takes none for `sed` and `grep`, so
/// `sed -n 1p .env` spent `1p` on the flag, then spent `.env` on the script,
/// and named no file at all — `Read(.env)` read as protection and was none.
/// Found by the harness's deny axis.
fn value_flags(program: &str) -> &'static [&'static str] {
    match program {
        "head" | "tail" => &["-n", "-c", "--lines", "--bytes"],
        "sed" => &["-e", "-f", "--expression", "--file"],
        "grep" | "egrep" | "fgrep" | "rgrep" | "zgrep" => &[
            "-e",
            "-f",
            "-m",
            "-A",
            "-B",
            "-C",
            "--regexp",
            "--file",
            "--max-count",
        ],
        "awk" | "gawk" | "mawk" => &["-f", "-v", "--file", "--assign"],
        "jq" | "yq" => &["-f", "--from-file", "--arg", "--argjson"],
        "cut" => &["-d", "-f", "-b", "-c", "--delimiter", "--fields"],
        "sort" => &["-k", "-t", "-o", "-S", "--key", "--output"],
        "od" | "hexdump" => &["-N", "-j", "-t", "-A", "-e", "-s", "-n"],
        "xxd" => &["-l", "-s", "-c", "-g"],
        "split" | "csplit" => &["-b", "-l", "-n", "-a", "--bytes", "--lines"],
        "fold" | "column" => &["-w", "-c", "-s", "-t"],
        "cp" | "mv" | "install" | "rsync" => &["-t", "--target-directory", "--suffix"],
        "truncate" => &["-s", "-r", "--size", "--reference"],
        _ => &[],
    }
}

/// Targets Claude Code does not check because no file is behind them.
fn no_file_behind(target: &str) -> bool {
    target == "/dev/null" || target.starts_with('&')
}

/// Whether a path cannot be pinned to one file from the text alone.
fn unresolvable(path: &str) -> bool {
    path.starts_with('~')
        || path.starts_with('$')
        || path.contains('*')
        || path.contains('?')
        || path.contains('[')
        || path.contains('$')
}

/// Every file the command names, through a redirection or as an argument of a
/// file command Claude Code recognises.
///
/// This is what makes `Read(.env)` reach `cat .env` and `Edit(.env)` reach
/// `echo x > .env`. Without it a `Bash` rule is matched against the command
/// *text* only, and both of those prohibitions read as protection and are
/// none — the failure this whole layer exists to prevent.
///
/// It reaches into subshells and substitutions for the same reason a deny rule
/// does: `(echo x > .env)` is the same write.
///
/// The honest limit, which is Claude Code's limit too: a file a program opens
/// itself is not named here, so a script that writes `.env` is not covered by
/// anything short of the sandbox.
pub fn file_targets(text: &str) -> Targets {
    memo(&TARGETS, text, file_targets_uncached)
}

// How many times a command has actually been parsed, as opposed to the memo
// answering.
//
// A counter rather than a stopwatch, for the reason `policy_cache` gives: the
// property is *one parse per command per evaluation, whatever the rule count*,
// and a wall-clock assertion for it would be flaky on a loaded machine and
// fail for reasons that have nothing to do with parsing.
// Thread-local, because the memo is: a global counter would be inflated by
// every other test parsing on another thread at the same time.
#[cfg(test)]
thread_local! {
    pub static PARSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn file_targets_uncached(text: &str) -> Targets {
    #[cfg(test)]
    PARSES.with(|c| c.set(c.get() + 1));

    // Claude Code states that a command past this length *"always prompts
    // because it exceeds what the analysis parses"*. Nothing here reads it
    // either, so the honest answer is the empty set marked unread: an allow
    // rule may not speak for it, and a restrictive one may not conclude it
    // names nothing. Answering before parsing also keeps a 10 KB line from
    // costing 60 ms on the hook.
    if text.len() > MAX_ANALYSED {
        return Targets {
            list: Vec::new(),
            truncated: true,
        };
    }
    let mut out = Targets::default();
    let (commands, split_truncated) = nested_commands_bounded(text);
    out.truncated |= split_truncated;
    for part in commands {
        redirect_targets(&part, &mut out);
        file_command_targets(&part, &mut out);
    }
    out.list.dedup();
    out
}

// ---------------------------------------------------------------------------
// One parse per command, not one per rule
// ---------------------------------------------------------------------------

/// Every rule in a policy asks about the **same** command line, and each of the
/// questions below used to re-parse it: `file_targets` for path rules,
/// `nested_commands` for text rules, once per rule. On a forty-rule policy that
/// is forty parses of one string, and a line an agent wrote can be long — a
/// thousand chained commands measured at **97 ms** per evaluation, on the
/// synchronous hook a session is blocked on.
///
/// That is not only slow, it is the shape [the guardrail-timeout
/// literature](https://arxiv.org/abs/2606.14517) warns about: a `command` hook
/// that reaches its timeout *renders no decision*, which is fail-open by
/// another name, and the input that gets it there is chosen by the thing being
/// governed.
///
/// These are **pure functions of their argument**, so a memo keyed on the text
/// is sound with no invalidation at all — there is nothing to go stale. Four
/// entries, because one evaluation asks about one command and a pipeline step
/// may interleave a second.
fn memo<T: Clone + 'static>(
    cell: &'static std::thread::LocalKey<Memo<T>>,
    key: &str,
    compute: fn(&str) -> T,
) -> T {
    if let Some(hit) = cell.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }) {
        return hit;
    }
    let value = compute(key);
    cell.with(|c| {
        let mut b = c.borrow_mut();
        if b.len() >= 4 {
            b.remove(0);
        }
        b.push((key.to_string(), value.clone()));
    });
    value
}

/// One command's split, and whether the split ran out of room.
type Split = (Vec<String>, bool);
/// A few recent answers, keyed by the text they were derived from.
type Memo<T> = std::cell::RefCell<Vec<(String, T)>>;

thread_local! {
    static TARGETS: Memo<Targets> = const { std::cell::RefCell::new(Vec::new()) };
    static SPLITS: Memo<Split> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Every file a command names, and whether the parse ran out of room.
///
/// The second field is the whole point. Each cap below exists because a command
/// is untrusted input on a synchronous hook, and for a long time each of them
/// simply stopped collecting — which drops a target, and a dropped target is a
/// `never_auto` that does not fire. `cat f0 … f79 .env` reached `Undecided`
/// under `Read(.env)`, and so did a substitution nested past the depth cap.
///
/// A cap is now a *fact the caller is told*, so a restrictive rule can treat the
/// unread remainder as "might be anything" instead of as "nothing". That is the
/// direction this module is allowed to be wrong in; silence was the other one.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Targets {
    pub list: Vec<FileTarget>,
    /// An operand, nesting or command cap was reached, so there is command
    /// left that nothing here has looked at.
    pub truncated: bool,
}

impl std::ops::Deref for Targets {
    type Target = [FileTarget];
    fn deref(&self) -> &[FileTarget] {
        &self.list
    }
}

impl IntoIterator for Targets {
    type Item = FileTarget;
    type IntoIter = std::vec::IntoIter<FileTarget>;
    fn into_iter(self) -> Self::IntoIter {
        self.list.into_iter()
    }
}

impl Targets {
    /// Room for one more target, and a truncation flag when there is not.
    ///
    /// The bound is high enough that no command a person writes reaches it and
    /// low enough that a generated one cannot make the hook slow: the cost of a
    /// target is a string and a few segment comparisons per rule.
    fn room(&mut self) -> bool {
        if self.list.len() >= MAX_TARGETS {
            self.truncated = true;
            return false;
        }
        true
    }
}

/// The most files one command may name before the parse gives up and says so.
pub const MAX_TARGETS: usize = 512;
/// The deepest substitution or subshell the parse descends into.
const MAX_DEPTH: usize = 16;
/// The most separate commands one line may split into.
const MAX_COMMANDS: usize = 1024;

/// The targets of `>`, `>>`, `2>`, `&>` and `<`, ignoring the forms with no
/// file behind them: `/dev/null`, `2>&1`, `<&3`, here-docs and here-strings.
fn redirect_targets(text: &str, out: &mut Targets) {
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    i += 1;
                    continue;
                }
                if c == '\\' {
                    i += 2;
                    continue;
                }
            }
        }
        if c != '>' && c != '<' {
            i += 1;
            continue;
        }
        // A here-doc or here-string carries its text inline, not a filename.
        if c == '<' && b.get(i + 1) == Some(&'<') {
            i += 2;
            while b.get(i) == Some(&'<') {
                i += 1;
            }
            // Skip the delimiter word.
            while i < b.len() && b[i].is_whitespace() {
                i += 1;
            }
            while i < b.len() && !b[i].is_whitespace() {
                i += 1;
            }
            continue;
        }
        let access = if c == '>' {
            Access::Write
        } else {
            Access::Read
        };
        let mut j = i + 1;
        // `>>` appends; `>|` forces. Both still name a file.
        while b.get(j) == Some(&'>') || b.get(j) == Some(&'|') {
            j += 1;
        }
        // `>&1` / `<&3` duplicate a descriptor.
        if b.get(j) == Some(&'&') {
            i = j + 1;
            continue;
        }
        while j < b.len() && b[j].is_whitespace() {
            j += 1;
        }
        let (word, next) = word_at(&b, j);
        i = next.max(i + 1);
        if word.is_empty() || no_file_behind(&word) {
            continue;
        }
        if !out.room() {
            return;
        }
        out.list.push(FileTarget {
            unresolvable: unresolvable(&word),
            path: word,
            access,
            via: Via::Redirect,
            subtree: false,
        });
    }
}

/// Every file an operand or an option value of a recognised command names.
///
/// Five families, each pinned to the release that put it in Claude Code:
/// the table in `FILE_COMMANDS`; `cp`-shaped commands whose last operand is
/// written; `git` subcommands whose operands are paths (2.1.268); option
/// *values* such as `--ignore-revs-file=.env`, `-f.env` and `@file` (2.1.266);
/// and the subtree a `-r` reaches (2.1.268).
///
/// The extraction looks through `env`, `eval` and `sudo`, which assemble a
/// command from their own arguments and which Claude Code's deny rules now see
/// past. It does **not** strip them for allow matching — the reference's
/// wrapper list is fixed and does not contain them.
fn file_command_targets(text: &str, out: &mut Targets) {
    file_command_targets_at(text, out, 0);
}

fn file_command_targets_at(text: &str, out: &mut Targets, depth: usize) {
    if depth > MAX_DEPTH {
        // There is command in here that nothing has read. Saying so is what
        // keeps a deny rule from concluding this line names no protected file.
        out.truncated = true;
        return;
    }
    if !out.room() {
        return;
    }
    // Redirections first: their targets are not operands. `touch f > /dev/null`
    // names one file, and `cat secrets > out` reads one and writes the other.
    let stripped = strip(&without_redirections(text), true);
    let words = words_of(&stripped);
    let Some(first) = words.first() else { return };
    let program = first.rsplit('/').next().unwrap_or(first);

    // `env -C dir cat .env`, `eval "cat .env"`, `sudo cat .env`. Drop the
    // barrier and its own options, then analyse what is left.
    if ANALYSIS_BARRIERS.contains(&program) {
        if let Some(inner) = past_barrier(program, &words) {
            file_command_targets_at(&inner, out, depth + 1);
        }
        return;
    }

    if program == "git" {
        git_targets(&words, out);
        return;
    }

    let recursive = words.iter().any(|w| RECURSIVE_FLAGS.contains(&w.as_str()));

    let Some((_, base_access)) = FILE_COMMANDS.iter().find(|(n, _)| *n == program) else {
        return;
    };
    // `sed -i` rewrites the files it is given; `sed 's/a/b/' f` reads them, and
    // its first non-flag argument is the script rather than a file. `grep`
    // spends its first operand on the pattern the same way.
    // `sed -i` rewrites the files it is given; so does `-i.bak`.
    let in_place = program == "sed" && words.iter().any(|w| w.starts_with("-i"));
    let access = if in_place {
        Access::Write
    } else {
        *base_access
    };
    // `sed` spends its first operand on the script and `grep` on the pattern —
    // **unless a flag already supplied it**. `grep -f pats.txt .env` and
    // `sed -e s/a/b/ .env` name a file in the position the skip would eat, so
    // skipping it there loses the path entirely.
    let supplied = words.iter().skip(1).any(|w| {
        matches!(w.as_str(), "-e" | "-f" | "--regexp" | "--file")
            || w.starts_with("-e")
            || w.starts_with("-f")
            || w.starts_with("--regexp=")
            || w.starts_with("--file=")
    });
    let skip = usize::from(SCRIPT_FIRST.contains(&program) && !supplied);
    let operands = positional(&words, out, access);
    // `touch` creates its operands and is not a command Claude Code recognises
    // by name, so only `Edit` rules reach them.
    let via = if program == "touch" {
        Via::WritePath
    } else {
        Via::FileCommand
    };
    for w in operands.iter().skip(skip) {
        push_target(out, w, access, via, recursive);
    }
}

/// The command a barrier program will actually run, or `None` when there is
/// nothing left of it.
fn past_barrier(program: &str, words: &[String]) -> Option<String> {
    let mut rest = &words[1..];
    if program == "env" {
        // `-i`, `-0`, `-u NAME`, `-C DIR`, `--chdir=DIR`, and `NAME=value`.
        while let Some(w) = rest.first() {
            if w == "-u" || w == "-C" || w == "--unset" || w == "--chdir" {
                rest = rest.get(2..)?;
            } else if w.starts_with('-') || w.split_once('=').is_some_and(|(n, _)| is_env_name(n)) {
                rest = rest.get(1..)?;
            } else {
                break;
            }
        }
    } else if program == "sudo" || program == "doas" {
        while let Some(w) = rest.first() {
            if w == "-u" || w == "-g" {
                rest = rest.get(2..)?;
            } else if w.starts_with('-') {
                rest = rest.get(1..)?;
            } else {
                break;
            }
        }
    }
    if rest.is_empty() {
        return None;
    }
    // `eval "cat .env"` and `sh -c 'cat .env'` carry their command as one
    // quoted word; `words_of` has already removed the quotes.
    Some(rest.join(" "))
}

/// `git diff .env`, `git grep x -- .env`, `git blame --ignore-revs-file=.env`.
///
/// Everything past `--` is a path by definition. Before it, the subcommand's
/// own arity decides: `git grep` spends one operand on the pattern and `git
/// diff` spends none, which is the difference between the two entries in
/// `GIT_FILE_SUBCOMMANDS`.
fn git_targets(words: &[String], out: &mut Targets) {
    // No option-value scan. `git blame --ignore-revs-file=.env` is the
    // changelog's own example of the 2.1.266 fix and the running product
    // **runs** it under `Read(.env)` — so the fix was inside the recognised
    // file commands, and `git` is reached through its operands only. Scanning
    // it here made this matcher stricter than the product, which refuses calls
    // the user's own settings allow.
    let Some(pos) = words.iter().skip(1).position(|w| !w.starts_with('-')) else {
        return;
    };
    let sub = &words[pos + 1];
    let Some((_, arity)) = GIT_FILE_SUBCOMMANDS.iter().find(|(n, _)| n == sub) else {
        return;
    };
    let mut spent = 0usize;
    let mut only_paths = false;
    for w in words.iter().skip(pos + 2) {
        if w == "--" {
            only_paths = true;
            continue;
        }
        if !only_paths {
            if w.starts_with('-') {
                continue;
            }
            if spent < *arity {
                spent += 1;
                continue;
            }
        }
        push_target(out, w, Access::Read, Via::FileCommand, false);
        // `git show HEAD:.env` and `git cat-file -p HEAD:.env` name a path
        // *inside a revision*, which is the same secret arriving through git's
        // object store rather than through the working tree. The operand is
        // emitted whole above — a path called `HEAD:.env` — and the path after
        // the revision separator is emitted here as well, so `Read(.env)`
        // reaches it. A colon is legal in a filename, so both forms are
        // offered rather than one replacing the other.
        // ── inferred from a measured sibling, and not yet measured itself ──
        // `show` is in the table above because deny rules were measured
        // reaching its operands. `HEAD:.env` is that same subcommand naming
        // that same file through git's object store instead of the working
        // tree, so the path after the revision separator is emitted too.
        // Whether the running product agrees is a question for the differential
        // harness and is listed there; until it answers, this is the weaker
        // tier — stronger than a guess, weaker than a measurement.
        if let Some((rev, path)) = w.split_once(':')
            && !path.is_empty()
            && !rev.contains('/')
        {
            push_target(out, path, Access::Read, Via::FileCommand, false);
        }
    }
}

/// The positional operands of a command, emitting any option values on the way.
fn positional(words: &[String], out: &mut Targets, access: Access) -> Vec<String> {
    option_values(words, out, access);
    let program = words
        .first()
        .map(|w| w.rsplit('/').next().unwrap_or(w))
        .unwrap_or_default();
    let flags = value_flags(program);
    let mut operands = Vec::new();
    let mut skip_next = false;
    let mut only_operands = false;
    for w in words.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if w == "--" {
            only_operands = true;
            continue;
        }
        if !only_operands && w.starts_with('-') && w.len() > 1 {
            if flags.contains(&w.as_str()) {
                skip_next = true;
            }
            continue;
        }
        if w.starts_with('>') || w.starts_with('<') {
            continue;
        }
        operands.push(w.clone());
    }
    operands
}

/// Paths hidden in option values, which a positional scan skips entirely:
/// *"Fixed Bash `Read()` deny rules missing option values
/// (`--ignore-revs-file=.env`, `-f.env`, `@file`)"* (2.1.266).
///
/// Generous on purpose. A value that is not a path — the `5` in `-n5` — costs
/// one target that no rule matches, while a value that is one and goes
/// unextracted is a deny rule that reads as protection and is none. It reaches
/// deny rules only: `Via::OptionValue` is excluded from `allow_side_applies`.
fn option_values(words: &[String], out: &mut Targets, access: Access) {
    for w in words.iter().skip(1) {
        let value = if let Some(rest) = w.strip_prefix('@') {
            rest
        } else if w.starts_with("--") {
            match w.split_once('=') {
                Some((_, v)) => v,
                None => continue,
            }
        } else if w.starts_with('-') {
            // `-f.env`, `-I/etc`: one flag letter, then the value. Counted in
            // **characters**, not bytes. `&w[2..]` panics when the second
            // character is multi-byte, and the text is a command an agent
            // wrote — a crash in the matcher is not a wrong answer, it is
            // every tool call on the machine blocked, on the hook the session
            // waits for.
            let mut rest = w.chars();
            rest.next();
            rest.next();
            rest.as_str()
        } else {
            continue;
        };
        if value.is_empty() || no_file_behind(value) {
            continue;
        }
        push_target(out, value, access, Via::OptionValue, false);
    }
}

fn push_target(out: &mut Targets, path: &str, access: Access, via: Via, subtree: bool) {
    if !out.room() {
        return;
    }
    out.list.push(FileTarget {
        unresolvable: unresolvable(path),
        path: path.to_string(),
        access,
        via,
        subtree,
    });
}

/// The command with its redirections removed.
///
/// Claude Code matches a Bash rule against the text **without** its redirect
/// clauses, so an exact `Bash(touch ran.txt)` covers
/// `touch ran.txt > /dev/null`. The redirect is checked separately against the
/// file rules. Undocumented; measured against the running product.
pub fn without_redirections(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' && q == '"' && i + 1 < b.len() {
                out.push(b[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            quote = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        // A file descriptor prefix (`2>`), then the operator, then the target.
        let start = i;
        let mut j = i;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j < b.len() && (b[j] == '>' || b[j] == '<') {
            // A here-doc carries its text inline and is not a redirect to a
            // file; leaving it alone is safer than trying to excise it.
            if b[j] == '<' && b.get(j + 1) == Some(&'<') {
                out.push(c);
                i += 1;
                continue;
            }
            j += 1;
            while b.get(j) == Some(&'>') || b.get(j) == Some(&'|') || b.get(j) == Some(&'&') {
                j += 1;
            }
            while j < b.len() && b[j].is_whitespace() {
                j += 1;
            }
            let (_, next) = word_at(&b, j);
            i = next.max(start + 1);
            // Leave one space so `a > f b` does not become `ab`.
            if !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}

/// Splits into words, honouring quotes and dropping the quote characters.
fn words_of(text: &str) -> Vec<String> {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let (word, next) = word_at(&b, i);
        if !word.is_empty() {
            out.push(word);
        }
        i = next.max(i + 1);
    }
    out
}

/// One word starting at `from`, with quotes removed, stopping at whitespace or
/// a redirection operator.
fn word_at(b: &[char], from: usize) -> (String, usize) {
    let mut s = String::new();
    let mut i = from;
    let mut quote: Option<char> = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == '\\' && q == '"' {
                    if let Some(&n) = b.get(i + 1) {
                        s.push(n);
                    }
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                } else {
                    s.push(c);
                }
                i += 1;
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    i += 1;
                    continue;
                }
                if c == '\\' {
                    if let Some(&n) = b.get(i + 1) {
                        s.push(n);
                    }
                    i += 2;
                    continue;
                }
                if c.is_whitespace() || c == '>' || c == '<' {
                    break;
                }
                s.push(c);
                i += 1;
            }
        }
    }
    (s, i)
}

// ---------------------------------------------------------------------------
// Suggesting a rule from calls that reached a person.

/// The rule that would have answered every one of these commands, if one
/// sensible rule does.
///
/// This is the inverse of everything else in this module. The matcher asks
/// *does this rule cover that command*; a person looking at an inbox full of
/// the same permission prompt is asking *what rule do I write so this stops*,
/// and answering it by hand means knowing that `*` goes after the subcommand
/// and that a rule which cannot match anything is worse than no rule.
///
/// Returns the **narrowest** rule that covers the set:
///
/// * one distinct command → that exact command, which approves nothing else;
/// * several sharing a program and subcommand → `prog sub *`, the form the
///   reference asks for (*"put the `*` after the subcommand"*);
/// * several sharing only a program → `prog *`, and only when the program has
///   no subcommand to speak of, because `Bash(git *)` is a much larger grant
///   than the person is asking for.
///
/// `None` when the set has no shape worth suggesting, which is the honest
/// answer far more often than a clever one would be.
/// The host of a URL, without scheme, credentials, port or path.
fn web_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?
        .split(':')
        .next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The rule that would answer these calls to `tool`, in the vocabulary that
/// tool's rules are written in.
///
/// A shell rule is a command prefix; a path rule is a directory glob; a
/// `WebFetch` rule is a domain. Offering the wrong one is worse than offering
/// nothing: a person pastes `Read(src/main.rs)` into their config and is asked
/// again about the next file in the same directory.
pub fn suggest_rule_for(tool: &str, calls: &[String]) -> Option<String> {
    // Every tool whose rules carry a command pattern, not just `Bash`:
    // `PowerShell` and `Monitor` are command tools too, and offering somebody
    // the literal command line as their rule is offering a rule that covers
    // the call they just saw and nothing else.
    //
    // **This composes; it does not check.** It once said the result was
    // "validated by deterministic replay before it is shown", and nothing
    // replayed anything — the only protection here is the *static* refusal in
    // [`suggest_rule`], which drops a command no prefix rule may approve before
    // a rule is composed at all. Whether the composed rule actually decides the
    // call is a different question and is [`crate::core::offer`]'s, which
    // parses the text and matches it before anybody is handed it.
    if crate::core::policy::is_command_tool(tool) {
        return suggest_rule(calls);
    }
    if tool.eq_ignore_ascii_case("Read") || tool.eq_ignore_ascii_case("Edit") {
        let family = rule_family(tool, calls.first()?);
        if calls.iter().any(|c| rule_family(tool, c) != family) {
            return None;
        }
        // A bare filename is its own rule; a directory takes everything under
        // it, which is the shape gitignore syntax is for.
        return Some(
            if family.contains('/') || calls.iter().any(|c| c.contains('/')) {
                format!("{family}/**")
            } else {
                family
            },
        );
    }
    if tool.eq_ignore_ascii_case("WebFetch") {
        // **`domain:` is not decoration.** It is the vendor's documented
        // specifier for this tool, and `WebFetch(docs.rs)` without it is not a
        // domain grant — it is read as something else and answers nothing.
        // This returned the bare host until `core::offer` started replaying
        // suggestions against the call they were composed from, and the first
        // run refused it.
        let host = web_host(calls.first()?)?;
        return calls
            .iter()
            .all(|c| web_host(c).as_deref() == Some(host.as_str()))
            .then(|| format!("domain:{host}"));
    }
    // Everything else is matched on its whole specifier, so one distinct value
    // is a rule and several are not.
    let first = calls.first()?;
    calls.iter().all(|c| c == first).then(|| first.clone())
}

pub fn suggest_rule(commands: &[String]) -> Option<String> {
    let mut distinct: Vec<&str> = Vec::new();
    for c in commands {
        let t = c.trim();
        if t.is_empty() || t.chars().count() > MAX_ANALYSED {
            return None;
        }
        if !distinct.contains(&t) {
            distinct.push(t);
        }
    }
    let first = *distinct.first()?;
    // A command no prefix rule may approve is a command no suggestion should
    // offer: the person would paste it in and it would still prompt.
    if distinct.iter().any(|c| unapprovable_by_prefix(c).is_some()) {
        return None;
    }
    // Never suggest a rule for a compound: the parts are separate grants and
    // the person should see them separately.
    if distinct.iter().any(|c| match subcommands(c) {
        Some(parts) => parts.len() > 1,
        None => true,
    }) {
        return None;
    }
    if distinct.len() == 1 {
        return Some(first.to_string());
    }

    let words = |c: &str| -> Vec<String> { words_of(&strip(c, false)) };
    let head = words(first);
    let program = head.first()?.clone();
    if distinct.iter().any(|c| words(c).first() != Some(&program)) {
        return None;
    }
    // The subcommand is the word that decides what the program does, so it is
    // part of the grant. A leading `-flag` is not one.
    let subcommand = head
        .get(1)
        .filter(|w| !w.starts_with('-'))
        .filter(|_| distinct.iter().all(|c| words(c).get(1) == head.get(1)));
    match subcommand {
        Some(sub) => Some(format!("{program} {sub} *")),
        // `Bash(git *)` grants every git command to answer a prompt about
        // `git status`. A person may still want it; suggesting it is another
        // matter.
        None if head.len() == 1 => Some(format!("{program} *")),
        None => None,
    }
}

/// The family a call belongs to when grouping calls that reached a person:
/// the program and the subcommand that decides what it does.
///
/// Two calls in the same family can plausibly share one rule; two in different
/// families cannot, and asking [`suggest_rule`] to find one spanning them
/// returns `None` for a screen full of interruptions that each had an obvious
/// answer.
///
/// For a tool that is not a shell the family is the whole specifier, because a
/// path or a URL has no head to group by that a person would recognise.
pub fn rule_family(tool: &str, content: &str) -> String {
    // A path rule is written about a *directory*, so two calls in one directory
    // share a rule and two in different ones do not. Suggesting the file
    // instead gives a person one rule per file, which is not a suggestion.
    if tool.eq_ignore_ascii_case("Read") || tool.eq_ignore_ascii_case("Edit") {
        return match content.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => dir.to_string(),
            // A bare filename at the working-directory root: the file is the
            // family, and `Read(.env)` is a rule somebody would really write.
            _ => content.to_string(),
        };
    }
    // A `WebFetch` rule takes a **domain**, so the host is the family and the
    // URL is noise: `WebFetch(docs.rs)`, not `WebFetch(https://docs.rs/x)`.
    if tool.eq_ignore_ascii_case("WebFetch") {
        return web_host(content).unwrap_or_else(|| content.to_string());
    }
    if !tool.eq_ignore_ascii_case("Bash") {
        return content.to_string();
    }
    let stripped = strip(content.trim(), false);
    let words = words_of(&stripped);
    let Some(program) = words.first() else {
        return content.to_string();
    };
    match words.get(1).filter(|w| !w.starts_with('-')) {
        Some(sub) => format!("{program} {sub}"),
        None => program.clone(),
    }
}

#[cfg(test)]
mod suggestion_reach_tests {
    use super::*;

    #[test]
    fn a_rule_is_suggested_for_every_command_tool_not_only_bash() {
        // `PowerShell` and `Monitor` carry command patterns like `Bash`, so the
        // rule offered for one of their calls has to be a *pattern*. This used
        // to fall through to the opaque branch and offer the literal command
        // line, which covers the call somebody just saw and nothing else — the
        // shape of rule that gets written once and never fires again.
        // One call is its own narrowest rule, which was already true. The
        // difference is *several* calls that share a program and a subcommand:
        // a command tool gets one pattern spanning them, where the opaque
        // branch gave up because the strings differ. The vendor's own example
        // of a PowerShell rule is `PowerShell(git commit *)`, which is this.
        let calls = [
            "git commit -m one".to_string(),
            "git commit -m two".to_string(),
        ];
        for tool in ["Bash", "PowerShell", "Monitor"] {
            let s = suggest_rule_for(tool, &calls)
                .unwrap_or_else(|| panic!("{tool} should get a suggestion"));
            assert_eq!(s, "git commit *", "{tool} was offered `{s}`");
        }
        // A tool whose specifier is opaque still needs the calls to agree, and
        // these do not.
        assert_eq!(suggest_rule_for("Agent", &calls), None);
        // The conservatism is shared too, and that is the point of routing
        // them through one function: no tool is offered a cmdlet-wide or
        // program-wide grant to answer a prompt about one operand.
        for tool in ["Bash", "PowerShell"] {
            assert_eq!(
                suggest_rule_for(tool, &["rm a".to_string(), "rm b".to_string()]),
                None,
                "{tool} must not be offered `rm *`"
            );
        }
        // And a tool whose rules are not command patterns is unaffected.
        assert_eq!(
            suggest_rule_for("Agent", &["Explore".to_string()]).as_deref(),
            Some("Explore")
        );
    }

    #[test]
    fn the_command_tools_are_the_ones_the_policy_calls_command_shaped() {
        // Two lists that must not drift: `shape_of` decides how a rule's
        // specifier is *parsed*, and this decides which tools get a pattern
        // *offered*. They were one list and a hardcoded `"Bash"` before.
        for tool in ["Bash", "PowerShell", "Monitor"] {
            assert!(crate::core::policy::is_command_tool(tool), "{tool}");
        }
        for tool in ["Read", "Edit", "Glob", "LSP", "WebFetch", "Agent"] {
            assert!(!crate::core::policy::is_command_tool(tool), "{tool}");
        }
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;

    #[test]
    fn a_transparent_wrapper_is_looked_through() {
        for (line, want) in [
            ("sudo rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("sudo -u root rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("doas rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("exec rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("env FOO=1 rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("env -C /tmp FOO=1 rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("watch rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("sudo env FOO=1 watch rm -rf /tmp/x", "rm -rf /tmp/x"),
            // Not a wrapper, so nothing is taken off.
            ("rm -rf /tmp/x", "rm -rf /tmp/x"),
            ("git commit -m x", "git commit -m x"),
        ] {
            assert_eq!(strip_transparent(line), want, "{line}");
        }
    }

    #[test]
    fn a_wrapper_with_nothing_after_it_is_left_alone() {
        // The loop must not walk off the end and return an empty command, which
        // would make every rule match nothing at all.
        for line in ["sudo", "env", "sudo -u root", "env -C /tmp"] {
            assert!(!strip_transparent(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn a_program_path_reduces_to_its_name() {
        assert_eq!(
            basename_program("/bin/rm -rf x").as_deref(),
            Some("rm -rf x")
        );
        assert_eq!(
            basename_program("./scripts/deploy").as_deref(),
            Some("deploy")
        );
        // No separator: nothing to do, and the caller skips a second match.
        assert_eq!(basename_program("rm -rf x"), None);
        // A leading flag is not a program.
        assert_eq!(basename_program("-x /a/b"), None);
    }

    #[test]
    fn undecidable_is_about_command_position_only() {
        // A substitution in an *argument* leaves the program readable.
        assert_eq!(undecidable("echo \"$(date)\""), None);
        assert_eq!(undecidable("grep -r \"$PATTERN\" ."), None);
        // In command position it does not.
        assert!(undecidable("$(echo rm) -rf /").is_some());
        assert!(undecidable("rm$IFS-rf /").is_some());
    }

    #[test]
    fn an_interpreter_given_a_script_is_readable() {
        // The distinction that keeps this from firing on every python call.
        assert_eq!(undecidable("python manage.py migrate"), None);
        assert_eq!(undecidable("node build.js --watch"), None);
        assert!(undecidable("python -c \"import os\"").is_some());
        assert!(undecidable("node -e \"require('fs')\"").is_some());
        // No script at all means the program arrives on standard input, which
        // is what the tail of `… | sh` looks like once the pipe is split.
        assert!(undecidable("sh").is_some());
        assert!(undecidable("echo x | bash").is_some());
    }

    #[test]
    fn an_unbalanced_quote_is_not_a_readable_line() {
        assert!(undecidable("rm -rf \"/tmp").is_some());
    }

    #[test]
    fn a_line_past_the_analysis_limit_is_not_readable() {
        let long = format!("ls {}", "x".repeat(MAX_ANALYSED));
        assert!(undecidable(&long).is_some());
    }
}
