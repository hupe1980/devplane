+++
title = "Configuration"
description = "The complete devplane.toml reference. Every key is read by the code; an unknown key is a parse error rather than a silent default."
weight = 21
[extra]
group = "reference"
+++

`devplane.toml` sits at the repository root and is **committed**. Without one: no gates, no rules.
Devplane never writes it.

> [!IMPORTANT]
> **An unknown key is a parse error.** A file that will not load **fails closed**: every gated call in
> that repository goes to a person.

`devplane config check [path]` says what the file will do and exits non-zero on error; offline, so it
runs in CI.

## The complete file

```toml
[project]
name        = "saas"
base_branch = "main"        # discovered from the repository when unset

[agent]
default          = "claude" # used by `change start` without --agent
max_parallel     = 2        # changes with an agent in them at once
stall_timeout    = "12m"
keep_transcripts = true

[workspace]
setup = "pnpm install --frozen-lockfile"   # run once in a new worktree
share = ["cargo"]                          # build caches shared by changes

[gates]
check   = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout = "10m"             # for the whole gate
max_feedback_rounds = 2     # then the change stops for a person
junit       = ["reports/junit.xml"]   # JUnit files check writes; names tests
base_checks = "offer"                 # offer | auto | off
serial      = false                   # true: one gate at a time (a shared database)

[gates.e2e]                 # run by name; never what verifies a change
run     = ["pnpm test -- --run tests/e2e"]
timeout = "20m"

[policy]                    # refuse and defer; Devplane never approves
deny = ["Bash(rm *)", "Read(.env)"]
ask  = ["Bash(git push *)"]

[budget]                    # per change
max_usd     = 10
max_turns   = 60
max_runtime = "45m"

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
pull_request = true         # lets `change offer` push and open the PR, as a draft
ready_label  = "devplane:ready"
```

Durations are strings: `"30s"`, `"10m"`, `"1h30m"`; `"600"` is seconds. A bare number (`timeout =
600`) is a parse error. `[questions] deadline` takes one unit (`"90s"`, `"30m"`, `"4h"`) or `"never"`.

## `[project]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | the directory name | what the workbench calls it |
| `base_branch` | string | discovered from the repository | what worktrees branch from; must be a name git accepts |

## `[agent]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `default` | string | `claude` | the agent `devplane change start` uses when `--agent` is not given |
| `max_parallel` | integer | unlimited | changes with an agent in them at once in this project; sessions you started yourself are not counted |
| `stall_timeout` | duration | `10m` | how long a working run may produce nothing before it is called stalled |
| `keep_transcripts` | bool | `true` | whether what a driven agent says in this repository is stored |

`keep_transcripts = false` stores nothing a driven agent says, prompts included. Watched sessions
carry no prose either way.

## `[workspace]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `setup` | string | none | a command run once in a new worktree, before the agent starts |
| `share` | list of ecosystem names | empty | build caches shared between this project's changes |

- `setup` is not a gate. While it runs the change reads *installing · `<command>` · <elapsed>*.
- `share` is never inferred. The known name is `cargo`: setup, gates and agent get `CARGO_TARGET_DIR`
  under `.claude/worktrees/.shared/`. pnpm, npm, go and uv share a home store already.
- Files to copy into a fresh tree (`.env`, local config) go in the repository's `.worktreeinclude`
  (gitignore syntax, also read by Claude Code). Only gitignored files are copied; symlinks out are
  refused; nothing at the destination is overwritten.

## `[gates]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `check` | list of commands | empty | the definition of done; all must succeed, in order |
| `timeout` | duration | `10m` | for the whole gate |
| `max_feedback_rounds` | integer | `2` | how many times failures go back to the agent before the change stops for a person; `0` stops at once |
| `junit` | list of paths | empty | JUnit XML files `check` writes, relative to the checkout; read only when the run wrote them, inside the checkout, up to 16 MiB, to name tests. Git should ignore them: writing a tracked path moves the tree `check` is judged on |
| `base_checks` | `offer` · `auto` · `off` | `offer` | when [base checks](@/docs/verified-done.md#base-checks) run: when you offer or ask, also after every passing `check`, or never |
| `serial` | bool | `false` | run this repository's gates one at a time on this machine — `setup`, `check`, named gates and base checks, from every change's worktree and from `devplane gate`. For checks that share something two runs cannot both hold, such as one test database or a fixed port. A gate waiting its turn is not timed; its `timeout` starts when its commands do. Not serialised across machines: two CI jobs are two machines |

### `[gates.<name>]`

Any other table under `[gates]` is a named gate. `check`, `timeout`, `max_feedback_rounds`, `junit`,
`base_checks` and `serial` are reserved and cannot name one. Any other key that is not a table is
refused.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `run` | list of commands | required | the commands; an empty list is an error |
| `timeout` | duration | the `[gates]` timeout | optional override |

A named gate runs on request (`devplane gate --name e2e`) and never verifies a change. See
[Verified done](@/docs/verified-done.md).

## `[policy]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `deny` | list of rules | empty | refused, without asking anybody |
| `ask` | list of rules | empty | put in front of a person |

Neither grants anything. Syntax: [Permissions](@/docs/permissions.md).

## `[budget]`

Per change. Passing a bound stops the change with a `cost_spike` item naming it. Zero means off.

| Key | Type | Meaning | Always fires? |
|---|---|---|---|
| `max_usd` | number | what one change may spend, as its agent reports it | **no** |
| `max_turns` | integer | agent turns one change may take | yes |
| `max_runtime` | duration | how long one change may run, from when it started | yes |

`max_usd` binds only agents that report cost (optional in ACP). `config check` warns when it is the
only bound; `change show` says *not reported* rather than `$0.00`.

## `[spec]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `plans` | path | none | where this repository keeps its specifications |
| `open_questions` | list of strings | empty | words that mark a question a specification has not answered |
| `tokens` | list of prefixes | `FR-` `NFR-` `SC-` `REQ-` `US-` `AC-` | the shapes of requirement ids; a prefix followed by digits |

```toml
[spec]
plans          = "specs"   # Kiro ".kiro/specs", OpenSpec "openspec/changes"
open_questions = ["NEEDS CLARIFICATION"]
tokens         = ["REQ-"]  # REQ-3 in heading and task line: one edge
```

- `plans` unset: the Specifications view says *not configured*.
- `open_questions` matches per line, case-insensitively, outside code; one inbox item per project
  names the count.
- `tokens` replaces the defaults. No digits in a prefix; for bare numbers write the word
  (`tokens = ["Req "]` matches `Req 12`). An empty list is an error.

See [Specifications](@/docs/specs.md).

## `[questions]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `deadline` | `never` or a duration | `never` | how long an unanswered question or permission waits before the agent is told no |
| `hold` | `true`, `false` or a duration | none | how long a permission on a **watched** session waits for an answer from Devplane before the agent's own dialog appears |

```toml
[questions]
deadline = "4h"
hold     = true     # 30s; a duration names its own, up to 120s
```

- By default only a person ends a question. A fired `deadline` tells the agent no; `devplane audit`
  names `timer` with the duration and file. The clock runs only while a host is up. An unparsable value
  fails `config check` and the wait stays unbounded.
- `hold` applies only to `ask` matches, never to `deny`. See
  [answering a watched session's permission](@/docs/permissions.md#answering-a-watched-session-s-permission).
  It does not fire in auto mode.

## `[review]`

How `devplane change review` and the Review tab order files, after checks weakened or changed.
Nothing is inferred from file names.

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

- Patterns are gitignore syntax, repository-relative. First matching role wins; unmatched files follow
  in path order.
- Without roles the review says it is unordered. Without `[[review.covers]]` the coverage column is
  absent (not *not covered*).

## `[github]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `pull_request` | bool | `false` | let `devplane change offer` push and open a draft pull request; off, it prints the `git push` line and compare address. Passing gates never opens one |
| `ready_label` | string | none | issues with this label are offered as work — `devplane issue list --ready`, `change start --issue` |

## Machine-wide files

All under `~/.devplane/` (or `DEVPLANE_HOME`).

**`policy.toml`**: a `[policy]` with `deny` and `ask`, applied to every project, and your own
grants — `[grants]` (`offer_after`, `offer_again_after`) and `[projects."<root>"]` with `allow` —
described under [Permissions](@/docs/permissions.md#grants-your-own-rule-per-project). Fails closed;
a `[policy] allow` is refused, and so is any of these keys in a repository's `devplane.toml`.

**`app.toml`**: the window, and which GitHub this machine signs in to:

```toml
# ~/.devplane/app.toml
[app]
shortcut = "CmdOrCtrl+Shift+Space"   # the one global shortcut
port     = 47831                     # the host's port; 0 picks a free one

[github]
host      = "ghe.corp"               # the configured host; github.com unless set
client_id = "Iv1.…"                  # that host's OAuth App, for the device flow

[github.hosts."ghe.other"]
client_id = "Iv1.…"                  # another Enterprise server's own app
```

- A client id is public. Release builds carry one for `github.com` only; other hosts use
  `devplane auth login --with-token`.
- Missing file: defaults. `devplane doctor` reports a parse error or an unregistrable shortcut.
- `port = 0` picks a new port each start, so agent telemetry (cost, context) no longer arrives.

**`agents.toml`**: agents added by name, beside the built-in ones. See
[Driving agents](@/docs/agents.md).

**`trusted.toml`**: the repositories you trusted, written by `devplane project trust` and by the
window, never by hand-off from an agent. One key, `roots`, a list of absolute paths; any other key is
refused and trusts nothing. Delete a root to take its trust back; the host reads the file when it
starts. The store never holds trust: deleting it loses history, not this.

```toml
# ~/.devplane/trusted.toml
roots = ["/Users/you/src/api"]
```

## Environment

| Variable | What |
|---|---|
| `DEVPLANE_HOME` | where the store, token and host record live (default `~/.devplane`) |
| `DEVPLANE_PORT` | the port `devplane up` binds (default `47831`) |
| `DEVPLANE_LOG` | the host's log filter, e.g. `devplane=debug` |
| `DEVPLANE_NOTIFY` | `0` turns desktop notifications off |
| `DEVPLANE_CLAUDE_BIN` | the `claude` binary, when it is not on `PATH` |
| `DEVPLANE_UI` | serve the interface from this built directory instead of the embedded copy |
| `DEVPLANE_OPENCODE_URL` | a running `opencode serve` to watch; see [Watching sessions](@/docs/observe.md#opencode) |
| `VISUAL`, `EDITOR` | the editor the window's **Editor** button opens a worktree in |
| `DEVPLANE_RUN` | set by Devplane on every agent it starts, so `devplane inbox answer --allow` refuses inside it |
| `CLAUDE_CONFIG_DIR` | which Claude Code configuration `devplane agent connect claude` writes to |
| `CODEX_HOME`, `COPILOT_HOME` | where Codex and Copilot keep their configuration |
| `NO_COLOR`, `CLICOLOR_FORCE` | colour off, or forced on |
