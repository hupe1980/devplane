+++
title = "Configuration"
description = "The complete devplane.toml reference. Every key is read by the code; an unknown key is a parse error rather than a silent default."
weight = 21
[extra]
group = "reference"
+++

`devplane.toml` lives at the repository root and is **committed**, so the definition of done is
reviewed like any other file. A repository without one still works: no gates, no rules. Devplane
reads this file and never writes it.

> [!IMPORTANT]
> Every key below is read by the code, and there are no others. **An unknown key is a parse error.**
> A file that will not load **fails closed**: every gated call in that repository goes to a person,
> naming the file and the error.

`devplane check [path]` prints what the file will do and exits non-zero on an error. It is offline,
so it runs in CI.

## The complete file

```toml
[project]
name          = "saas"
base_branch   = "main"      # discovered from the repository when unset
default_agent = "claude"    # what `devplane change start` uses without --agent

[workspace]
setup = "pnpm install --frozen-lockfile"   # run once in a new worktree
share = ["cargo"]                          # build caches shared between changes

[gates]
check   = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout = "10m"             # for the whole gate
on_fail = "feedback"        # feedback | escalate | ignore
max_feedback_rounds = 2

[gates.named.e2e]           # run by name; never what verifies a change
run     = ["pnpm test -- --run tests/e2e"]
timeout = "20m"

[policy]                    # refuse and defer; Devplane never approves
never_auto = ["Bash(rm -rf *)", "Read(.env)"]
always_ask = ["Bash(git push *)"]
max_parallel_runs = 2
stall_timeout     = "12m"

[budget]                    # per change
usd         = 10
max_turns   = 60
max_runtime = "45m"

[transcripts]
keep = true

[spec]
plans          = "specs"
open_questions = ["NEEDS CLARIFICATION", "TBD"]
tokens         = ["REQ-", "US-"]

[questions]
deadline = "never"          # never | 90s | 30m | 4h
hold     = "30s"            # up to 120s

[review.roles]              # first matching role wins
shared   = ["src/types/**", "migrations/**"]
security = ["src/auth/**"]
tests    = ["tests/**", "*.md"]

[[review.covers]]
test  = "tests/auth.rs"
paths = ["src/auth/**"]

[github]
pull_request = true         # lets `devplane change offer` push and open the PR
draft        = true
ready_label  = "devplane:ready"

[reports]
deliver_from = ["api"]
```

Durations are written the way people say them: `30s`, `10m`, `1h30m`. A bare number is seconds.

## `[project]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | the directory name | what the workbench calls it |
| `base_branch` | string | discovered from the repository | what worktrees branch from; must be a name git accepts |
| `default_agent` | string | `claude` | the agent `devplane change start` uses when `--agent` is not given |

## `[workspace]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `setup` | string | none | a command run once in a new worktree, before the agent starts |
| `share` | list of ecosystem names | empty | build caches shared between this project's changes |

`setup` is not a gate. While it runs the change reads *installing · `<command>` · <elapsed>*.

`share` is declared, never inferred. The known name is `cargo`: every isolated change's setup, gates
and agent get `CARGO_TARGET_DIR` pointing at one directory under `.claude/worktrees/.shared/`.
pnpm, npm, go and uv already share a store in your home directory and need no entry.

Files to copy into a fresh tree (`.env`, local config) are listed in the repository's own
`.worktreeinclude` (gitignore syntax, also read by Claude Code). Only gitignored files are copied; a
symlink out of the repository is refused, nothing is written through a symlink, and nothing at the
destination is overwritten.

## `[gates]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `check` | list of commands | empty | the definition of done; all must succeed, in order |
| `timeout` | duration | `10m` | for the whole gate |
| `on_fail` | `feedback` · `escalate` · `ignore` | `feedback` | hand failures back to the agent, ask a person at once, or record and carry on |
| `max_feedback_rounds` | integer | `2` | how many times failures go back before a person is asked |

### `[gates.named.<name>]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `run` | list of commands | required | the commands; an empty list is an error |
| `timeout` | duration | the `[gates]` timeout | optional override |

```toml
[gates]
check   = ["cargo test --locked"]
timeout = "20m"
on_fail = "escalate"

[gates.named.bench]
run     = ["cargo bench --no-run"]
timeout = "5m"
```

A change is **verified** when the latest `check` report passed and its tree digest equals the working
tree's. A `[gates.named]` gate is evidence you ask for (`devplane gate run --name bench`) and never
makes a change verified; an undeclared name is an error. See [Verified done](@/docs/verified-done.md).

## `[policy]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `never_auto` | list of rules | empty | refused, without asking anybody |
| `always_ask` | list of rules | empty | put in front of a person |
| `max_parallel_runs` | integer | unlimited | changes with an agent in them at once in this project; sessions you started yourself are not counted |
| `stall_timeout` | duration | the machine's | how long a working run may produce nothing before it is called stalled |

```toml
[policy]
never_auto = ["Bash(rm -rf *)", "Read(./.env)", "WebFetch(domain:pastebin.com)"]
always_ask = ["Bash(git push *)", "Bash(npm publish *)"]
max_parallel_runs = 3
```

Neither list grants anything; allow rules go in your agent's own `permissions.allow`. The rule syntax
is on [Permissions](@/docs/permissions.md).

## `[budget]`

One budget per change. A change that passes any bound stops and raises a `cost_spike` item naming
which one. Zero means off.

| Key | Type | Meaning | Always fires? |
|---|---|---|---|
| `usd` | number | what one change may spend, as its agent reports it | **no** |
| `max_turns` | integer | agent turns one change may take | yes |
| `max_runtime` | duration | how long one change may run, from when it started | yes |

`usd` binds only an agent that reports its cost, which the Agent Client Protocol makes optional.
Turns and runtime are counted by Devplane and bind every agent. `devplane check` warns when `usd` is
the only bound, and `devplane change show` says *not reported* rather than `$0.00`.

## `[transcripts]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `keep` | bool | `true` | whether what a driven agent says in this repository is stored |

Off stores nothing, including the prompts Devplane sent. Sessions Devplane only watches carry no prose
either way.

## `[spec]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `plans` | path | none | where this repository keeps its specifications |
| `open_questions` | list of strings | empty | words that mark a question a specification has not answered |
| `tokens` | list of prefixes | `FR-` `NFR-` `SC-` `REQ-` `US-` `AC-` | the shapes of requirement ids; a prefix followed by digits |

```toml
[spec]
plans          = "specs"      # Kiro: ".kiro/specs", OpenSpec: "openspec/changes"
open_questions = ["NEEDS CLARIFICATION"]
tokens         = ["REQ-"]     # REQ-3 in a heading and in a task line is one edge
```

- `plans` has no default. Unset, the Specifications view says *not configured* for this project.
- `open_questions` is matched per line, case-insensitively, outside code. Each project with an open
  question in a specification under way gets one inbox item naming the count.
- `tokens` replaces the default list. A prefix may not contain a digit; a repository that numbers
  requirements bare writes the word in front (`tokens = ["Req "]` matches `Req 12`). An empty list is
  an error.

See [Specifications](@/docs/specs.md).

## `[questions]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `deadline` | `never` or a duration | `never` | how long an unanswered question or permission waits before the agent is told no |
| `hold` | `true` or a duration | none | how long a permission on a **watched** session waits for an answer from Devplane before the agent's own dialog appears |

```toml
[questions]
deadline = "4h"
hold     = true     # 30s; a duration names its own, up to 120s
```

By default only a person ends a question. A `deadline` that fires tells the agent no, and `devplane
audit` names a `timer` as the authority, with the duration and this file. The clock runs only while a
host is up. A value that will not parse fails `devplane check` and the wait stays unbounded.

`hold` applies only to calls your `always_ask` rules matched; a `never_auto` refusal is never held.
The permission becomes an ask that `devplane answer` or the workbench can answer, with no host
needed. If nobody answers, the ask ends with `nobody` as the authority and the vendor's dialog
appears. It does not fire in Claude Code's auto mode, where no dialog would appear.

## `[review]`

How `devplane change review` and the Review tab order a change. Nothing is inferred from file names.
Checks weakened or changed always come first, before any role.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `roles.shared` | patterns | empty | shared types, schemas, migrations — read first |
| `roles.logic` | patterns | empty | business logic |
| `roles.security` | patterns | empty | security boundaries |
| `roles.integration` | patterns | empty | integration points |
| `roles.wiring` | patterns | empty | routes, commands, glue |
| `roles.tests` | patterns | empty | tests and docs — read last |
| `covers.test` | string | — | a test, as you name it; a gate command containing it is shown as the command that runs it |
| `covers.paths` | patterns | — | what that test covers; must not be empty |

```toml
[review.roles]
shared      = ["src/schema.sql"]
logic       = ["src/core/**", "!src/core/auth.rs"]
security    = ["src/core/auth.rs"]
integration = ["src/clients/**"]
wiring      = ["src/cli/**"]
tests       = ["tests/**"]

[[review.covers]]
test  = "tests/auth.rs"
paths = ["src/core/auth.rs"]
```

Patterns are gitignore syntax, relative to the repository. A file takes the first role that
matches; unmatched files follow, in path order. Without `[review] roles` the review says it is
unordered. Without any `[[review.covers]]` the coverage column is absent, which is different from
*not covered*.

## `[github]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `pull_request` | bool | `false` | let `devplane change offer` push and open the pull request; off, it prints the two commands |
| `draft` | bool | `true` | open it as a draft |
| `ready_label` | string | none | issues with this label are offered as work — `devplane issues --ready`, `change start --issue` |

## `[reports]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `deliver_from` | list of project names | empty | projects whose reports about this one are also handed to its live agent |

A report always reaches this project's person in the inbox. A report from a project named here is
also handed to the latest open change's live agent, quoted and attributed, and recorded with
authority `rule`. `"*"`, empty names and unregistered projects are refused by `devplane check`. See
[Reports between projects](@/docs/reports.md).

## Machine-wide files

All under `~/.devplane/` (or `DEVPLANE_HOME`).

**`policy.toml`**: the same `[policy]` section, applied to every project. Only `[policy]` is allowed.
A file that will not load fails closed, like a broken `devplane.toml`.

**`app.toml`**: the window of `devplane app`:

```toml
# ~/.devplane/app.toml
[app]
shortcut = "CmdOrCtrl+Shift+Space"   # the one global shortcut
port     = 0                         # the port the in-process host binds; 0 picks a free one
```

A missing file means these defaults. A file that will not parse, or a shortcut that cannot be
registered, is reported by `devplane doctor`.

**`agents.toml`**: agents added by name, beside the built-in ones. See
[Driving agents](@/docs/agents.md).

## Environment

| Variable | What |
|---|---|
| `DEVPLANE_HOME` | where the store, token and host record live (default `~/.devplane`) |
| `DEVPLANE_PORT` | the port `devplane serve` binds (default `47831`) |
| `DEVPLANE_LOG` | the host's log filter, e.g. `devplane=debug` |
| `DEVPLANE_NOTIFY` | `0` turns desktop notifications off |
| `DEVPLANE_CLAUDE_BIN` | the `claude` binary, when it is not on `PATH` |
| `DEVPLANE_UI` | serve the interface from this built directory instead of the embedded copy |
| `DEVPLANE_RUN` | set by Devplane on every agent it starts; read by `devplane report file` |
| `CLAUDE_CONFIG_DIR` | which Claude Code configuration `devplane connect claude` writes to |
| `CODEX_HOME`, `COPILOT_HOME` | where Codex and Copilot keep their configuration |
| `NO_COLOR`, `CLICOLOR_FORCE` | colour off, or forced on |
