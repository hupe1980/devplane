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
//!   explain why it splits. Vibeplane answers the permission prompt, so that
//!   was a destructive command approved without anybody being asked.
//!
//! Everything here is a pure function of the command text, because it runs on
//! the synchronous hook a session is blocked on.
//!
//! It is a splitter, not a shell. Claude Code's own documentation is explicit
//! that a Bash rule "isn't a security boundary around the program" — `/bin/rm`,
//! `sh -c 'rm …'` and `git -C . push` are not covered by rules naming `rm` or
//! `git push`, here or there. The goal is to agree with Claude Code, not to
//! outdo it: a matcher that is stricter than the thing it mirrors would refuse
//! calls the user's own settings allow.

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

/// Why no *prefix* allow rule may answer for this command, if there is a reason.
///
/// These are the forms Claude Code puts in front of a person whatever an allow
/// rule says, and the reason they need naming here is the direction the mistake
/// runs in: Vibeplane is the thing answering the prompt, so a rule that is
/// broader here than there is a call approved without anybody being asked. The
/// escape hatch is the one Claude Code documents — *"write an exact-match rule
/// for the full command string"* — so a rule with no wildcard in it still
/// works, and only the prefix form is refused.
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
        if program == "find"
            && let Some(p) = words.find(|w| FIND_EXEC_PREDICATES.contains(w))
        {
            return Some(format!("`find {p}` runs a program or deletes files"));
        }
    }
    None
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
/// Vibeplane refuse calls the user's own settings allow — the mistake this
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
    let mut out = Vec::new();
    collect(text, &mut out, 0);
    out
}

fn collect(text: &str, out: &mut Vec<String>, depth: usize) {
    // Bounded: a command an agent wrote is untrusted input, and this runs on a
    // hook a session is blocked on.
    if depth > 8 || out.len() > 256 {
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
            collect(&inner, out, depth + 1);
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
            if let Some((name, _)) = head.split_once('=')
                && is_env_name(name)
                && (any_assignment || SAFE_ASSIGNMENTS.contains(&name))
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
/// Kept for the audit trail and for messages; **it is no longer what decides
/// which rules reach the target.** It used to be, and that was wrong in the
/// direction this layer is always wrong in. The asymmetry Claude Code
/// documents is about whether anybody was going to be *asked*: a recognised
/// file command like `cat` is in the built-in read-only set, so there is no
/// prompt for an allow rule to skip and only a deny rule applies. That
/// reasoning is about the **access**, not about the syntax — and `tee` is a
/// recognised file command that *writes*, for which Claude Code checks the
/// destination against `Edit` rules and the working directories exactly as if
/// it were a redirect (2.1.269). Keying the asymmetry on `Via` meant
/// `Edit(.env)` stopped `echo x > .env` and not `echo x | tee .env`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// `> file`, `>> file`, `2> file`, `< file`.
    Redirect,
    /// An operand of a file command Claude Code recognises.
    FileCommand,
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
}

impl FileTarget {
    /// Whether an **allow** rule, and the working-directory check behind it,
    /// may speak for this target.
    ///
    /// Everything a command writes, and everything a redirect names. What is
    /// excluded is exactly the case where no prompt was ever coming: an
    /// operand of a recognised file command that only *reads*, all of which
    /// are in Claude Code's built-in read-only set.
    pub fn allow_side_applies(&self) -> bool {
        self.access == Access::Write || self.via == Via::Redirect
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
    ("cat", Access::Read),
    ("head", Access::Read),
    ("tail", Access::Read),
    ("sed", Access::Read),
    // Writes every operand. `-a` and `-i` take no value, so no flag here
    // consumes the word after it.
    ("tee", Access::Write),
    // Creates its operands. An `Edit` deny on the path blocks it; an `Edit`
    // allow alone does not run it, so the `Bash` rule is still required.
    // Neither half is in the reference — both are measured.
    ("touch", Access::Write),
];

/// Flags of those commands that consume the word after them, so that the `5`
/// in `head -n 5 f` is not mistaken for a filename.
const VALUE_FLAGS: &[&str] = &["-n", "-c", "-e", "-f", "--lines", "--bytes", "--expression"];

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
pub fn file_targets(text: &str) -> Vec<FileTarget> {
    let mut out = Vec::new();
    for part in nested_commands(text) {
        redirect_targets(&part, &mut out);
        file_command_targets(&part, &mut out);
        if out.len() > 64 {
            break;
        }
    }
    out.dedup();
    out
}

/// The targets of `>`, `>>`, `2>`, `&>` and `<`, ignoring the forms with no
/// file behind them: `/dev/null`, `2>&1`, `<&3`, here-docs and here-strings.
fn redirect_targets(text: &str, out: &mut Vec<FileTarget>) {
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
        out.push(FileTarget {
            unresolvable: unresolvable(&word),
            path: word,
            access,
            via: Via::Redirect,
        });
    }
}

/// The file arguments of `cat`, `head`, `tail` and `sed`.
fn file_command_targets(text: &str, out: &mut Vec<FileTarget>) {
    // Redirections first: their targets are not operands. `touch f > /dev/null`
    // names one file, and `cat secrets > out` reads one and writes the other.
    let stripped = strip(&without_redirections(text), true);
    let words = words_of(&stripped);
    let Some(first) = words.first() else { return };
    let program = first.rsplit('/').next().unwrap_or(first);
    let Some((_, base_access)) = FILE_COMMANDS.iter().find(|(n, _)| *n == program) else {
        return;
    };
    // `sed -i` rewrites the files it is given; `sed 's/a/b/' f` reads them, and
    // its first non-flag argument is the script rather than a file.
    let in_place = program == "sed" && words.iter().any(|w| w == "-i" || w.starts_with("-i"));
    let mut skip_script = program == "sed";
    let mut skip_next = false;
    for w in words.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if w == "--" {
            continue;
        }
        if w.starts_with('-') && w.len() > 1 {
            if VALUE_FLAGS.contains(&w.as_str()) {
                skip_next = true;
            }
            continue;
        }
        if skip_script {
            skip_script = false;
            continue;
        }
        if w.starts_with('>') || w.starts_with('<') {
            continue;
        }
        out.push(FileTarget {
            unresolvable: unresolvable(w),
            path: w.clone(),
            access: if in_place {
                Access::Write
            } else {
                *base_access
            },
            via: Via::FileCommand,
        });
    }
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
