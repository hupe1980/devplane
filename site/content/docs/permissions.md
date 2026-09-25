+++
title = "Permissions"
description = "Devplane never approves a tool call. It refuses one (never_auto) or puts one in front of you (always_ask), in Claude Code's rule syntax, per repository — and fails closed when the rules will not load."
weight = 22
[extra]
group = "reference"
+++

**Devplane never approves a tool call**; your agent's own permission system does that. Devplane can
*refuse* a call or *defer* it to a person. There are two lists:

```toml
# devplane.toml
[policy]
never_auto = [
  "Bash(rm *)",
  "Read(.env)",
  "mcp__*",
]
always_ask = [
  "Bash(git push *)",
]
```

A machine-wide set with the same shape lives in `~/.devplane/policy.toml`. There is no allow list;
grants belong in your agent's own `settings.json`, under `permissions.allow`.

The same rules answer a watched session's hooks (no host needed), the permission requests of runs
Devplane drives, and Devplane's own actions. Every verdict lands in the
[decision log](@/docs/decisions.md) with the rule named.

Test a rule before you commit it:

```sh
devplane explain 'pnpm test && rm -rf /'
devplane explain --tool Read .env
devplane explain --replay          # every call already observed, against the rules now
devplane check                     # does devplane.toml parse, and is any rule refused
```

## The syntax is Claude Code's

A prohibition moves between `settings.json` and `devplane.toml` by copy and paste. `devplane doctor`
prints the baseline: rule syntax modelled on Claude Code 2.1.273. What a rule *reaches* is wider
here, only ever toward a prompt; see
[Where Devplane is stricter](#where-devplane-is-stricter-than-claude-code).

### Tools

| You write | It matches |
|---|---|
| `Read`, `Bash`, `Bash(*)` | every use of that tool |
| `mcp__github`, `mcp__github__*` | every tool from the `github` MCP server |
| `mcp__github__get_*` | that server's `get_` tools |
| `mcp__github__create_issue` | one tool |
| `mcp__*`, `*` | a glob over the whole tool name |

### Commands

For `Bash`, `PowerShell` and `Monitor` the specifier is a command pattern, matched against **tokens,
not text**. The line is split into simple commands on `&&`, `||`, `;`, `|`, `&` and newlines;
each is reduced to a program and its arguments, quotes removed the way the shell removes them.
A rule is a token prefix: the program, then the flags **as a set**, then the operands.

| You write | Matches | Does not match |
|---|---|---|
| `Bash(npm run build)` | `npm run build` | `npm run build --watch` |
| `Bash(npm run *)` | `npm run build`, `npm run test --watch`, `npm run` | `npm install` |
| `Bash(npm run:*)` | the same — `:*` is the form the permission dialog writes | |
| `Bash(ls *)` | `ls -la`, `ls` | `lsof` |
| `Bash(rm *)` | `rm x`, `rm -rf /`, `/bin/rm -rf /`, `RM -rf /`, `r''m -rf /` | `rmdir x` |
| `Bash(rm -rf /)` | `rm -rf /`, `rm -r -f /`, `rm -fr /` | `rm -rfv /` |

`:*` is recognised only at the end: in `Bash(git:* push)` the colon is literal. A `Bash(…)` rule also
governs `Monitor`. `devplane check` warns when a rule's second token is a flag cluster; `Bash(rm *)` is
the robust spelling.

**PowerShell is read as literal tokens.** `PowerShell(Remove-Item *)` stops `Remove-Item x`; aliases
such as `rm` or `del` are not resolved, and under a `PowerShell(…)` rule they go to a person.

### A prohibition reaches every simple command on the line

A `never_auto` rule fires when **any** simple command matches, wherever it sits and however it is
spelled. `Bash(rm *)` refuses all of these:

| Shape | Example |
|---|---|
| a compound line | `ls && rm -rf /` |
| a subshell, backtick or `sh -c` (four levels deep) | `( cd /x && rm -rf . )`, `` echo `rm -rf /` ``, `sh -c 'rm -rf /'` |
| a wrapper and its flags | `sudo --user root rm -rf /`, `env -P /bin rm x`, `timeout 30 rm x`, `nohup`, `flock`, `busybox`, `builtin` |
| a function body or coprocess | `function f { rm -rf /; }`, `coproc rm -rf /` |
| a brace expansion | `{rm,-rf,/}` |
| a here-string given to a shell | `bash <<< 'rm -rf /'` |
| a program named by its path or in another case | `/usr/bin/rm -rf /`, `./rm x`, `RM -rf /` |
| a globbed program name | `/bin/r? -rf x`, `/bin/r[m] -rf x` — the shell picks the program, so a person is asked |

Quoted text is data: `echo "a; rm -rf x"` is one `echo` and meets no `rm` rule, and a text tool's
`-e` is a pattern, not a command (`grep -e 'rm -rf' f`).

> [!WARNING]
> A `Bash` rule reads the line; it cannot follow a program that decides at run time what to execute.
> For containment, use a sandbox.

### Paths

For `Read` and `Edit` the specifier is a **gitignore pattern**: `*` stays inside one segment, `**`
crosses them. One `Edit(…)` rule covers every built-in tool that writes files, and one `Read(…)`
rule every one that reads them (`Grep`, `Glob` and `LSP` included). A `Read` deny also blocks writing
that path with a file tool; it does not reach `NotebookEdit`, which needs an `Edit` deny.

| You write | Anchored at | Example |
|---|---|---|
| `//path` | the filesystem root | `Read(//tmp/**)` |
| `~/path` | your home directory | `Read(~/.ssh/**)` |
| `/path` | the file the rule is written in | `Read(/secrets/**)` in a `devplane.toml` means that repository's `secrets` |
| `path`, `./path` | the agent's working directory | `Edit(src/**)` |

A bare filename matches at any depth: `Read(.env)` is `Read(**/.env)`. Symlinks are resolved at both
ends, so a repository shipping `config/key -> ~/.ssh/id_rsa` does not get past `Read(~/.ssh/**)`.

### Path rules reach shell commands

A `Read` or `Edit` rule also governs the files a shell command **names**:

| Command | Reached by |
|---|---|
| `cat .env`, `grep TOKEN .env`, `head .env`, `less .env`, `base64 < .env` | `Read` — the operands of known readers, and input redirections |
| `cp .env /tmp/b`, `mv .env x`, `rm .env` | `Read` — never looking at a file also means never moving or replacing it |
| `cp /tmp/a .env`, `tee .env`, `touch .env`, `dd of=.env`, `ln -s x .env` | `Edit` and `Read` |
| `echo x > .env` | `Edit` only — an output redirection is `Edit` business |
| `grep -r key secrets` | a recursive command reaches everything under its directory |
| `cat .en?`, `cat conf*` | a glob operand that could expand onto the protected file |

To protect a file from a shell, write both halves (`devplane check` notes when only one is there):

```toml
[policy]
never_auto = ["Read(.env)", "Edit(.env)"]
```

Not reached: a program that opens a file itself (a script, a build step), a path inside an option
value (`grep -f.env x`), and `cat *`, which does not expand onto a dotfile.

### Exceptions, with `!`

```toml
[policy]
never_auto = ["Bash(git *)", "!Bash(git status *)"]
```

An exception applies **per simple command, never to the whole line**: in `git status && git push
--force` the first command is excused and the second is not, so the line is refused. An exception is
scoped to the file it is written in, so a project cannot cancel a machine-wide prohibition.

### Hosts and parameters

```toml
[policy]
never_auto = [
  "WebFetch(domain:evil.example)",
  "Agent(isolation:worktree)",
]
```

`domain:` matches the URL's parsed host, not a substring. `Tool(param:value)` matches a top-level
input field, with `*` as a wildcard; a parameter the model omits never matches.

## Order: deny wins

`never_auto` is evaluated first across **both** files, then `always_ask`, then *unresolved* (the line
hides what it runs, so a person is asked), then *undecided* (no answer; your agent asks as usual).
Specificity does not change the order, and neither file can cancel the other's prohibitions.

The governing `devplane.toml` is the one at the root of the checkout the command runs in. A worktree
inherits the rules — and the gates — of the checkout that owns it, so a rule an agent relaxes on its
own branch changes nothing about what it may do.

## Rules that cannot work are refused

A `never_auto` rule that silently matches nothing would read as protection. `devplane check` and
`devplane change start` report these before an agent starts:

| Refused | Why |
|---|---|
| `Write(src/**)`, `Glob(src/**)`, `NotebookEdit(x)` | file permissions are checked only against `Read(…)` and `Edit(…)`; these are never consulted |
| `mcp__github(create_issue)` | an `mcp__` rule with brackets is skipped on load |
| `Bash(command:rm *)` | ignored because a compound command bypasses it; write `Bash(rm *)` |
| `Agent(researcher)` | that tool has no field for a bare specifier to match |
| `Bash(rm -rf *` | the bracket is never closed |

And one **warning**, because the list behind it is a snapshot of Claude Code's tool reference:

| Warned about | Why |
|---|---|
| `Bahs(rm *)`, `Stop Task` | not a tool name Claude Code documents, so the rule matches nothing. The name shown in a transcript is not always the rule name — `Stop Task` is written `TaskStop` |

`Cd(<path>)` rules govern a slash command, not a tool call, so nothing reaches Devplane to match
one; `devplane check` says so. Keep them in `settings.json`.

## Rules that do nothing

`devplane check` also reports a rule that is valid but can never take effect, because an earlier
rule in the same list already covers every call it names:

```console
$ devplane check
  policy    2 deny, 0 ask
            deny   Bash(rm *)
            deny   Bash(rm -rf /tmp/build)
  unused    `Bash(rm -rf /tmp/build)` does nothing: `Bash(rm *)` above it already covers
            every call it names
```

It is decided by pattern containment and stays silent when it cannot prove the claim; a list with a
`!` exception is never analysed. `check` also names allow rules in `.claude/settings.json` that grant
more than they look like (`Bash(python:*)` approves `python -c` with any code). It writes nothing.

## Where Devplane is stricter than Claude Code

Devplane cannot approve, so it can only err toward a prompt:

- **Wrappers are looked through.** `sudo`, `env`, `exec`, `timeout`, `nohup`, `flock`, `busybox` and
  the rest run another command, and a shell's `-c` is read as a line. `Bash(rm *)` covers
  `sudo rm -rf /` here.
- **The program's name matches through its path, case-folded.** `Bash(rm *)` covers `/bin/rm` and
  `RM`.
- **Flags are a set**, even in an exact rule: `Bash(rm -rf /)` meets `rm -r -f /` and `rm -fr /`.
- **More writers count.** The destination of `cp`, `install`, `rsync` and `ln`, `truncate`'s operand
  and `dd`'s `of=` are all reached by an `Edit` rule.
- **An unreadable line is asked about** (next section).

## When a line cannot be read, a person is asked

Some lines hide what they run:

```console
$ devplane explain 'rm$IFS-rf node_modules'      # never_auto = ["Bash(rm *)"]
ask — unreadable  Bash
        because the program is named by a variable the shell expands; `Bash(rm *)` constrains what Bash may run
```

| Shape | Example |
|---|---|
| a program name the shell builds | `rm$IFS-rf x`, `$(echo rm) -rf x` |
| a command line built from input | `xargs rm -rf`, `eval "$cmd"` |
| code in a string, file or stdin | `python -c "…"`, `node -e "…"`, `python3 script.py`, `curl … \| sh` |
| a command run somewhere else | `ssh host …`, `docker run …`, `su`, `chroot` |
| `find` that runs or deletes | `find . -delete`, `find . -exec …` |
| a wrapper flag the reader does not know | `sudo --made-up x rm …` |
| a line past 65,536 characters | — |

This happens only where a rule **could** have applied, and a rule that answers the call always wins.
The audit row's outcome is `unresolved`, under the authority of the rules that made it a question.

A heredoc's body fed to a program that cannot execute it (`cat`, `tee`, `grep`, `jq` and a fixed
list of others) is data; fed to anything else (`bash`, `| sudo bash`, `| python3`) it is read.

## When the file is broken, every call is asked

A `devplane.toml` that will not load **fails closed**: every gated call in that repository goes to a
person, naming the file and the parser's error. A broken `~/.devplane/policy.toml` does the same
machine-wide. A critical inbox item names the file until it loads.

```console
$ devplane explain 'cat .env'
ask — unreadable  Bash
        because devplane.toml is not valid: … unknown field `polcy` …; until it loads, a person decides every call
```

## Auto mode

Claude Code's auto mode approves routine calls with a classifier and never shows a prompt, so a
`PermissionRequest` hook never fires there. `devplane connect claude` installs two synchronous hooks:

| Hook | Fires | Carries |
|---|---|---|
| `PreToolUse` | before every tool call, in every mode | a prohibition, or nothing |
| `PermissionRequest` | when a person was going to be asked | the same, plus the request to show you |

Neither answers *allow*, and prohibitions hold in every mode. `devplane modes` shows which sessions
run without prompts. Evaluation is in-process and linear-time; `devplane doctor` probes the installed
gate and prints the round trip.

## Answering a watched session's permission

A permission in a session **you** started can be answered away from its terminal if the project sets
a hold:

```toml
[questions]
hold = true        # or "45s"
```

A call your `always_ask` rules matched is written as an ask, and the hook waits for the hold. Answer
from the workbench's Inbox or with `devplane answer <ask> --allow` (or `--deny`); the first answer
wins and **you** are the authority. If nobody answers, the ask ends with **nobody** as the authority
and your agent shows its own dialog. A prohibition is applied first and never held.

**An agent cannot answer its own permission.** `devplane answer --allow` is refused inside an agent
session (`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `DEVPLANE_RUN`, or a Codex or Copilot session
variable set); `--deny` still works. This stops the easy path, not a hostile process; see
[Security](@/docs/security.md).

## Tuning the rules

- **Asked too often:** a permission in `devplane inbox` carries the narrowest rule that would stop it
  being asked, and the `settings.json` to paste it into. `devplane rules <rule>` checks every
  registered project.
- **A rule too tight:** five refusals in one run raise a `refused` item naming the rule.
