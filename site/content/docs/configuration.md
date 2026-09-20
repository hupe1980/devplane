+++
title = "Configuration"
description = "The complete devplane.toml reference. Every key is read by the code; an unknown key is a parse error rather than a silent default."
weight = 21
[extra]
group = "reference"
+++

`devplane.toml` lives at the repository root and is **committed**. That is the point: the commands
that decide whether work is finished are the project's, reviewed like anything else, and never
something an agent wrote for itself.

A repository with no `devplane.toml` still works — the gates are empty, no rule prohibits anything,
and nothing changes.

**Devplane reads this file and never writes it.** `devplane check` prints it back in a terminal;
<kbd>,</kbd> on the board does the same for every registered repository at once, including the one
whose file stopped parsing. There is no settings form on purpose: an agent here runs as you, so a
write path to `[policy]` would be reachable by the thing those rules govern.

> [!IMPORTANT]
> **This is the whole file format.** Every key below is read by the code and there are no others. An
> unknown key is a **parse error**, not a silent default — so one key that does not exist fails the
> *whole* file and takes that repository's permission rules down with it. That is why the reference
> is checked by a test rather than written by hand.

## The complete file

```toml
[project]
name          = "saas"
base_branch   = "main"    # discovered from origin/HEAD when unset
default_agent = "claude"  # what `agent = "any"` means here

[workspace]
setup   = "pnpm install --frozen-lockfile"   # run in a new tree
include = [".env", ".env.local"]             # repo-relative paths

[gates]
check   = ["pnpm typecheck", "pnpm lint", "pnpm test -- --run"]
timeout = "10m"           # for the whole gate, not per command
on_fail = "feedback"      # feedback | escalate | ignore
max_feedback_rounds = 2

[gates.named.repro]       # asked for by name from a pipeline step
run     = ["pnpm test -- --run tests/repro"]
expect  = "fail"          # one that passes proved nothing
timeout = "2m"            # optional; the project's otherwise

[questions]
deadline = "never"        # never | 90s | 30m | 4h — how long an ask waits

[policy]                  # prohibit and defer; Devplane never approves
never_auto = ["Bash(rm -rf *)", "Read(.env)"]
always_ask = ["Bash(git push *)"]
max_parallel_runs = 2     # pieces of work with an agent in them
stall_timeout     = "12m" # how long this work may be quiet

[budget]                  # what work may spend before asking
default_usd = 10
feature_usd = 25
max_turns   = 60          # counted here, so it binds every agent
max_runtime = "45m"       # likewise

[transcripts]
keep = true               # the default

[spec]                    # words that mark a question your spec has not answered
open_questions = ["NEEDS CLARIFICATION", "TBD"]

[github]                  # off by default: a push is visible
pull_request = true
draft        = true
ready_label  = "devplane:ready"

[pipelines.feature]
steps = [
  { role = "implement", prompt = "implement", gate = "check" },
  { role = "review", agent = "codex", prompt = "review",
    findings = { back_to = "implement", max = 2 } },
  { role = "verify", prompt = "write-tests", gate = "check" },
  { human = "merge" },
]
```

```sh
devplane check          # what will this file actually do?
devplane check ../lib   # somewhere else
devplane check --json   # exits non-zero on an error; for CI
```

`check` is offline and daemon-free, so it runs in a repository nothing is connected to yet.

## `[project]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | the directory name | what the board calls it |
| `base_branch` | string | discovered from `origin/HEAD` | what worktrees branch from |
| `default_agent` | string | `claude` | what `agent = "any"` means, and what `work start` uses |

## `[workspace]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `setup` | string | none | run once in a new worktree, before the agent starts |
| `include` | list of paths | empty | gitignored files copied into a new worktree |

`setup` is **not a gate**: `pnpm install` says nothing about whether the work is done, so its report
is recorded separately.

`include` takes **paths, not globs**, and a path that leaves the repository is refused — the file is
committed, so it arrives with somebody else's code, from a fork, a pull request, or a vendored
dependency.

Refused by spelling *and* by where it points:

- `include = ["../../.ssh/id_rsa"]` and absolute paths are rejected outright.
- A **symlink out of the repository** is rejected too. Git tracks symlinks, so
  `config/local.env -> /home/you/.ssh/id_rsa` is a file a repository can ship; following it would
  copy a private key into the directory an agent is about to read. The source is resolved and must
  still be inside the repository.
- Nothing is ever written **through** a symlink at the destination. The worktree is a checkout of the
  same repository, so a committed dangling symlink is already sitting there — copying through it
  would write to whatever it names, anywhere on your disk.

A symlink that stays inside the repository works normally. The rule is containment, not a ban on
symlinks.

## `[gates]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `check` | list of commands | empty | the definition of done; all must succeed, in order |
| `timeout` | duration | `10m` | for the whole gate |
| `on_fail` | `feedback` · `escalate` · `ignore` | `feedback` | what happens when it is red |
| `max_feedback_rounds` | integer | `2` | how many times failures go back before a person is asked |

Durations are written the way people say them: `30s`, `10m`, `1h30m`. A bare number is seconds.

### `[gates.named.<name>]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `run` | list of commands | required | the commands |
| `expect` | `pass` · `fail` | `pass` | `fail` inverts the verdict |
| `timeout` | duration | the project's | optional override |

`check` always resolves by that name. Anything else has to be declared, and asking for a gate nobody
wrote is an error rather than a silent pass.

See [Verified done](/docs/verified-done/) for how gates run.

## `[policy]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `never_auto` | list of rules | empty | refused |
| `always_ask` | list of rules | empty | put in front of a person |
| `max_parallel_runs` | integer | unlimited | pieces of work with an agent in them |
| `stall_timeout` | duration | the machine's | how long this project's work may be quiet |

The rule syntax has [a page of its own](/docs/permissions/).

**There are two lists and neither of them grants anything.** Devplane refuses and defers; it does
not approve a tool call, because approving would be a claim that your agent would have approved too.

Any other key under `[policy]` **fails the file and names the line**. Rules you want *enforced* go in
your agent's own `settings.json` under `permissions.allow`, where the thing enforcing them lives.

`max_parallel_runs` counts **pieces of work with an agent in them**, not runs. A declared chain is
one unit however many steps it has taken — its steps share a worktree and run one after another, so
they are not the colliding agents the limit exists to prevent. Sessions you started yourself are
never counted.

`stall_timeout` is per project because the number is a statement about the work: a repository whose
suite takes twelve minutes and one that answers in seconds cannot share a threshold. Unset inherits
the machine's rather than defaulting to one of its own.

## `[budget]`

Three bounds on two kinds of axis. Work that passes any of them stops and raises a `cost_spike` item
naming which one.

| Key | Type | Meaning | Always fires? |
|---|---|---|---|
| `default_usd` | number | any kind that names no figure of its own | **no** |
| `quick_usd`, `chore_usd`, `bug_usd`, `feature_usd` | number | per kind | **no** |
| `max_turns` | number | agent turns this work may take | yes |
| `max_runtime` | duration | how long it may run, from when it started | yes |

Zero means “off”, not “a ceiling of nothing”.

### Why two of them are marked “no”

A ceiling in dollars only bites when the agent reports what it spent, and that is optional twice
over. The Agent Client Protocol marks the cost field optional. And the **OpenTelemetry GenAI
semantic conventions** — the dialect GitHub Copilot and Codex emit — have no notion of money
anywhere in them, so a run observed that way reports tokens, models and durations and no dollars.
That is not an agent declining to answer; it is a schema with nowhere to put the answer.

`max_turns` and `max_runtime` exist because of that. A turn is an API request Devplane saw itself,
and elapsed time is arithmetic on the work's own timestamp, so neither depends on an agent's
cooperation and both bind every agent on every provider. They are checked before the money bound.

> [!WARNING]
> If a `_usd` ceiling is the only bound you set, `devplane check` warns — and `devplane work show`
> says whether your agent reports a cost at all, rather than printing `$0.00` and letting you assume
> you are covered. A guard that may not fire has to say so.

## `[transcripts]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `keep` | bool | `true` | whether what a *driven* agent says in this repository is written down |

On by default, because such a run has no window of its own and the board would otherwise be able to
say that a tool ran and not one word about why — and because the protocol streams the text to
Devplane regardless, so discarding it was never a privacy measure.

Off writes nothing, including the prompts Devplane itself sent, and does not stop the agent reading
anything. Sessions Devplane merely watches are unaffected either way.

## `[questions]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `deadline` | duration or `never` | `never` | how long an unanswered question or permission waits in this repository |

**The default is that it waits.** Devplane will not pick an answer for you, and it will not let a
clock pick one unless you write the clock down here. Your agent's own vendor makes the same choice:
Claude Code's question timeout is off unless you turn it on, and permission prompts never
auto-resolve on idle.

Set one where a repository runs unattended and you would rather the agent stopped than sat:

```toml
[questions]
deadline = "4h"    # never | 90s | 30m | 4h
```

When it fires the agent is told **no**, and `devplane audit` names **a clock** as the authority, with
the duration and this file.

A value that will not parse fails `devplane check` and the wait stays unbounded — ending a question
early on the strength of a typo is the one outcome that must not happen.

## `[spec]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `open_questions` | list of strings | empty | words that mark a question the specification has not answered |

Counted per gate for the specification a piece of work names with
[`--spec`](/devplane/docs/pipelines/#spec-driven-development), alongside its `- [ ]` task list, and
shown on the board beside the verdict.

The words are yours. `NEEDS CLARIFICATION` is Spec Kit's spelling, `TBD` is everybody's, and the
next tool will have a third — so the list is empty unless your repository writes one. Matched per
line and case-insensitively.

Nothing else about the specification is interpreted: the outline is the Markdown headings and the
progress is the boxes, because those are the only things the frameworks in this category agree on.

## `[github]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `pull_request` | bool | `false` | open one when the gates pass |
| `draft` | bool | `true` | open it as a draft |
| `ready_label` | string | none | which issues `devplane issues --ready` offers |

Off by default because pushing a branch is the first thing Devplane does that other people can see.

## `[pipelines.<kind>]`

Keyed by the work kind it serves: `quick`, `chore`, `bug`, `feature`.

```toml
[gates]
check = ["cargo test"]

[gates.named.repro]
run    = ["cargo test --test repro"]
expect = "fail"

[pipelines.bug]
steps = [
  { role = "reproduce", prompt = "repro", gate = "repro" },
  { role = "fix", prompt = "fix", gate = "check" },
  { role = "review", agent = "any", prompt = "review",
    findings = { back_to = "fix", max = 1 } },
  { human = "merge" },
]
```

| Key | Meaning |
|---|---|
| `role` | what the step is for, and the name `back_to` refers to |
| `agent` | an agent id, or `any` for the project's default |
| `prompt` | a template in `.devplane/prompts/<name>.md`, or the text itself |
| `gate` | `check`, or a name from `[gates.named]` |
| `findings.back_to` | an **earlier** role, which must declare a `gate` |
| `findings.max` | how many times work may go back |
| `findings.file` | where the reviewer writes them (default `.devplane/findings.md`) |
| `findings.only` | words that make a finding worth returning the work for; empty (the default) means every finding counts |
| `human` | suspends the chain until `devplane work approve` |

A step is one or the other: setting both `role` and `human` is an error, and so is giving a human
step a `prompt`.

See [Pipelines](/docs/pipelines/).

## Machine-wide files

`~/.devplane/policy.toml` takes the same `[policy]` shape, so a rule moves between the two by
cutting and pasting it.

`~/.devplane/agents.toml` adds agents by name — see [Driving agents](/docs/agents/).

## Environment

| Variable | What |
|---|---|
| `DEVPLANE_HOME` | where the database, token and daemon record live (default `~/.devplane`) |
| `DEVPLANE_PORT` | the port `serve` asks for (default `47831`) |
| `DEVPLANE_CLAUDE_BIN` | the `claude` binary, when it is not where Devplane looks |
| `DEVPLANE_NOTIFY` | `0` turns desktop notifications off |
| `DEVPLANE_LOG` | tracing filter, e.g. `devplane=debug` |
| `DEVPLANE_UI` | serve the board from this file instead of the copy compiled into the binary — for working on the page |
| `CLAUDE_CONFIG_DIR` | which Claude Code config `connect` writes to |
