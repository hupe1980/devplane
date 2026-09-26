//! Reading a shell command line into tokens: simple commands, each reduced to
//! a program and arguments with transparent wrappers stripped. Every construct
//! the reader cannot see through is a barrier, never a guess, which makes a
//! verdict [`crate::core::Verdict::Unresolved`] rather than silent.

/// One program invocation, wrappers stripped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Simple {
    /// The basename, lower-cased: on a case-insensitive filesystem `RM` runs `rm`.
    pub program: String,
    pub args: Vec<String>,
    pub redirects: Vec<Redirect>,
    /// The command's stdin is a heredoc or another command's stdout.
    pub fed: bool,
    /// What the shell does to each argument, index for index with `args`.
    pub kinds: Vec<ArgKind>,
}

/// Whether the reader knows an argument's final text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArgKind {
    #[default]
    Literal,
    /// An unquoted `*`, `?` or `[`: files on the disk decide the words.
    Glob,
    /// A parameter or command substitution, or a brace sequence: any number
    /// of words of any text.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub target: String,
    pub write: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    pub commands: Vec<Simple>,
    /// The first construct the reader could not see through.
    pub barrier: Option<String>,
}

/// Longer lines are not read at all (a barrier).
const MAX_LEN: usize = 65_536;
const MAX_DEPTH: usize = 4;

/// Programs that run code given on their command line or on their input.
const INTERPRETERS: &[&str] = &[
    "sh", "bash", "zsh", "dash", "ksh", "fish", "python", "python2", "python3", "perl", "ruby",
    "node", "nodejs", "deno", "bun", "php", "lua", "rscript", "awk", "gawk", "mawk", "nawk",
];
/// Interpreters whose `-c` argument is itself a shell line worth reading.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];
/// Programs that hand their arguments to a shell somewhere else.
const ELSEWHERE: &[&str] = &[
    "ssh",
    "su",
    "nix-shell",
    "chroot",
    "nsenter",
    "sudoedit",
    "script",
];
/// Programs that build a command line from their input.
/// `alias`, `trap`, `hash`, `enable` and `bind` make a later word run code.
const BUILDERS: &[&str] = &[
    "xargs", "parallel", "eval", "source", ".", "alias", "trap", "hash", "enable", "bind",
];
/// Builtins that evaluate an array subscript in an argument, and so run a
/// command substitution inside it.
const SUBSCRIPTING: &[&str] = &[
    "[[",
    "let",
    "declare",
    "typeset",
    "local",
    "export",
    "readonly",
    "read",
    "printf",
    "unset",
    "test",
    "[",
    "mapfile",
    "readarray",
];
/// How many wrappers the reader looks through; one more is a barrier.
const MAX_WRAPPERS: usize = 8;

/// Variables whose value a program runs (`GIT_SSH_COMMAND`, `PAGER`, …) or
/// that change what runs (`LD_PRELOAD`, `BASH_ENV`). Not `PATH` or `IFS`:
/// the program a rule names is matched by its basename wherever it is found,
/// and an expansion `IFS` would split is already unknown to this reader.
fn runs_its_value(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    matches!(
        n.as_str(),
        "VISUAL"
            | "LD_PRELOAD"
            | "LD_LIBRARY_PATH"
            | "LD_AUDIT"
            | "BASH_ENV"
            | "PROMPT_COMMAND"
            | "PS0"
            | "PS1"
            | "PS2"
            | "PS4"
            | "SHELLOPTS"
            | "BASHOPTS"
            | "GIT_EXTERNAL_DIFF"
            | "GIT_EXEC_PATH"
            | "GIT_DIR"
            | "GIT_CONFIG"
            | "GIT_TEMPLATE_DIR"
            | "PYTHONSTARTUP"
            | "PERL5OPT"
            | "RUBYOPT"
            | "ZDOTDIR"
    ) || n.starts_with("DYLD_")
        || n.starts_with("GIT_SSH")
        || n.starts_with("GIT_CONFIG_")
        || n.ends_with("_COMMAND")
        || n.ends_with("EDITOR")
        || n.ends_with("PAGER")
        || n.ends_with("ASKPASS")
}

/// A `git -c key=value` whose value git runs.
fn git_config_runs(pair: &str) -> bool {
    let key = pair
        .split_once('=')
        .map_or(pair, |(k, _)| k)
        .to_ascii_lowercase();
    key.starts_with("alias.")
        || key.starts_with("filter.")
        || key.starts_with("pager.")
        || key.starts_with("include")
        || [
            "pager",
            "editor",
            "sshcommand",
            "fsmonitor",
            "hookspath",
            "external",
            "textconv",
            "program",
            "helper",
            "cmd",
            "command",
            "askpass",
            // `core.gitProxy` runs; `http.proxy` names a host.
            "gitproxy",
            "exec",
            "tool",
            "receivepack",
            "uploadpack",
        ]
        .iter()
        .any(|k| key.contains(k))
}
/// Programs whose `-e`/`-c` take a pattern or a count, never a command line.
const TEXT_TOOLS: &[&str] = &[
    "grep", "egrep", "fgrep", "rg", "ag", "ack", "sed", "wc", "cut", "echo", "printf", "tee",
    "sort", "uniq", "tr", "head", "tail", "diff", "cat", "ls", "od", "xxd",
];

/// Programs that only ever read the data fed to them.
const SINKS: &[&str] = &[
    "cat", "tee", "head", "tail", "wc", "sort", "uniq", "cut", "tr", "base64",
];
/// Flags an interpreter may take without running anything.
const HARMLESS: &[&str] = &["--version", "-V", "--help", "-h"];

/// A program that runs the command after it, in place.
struct Wrapper {
    name: &'static str,
    /// Flags that take the next word (or a glued `=value`).
    values: &'static [&'static str],
    plain: &'static [&'static str],
    /// Operands before the command.
    operands: usize,
    /// Flags that start a shell instead of running a command.
    shell: &'static [&'static str],
    /// The rest is joined and handed to `sh -c`, as `watch` does.
    joins: bool,
}

/// Every flag a wrapper takes is listed: for an unknown flag the reader cannot
/// tell a value from the command, so it is a barrier.
#[rustfmt::skip]
const WRAPPERS: &[Wrapper] = &[
    Wrapper { name: "sudo",
        values: &["-u", "--user", "-g", "--group", "-h", "--host", "-p", "--prompt", "-C", "--close-from",
                  "-r", "--role", "-t", "--type", "-T", "--command-timeout", "-U", "--other-user",
                  "-D", "--chdir", "-R", "--chroot"],
        plain: &["-A", "--askpass", "-b", "--background", "-E", "--preserve-env", "-H", "--set-home",
                 "-k", "--reset-timestamp", "-K", "--remove-timestamp", "-n", "--non-interactive",
                 "-P", "--preserve-groups", "-S", "--stdin", "-B", "--bell", "-N", "--no-update"],
        operands: 0, shell: &["-s", "--shell", "-i", "--login"], joins: false },
    Wrapper { name: "doas", values: &["-u", "-C"], plain: &["-n", "-L"], operands: 0, shell: &["-s"], joins: false },
    Wrapper { name: "env",
        values: &["-u", "--unset", "-C", "--chdir", "-P", "-S", "--split-string"],
        plain: &["-i", "--ignore-environment", "-0", "--null", "-v", "--debug", "--default-signal",
                 "--ignore-signal", "--block-signal", "--list-signal-handling"],
        operands: 0, shell: &["-S", "--split-string"], joins: false },
    Wrapper { name: "exec", values: &["-a"], plain: &["-c", "-l"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "command", values: &[], plain: &["-p"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "builtin", values: &[], plain: &[], operands: 0, shell: &[], joins: false },
    Wrapper { name: "nohup", values: &[], plain: &[], operands: 0, shell: &[], joins: false },
    Wrapper { name: "time",
        values: &["-o", "--output", "-f", "--format"],
        plain: &["-p", "--portability", "-a", "--append", "-v", "--verbose", "-q", "--quiet", "-l", "-h"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "nice", values: &["-n", "--adjustment"], plain: &[], operands: 0, shell: &[], joins: false },
    Wrapper { name: "ionice",
        values: &["-c", "--class", "-n", "--classdata", "-p", "--pid", "-P", "--pgid", "-u", "--uid"],
        plain: &["-t", "--ignore"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "timeout",
        values: &["-s", "--signal", "-k", "--kill-after"],
        plain: &["--preserve-status", "--foreground", "-v", "--verbose", "-f", "-p"],
        operands: 1, shell: &[], joins: false },
    Wrapper { name: "stdbuf", values: &["-i", "--input", "-o", "--output", "-e", "--error"], plain: &[],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "flock",
        values: &["-w", "--timeout", "--wait", "-E", "--conflict-exit-code", "-c", "--command"],
        plain: &["-s", "--shared", "-x", "-e", "--exclusive", "-u", "--unlock", "-n", "--nb",
                 "--nonblock", "-o", "--close", "-F", "--no-fork", "--verbose"],
        operands: 1, shell: &["-c", "--command"], joins: false },
    Wrapper { name: "setsid", values: &[], plain: &["-c", "--ctty", "-f", "--fork", "-w", "--wait"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "busybox", values: &[], plain: &[], operands: 0, shell: &[], joins: false },
    // zsh precommand modifiers: the macOS shell Claude Code runs in.
    Wrapper { name: "noglob", values: &[], plain: &[], operands: 0, shell: &[], joins: false },
    Wrapper { name: "nocorrect", values: &[], plain: &[], operands: 0, shell: &[], joins: false },
    Wrapper { name: "repeat", values: &[], plain: &[], operands: 1, shell: &[], joins: false },
    Wrapper { name: "caffeinate", values: &["-t", "-w"], plain: &["-d", "-i", "-m", "-s", "-u"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "arch",
        values: &["-arch", "-d", "-e"],
        plain: &["-32", "-64", "-c", "-h", "-arm64", "-arm64e", "-x86_64", "-x86_64h", "-i386"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "strace",
        values: &["-e", "-o", "-p", "-s", "-u", "-E", "-a", "-b", "-I", "-O", "-S", "-P", "-X",
                  "--output", "--attach", "--string-limit", "--user", "--env", "--trace", "--signal",
                  "--status", "--quiet", "--decode-fds"],
        plain: &["-f", "-ff", "-c", "-C", "-d", "-D", "-DD", "-DDD", "-F", "-h", "-i", "-k", "-n", "-q",
                 "-qq", "-r", "-t", "-tt", "-ttt", "-T", "-v", "-V", "-w", "-x", "-xx", "-y", "-yy",
                 "-z", "-Z", "--follow-forks", "--output-separately", "--summary-only", "--summary",
                 "--no-abbrev", "--syscall-times", "--absolute-timestamps", "--relative-timestamps",
                 "--instruction-pointer", "--stack-trace", "--seccomp-bpf"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "ltrace", values: &["-e", "-o", "-p", "-s", "-u", "-a", "-n", "-A", "-D", "-F", "-l", "-L", "-w"],
        plain: &["-b", "-c", "-C", "-f", "-h", "-i", "-r", "-S", "-t", "-tt", "-ttt", "-T", "-V"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "gtimeout",
        values: &["-s", "--signal", "-k", "--kill-after"],
        plain: &["--preserve-status", "--foreground", "-v", "--verbose", "-f", "-p"],
        operands: 1, shell: &[], joins: false },
    Wrapper { name: "pkexec", values: &["--user"], plain: &["--disable-internal-agent", "--keep-cwd"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "runuser",
        values: &["-u", "--user", "-g", "--group", "-G", "--supp-group", "-w",
                  "--whitelist-environment", "-s", "--shell"],
        plain: &["-m", "-p", "--preserve-environment", "-f", "--fast", "-P", "--pty"],
        operands: 0, shell: &["-c", "--command", "-l", "--login", "--session-command"], joins: false },
    Wrapper { name: "unbuffer", values: &[], plain: &["-p"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "chrt",
        values: &["-T", "--sched-runtime", "-P", "--sched-period", "-D", "--sched-deadline"],
        plain: &["-a", "--all-tasks", "-b", "--batch", "-d", "--deadline", "-f", "--fifo", "-i",
                 "--idle", "-o", "--other", "-r", "--rr", "-R", "--reset-on-fork", "-v", "--verbose",
                 "-p", "--pid"],
        operands: 1, shell: &[], joins: false },
    Wrapper { name: "taskset",
        values: &[], plain: &["-a", "--all-tasks", "-c", "--cpu-list", "-p", "--pid"],
        operands: 1, shell: &[], joins: false },
    Wrapper { name: "firejail", values: &[], plain: &["-q", "--quiet", "--noprofile", "--private"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "valgrind", values: &[], plain: &["-q", "--quiet", "-v", "--verbose"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "systemd-run",
        values: &["-u", "--unit", "-p", "--property", "-M", "--machine", "-H", "--host",
                  "--description", "--slice", "-E", "--setenv", "--uid", "--gid", "--nice",
                  "-D", "--working-directory", "--on-active", "--on-calendar"],
        plain: &["--user", "--system", "--scope", "-t", "--pty", "-P", "--pipe", "-q", "--quiet",
                 "-G", "--collect", "-r", "--remain-after-exit", "--wait", "--no-block",
                 "-d", "--same-dir"],
        operands: 0, shell: &["-S", "--shell"], joins: false },
    Wrapper { name: "unshare",
        values: &["-S", "--setuid", "-G", "--setgid", "-R", "--root", "-w", "--wd",
                  "--propagation", "--setgroups"],
        plain: &["-m", "--mount", "-u", "--uts", "-i", "--ipc", "-n", "--net", "-p", "--pid",
                 "-U", "--user", "-C", "--cgroup", "-T", "--time", "-f", "--fork", "-r",
                 "--map-root-user", "-c", "--map-current-user", "--mount-proc", "--kill-child",
                 "--keep-caps"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "fakeroot", values: &["-l", "--lib", "--faked", "-s", "-i", "-b"],
        plain: &["-u", "--unknown-is-real"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "rlwrap",
        values: &["-C", "-D", "-e", "-f", "-g", "-H", "-l", "-M", "-O", "-p", "-P", "-q", "-S", "-s",
                  "-t", "-w", "-z"],
        plain: &["-A", "-a", "-c", "-i", "-n", "-N", "-o", "-r", "-R", "-U", "-v", "-W", "-m"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "torsocks",
        values: &["-u", "--user", "-p", "--pass", "-a", "--address", "-P", "--port"],
        plain: &["-i", "--isolate", "-d", "--debug", "-q", "--quiet"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "prlimit", values: &["-p", "--pid", "-o", "--output"],
        plain: &["--noheadings", "--raw", "--verbose"], operands: 0, shell: &[], joins: false },
    Wrapper { name: "setarch",
        values: &[],
        plain: &["-R", "--addr-no-randomize", "-B", "--32bit", "-F", "--fdpic-funcptrs", "-I",
                 "--short-inode", "-L", "--addr-compat-layout", "-S", "--whole-seconds", "-T",
                 "--sticky-timeouts", "-X", "--read-implies-exec", "-Z", "--mmap-page-zero",
                 "-3", "--3gb", "-v", "--verbose"],
        operands: 1, shell: &[], joins: false },
    Wrapper { name: "sshpass", values: &["-p", "-f", "-d", "-P"], plain: &["-e", "-v"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "xcrun",
        values: &["--sdk", "--toolchain"],
        plain: &["-v", "--verbose", "-l", "--log", "-n", "--no-cache", "-k", "--kill-cache",
                 "-r", "--run"],
        operands: 0, shell: &[], joins: false },
    Wrapper { name: "watch",
        values: &["-n", "--interval", "-q", "--equexit"],
        plain: &["-d", "--differences", "-g", "--chgexit", "-e", "--errexit", "-t", "--no-title",
                 "-b", "--beep", "-c", "--color", "-C", "--no-color", "-p", "--precise", "-w",
                 "--no-wrap", "-r", "--no-rerun", "-x", "--exec"],
        operands: 0, shell: &[], joins: true },
];

/// Reads a line. Never fails: what it cannot read becomes a barrier.
pub fn read(text: &str) -> Line {
    read_at(text, 0)
}

fn read_at(text: &str, depth: usize) -> Line {
    let mut line = Line::default();
    if text.len() > MAX_LEN {
        line.barrier = Some("the command line is longer than the reader will look at".into());
        return line;
    }
    let (pieces, barrier) = tokenize(text);
    line.barrier = barrier;
    let mut words: Vec<Word> = Vec::new();
    let mut redirects = Vec::new();
    let mut heredoc = false;
    let mut piped = false;
    let mut here: Option<Word> = None;
    let mut pending: Option<Pending> = None;
    for piece in pieces {
        match piece {
            Piece::Word(w) => match pending.take() {
                Some(Pending::Target(write)) => redirects.push(Redirect {
                    target: w.text,
                    write,
                }),
                Some(Pending::Dup(write)) => {
                    // `<&3`: input this reader cannot see into.
                    heredoc |= !write;
                    if !w.text.chars().all(|c| c.is_ascii_digit() || c == '-') {
                        redirects.push(Redirect {
                            target: w.text,
                            write,
                        });
                    }
                }
                Some(Pending::Skip) => {}
                Some(Pending::Here) => here = Some(w),
                None => words.push(w),
            },
            Piece::Redirect(write) => pending = Some(Pending::Target(write)),
            Piece::Dup(write) => pending = Some(Pending::Dup(write)),
            Piece::Heredoc => {
                heredoc = true;
                pending = Some(Pending::Skip);
            }
            // `bash <<< 'rm -rf /'` runs its word.
            Piece::HereString => {
                heredoc = true;
                pending = Some(Pending::Here);
            }
            Piece::Break(pipe) => {
                finish(
                    &mut line,
                    std::mem::take(&mut words),
                    std::mem::take(&mut redirects),
                    Input {
                        fed: heredoc || piped,
                        here: here.take(),
                    },
                    depth,
                );
                heredoc = false;
                piped = pipe;
                pending = None;
            }
        }
    }
    finish(
        &mut line,
        words,
        redirects,
        Input {
            fed: heredoc || piped,
            here,
        },
        depth,
    );
    line
}

enum Pending {
    Target(bool),
    Dup(bool),
    Skip,
    Here,
}

struct Input {
    /// Stdin is a heredoc, a here-string, a descriptor or a pipe.
    fed: bool,
    here: Option<Word>,
}

const MAX_BRACE_WORDS: usize = 256;

fn finish(line: &mut Line, words: Vec<Word>, redirects: Vec<Redirect>, input: Input, depth: usize) {
    let Input { fed, here } = input;
    let mut why: Option<String> = None;
    let mut nested: Option<String> = None;
    // Brace expansion comes first: `{rm,-rf,/}` is `rm -rf /`.
    let mut unfolded: Vec<Word> = Vec::with_capacity(words.len());
    let mut budget = MAX_BRACE_WORDS;
    for w in words {
        if !w.brace || w.expands || !w.text.contains(',') {
            unfolded.push(w);
            continue;
        }
        match expand_braces(&w.text, &mut budget, 0) {
            Some(texts) => unfolded.extend(texts.into_iter().map(|text| Word {
                brace: text.contains('{'),
                text,
                ..w.clone()
            })),
            None => {
                first(
                    &mut why,
                    "a brace expansion is larger than this reader will unfold".into(),
                );
                unfolded.push(w);
            }
        }
    }
    let mut words: &[Word] = &unfolded;
    // Reserved words are skipped; the command they introduce is still read.
    while let Some(w) = words.first()
        && !w.quoted
    {
        match w.text.as_str() {
            "if" | "then" | "else" | "elif" | "while" | "until" | "do" | "!" | "{" | "}" | "fi"
            | "done" | "esac" => words = &words[1..],
            // `function name`, then the body.
            "function" => words = words.get(2..).unwrap_or(&[]),
            // `coproc cmd`, or `coproc NAME { cmd; }`.
            "coproc" => {
                words = &words[1..];
                if words.get(1).is_some_and(|b| b.text == "{" && !b.quoted) {
                    words = &words[1..];
                }
            }
            // A header names words; the body is read after `do` or `)`.
            "for" | "select" | "case" | "in" => return,
            _ => break,
        }
    }
    // `VAR=value cmd`: the assignment is not the command either.
    while let Some(w) = words.first()
        && is_assignment(&w.text)
    {
        exec_assignment(&w.text, &mut why);
        words = &words[1..];
    }
    // `x='a[$(rm -rf /)]'`: a subscript arithmetic will evaluate later —
    // in an assignment, or handed to a builtin that evaluates a subscript;
    // never in, say, a commit message.
    let evaluates = words
        .iter()
        .find(|w| !is_assignment(&w.text))
        .is_none_or(|w| SUBSCRIPTING.contains(&basename(&w.text)));
    if unfolded.iter().any(|w| {
        (w.text.contains("[$(") || w.text.contains("[`")) && (evaluates || is_assignment(&w.text))
    }) {
        first(
            &mut why,
            "a quoted command substitution sits in an array subscript, which arithmetic runs"
                .into(),
        );
    }
    let mut steps = 0;
    while let Some(w) = words.first() {
        let Some(wr) = WRAPPERS.iter().find(|x| x.name == basename(&w.text)) else {
            break;
        };
        if w.expands {
            break;
        }
        if steps >= MAX_WRAPPERS {
            first(
                &mut why,
                format!(
                    "more than {MAX_WRAPPERS} wrappers stand before the command, more than \
                     this reader follows"
                ),
            );
            words = &[];
            break;
        }
        steps += 1;
        let name = wr.name;
        let mut rest = &words[1..];
        if name == "command"
            && rest
                .first()
                .is_some_and(|a| a.text.starts_with("-v") || a.text.starts_with("-V"))
        {
            break; // `command -v x` asks about a program and runs nothing
        }
        // Flags may also follow the operands (`flock file -c …`, `setarch
        // x86_64 -R …`), so they are read again after them.
        let mut ended = false;
        for pass in 0..2 {
            while !ended && let Some(a) = rest.first() {
                let t = a.text.as_str();
                if name == "env" && (is_assignment(t) || t == "-") {
                    exec_assignment(t, &mut why);
                    rest = &rest[1..];
                    continue;
                }
                if t == "--" {
                    rest = &rest[1..];
                    ended = true;
                    break;
                }
                if !t.starts_with('-') || t.len() < 2 {
                    break;
                }
                rest = &rest[1..];
                if name == "flock" && matches!(t, "-c" | "--command") && pass == 1 {
                    // `flock file -c '…'` hands the line to `sh -c`.
                    match rest.first() {
                        Some(w) if !w.expands => nested = Some(w.text.clone()),
                        _ => {}
                    }
                    rest = &[];
                }
                if !wrapper_flag(wr, t, &mut rest, &mut why) {
                    first(
                        &mut why,
                        format!(
                            "`{name} {t}` is a flag this reader does not know, so it cannot \
                             tell where the command starts"
                        ),
                    );
                }
            }
            if pass == 0 {
                rest = rest.get(wr.operands..).unwrap_or(&[]);
                if wr.operands == 0 {
                    break;
                }
            }
        }
        if wr.joins {
            // `watch` hands its words to `sh -c`: they are a line.
            if rest.iter().any(|w| w.expands) {
                first(
                    &mut why,
                    format!("`{name}` hands a line built from variables to a shell"),
                );
            } else if !rest.is_empty() {
                nested = Some(
                    rest.iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            rest = &[];
        }
        words = rest;
    }
    let simple = match words.first() {
        None => (!redirects.is_empty()).then(|| Simple {
            redirects,
            fed,
            ..Default::default()
        }),
        Some(head) => {
            if head.expands {
                first(
                    &mut why,
                    "the program is named by a variable the shell expands".into(),
                );
            }
            // `/bin/r? -rf /`: the shell picks the program from the disk.
            if head.glob && !matches!(head.text.as_str(), "[" | "[[") {
                first(
                    &mut why,
                    "the program is named by a pattern the shell matches against the disk".into(),
                );
            }
            // zsh's `=rm` is the path of `rm`, found on `PATH`.
            if head.text.starts_with('=') && !head.quoted {
                first(
                    &mut why,
                    "the program is named by a zsh `=` expansion".into(),
                );
            }
            if head.brace && head.text.contains('{') && head.text.contains('}') {
                first(
                    &mut why,
                    "the program is named by a brace expansion this reader does not unfold".into(),
                );
            }
            let program = basename(&head.text).to_ascii_lowercase();
            let args: Vec<String> = words[1..].iter().map(|w| w.text.clone()).collect();
            let kinds: Vec<ArgKind> = words[1..].iter().map(Word::kind).collect();
            if matches!(
                program.as_str(),
                "export" | "declare" | "typeset" | "local" | "readonly"
            ) {
                for a in &args {
                    exec_assignment(a, &mut why);
                }
            }
            if program == "git" {
                let mut it = words[1..].iter();
                while let Some(a) = it.next() {
                    if a.text == "--config-env" || a.text.starts_with("--config-env=") {
                        first(
                            &mut why,
                            "`git --config-env` takes a setting from the environment".into(),
                        );
                    }
                    let pair = if a.text == "-c" {
                        it.next().map(|v| v.text.as_str())
                    } else {
                        None
                    };
                    if let Some(pair) = pair
                        && git_config_runs(pair)
                    {
                        first(&mut why, format!("`git -c {pair}` sets a command git runs"));
                    }
                }
            }
            let literal = |i: usize| {
                words
                    .get(i + 1)
                    .filter(|w| !w.expands)
                    .map(|w| w.text.as_str())
            };
            let p = program.as_str();
            // `sh < x.sh` runs the file as surely as `sh x.sh` does.
            let reads_file = redirects.iter().any(|r| !r.write);
            if INTERPRETERS.contains(&p) {
                if fed || reads_file || !args.iter().all(|a| HARMLESS.contains(&a.as_str())) {
                    first(
                        &mut why,
                        format!("`{p}` runs code given on its command line or its input"),
                    );
                }
                if SHELLS.contains(&p)
                    && let Some(h) = here.as_ref().filter(|h| !h.expands)
                {
                    // `bash <<< '…'`: the here-string is the program.
                    nested = Some(h.text.clone());
                }
                if SHELLS.contains(&p) && nested.is_none() {
                    // `sh -c '…'` with the code in the clear is a line like any other.
                    nested = args.iter().enumerate().find_map(|(i, a)| {
                        let cluster = a.strip_prefix('-').filter(|c| !c.starts_with('-'))?;
                        let at = cluster.find('c')?;
                        match &cluster[at + 1..] {
                            "" => literal(i + 1).map(str::to_string),
                            glued if cluster[..at].chars().all(|c| c.is_ascii_alphabetic()) => {
                                Some(glued.to_string())
                            }
                            _ => None,
                        }
                    });
                }
            } else if BUILDERS.contains(&p)
                || (matches!(p, "mapfile" | "readarray")
                    && args.iter().any(|a| a.starts_with("-C")))
            {
                first(
                    &mut why,
                    format!("`{p}` runs a command line built from its arguments or input"),
                );
                if p == "eval" && words[1..].iter().all(|w| !w.expands) {
                    nested = Some(args.join(" "));
                }
            } else if ELSEWHERE.contains(&p) {
                first(
                    &mut why,
                    format!("`{p}` runs a command somewhere this reader cannot see"),
                );
                if matches!(p, "su" | "script")
                    && let Some(i) = args.iter().position(|a| {
                        a == "-c"
                            || a == "--command"
                            || (a.starts_with('-') && !a.starts_with("--") && a.ends_with('c'))
                    })
                {
                    nested = literal(i + 1).map(str::to_string);
                }
            } else if (p == "find"
                && args.iter().any(|a| {
                    matches!(
                        a.as_str(),
                        "-exec" | "-execdir" | "-ok" | "-okdir" | "-delete"
                    )
                }))
                || (p == "fd"
                    && args
                        .iter()
                        .any(|a| matches!(a.as_str(), "-x" | "--exec" | "-X" | "--exec-batch")))
            {
                first(
                    &mut why,
                    format!("`{p}` runs a command for each file it finds"),
                );
            } else if matches!(p, "docker" | "podman" | "kubectl" | "oc" | "nerdctl")
                && args
                    .iter()
                    .any(|a| matches!(a.as_str(), "run" | "exec" | "compose"))
            {
                first(&mut why, format!("`{p}` runs a command inside a container"));
            } else if !TEXT_TOOLS.contains(&p)
                && let Some(i) = args.iter().position(|a| {
                    let (flag, _) = a.split_once('=').unwrap_or((a, ""));
                    matches!(
                        flag,
                        "-c" | "-e" | "-E" | "-x" | "--eval" | "--exec" | "--command"
                    )
                })
            {
                let value = args[i]
                    .split_once('=')
                    .map(|(_, v)| v)
                    .or_else(|| args.get(i + 1).map(String::as_str));
                if value.is_some_and(|v| v.contains(char::is_whitespace)) {
                    first(
                        &mut why,
                        format!("`{p} {}` is given a command line to run", args[i]),
                    );
                }
            }
            if fed && !SINKS.contains(&p) && !INTERPRETERS.contains(&p) {
                first(
                    &mut why,
                    format!("`{p}` is fed input this reader cannot see it use"),
                );
            }
            Some(Simple {
                program,
                args,
                redirects,
                fed,
                kinds,
            })
        }
    };
    if line.barrier.is_none() {
        line.barrier = why;
    }
    line.commands.extend(simple);
    if let Some(code) = nested {
        if depth < MAX_DEPTH {
            nest(line, &code, depth);
        } else if line.barrier.is_none() {
            line.barrier = Some("a command line is nested deeper than this reader follows".into());
        }
    }
}

/// Consumes one wrapper flag and its value; `false` for an unknown flag.
fn wrapper_flag(wr: &Wrapper, t: &str, rest: &mut &[Word], why: &mut Option<String>) -> bool {
    let name = wr.name;
    let (flag, glued) = match t.split_once('=') {
        Some((f, _)) if f.starts_with("--") => (f, true),
        _ => (t, false),
    };
    let shell = |f: &str, why: &mut Option<String>| {
        if wr.shell.contains(&f) {
            first(why, format!("`{name} {t}` starts a shell of its own"));
        }
    };
    shell(flag, why);
    // These take every long option glued (`--tool=memcheck`, `--nofile=64`).
    if glued && matches!(name, "valgrind" | "firejail" | "prlimit") {
        return true;
    }
    if wr.values.contains(&flag) {
        if !glued {
            *rest = rest.get(1..).unwrap_or(&[]);
        }
        return true;
    }
    if wr.plain.contains(&flag) || wr.shell.contains(&flag) {
        return true;
    }
    if name == "nice" && flag[1..].chars().all(|c| c.is_ascii_digit()) {
        return true; // `nice -5 cmd`
    }
    if flag.starts_with("--") {
        return false;
    }
    // A bundle of short flags: `-Eu root`, `-o0`, `-qc`.
    let letters: Vec<char> = flag[1..].chars().collect();
    for (i, c) in letters.iter().enumerate() {
        let one = format!("-{c}");
        shell(&one, why);
        if wr.values.contains(&one.as_str()) {
            if i + 1 == letters.len() {
                *rest = rest.get(1..).unwrap_or(&[]);
            }
            return true; // the rest of the bundle is the value
        }
        if !wr.plain.contains(&one.as_str()) && !wr.shell.contains(&one.as_str()) {
            return false;
        }
    }
    true
}

/// Unfolds `a{b,c}d` into `abd acd`; `None` past the budget.
fn expand_braces(s: &str, budget: &mut usize, depth: usize) -> Option<Vec<String>> {
    if depth > 16 {
        return None;
    }
    let chars: Vec<char> = s.chars().collect();
    for open in 0..chars.len() {
        if chars[open] != '{' {
            continue;
        }
        let (mut level, mut commas, mut close) = (0usize, Vec::new(), None);
        for (j, c) in chars.iter().enumerate().skip(open) {
            match c {
                '{' => level += 1,
                '}' => {
                    level -= 1;
                    if level == 0 {
                        close = Some(j);
                        break;
                    }
                }
                ',' if level == 1 => commas.push(j),
                _ => {}
            }
        }
        let Some(close) = close else { continue };
        if commas.is_empty() {
            continue;
        }
        let prefix: String = chars[..open].iter().collect();
        let suffix: String = chars[close + 1..].iter().collect();
        let mut out = Vec::new();
        let mut start = open + 1;
        for end in commas.iter().copied().chain(std::iter::once(close)) {
            let part: String = chars[start..end].iter().collect();
            start = end + 1;
            for e in expand_braces(&format!("{prefix}{part}{suffix}"), budget, depth + 1)? {
                *budget = budget.checked_sub(1)?;
                out.push(e);
            }
        }
        return Some(out);
    }
    Some(vec![s.to_string()])
}

fn first(slot: &mut Option<String>, why: String) {
    if slot.is_none() {
        *slot = Some(why);
    }
}

fn nest(line: &mut Line, code: &str, depth: usize) {
    let inner = read_at(code, depth + 1);
    line.commands.extend(inner.commands);
    if line.barrier.is_none() {
        line.barrier = inner.barrier;
    }
}

/// An assignment to a variable whose value runs is a barrier.
fn exec_assignment(word: &str, why: &mut Option<String>) {
    if !is_assignment(word) {
        return;
    }
    let name = word.split_once('=').map_or(word, |(n, _)| n);
    let name = name.strip_suffix('+').unwrap_or(name);
    if runs_its_value(name) {
        first(
            why,
            format!("`{name}` names a command or library something runs later"),
        );
    }
}

fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let name = name.strip_suffix('+').unwrap_or(name);
    !name.is_empty()
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

impl Word {
    fn kind(&self) -> ArgKind {
        if self.expands || (self.brace && self.text.contains('{') && self.text.contains('}')) {
            ArgKind::Unknown
        } else if self.glob {
            ArgKind::Glob
        } else {
            ArgKind::Literal
        }
    }
}

#[derive(Debug, Default, Clone)]
struct Word {
    text: String,
    /// Contains a `$` the shell expands.
    expands: bool,
    /// Partly quoted, so not a reserved word.
    quoted: bool,
    /// An unquoted `*`, `?` or `[`.
    glob: bool,
    /// An unquoted `{`.
    brace: bool,
}

enum Piece {
    Word(Word),
    /// `>`, `>>`, `<`.
    Redirect(bool),
    /// `>&`, `<&`: a descriptor or a file follows.
    Dup(bool),
    Heredoc,
    HereString,
    /// End of a simple command; `true` when the next one reads this one's output.
    Break(bool),
}

fn tokenize(text: &str) -> (Vec<Piece>, Option<String>) {
    let b: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut barrier: Option<String> = None;
    let note = |b: &mut Option<String>, why: &str| {
        if b.is_none() {
            *b = Some(why.to_string());
        }
    };
    let mut cur = Word::default();
    let mut open = false;
    // Each heredoc opened on this line: its delimiter, and whether `<<-`
    // strips leading tabs from the body's lines.
    let mut delimiters: Vec<(String, bool)> = Vec::new();
    let mut want_delimiter: Option<bool> = None;
    let mut i = 0;
    let flush = |cur: &mut Word,
                 open: &mut bool,
                 out: &mut Vec<Piece>,
                 want: &mut Option<bool>,
                 delims: &mut Vec<(String, bool)>| {
        if *open {
            let w = std::mem::take(cur);
            if let Some(dash) = want.take() {
                delims.push((w.text.clone(), dash));
            }
            out.push(Piece::Word(w));
            *open = false;
        }
    };
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        match c {
            '\'' => {
                open = true;
                cur.quoted = true;
                let Some(end) = b[i + 1..].iter().position(|x| *x == '\'') else {
                    note(&mut barrier, "a quote is never closed");
                    cur.text.extend(&b[i + 1..]);
                    i = b.len();
                    continue;
                };
                cur.text.extend(&b[i + 1..i + 1 + end]);
                i += end + 2;
            }
            '"' => {
                open = true;
                cur.quoted = true;
                i += 1;
                let mut closed = false;
                while i < b.len() {
                    match b[i] {
                        '"' => {
                            closed = true;
                            i += 1;
                            break;
                        }
                        '\\' if matches!(b.get(i + 1), Some('"' | '\\' | '$' | '`')) => {
                            cur.text.push(b[i + 1]);
                            i += 2;
                        }
                        '`' => {
                            note(
                                &mut barrier,
                                "a command substitution builds part of this line",
                            );
                            cur.text.push('`');
                            i += 1;
                        }
                        '$' => {
                            if b.get(i + 1) == Some(&'(') {
                                note(
                                    &mut barrier,
                                    "a command substitution builds part of this line",
                                );
                            }
                            cur.expands = true;
                            cur.text.push('$');
                            i += 1;
                        }
                        x => {
                            cur.text.push(x);
                            i += 1;
                        }
                    }
                }
                if !closed {
                    note(&mut barrier, "a quote is never closed");
                }
            }
            '\\' => {
                match next {
                    Some('\n') => {}
                    Some(x) => {
                        open = true;
                        cur.quoted = true;
                        cur.text.push(x);
                    }
                    None => {
                        open = true;
                        cur.text.push('\\');
                    }
                }
                i += 2;
            }
            '$' if next == Some('\'') => {
                note(&mut barrier, "`$'…'` quoting is not decoded by this reader");
                open = true;
                cur.quoted = true;
                let end = b[i + 2..]
                    .iter()
                    .position(|x| *x == '\'')
                    .map(|e| i + 2 + e)
                    .unwrap_or(b.len());
                cur.text.extend(&b[i..end.min(b.len())]);
                i = end + 1;
            }
            '$' if next == Some('(')
                && b.get(i + 2) == Some(&'(')
                && arithmetic_end(&b, i + 3).is_some() =>
            {
                // `$((…))` is arithmetic: a number, unless it hides a command.
                // `$((cmd) )` is not: it falls to the command substitution arm.
                let end = arithmetic_end(&b, i + 3).unwrap_or(b.len());
                let body: String = b[i + 3..end.min(b.len())].iter().collect();
                if body.contains("$(") || body.contains('`') || body.contains('[') {
                    note(&mut barrier, "an arithmetic expansion can run a command");
                }
                open = true;
                cur.expands = true;
                cur.text.extend(&b[i..(end + 2).min(b.len())]);
                i = end + 2;
            }
            '(' if !open && next == Some('(') && arithmetic_end(&b, i + 2).is_some() => {
                // `(( … ))`: arithmetic, where `<<` is a shift and no heredoc.
                // `((cmd) )` and `((a);(b))` are not: the shell re-reads them
                // as nested subshells, and so does the group arm below.
                let end = arithmetic_end(&b, i + 2).unwrap_or(b.len());
                let body: String = b[i + 2..end.min(b.len())].iter().collect();
                if body.contains("$(") || body.contains('`') || body.contains('[') {
                    note(&mut barrier, "an arithmetic command can run a command");
                }
                out.push(Piece::Break(false));
                i = end + 2;
            }
            '(' if open && cur.glob && !cur.quoted => {
                // zsh: `*(e:'…':)` is a glob qualifier, and `e` runs code.
                note(
                    &mut barrier,
                    "a zsh glob qualifier can run a command for each file",
                );
                flush(
                    &mut cur,
                    &mut open,
                    &mut out,
                    &mut want_delimiter,
                    &mut delimiters,
                );
                out.push(Piece::Break(false));
                i += 1;
            }
            '$' => {
                if next == Some('(') {
                    note(
                        &mut barrier,
                        "a command substitution builds part of this line",
                    );
                    i += 1; // the `(` opens a group like any other
                    continue;
                }
                open = true;
                cur.expands = true;
                cur.text.push('$');
                i += 1;
            }
            '`' => {
                note(
                    &mut barrier,
                    "a command substitution builds part of this line",
                );
                flush(
                    &mut cur,
                    &mut open,
                    &mut out,
                    &mut want_delimiter,
                    &mut delimiters,
                );
                out.push(Piece::Break(false));
                i += 1;
            }
            '#' if !open => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            ' ' | '\t' => {
                flush(
                    &mut cur,
                    &mut open,
                    &mut out,
                    &mut want_delimiter,
                    &mut delimiters,
                );
                i += 1;
            }
            '\n' => {
                flush(
                    &mut cur,
                    &mut open,
                    &mut out,
                    &mut want_delimiter,
                    &mut delimiters,
                );
                out.push(Piece::Break(false));
                i += 1;
                // The bodies of every heredoc opened on the line just ended.
                for (d, dash) in delimiters.drain(..) {
                    loop {
                        let end = b[i..]
                            .iter()
                            .position(|x| *x == '\n')
                            .map(|e| i + e)
                            .unwrap_or(b.len());
                        let row: String = b[i..end].iter().collect();
                        i = (end + 1).min(b.len());
                        let row = if dash {
                            row.trim_start_matches('\t')
                        } else {
                            row.as_str()
                        };
                        if row == d || i >= b.len() {
                            break;
                        }
                    }
                }
            }
            '<' if next == Some('(') => {
                note(
                    &mut barrier,
                    "a process substitution runs a command for its output",
                );
                i += 1;
            }
            '>' if next == Some('(') => {
                note(
                    &mut barrier,
                    "a process substitution runs a command for its input",
                );
                i += 1;
            }
            '<' | '>' | '&' | '|' | ';' | '(' | ')' => {
                // `2>`, `2>&1`: a descriptor number glued to the operator.
                if open
                    && cur.text.chars().all(|x| x.is_ascii_digit())
                    && !cur.quoted
                    && matches!(c, '<' | '>')
                {
                    cur = Word::default();
                    open = false;
                }
                flush(
                    &mut cur,
                    &mut open,
                    &mut out,
                    &mut want_delimiter,
                    &mut delimiters,
                );
                let rest: String = b[i..(i + 3).min(b.len())].iter().collect();
                let (piece, len) = if rest.starts_with("<<<") {
                    (Piece::HereString, 3)
                } else if rest.starts_with("<<-") || rest.starts_with("<<") {
                    want_delimiter = Some(rest.starts_with("<<-"));
                    (Piece::Heredoc, if rest.starts_with("<<-") { 3 } else { 2 })
                } else if rest.starts_with("&>>") {
                    (Piece::Redirect(true), 3)
                } else if rest.starts_with(">>") || rest.starts_with("&>") || rest.starts_with(">|")
                {
                    (Piece::Redirect(true), 2)
                } else if rest.starts_with(">&") {
                    (Piece::Dup(true), 2)
                } else if rest.starts_with("<&") {
                    (Piece::Dup(false), 2)
                } else if rest.starts_with("&&") || rest.starts_with("||") || rest.starts_with(";;")
                {
                    (Piece::Break(false), 2)
                } else if rest.starts_with("|&") {
                    (Piece::Break(true), 2)
                } else {
                    match c {
                        '>' => (Piece::Redirect(true), 1),
                        '<' => (Piece::Redirect(false), 1),
                        '|' => (Piece::Break(true), 1),
                        _ => (Piece::Break(false), 1),
                    }
                };
                out.push(piece);
                i += len;
            }
            x => {
                open = true;
                cur.glob |= matches!(x, '*' | '?' | '[');
                cur.brace |= x == '{';
                cur.text.push(x);
                i += 1;
            }
        }
    }
    flush(
        &mut cur,
        &mut open,
        &mut out,
        &mut want_delimiter,
        &mut delimiters,
    );
    (out, barrier)
}

/// Where the `))` closing arithmetic that starts at `from` is. `None` when
/// the body is not arithmetic: its first `)` at depth 0 is not followed by
/// another `)` (bash and zsh then re-read `((` as two subshells), or it never
/// closes.
fn arithmetic_end(b: &[char], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = from;
    while i < b.len() {
        match b[i] {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ')' if b.get(i + 1) == Some(&')') => return Some(i),
            ')' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    /// Named as an argument of a program this reader does not know the use
    /// of: a restrictive `Read` treats it as read, `Edit` as a question.
    Named,
}

/// How the line names the file. As the vendor does, a `Read` prohibition
/// reaches a recognised command's write but not a redirection's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Redirect,
    Command,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTarget {
    pub path: String,
    pub access: Access,
    pub via: Via,
    /// A `~`, a glob or a variable: not one nameable file.
    pub unresolvable: bool,
    /// `rm -r`, `grep -r`: everything under the path.
    pub subtree: bool,
}

pub struct Targets {
    pub list: Vec<FileTarget>,
}

const READERS: &[&str] = &[
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "wc",
    "sort",
    "uniq",
    "cut",
    "tr",
    "diff",
    "cmp",
    "file",
    "stat",
    "strings",
    "hexdump",
    "xxd",
    "od",
    "md5sum",
    "sha1sum",
    "sha256sum",
    "shasum",
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "ag",
    "ack",
];
/// Readers whose first operand is a pattern rather than a file.
const PATTERN_FIRST: &[&str] = &["grep", "egrep", "fgrep", "rg", "ag", "ack"];
const WRITERS: &[&str] = &[
    "tee", "touch", "truncate", "mkdir", "rmdir", "rm", "shred", "unlink",
];
/// The last operand is written, the rest are read.
const COPIERS: &[&str] = &["cp", "mv", "install", "rsync", "ln"];
/// The first operand is a mode or an owner; the rest are written.
const CHANGERS: &[&str] = &["chmod", "chown", "chgrp"];

pub fn file_targets(text: &str) -> Targets {
    let line = read(text);
    Targets {
        list: targets_of(&line),
    }
}

/// Whether a program this reader does not see through may run one of its
/// words as a command (`unbuffer rm -rf /`): anything not known to only print,
/// test or match text.
pub(crate) fn may_run_a_word(program: &str) -> bool {
    !program.is_empty() && !INERT.contains(&program) && !TEXT_TOOLS.contains(&program)
}

/// Programs that never open the files their arguments name.
const INERT: &[&str] = &[
    "echo", "printf", "ls", "test", "[", "[[", "cd", "pushd", "which", "type",
];

pub(crate) fn targets_of(line: &Line) -> Vec<FileTarget> {
    line.commands.iter().flat_map(command_targets).collect()
}

/// The files one simple command names, and how.
pub(crate) fn command_targets(cmd: &Simple) -> Vec<FileTarget> {
    let mut out = Vec::new();
    let mut push = |path: &str, access: Access, via: Via, subtree: bool, unknown: bool| {
        if path.is_empty() || path == "/dev/null" || path == "-" {
            return;
        }
        let unresolvable = unknown || path.starts_with('~') || path.contains(['$', '*', '?', '[']);
        out.push(FileTarget {
            path: path.to_string(),
            access,
            via,
            unresolvable,
            subtree,
        });
    };
    {
        for r in &cmd.redirects {
            let access = if r.write { Access::Write } else { Access::Read };
            push(&r.target, access, Via::Redirect, false, false);
        }
        let p = cmd.program.as_str();
        let unknown = |i: usize| cmd.kinds.get(i) == Some(&ArgKind::Unknown);
        let recursive = cmd.args.iter().any(|a| {
            a == "--recursive"
                || (a.starts_with('-') && !a.starts_with("--") && a.contains(['r', 'R']))
        });
        let operands: Vec<(usize, &String)> = {
            let mut seen_end = false;
            cmd.args
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    if seen_end {
                        return true;
                    }
                    if *a == "--" {
                        seen_end = true;
                        return false;
                    }
                    !a.starts_with('-') || a.len() == 1
                })
                .collect()
        };
        if p == "dd" {
            for (i, a) in cmd.args.iter().enumerate() {
                if let Some(f) = a.strip_prefix("if=") {
                    push(f, Access::Read, Via::Command, false, unknown(i));
                }
                if let Some(f) = a.strip_prefix("of=") {
                    push(f, Access::Write, Via::Command, false, unknown(i));
                }
            }
        } else if READERS.contains(&p) {
            let skip = usize::from(
                PATTERN_FIRST.contains(&p)
                    && !cmd.args.iter().any(|a| a == "-e" || a == "--regexp"),
            );
            for (i, a) in operands.iter().skip(skip) {
                push(a, Access::Read, Via::Command, recursive, unknown(*i));
            }
        } else if WRITERS.contains(&p) {
            for (i, a) in &operands {
                push(
                    a,
                    Access::Write,
                    Via::Command,
                    recursive && p == "rm",
                    unknown(*i),
                );
            }
        } else if COPIERS.contains(&p) {
            // `mv` removes its sources, and so does `rsync --remove-source-files`:
            // each source is written, whole directories included.
            let removes = p == "mv"
                || (p == "rsync" && cmd.args.iter().any(|a| a == "--remove-source-files"));
            if let Some(((li, last), rest)) = operands.split_last() {
                for (i, a) in rest {
                    if removes {
                        push(a, Access::Write, Via::Command, true, unknown(*i));
                    } else {
                        push(a, Access::Read, Via::Command, recursive, unknown(*i));
                    }
                }
                push(last, Access::Write, Via::Command, false, unknown(*li));
            }
        } else if CHANGERS.contains(&p) {
            for (i, a) in operands.iter().skip(1) {
                push(a, Access::Write, Via::Command, recursive, unknown(*i));
            }
        } else if !p.is_empty() && !INERT.contains(&p) {
            // A program whose use of its arguments is unknown (`base64 .env`,
            // `git show HEAD:.env`, `curl -d @.env`): every spelling of a
            // path in an argument is named, never guessed harmless.
            for (i, a) in cmd.args.iter().enumerate() {
                for form in named_forms(a) {
                    push(form, Access::Named, Via::Command, recursive, unknown(i));
                }
            }
        }
    }
    out
}

/// The spellings of a path an argument may carry: itself, `--flag=path`,
/// `@path` and `rev:path`.
fn named_forms(arg: &str) -> Vec<&str> {
    let mut out = Vec::new();
    if !arg.starts_with('-') {
        out.push(arg);
    }
    if let Some((_, v)) = arg.split_once('=') {
        out.push(v);
    }
    if let Some(v) = arg.strip_prefix('@') {
        out.push(v);
    }
    if let Some((_, v)) = arg.split_once(':')
        && !arg.contains("://")
    {
        out.push(v);
    }
    out
}

/// The family a call belongs to, which one rule could cover: program and
/// subcommand for a shell, directory for a path, host for a URL.
pub fn rule_family(tool: &str, content: &str) -> String {
    if tool.eq_ignore_ascii_case("Read") || tool.eq_ignore_ascii_case("Edit") {
        return match content.rsplit_once('/') {
            Some((dir, _)) if !dir.is_empty() => dir.to_string(),
            _ => content.to_string(),
        };
    }
    if tool.eq_ignore_ascii_case("WebFetch") {
        return crate::core::policy::host_of(content).unwrap_or_else(|| content.to_string());
    }
    if !crate::core::policy::is_command_tool(tool) {
        return content.to_string();
    }
    match read(content).commands.first() {
        Some(c) => match subcommand(c) {
            Some(sub) => format!("{} {sub}", c.program),
            None => c.program.clone(),
        },
        None => content.to_string(),
    }
}

fn subcommand(c: &Simple) -> Option<&str> {
    c.args
        .first()
        .filter(|a| !a.starts_with('-'))
        .map(String::as_str)
}

/// The narrowest specifier covering every call, or `None` when no one rule
/// should: a barrier, a compound, or calls that share no head.
pub fn suggest_rule_for(tool: &str, calls: &[String]) -> Option<String> {
    if crate::core::policy::is_command_tool(tool) {
        let mut distinct: Vec<&str> = Vec::new();
        for c in calls {
            let t = c.trim();
            if t.is_empty() {
                return None;
            }
            if !distinct.contains(&t) {
                distinct.push(t);
            }
        }
        let read: Vec<Line> = distinct.iter().map(|c| read(c)).collect();
        if read
            .iter()
            .any(|l| l.barrier.is_some() || l.commands.len() != 1)
        {
            return None;
        }
        if distinct.len() == 1 {
            return Some(distinct[0].to_string());
        }
        let heads: Vec<&Simple> = read.iter().map(|l| &l.commands[0]).collect();
        let program = &heads[0].program;
        if heads.iter().any(|h| &h.program != program) {
            return None;
        }
        return match subcommand(heads[0]) {
            Some(sub) if heads.iter().all(|h| subcommand(h) == Some(sub)) => {
                Some(format!("{program} {sub} *"))
            }
            None if heads.iter().all(|h| h.args.is_empty()) => Some(format!("{program} *")),
            _ => None,
        };
    }
    if tool.eq_ignore_ascii_case("Read") || tool.eq_ignore_ascii_case("Edit") {
        let family = rule_family(tool, calls.first()?);
        if calls.iter().any(|c| rule_family(tool, c) != family) {
            return None;
        }
        return Some(
            if family.contains('/') || calls.iter().any(|c| c.contains('/')) {
                format!("{family}/**")
            } else {
                family
            },
        );
    }
    if tool.eq_ignore_ascii_case("WebFetch") {
        let host = crate::core::policy::host_of(calls.first()?)?;
        return calls
            .iter()
            .all(|c| crate::core::policy::host_of(c).as_deref() == Some(host.as_str()))
            .then(|| format!("domain:{host}"));
    }
    let first = calls.first()?;
    calls.iter().all(|c| c == first).then(|| first.clone())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_text_tools_e_flag_is_not_a_command_line() {
        for line in ["sed -e 's/a b/c/' f", "grep -e 'a b' f", "echo -e 'a\\tb'"] {
            let read = super::read(line);
            assert!(read.barrier.is_none(), "{line}: {:?}", read.barrier);
        }
        let read = super::read("weird -e 'rm -rf /'");
        assert!(
            read.barrier.is_some(),
            "an unknown program given a command line"
        );
    }

    use super::*;

    fn programs(text: &str) -> Vec<String> {
        read(text)
            .commands
            .iter()
            .map(|c| c.program.clone())
            .collect()
    }

    #[test]
    fn a_line_is_split_on_every_operator() {
        assert_eq!(
            programs("a; b && c || d | e & f\ng"),
            ["a", "b", "c", "d", "e", "f", "g"]
        );
        assert_eq!(programs("(a; b) && { c; }"), ["a", "b", "c"]);
        assert_eq!(programs("if x; then y; else z; fi"), ["x", "y", "z"]);
        assert_eq!(programs("for f in a b; do rm $f; done"), ["rm"]);
    }

    #[test]
    fn quotes_keep_a_word_together_and_a_backslash_is_an_escape() {
        let l = read(r#"echo "a; rm -rf x" 'b c' d\ e \rm"#);
        assert_eq!(l.commands.len(), 1);
        assert_eq!(l.commands[0].args, ["a; rm -rf x", "b c", "d e", "rm"]);
        assert!(l.barrier.is_none());
        assert_eq!(programs(r#"\rm -rf /"#), ["rm"]);
    }

    #[test]
    fn wrappers_and_assignments_are_looked_through() {
        for line in [
            "sudo rm -rf /",
            "sudo -u root -E rm -rf /",
            "doas rm -rf /",
            "env -i FOO=1 rm -rf /",
            "exec rm -rf /",
            "exec -a x rm -rf /",
            "command rm -rf /",
            "nohup nice -n 5 rm -rf /",
            "timeout -s KILL 5 rm -rf /",
            "stdbuf -o0 rm -rf /",
            "flock /tmp/l rm -rf /",
            "busybox rm -rf /",
            "FOO=bar BAR=baz /bin/rm -rf /",
            "RM -rf /",
            "time rm -rf /",
        ] {
            let l = read(line);
            assert_eq!(l.commands.len(), 1, "{line}");
            assert_eq!(l.commands[0].program, "rm", "{line}");
            assert_eq!(l.commands[0].args, ["-rf", "/"], "{line}");
            assert!(l.barrier.is_none(), "{line}: {:?}", l.barrier);
        }
        assert_eq!(programs("command -v rm"), ["command"]);
    }

    #[test]
    fn what_cannot_be_read_is_a_barrier_and_never_silence() {
        for line in [
            "$(echo rm) -rf /",
            "`echo rm` -rf /",
            "cat <(rm -rf /)",
            "rm -rf 'unterminated",
            "$CMD -rf /",
            "${CMD} -rf /",
            "sh -c 'rm -rf /'",
            "bash x.sh",
            "python x.py",
            "node --eval 'x'",
            "awk 'BEGIN{system(\"rm -rf /\")}'",
            "echo / | xargs -0 rm -rf",
            "xargs -I{} rm -rf {}",
            "eval \"$CMD\"",
            "env -S 'rm -rf /'",
            "su -c 'rm -rf /'",
            "sudo -s",
            "ssh h rm -rf /",
            "find . -exec rm {} \\;",
            "find . -delete",
            "docker run x rm -rf /",
            "kubectl exec p -- rm -rf /",
            "git -c core.pager='rm -rf /' log",
            "sh <<EOF\nrm -rf /\nEOF",
            "echo x | sh",
            "cat $'\\x2e\\x65nv'",
            "source x.sh",
            ". x.sh",
        ] {
            assert!(read(line).barrier.is_some(), "{line}");
        }
        for line in [
            "cat <<EOF\nhello\nEOF",
            "echo \"a; rm -rf x\"",
            "git log",
            "python --version",
            "cargo test",
        ] {
            assert!(read(line).barrier.is_none(), "{line}");
        }
    }

    /// What a reserved word, pattern, brace, here-string or wrapper flag hides
    /// is either read or a barrier.
    #[test]
    fn nothing_the_shell_unfolds_hides_a_command() {
        let runs_rm = |line: &str| {
            read(line)
                .commands
                .iter()
                .any(|c| c.program == "rm" && c.args.iter().any(|a| a == "-rf"))
        };
        for line in [
            "function f { rm -rf /; }; f",
            "coproc rm -rf /",
            "coproc NAME { rm -rf /; }",
            "{rm,-rf,/}",
            "bash <<< 'rm -rf /'",
            "builtin rm -rf /",
            "sudo --user root rm -rf /",
            "sudo --user=root rm -rf /",
            "sudo -Eu root rm -rf /",
            "sudo -R /tmp rm -rf /",
            "env -P /bin rm -rf /",
            "env - rm -rf /",
            "/usr/bin/time -o f rm -rf /",
            "caffeinate -i rm -rf /",
            "arch -arm64 rm -rf /",
            "strace -f -o log rm -rf /",
            "watch rm -rf /",
            "watch -n 1 'rm -rf /'",
            "script -qc 'rm -rf /' /dev/null",
            "nice -5 rm -rf /",
        ] {
            assert!(runs_rm(line), "{line}: {:?}", read(line));
        }
        for line in [
            "/bin/r? -rf /",
            "/bin/r[m] -rf /",
            "/bin/*m -rf /",
            "/bin/{r..r}m -rf /",
            "sh < x.sh",
            "bash <<< \"$CMD\"",
            "sh <&3",
            "sudo --made-up-flag x rm -rf /",
            "script -q /dev/null rm -rf /",
        ] {
            assert!(read(line).barrier.is_some(), "{line}: {:?}", read(line));
        }
        // And nothing ordinary became a question.
        for line in [
            "[ -f x ] && echo y",
            "mkdir -p src/{a,b}",
            "cp file{,.bak}",
            "find . -name '*.rs'",
            "grep x < file",
            "cat <<< hello",
            "sudo -E ls",
        ] {
            assert!(read(line).barrier.is_none(), "{line}: {:?}", read(line));
        }
        assert_eq!(
            read("mkdir -p src/{a,b}").commands[0].args,
            ["-p", "src/a", "src/b"]
        );
        let huge = "x{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}{a,b}";
        assert!(
            read(huge).barrier.is_some(),
            "an unbounded unfolding is refused"
        );
    }

    #[test]
    fn code_given_to_a_shell_in_the_clear_is_read_as_well() {
        for line in [
            "sh -lc 'rm -rf /'",
            "bash -ec 'rm -rf /'",
            "bash -c'rm -rf /'",
            "eval 'rm -rf /'",
            "su -c 'rm -rf /' root",
            "sudo bash -c \"rm -rf /\"",
        ] {
            let l = read(line);
            assert!(l.barrier.is_some(), "{line}");
            assert!(
                l.commands
                    .iter()
                    .any(|c| c.program == "rm" && c.args == ["-rf", "/"]),
                "{line}: {l:?}"
            );
        }
    }

    #[test]
    fn a_heredoc_body_is_data_and_its_delimiter_ends_it() {
        let l = read("cat <<EOF > out\nrm -rf /\nEOF\nls");
        assert_eq!(
            l.commands
                .iter()
                .map(|c| c.program.as_str())
                .collect::<Vec<_>>(),
            ["cat", "ls"]
        );
        assert!(l.commands[0].fed);
        assert_eq!(
            l.commands[0].redirects,
            [Redirect {
                target: "out".into(),
                write: true
            }]
        );
        assert!(l.barrier.is_none());
    }

    #[test]
    fn redirections_and_file_commands_name_their_files() {
        let t = |s: &str| {
            file_targets(s)
                .list
                .into_iter()
                .map(|t| (t.path, t.access, t.via))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            t("echo x > .env 2>&1"),
            [(".env".to_string(), Access::Write, Via::Redirect)]
        );
        assert_eq!(
            t("cat .env"),
            [(".env".to_string(), Access::Read, Via::Command)]
        );
        assert_eq!(
            t("echo x | tee -a .env"),
            [(".env".to_string(), Access::Write, Via::Command)]
        );
        assert_eq!(
            t("grep -r secret src"),
            [("src".to_string(), Access::Read, Via::Command)]
        );
        assert!(file_targets("grep -r secret src").list[0].subtree);
        assert_eq!(
            t("cp a b"),
            [
                ("a".to_string(), Access::Read, Via::Command),
                ("b".to_string(), Access::Write, Via::Command)
            ]
        );
        assert_eq!(
            t("dd if=a of=b"),
            [
                ("a".to_string(), Access::Read, Via::Command),
                ("b".to_string(), Access::Write, Via::Command)
            ]
        );
        assert!(file_targets("cat ~/.ssh/id_rsa").list[0].unresolvable);
        assert!(file_targets("cat *.env").list[0].unresolvable);
    }

    #[test]
    fn a_family_and_a_suggested_rule_come_from_the_tokens() {
        assert_eq!(rule_family("Bash", "cargo test --lib x"), "cargo test");
        assert_eq!(rule_family("Bash", "sudo ls -la"), "ls");
        assert_eq!(rule_family("Read", "src/main.rs"), "src");
        assert_eq!(rule_family("WebFetch", "https://docs.rs/x"), "docs.rs");
        let s = |c: &[&str]| {
            suggest_rule_for("Bash", &c.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        assert_eq!(
            s(&["cargo test --lib a", "cargo test --doc"]).as_deref(),
            Some("cargo test *")
        );
        assert_eq!(s(&["ls", "ls"]).as_deref(), Some("ls"));
        assert_eq!(s(&["git status", "git push"]), None);
        assert_eq!(s(&["a && b"]), None);
        assert_eq!(s(&["sh -c x"]), None);
        assert_eq!(
            suggest_rule_for("Read", &["src/a.rs".into(), "src/b.rs".into()]).as_deref(),
            Some("src/**")
        );
        assert_eq!(
            suggest_rule_for("WebFetch", &["https://docs.rs/a".into()]).as_deref(),
            Some("domain:docs.rs")
        );
    }

    #[test]
    fn the_reader_never_panics_and_is_bounded() {
        for s in [
            "",
            "'",
            "\"",
            "\\",
            "$",
            "$(",
            "<<",
            "<<EOF",
            "2>",
            ">&",
            "|",
            "&&&",
            "((",
            "))",
            "$'",
            "a\\\nb",
            "#only a comment",
        ] {
            let _ = read(s);
        }
        let long = "a ".repeat(MAX_LEN);
        assert!(read(&long).barrier.is_some());
        let deep = (0..8).fold("rm -rf /".to_string(), |s, _| {
            format!("sh -c '{}'", s.replace('\'', "'\\''"))
        });
        let _ = read(&deep);
    }
}
