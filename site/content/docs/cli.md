+++
title = "CLI reference"
description = "Every Devplane command. All of them take --json, and all of them start the daemon if it is not already running."
weight = 20
[extra]
group = "reference"
+++

Two things are true of every command below: it takes `--json`, and it starts the daemon if one is
not already running. There is nothing to launch first.

Installing puts one binary on your `PATH`, named `devplane`. If you type it often enough to
want it shorter, alias it yourself — there is no second binary to install:

```sh
alias vp=devplane
```

### Naming a run

Anywhere a command takes `<run>`, you can give it **whatever the board printed** — Devplane
resolves it the way `git` resolves a short commit: the full id, any unambiguous prefix of it, or the
session's name.

```console
$ devplane ls
core-lib
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace

$ devplane show a1        # the label on the row is enough
```

If what you typed matches more than one session it says so and lists them, rather than picking one —
the actions here include stopping an agent, and a wrong guess is expensive. A session that has never
reported loses to a live one, so a week-old editor tab sharing a prefix does not cost today's session
its short name.

Work ids resolve by prefix the same way.

## Finding a command

`devplane --help` lists the commands under the five errands people arrive with, rather than in
declaration order:

| Group | Commands |
|---|---|
| **See what is happening** | `ls` `show` `tail` `watch` `search` `open` |
| **What needs you, and what happened without you** | `inbox` `asks` `answer` `attention` `audit` `modes` `issues` `prs` `snooze` |
| **Start and steer work** | `work` `dispatch` `batch` `say` `attach` `focus` `gate` `rewind` `library` |
| **Set up a project** | `connect` `disconnect` `trust` `check` `explain` `rules` `speckit` `agents` `doctor` |
| **The daemon** | `serve` `stop` |

The list is long because the product does a lot. The five *seat* surfaces — `inbox`, `asks`,
`attention`, `audit`, `modes` — each answer a question the others cannot, so none of them is a
duplicate of another.

Three commands are surfaces for a machine and stay out of the listing: `devplane mcp`,
`devplane hook` and `devplane statusline`. They are documented below, because hiding a command from
`--help` is not a reason to stop documenting it.

**Colour** follows `NO_COLOR` and turns itself off when output is not a terminal, so
`devplane ls > file` is readable. `CLICOLOR_FORCE=1` asks for it anyway — for a pager, or a CI log
that renders escapes.

## Asking

### `devplane connect copilot`

```sh
devplane connect copilot      # writes ~/.copilot/hooks/devplane.json
devplane disconnect copilot   # deletes it
```

One file, and two lines printed for you to export — Copilot reads telemetry from the environment.
See [Observing sessions](/docs/observe/#github-copilot).

### `devplane explain`

What the gate would decide about one call, and which rule says so. Offline: no daemon, no agent, no
bill — which is what you want while you are still writing the rule.

There are four answers and no `allow`: [`Verdict`](../permissions/) cannot express one.

```console
$ devplane explain 'pnpm test && rm -rf /'
deny  Bash
        by Bash(rm -rf *)

$ devplane explain 'sudo rm -rf /'
deny  Bash
        by Bash(rm -rf *)

$ devplane explain 'rm$IFS-rf node_modules'
ask — unreadable  Bash
        because the program in `rm$IFS-rf` is produced by the shell, so no rule can name it

$ devplane explain 'pnpm test --run'
undecided  Bash
        its rules loaded and none answers this one, so the provider's own dialog decides and it reaches your inbox
```

The third answer is the interesting one, and it is the one worth checking a rule against. A
prohibition is only worth writing if it fires, and some lines hide what runs behind something no
matcher can resolve without running it. Rather than report *no rule answers this* — which would be
true and misleading — Devplane says it could not tell, and the call goes to you.
[Permissions →](../permissions/#when-devplane-cannot-read-the-command-it-asks)

The fourth is the other one to read carefully: `undecided` is three different situations in one word
— no rules here, rules that would not load, and rules that loaded and did not match — and only the
last is a fact about the call. `explain` names which, because a `devplane.toml` with a typo in it has
*no* rules in force, and reporting that as *no rule answers this one* is a broken deny rule reading as
permission.

| Flag | What |
|---|---|
| `--tool <name>` | the tool, default `Bash`. `Read`, `Edit` and `WebFetch` take their specifier directly: `devplane explain --tool Read .env` |
| `--input '<json>'` | the whole tool input, for a call a specifier cannot express: `--tool Agent --input '{"isolation":"worktree"}'` |
| `--dir <path>` | the directory the agent would be working in, which decides whose rules apply. Default `.` |
| `--replay` | ask the same question of every call already observed, and name the rule that would answer the ones that reached you |
| `--limit <n>` | how many of the most recent calls `--replay` reads. Default 5000 |
| `--json` | the verdict, the rule that produced it, `why` for an `unresolved` one, and where the rules came from |

### `devplane explain --replay`

Which grant to write next, from the calls this machine has already made. It replays every observed
tool call against the rules **as they are now**, and groups the ones no rule here decides by the rule
that would answer them — in your agent's settings, which is where a grant is enforced.

```console
$ devplane explain --replay
1284 tool calls in saas replayed against the rules as they are now

      14    1%  ask
      31    2%  deny
    1239   97%  no rule here

one rule each, most interruptions first
   118×  Bash(pnpm typecheck)
    94×  Bash(git status)
    62×  Bash(cargo test *)
    41×  Read(src/**)
    12×  WebFetch(docs.rs)

1104 of the 1239 calls Devplane leaves to your agent · paste into permissions.allow in your agent's settings
```

`--replay` uses the rules this machine actually enforces, `~/.devplane/policy.toml` included.

Each suggestion is in the vocabulary that tool's rules use: a command prefix with the `*` after the
subcommand, a directory glob for a path rule, a domain for `WebFetch`.

- **Offline**, like the rest of `explain`: it opens the store read-only and asks no agent anything.
- **Nothing is written for you.** Paste what you want into your agent's own settings.
- A rule is offered only where one covers the set, and only after a command has interrupted you
  **three times** — which is what keeps `Bash(rm -rf node_modules)` off the list.

`--dir` scopes it to one project; the default is the repository you are standing in.

### `devplane rules`

Which of your repositories is missing a rule.

`devplane explain` answers for one call on one machine. This asks the same question across every
registered project — the half a person with six repositories actually has.

```console
$ devplane rules 'Bash(curl:*)'
Bash(curl:*)

  saas      devplane  missing     /Users/you/saas/devplane.toml
  saas      agent     has it      /Users/you/saas/.claude/settings.json
  core-lib  devplane  covered     by `Bash(*)`
  core-lib  agent     missing     /Users/you/core-lib/.claude/settings.json
  ai-tool   devplane  unreadable  expected `=` at line 4 — run `devplane check`
  ai-tool   agent     missing     /Users/you/ai-tool/.claude/settings.json

Paste this:
  Bash(curl:*)

into:
  policy.never_auto   /Users/you/saas/devplane.toml
  permissions.deny    /Users/you/core-lib/.claude/settings.json
  permissions.deny    /Users/you/ai-tool/.claude/settings.json
```

**Two files per project, never conflated.** A `devplane.toml` prohibition is what *Devplane* will
refuse; a `permissions.deny` entry is what the *agent* will refuse. They answer different questions
and every row says which one it is about.

`covered` means a wider rule already speaks for every call yours names — `Bash(*)` covers
`Bash(curl:*)` — which is a different fact from having the rule. `unreadable` means the file does not
parse, so [none of its settings are in effect](../permissions/); its fix is `devplane check`, not a
paste.

Coverage uses the same containment procedure `devplane check` uses to find redundant rules. A pair it
cannot decide is reported as missing rather than guessed at.

With no argument, the reverse question:

```console
$ devplane rules
Rules some projects have and others do not

  Read(./.env)  agent
     5 have  ai-tool, core-lib, infra, mobile, saas
     1 do not  docs-site
```

A rule held by exactly one project is shown too: it is either the project that learned something or
the one that is over-restricted.

#### There is no apply-to-all

Devplane writes nothing here, and there is no flag that does.

Across **15 549 agentic pull requests in 148 projects**, adding instruction files raised the merge
rate by ≥20% in **27.7%** of them and lowered it in **26.35%** — what separated the two was what the
rules said, not that they were there. Pasting one rule into six repositories is a coin flip.

An agent here also runs as your user and can read the daemon's token, so a route that edited a
permission file would be reachable by the party the file exists to bound.

| Flag | What |
|---|---|
| `--ask` | the rule belongs in the *ask* list rather than the deny list, which changes the key the paste names |
| `--json` | the same four states per project, which is what the board reads |


## Looking

### `devplane ls`

The working set — sessions in play, and anything asking for you. Alias: `ps`. This is also what
plain `devplane` does.

| Flag | What |
|---|---|
| `--all`, `-a` | include sessions nothing has been heard from for hours — editor tabs, usually |
| `--project <name>`, `-p` | one project; matches any part of the name, so `mat` finds `matter-kit` |
| `--needs-you` | only what is waiting on a human |

### `devplane inbox`

What needs a human, most urgent first. Derived from state rather than stored, so it is correct after
a restart. Ranked by level, then age, oldest first.

**And when nothing needs you, it says what the day came to.** Every board in this category is built to
be full; an empty list rendered as an absence is the surface failing at the moment it has the best
thing it will ever have to say.

```console
$ devplane inbox
since you last looked · 16h

Clear.

  14 decisions taken in your name today — 2 by you, 9 by a rule, 3 by nobody.
  2 questions waited for you, the longest for 4h.
  6 things Devplane ran for you — gates, pipelines, pull requests.

  next · a question in 7c has a deadline of 40m

The daemon keeps watching and the pipelines keep running. Nothing repeats if it
restarts, so closing this costs nothing.
```

Four things about it, and each is a decision rather than a layout:

- **What the tool did is counted apart from what was decided for you.** Gates and pipelines are
  Devplane doing the job it was configured to do; folding them into the headline would make it a
  measure of how much the tool did.
- **There is no rate anywhere in it.** *You answered 4 of 17* is one word from a performance metric
  about you. This names what happened, never how well.
- **The last line checks before it reassures.** Where a question has a deadline and will end without
  you, it says that instead — a close that is false is worse than none.
- **A day with nothing in it gets a sentence**, not a table of zeroes.

The hairline above the list is how long it has been since you **read** the inbox, which is not the
same as how long since a page fetched it: the board polls every couple of seconds and a poll is not a
look. There is no line at all before your first look, or for a gap under a minute.

**And the items raised inside that gap are marked `new`.** The word, not a colour — nothing here is
distinguishable by colour alone, because red and amber cannot be separated under deuteranopia at any
usable lightness. The daemon decides which rows are new, so `devplane inbox` and the board cannot
disagree about where the boundary falls, and nothing is marked before your first look: there is no
boundary yet to be on the far side of.

```console
$ devplane inbox
since you last looked · 16h

 ! Keep the legacy /v1/login route?   [question] new
   saas · 3m
   Gate failed after the agent claimed completion   [gate_failed]
   core-lib · 5h
```

### A list that can be read

**A long inbox is a list that stops being read exactly when it matters.** Oversight modelled as a
finite attention budget is an inverted U: at a reviewer capacity of 50, escalating 72 % of actions
lets 22 % of danger through and escalating **100 %** lets **39 %** through. So above twelve rows the
inbox summarises — and **nothing is ever hidden without a count**.

Two things happen, both of them reversible by reading the row:

**Folding.** Items of a kind whose members are interchangeable to you — issues assigned, reviews
requested, stalled sessions, context warnings — collapse into one row naming the kind, the project
and how many. The foldable kinds are enumerated in the code, and **a kind not on that list is always
listed in full**, so a kind added later is unfoldable until somebody decides otherwise.

**Four kinds are never folded**: a question, an abandoned question, a permission, and a human step in
a pipeline. Each needs an answer only you can give, and a summary row is a question nobody saw with a
number beside it.

**Inhibition.** Where one raised item is a *named consequence* of another — a project whose
configuration will not parse explains the refusals in it; a gate that is not answering explains calls
with no verdict; a leaked agent explains the sessions it is holding — the consequence is counted on
the cause's row instead of listed. The pairs are enumerated. Nothing is inferred from two things
going wrong in one project at the same time, because that is a correlation and this is a claim.

```console
$ devplane inbox
 ! Keep the legacy /v1/login route?   [question] new
   saas · 3m

   7 × issue_assigned  in saas
   4 × stalled  across projects
     folded because the list is long — `devplane inbox --json` has every id

     3 more items — the gate is not answering, so these calls got no verdict
```

**The arithmetic is the guarantee**: rendered + summarised + counted-on-a-cause equals raised, over
every input, asserted across every kind. An inbox short enough to read renders exactly as it did
before any of this existed, with no summary rows at all. And a suppressed consequence returns the
moment its cause resolves — nothing is stored, so a cause that is gone explains nothing.

**Whether folding was right is measured.** `devplane attention` reports, per kind, how often it was
folded and how often it was folded **and then acted on once opened** — a kind always folded and never
acted on is one nobody needed as a row, and a kind folded and then acted on is one the summary was
standing in front of.

A permission item also names **the rule to paste so it is never asked again**, and where it goes:

```console
 ! Permission: Bash [permission]
     cargo test --lib policy
     never asked again: "permissions": { "allow": ["Bash(cargo test *)"] }
     covers 6 calls like it · paste into /Users/me/work/saas/.claude/settings.json permissions.allow
```

**A pattern only where there is a count behind it.** One interruption says this command needed a
decision and says nothing about the shape of the calls like it, so the offer is the exact call until
this machine has seen three distinct ones in the same family — the same threshold
[`devplane explain --replay`](#devplane-explain-replay) uses, and `covers` is how it shows its
working. Past fifty it stops counting and says `50+`.

The rule offered is the **narrowest** that covers what it was composed from, never the permission the
agent asked for, and it is replayed against the call before you are shown it: a rule that would not
have decided it is refused rather than handed over. Where none can be, the item says which reason it
is — several commands in one call, a construct no prefix rule may approve, a tool whose rules take no
pattern, or a prompt Claude Code raised through its own dialog without naming the call.

**Nothing is written for you.** No command and no route edits `[policy]`, and there will not be one:
an agent on this machine runs as you and can read the daemon's token, so a write path to the rules
would be reachable by the thing the rules govern. The handover is a paste.

Two items are about the machine rather than about any run, and they are the only things Devplane
raises about **itself**:

```console
 ! The permission gate is installed and not answering [gate_down]
     No rule in any project is being enforced right now.
     it will not start: No such file or directory

 ! payments-api/devplane.toml will not load [config_broken]
     The rules this repository commits are not in force.
     TOML parse error at line 12, column 3
```

A broken gate and a quiet machine look identical from the outside: no hook arrives either way, so
the daemon runs the installed gate on a timer. A file that will not parse is the same failure one
repository wide — the last good rules are kept, and a daemon restarted against it has none to keep.

Both are critical and neither offers an action: the fixes are `devplane connect claude` and a text
editor, and this is not a product that rewrites your settings or your rules from a list.

### `devplane issues` · `devplane prs`

Every open issue, and every open pull request, across **every registered project** — read through
your own `gh`, grouped by project, with what needs you first:

```console
$ devplane prs
as hupe1980 · what needs you first

saas
  ◆ #142   Fix the flaky login test                     red  yours
    https://github.com/acme/saas/pull/142
  ◆ #150   Add rate limiting                            approved · green  review asked of you
    https://github.com/acme/saas/pull/150

core-lib
  ○ #31    Bump tokio                                   pending
    https://github.com/acme/core-lib/pull/31
```

The board has the same two lists behind `g`, and the counts in a project heading open them.

`◆` marks what is waiting on you: an issue assigned to you, a review requested from you, or your own
pull request that is red, has changes requested, or is approved and unmerged. A draft of your own is
never one of them — you marked it unfinished — but a review requested of you, or changes requested on
your own, reaches you through a draft. These are inbox items at normal urgency, so none of them
raises a desktop notification.

The daemon reads GitHub a few seconds after it starts and every five minutes after that. A project it
could not read keeps its last good numbers and is marked `· stale`. A project with no GitHub remote
is ruled out and asked again an hour later. `devplane doctor` says whose `gh` this is, when it last
read, and which projects were ruled out and why.

**Nothing here writes to GitHub** — every action is a link, and `devplane work start --issue <n>` is
how an issue becomes work.

`devplane issues --ready` asks a different question of one repository: which of its issues are
*offered* as work — the ones carrying `[github].ready_label` — which is the list
`work start --issue` picks from. It reads that repository live rather than serving the poller's
cached counts.

| Flag | What |
|---|---|
| `--ready` | only what this repository offers as work |
| `--label <l>` | default: `[github].ready_label`; implies `--ready` |
| `--cwd <path>` | which repository; implies `--ready` |

A project whose directory has no GitHub remote is asked once and then left alone.

### `devplane show <run>`

One run in detail: state, agent, model, surface, cost, tool calls, what it is blocked on, the agent's
own plan, recent tools, and — for a driven run — the last few things it said.

### `devplane tail <run>`

Follow what a driven agent is saying, like `tail -f`.

| Flag | What |
|---|---|
| `--thinking` | include its reasoning, where it streams any |
| `--history <n>` | how much of the conversation to print first (default 40) |

Only for runs Devplane drives. A session you started in a terminal or an editor is already showing
you its own transcript — `devplane focus` raises that window, and `tail` says so rather than
printing nothing for ever.

### `devplane search <query>`

Full-text search over tool commands, questions and errors across every session. What you type is a
phrase, not a query language.

The board has the same search — its own surface in the sidebar. A match names the session it
came from and opens it.

### `devplane work start --spec <path>`

The specification this work answers, relative to the repository — a file, or the folder your spec
tool wrote. Every gate stamps a fingerprint over every Markdown document under it and counts its
`- [ ]` task list.

**No methodology is learned**: no layout is detected and no section name is recognised, because the
frameworks in this category agree on none of them. The outline is the headings, the progress is the
boxes, and the `[gates]` commands you declare are what actually check the work.

A path outside the repository, or a folder with no Markdown in it, is an error at the point you can
still fix the typo. See [pipelines](/devplane/docs/pipelines/) for what the stamp buys you.

### `devplane rewind <run>`

Which of a session's files Claude Code's own checkpoint will **not** bring back.

Claude Code snapshots the files its own editing tools touch before each turn, and `/rewind` restores
them. Its documentation is explicit about the limit: *"files modified by bash commands are not
tracked."* That is the one class Devplane has a complete record of — every tool call the gate saw,
with the files the command named.

```console
$ devplane rewind s-4f2a
outside Claude Code's checkpoint for this session
  build/report.json
  notes.md

  a shell command named these for writing; /rewind restores only what
  Claude Code's own editing tools touched
```

It is a query over the decision log: no snapshots, no storage, no second copy of your files. It says
**named for writing** rather than *changed*, and the wording is deliberate — the gate sees a call
before the tool runs, so claiming the file changed would be an answer this evidence does not support.
A refused call is not listed, and a path nothing can pin to one file — a glob, a `~`, a variable — is
left out rather than printed as though it named a file.

### `devplane audit [id]`

What Devplane decided, and on whose authority. Narrow to a run or a piece of work by id. See
[the decision log](/docs/decisions/).

| Flag | What |
|---|---|
| `--limit <n>` | how many rows (default 50) |

**A row for an MCP tool call also says where that server came from**, which is the adjacent question
to *on whose authority*: `authority` answers who decided, and this answers what was acting and who
put it there.

```console
$ devplane audit
2026-09-20T14:02:11 rule    agent:tool.use   mcp__github__create_issue
                             ↳ mcp__github__create_issue was defined by: project
```

`project` means the server's definition came from a file in the repository — one a clone brought with
it — rather than from your own configuration. The other values Claude Code reports are `user`,
`plugin` and `sdk`, and **a value this build has never seen is printed as received** rather than
mapped to a guess.

**Nothing is derived from it.** Devplane reports where a tool came from; it does not decide that one
provenance is safer than another, and no rule may read the field. That judgement is yours, in your own
settings.

**Three states, and the middle one is the honest one:**

| The call | What the row says |
|---|---|
| `Bash(ls)` — cannot have a server | nothing, because there is no source to have |
| `mcp__github__create_issue`, source reported | `↳ … was defined by: project` |
| `mcp__github__create_issue`, nothing sent | `↳ …: where it was defined was not reported here` |

The third is a fact about the vendor and the version, not about the call: an MCP call whose origin is
unknowable must not read like an ordinary tool. That is the distinction the capability table draws
between *not probed* and *not supported*.

Requires Claude Code v2.1.274 or later; older versions send no such field, and **absent reads
differently from unknown**.

### `devplane attention`

Whether the inbox is worth reading, per kind. Every item Devplane raises is recorded, and every
resolution says what became of it.

```console
$ devplane attention
You answered 3 of the 619 decisions taken in your name.
There is no threshold for this ratio on its own. The published criterion (arXiv:2607.28317) is over
residual risk and needs an error rate nothing here can observe; this is the count, not a verdict.

kind               raised   acted  dismissed  elsewhere   open   acted
permission             41      36          0          4      1     90%
gate_failed             9       8          1          0      0     89%
refused                 3       3          0          0      0    100%
context_high           23       1         17          5      0      4%
stalled                12       0          2         10      0      0%
```

| Outcome | What it means |
|---|---|
| `acted` | you used one of the item's own actions — answered, allowed, denied, approved, retried, resumed |
| `dismissed` | you snoozed it. The clearest evidence a kind is too loud |
| `elsewhere` | it stopped asking on its own: the agent unblocked, the checks went green, you answered in a terminal |

**The first line is a different question from the table.** The table asks *is the inbox worth
reading* — its denominator is items Devplane raised. The line above it asks *is anything reaching you
at all* — its denominator is everything that happened in your name, including every action no
permission prompt was ever shown for. On a machine where each session runs in a mode that decides on
its own, the second number is much larger than the first, and nothing else will tell you.

**It is a count, and the threshold beside it is somebody else's result.** The idea comes from
published work that defines *vacuous* oversight — oversight statistically indistinguishable from
none — but that criterion is over **residual risk**, expected undetected errors per round, and needs
a per-agent error rate, a confidence distribution and an error-correlation structure. The fraction
you reviewed is one **input** to that model. Devplane has the fraction and cannot observe the rest
without knowing which of an agent's actions were wrong. So the paper is cited by identifier, and the
product does not tell you which side of a line you are on, because it does not know.

**And it will never rank anything by how confident an agent sounded.** The same work measures five of
six models' self-reported confidence as near-constant, AUROC ≈ 0.5, *"operationally useless"*, and
past a located threshold that ranking is worse than random.

Three numbers, not one score: `elsewhere` is ambiguous — the item was right that you were needed and
wrong about where — so it is reported rather than averaged away. A kind that is mostly `dismissed` is
a threshold to change.

| Flag | What |
|---|---|
| `--days <n>` | how far back to look (default 7) |

### `devplane watch`

Follow events as they arrive. Two kinds of frame, told apart on the wire: state changes, and
fragments of what a driven agent is saying.

### `devplane open`

Open the board in a browser. The token is handed over once in the URL and stripped from the address
bar, so it cannot end up in a screenshot or a bookmark.

### `devplane doctor`

Aliased as `devplane diagnostics`.

Whether the tool itself is telling you the truth: hook latency, when telemetry was last seen, the
roster, each channel's last error **with the date it happened**, and any project whose
`devplane.toml` will not parse — whose permission rules are therefore not in force.

**It says what is watched here, per vendor and per channel**, because *watched* and *driven* are two
different lists:

```console
watched
  a session you started yourself. Anything Devplane starts is driven over the protocol and reports in full. Checked 2026-09-21
  Claude Code     hooks       read           lifecycle, tool calls and the permission gate, all documented
  Claude Code     roster      read           the background roster names what the vendor's own daemon supervises
  GitHub Copilot  hooks       unproved       fourteen documented events; twelve are mapped and two are silent by decision
  GitHub Copilot  roster      not published  ~/.copilot holds flat process logs and no session roster
  Codex           hooks       not published  driven over the protocol only; no observation channel has been built
```

`read` means demonstrated end to end. `unproved` means the channels are read and that path has never
been run. `not published` means the vendor offers nothing to read — so an empty list for that vendor
means *Devplane cannot see it*, not *nothing is happening*.

`devplane ls` and the board read the same table, and both name the agents that appear only when
Devplane starts them.

**It also names the release this build's rule syntax was modelled on**, so a rule you write here and
a rule you write in Claude Code mean the same thing:

```console
gate
  rule syntax modelled on Claude Code 2.1.273
  prohibitions are Devplane's own and need no agreement from the agent
```

A fact with a date, not a warning — see
[the baseline](/docs/permissions/#the-release-the-rule-syntax-was-modelled-on).

**It runs the gate rather than reading about it.** No hook can enforce its own presence, so a
settings file containing the right line is evidence about a settings file. `doctor` writes a
throwaway project with one deny rule, runs the **installed command** with a real `PreToolUse`
payload, and says whether a refusal came back:

```console
claude code
  settings  ~/.claude/settings.json
  hooks     installed (22 events)
  gate      answering (21 ms, measured just now)
```

or, when something is wrong with it:

```console
  gate      INSTALLED AND NOT ANSWERING — every prohibition on this machine is inert
            /usr/local/bin/devplane hook
            it will not start: No such file or directory
            a hook that does not answer never blocks — the provider carries on
```

The probe is recorded nowhere: a diagnostic that files decisions would make the audit trail worse
every time you checked the tool was working.

If the gate has been deciding while no daemon was running, `doctor` says how many decisions are
waiting to be filed. Starting the daemon files them.

It starts with **which model provider you are on**, because that decides which of Claude Code's own
supervision surfaces exist here at all. On Bedrock, Google Cloud's Agent Platform, Microsoft Foundry,
an Anthropic Console key or a corporate gateway they are unavailable, while hooks, OpenTelemetry,
workflows and skills all still work:

```console
provider
  Amazon Bedrock  (CLAUDE_CODE_USE_BEDROCK is set)
  Devplane is the only gate on this machine.
  off here   Remote Control · Routines (/schedule) · ultrareview · Code Review · Channels · …
  partial    auto mode — fewer models, and sessions start in Manual
  still on   hooks · OpenTelemetry metrics · workflows · skills and commands · sandboxing · …
```

On a claude.ai sign-in it is one line. It names the variable that decided, and a variable exported
empty reads as unset.

It also reads back **Claude Code's own auto-mode classifier**, because in that mode the thing
actually deciding is configured somewhere Devplane does not write:

```console
auto mode (Claude Code's own classifier)
  21 trusted entries · 17 allow · 70 soft deny · 1 hard deny
  your deny and ask rules resolve before it; it cannot override them
```

Devplane reads this and never writes it. If nothing is configured it says so: an unconfigured
classifier trusts only the working repository and its remotes, which is the usual cause of denials
people blame on the agent. `/auto-mode-setup` in Claude Code drafts the entries.

### `devplane modes`

**Which of your projects is deciding without you.** One line per live session, grouped by project,
least-supervised first.

```console
$ devplane modes
a 60s timer on your questions was set for you, in managed settings — after that, whatever is
selected is submitted, and your own settings cannot turn it off
  /Library/Application Support/ClaudeCode/managed-settings.json

cim-rs
  cim-rs-e0             auto  seen 2026-09-19T18:34:4…

devplane
  devplane-d1           auto  seen 2026-09-19T18:34:4…
  devplane-ba           not reported yet

3 of 18 live session(s) decide without you.
15 have not reported a mode yet — the hook that fires on every tool call does not carry one, so this fills in at their next prompt.
Devplane reads the mode and never sets it: change it where the session runs.
```

**And what can answer without any of them.** `askUserQuestionTimeout` auto-continues an unanswered
question, *submitting whatever options you had already selected*. Permission prompts are exempt, and
the setting takes one of four values — `60s`, `5m`, `10m` or `never`, which is its default.

The line appears whenever the setting has a value, and **the value decides how it reads**:

- a duration you set is **dim** — your own choice, reported back to you;
- a duration **somebody else** set is **yellow**, because the scope is user *or managed* and the
  vendor's own settings UI hides that row while managed settings are in force;
- **`never` is dim and says your questions wait until you answer them.** It is not a finding and not
  an alarm: somebody wrote down that nothing may answer for you, and an administrator deploying it is
  hardening the machine rather than taking your attention.

Nothing at all means nothing was set — which behaves like `never`, and is a different fact from
somebody having chosen it.

Devplane reads it and never writes it. Managed settings also compose from drop-ins, a policy helper
and a Windows registry chain, so an absent line means *nothing in the two files I read*, never
*there is no timer*.

**And one source outranks both files, reported per session.** The `CLAUDE_AFK_TIMEOUT_MS`
environment variable takes precedence over the setting and turns auto-continue on **even where the
setting is unset or `never`**; setting it to `0` closes each question immediately rather than turning
the timeout off. Devplane reads it from the environment the session was started in — its `SessionStart`
hook runs as a child of that session — and prints it under the session it belongs to:

```
  a1b2c3                default (asks you)
                        this session was started with a 60s timer on its questions, from its
                        environment — it overrides your settings, including `never`
                        set in CLAUDE_AFK_TIMEOUT_MS
```

**Zero is a sentence rather than a duration**, because *after 0s* is arithmetic where an explanation
is owed:

```
                        questions in this session are closed immediately — nobody is asked, and
                        whatever happens to be selected is submitted
```

A session with such a clock sorts to the top, and the summary says how many have one — because
*every session that has reported asks you* is about the permission mode, and a timer answers
whatever the mode says.

**A session Devplane did not see start has no reading, and that is a third state.** Sessions older
than `devplane connect` are counted separately and named, because *not read* must never print as
*nothing is set*. They fill in when they restart.

**Why this exists.** Claude Code's `auto` mode reviews actions with a classifier rather than a
person, and it is the built-in starting mode on Pro, Max and Team. With six repositories open there
is no way to find out which of them are running that way — each session knows its own mode and
nothing collects them.

**Three states, and they do not read the same:**

| Shown | Means |
|---|---|
| a mode in red, e.g. `auto` | no person is asked about an ordinary call in this session |
| a mode in yellow with `(unknown to this build)` | the session reported a mode this version does not recognise, so whether anybody is asked **cannot be said**. It is not assumed to be the supervised one |
| `not reported yet` | nothing has said. Eleven of Claude Code's hook events carry the permission mode and `PreToolUse` — the one that fires on every tool call — is not among them, so a busy session may genuinely not have mentioned it. It fills in at the next prompt |

**"Seen", not "since".** No hook announces a mode *change*, so the timestamp is when Devplane first
heard the session at that mode — at best the person's next prompt after they switched. A line saying
*"in auto since 09:14"* would be a claim about a moment nothing here witnessed.

**Read, never set.** Devplane does not change a permission mode, for the same reason it writes no
permission rule: an agent on this machine runs as the same user, so a path that could loosen
supervision would be reachable by the party being supervised. Change it where the session runs.

Live sessions only — the question is present tense. A run that ended yesterday in `auto` is history.

`--json` gives the same answer with a count of `unsupervised`, `unknown` and `unreported`.

## Acting

### `devplane focus <run>`

Raise the editor window that owns a run's directory. When no window has it open, says so and prints
the resume command rather than claiming success.

### `devplane attach <run>`

Hand the terminal to the real agent, resuming its session — `claude --resume <id>`, or
`claude attach <id>` for a background session, whose daemon has its own way in. Replaces this process
rather than nesting one inside it.

### `devplane dispatch <prompt…>`

Start an agent and give it something to do.

| Flag | What |
|---|---|
| `--agent <id>` | `claude`, `codex`, `opencode`, `gemini`, anything in `agents.toml`, or a command line |
| `--cwd <path>` | where it runs (default: here) |
| `--to <projects>` | comma-separated project names. **Turns this into a fan-out** |
| `--mode <draft\|gate\|pr>` | how far it may go without you (default `draft`) |
| `--apply` | without it, the preflight prints and nothing is sent or opened |

#### One prompt, several repositories

```console
$ devplane dispatch --to api,web,jobs "bump deps and run gates"
  refused   web   uncommitted changes — commit or stash first

  draft
  nothing runs — each project opens with the prompt typed, not sent

  2 projects × one run

  run with --apply
```

**Every refusal is named before anything is written**, never as one failure after three successes.
The reasons are *not trusted*, *uncommitted changes*, *cannot run that agent*, *its `devplane.toml`
will not parse* and *over the parallel-run ceiling*; each carries the command that fixes it.

A project name that matches nothing **stops the whole dispatch** rather than sending to the rest.

**Cost is in runs, never in currency.**

#### The three positions

| `--mode` | Runs without you | Still stops |
|---|---|---|
| `draft` | nothing. Each project opens with the prompt typed and **not sent** | everything. You send it |
| `gate` | the agent works; your gates run when it stops | a failing gate, a prohibited permission, a question, the pull request |
| `pr` | the above, plus opening a pull request | **merging. Always.** |

**No position merges, and no flag adds one. No position weakens a permission**: each accepted target
goes through the same single-target dispatch, so a call inside a fan-out meets the rules a call
outside one meets, and the same rule is credited.

#### Draft is chosen for you above three targets

```console
$ devplane dispatch --to api,web,jobs,billing --mode gate "bump deps"
  draft
  draft was chosen for you: more than 3 targets
  `--mode gate` was not honoured, for the reason above
```

Above three, a mistake stops being a wrong row and starts being several repositories. You are told
the choice was made for you rather than discovering it.

### `devplane batch [id]`

A fan-out as one row with one outcome per target. Questions first, then failures, then the rest.

```console
$ devplane batch
b-01a0bbb6  bump deps and run gates
  to_gate       still going
  needs you     api             bump deps and run gates
  failed        web             bump deps and run gates
  done          jobs            bump deps and run gates
  refused       billing         untrusted
```

**There is no aggregate**: no percentage, no `n/m`, no pass rate, no colour on the batch itself. Four
green, one red and one asking is what a fan-out looks like, and a summary over that hides the row
that needs you.

Targets the preflight refused stay on the record.

**Four states look empty and are four different facts**: composed and not sent; every target refused
so nothing ran; the daemon stopped mid-flight; finished having produced nothing.

### `devplane say <run> <prompt…>`

Send another prompt to a run Devplane drives.

### `devplane answer <ask>`

Answer something an agent asked you — a permission or a question, with one command.

The id comes from `devplane inbox`. **It is not a session id**: it is the ask's own token, and it
outlives the process that asked. An answer given tomorrow morning still reaches the agent — through a
resumed session where the original one is gone, which the reply says out loud.

| Flag | What |
|---|---|
| `--allow` | allow it — a permission |
| `--deny` | refuse it — a permission; also what you get if you say nothing else |
| `--option <what the agent offered>` | one of the agent's own options, spelled as it wrote it |
| `--custom <your words>` | where the agent offered an *Other* box; beats `--option`, which is the agent's rule rather than ours |
| `--field <id>` | which question, when the agent asked several at once |

Option ids belong to the agent — one calls it `allow`, another `proceed_once` — so `--allow` and
`--deny` are resolved against the options it actually offered rather than a guessed string.

**There is no way to dismiss one.** An agent that asked and was told nothing proceeds on nothing,
which is the failure this exists to prevent.

**It answers exactly once.** Two surfaces, two devices, one agent: the first answer wins and the
second is told who gave it, rather than the agent hearing two different things.

### `devplane asks`

Everything an agent has asked you, and what became of each one.

Open ones first, oldest first among those — a queue of what is owed to you rather than a feed.
Settled ones follow with the sentence that ended them: **you answered it**, **a clock refused it
after 4h — set in devplane.toml**, or **nobody answered**. No two of those read alike, because
telling them apart without opening a transcript is the whole point.

**And below them, the questions the agent asked and moved past.** A session Devplane only watches can
ask you something and then start another tool call without an answer. Answered and abandoned produce
identical state, so this is its own row: the question as the agent wrote it, what it was choosing
between, and what it did instead:

```
Questions the agent asked and moved past
  Keep the legacy /v1/login route?
    nobody answered — it did `Bash: cargo test` instead
```

They carry **no answer action**: the tool call is over, and a button there would offer something
nothing can deliver. They appear in `devplane inbox` as `question_abandoned`, at normal level, and
offer the session instead.

**An empty list says which vendors it cannot speak for.** The derivation is Claude Code's own hook
events and no other vendor documents an equivalent, so *no question was abandoned* and *Devplane
cannot see abandoned questions for copilot* are two different sentences.

**One case it does not claim.** A question your agent's own auto-continue timer closed *submits* the
options that were selected, so the tool succeeds and from outside it is indistinguishable from an
answer. `devplane modes` is the surface for that half — it names which sessions have such a clock,
how long it is, and who set it.

### `devplane snooze <id>`

Hide a run's — or a piece of work's — inbox items for a while. Takes either id; the inbox prints
whichever one an item is about.

**A snooze covers only the kinds being asked when you take it.** Something new turns up, you see it.

A snoozed run stays on the board and a snoozed piece of work keeps running: this hides a request, not
the thing itself. Both ids are accepted because the items Work produces outlive their sessions — a
pull request that goes red hours later has no run left to quieten.

| Flag | What |
|---|---|
| `--minutes <n>` | default 60; `0` un-snoozes |

On the board a snooze offers to put it back. Answering does not: a permission or a question reaches
the agent and nothing recalls it, so the page says so rather than offering a control that cannot
deliver.

## Work

### `devplane work start <title…>`

Make an isolated checkout, prepare it, put an agent in it, and run the project's gates when the agent
says it is finished.

| Flag | What |
|---|---|
| `--kind <k>` | `quick` (default), `chore`, `bug`, `feature` — the kind chooses the pipeline |
| `--agent <id>` | overrides `[project] default_agent` |
| `--cwd <path>` | which repository |
| `--no-worktree` | work in the repository itself rather than an isolated checkout |
| `--issue <n>` | start from a GitHub issue; its body arrives marked as an untrusted report |

### `devplane work list`

Where everything is. Alias: `ls`.

### `devplane work show <id>`

Phase, pipeline stepper, runs, cost, and what each check actually said.

### `devplane work verify <id>`

Run the project's gates now. A read: it never changes a phase a person asked for.

### `devplane gate run`

Run this repository's `[gates]` now and report the verdict. Repository-scoped, daemon-free, and made
to be read by something other than a person — a Spec Kit hook invokes it and reports what it said.

| `state` | Exit | Means |
|---|---|---|
| `verified` | 0 | every declared command passed |
| `failed` | 1 | one did not, and it is named |
| `no_gates` | 1 | this repository declares no checks |
| `config_unreadable` | 1 | `devplane.toml` would not parse, so nothing ran |
| `unknown_gate` | 1 | `--name` asked for a gate nothing declares |

`--cwd <path>` picks the repository; `--json` gives `gate`, `state`, `passed`, `summary` and
`commands[]`.

**`--name <gate>` runs one gate from `[gates.named]` instead of `check`.** That is how you try a gate
before a pipeline depends on it — until now the only thing that could run one was a pipeline step, so
a gate you had written down could be validated and listed and never executed.

```console
$ devplane gate run --name notes
ok        bash scripts/concepts-check.sh exit 0

notes passed
```

`expect = "fail"` is honoured, so a gate that did exactly what it was asked to do is not reported as a
failure. A name nothing declares prints the names that are declared and exits 1, because a missing
gate that reads as success is the failure that layer exists to prevent.

**Only `verified` exits 0.** A workflow reading the exit code alone would otherwise treat an empty
`devplane.toml` as a green build. It decides on exit codes: no specification is read and no prose is
graded, and it records nothing — [`devplane work verify`](#devplane-work-verify-id) is the one that
leaves a row.

### `devplane speckit install`

Register that gate as a Spec Kit extension hook in `.specify/extensions.yml`, so a workflow whose own
commands only report gets a verdict from outside the agent.

| Flag | What |
|---|---|
| `--event <hook>` | which of the twenty hook points; default `after_implement` |
| `--dry-run` | print the entry and write nothing |
| `--anyway` | write the hook even where the repository declares no gate to run |

It writes the file only when there is none, and otherwise prints the entry and the key to add it
under. That file is committed, may carry other people's hooks, and round-tripping it through a YAML
parser would keep the entries and lose the comments.

`--anyway` exists because the default is to refuse: a hook that calls a gate nothing declares fails
every time it fires.

The entry carries no `condition` and no `priority`. A hook with a non-empty condition is skipped
unless the running Spec Kit evaluates it, and `priority` is documented upstream as not sorted on.

### `devplane work approve <id>`

Release a pipeline waiting at a declared human step.

### `devplane work retry <id>`

Hand the failures back to the agent once more, past the project's bound. Offered only while the
session that wrote the code still exists; recorded as a human decision, and counted.

### `devplane work resume <id>`

Pick work back up after the daemon that was running it stopped, **against the same agent-side
conversation**.

A restart takes every agent process with it. The Work row, the branch and the worktree all survive
one, and a resumable agent keeps the conversation — so this reconnects to that conversation rather
than starting a new one. The difference is not cosmetic: a fresh session would pay a second time to
rediscover what the first one already knew, and would look at the half-finished code in the worktree
without the context that produced it.

It is offered in the inbox on the `interrupted` item, and only when it can actually work: the run
recorded the id its agent answers `session/resume` on, and Devplane is not already holding a session
for it. Where the agent cannot resume — it does not advertise the capability, or it has forgotten the
session — this says so instead of silently starting again.

Deliberately not automatic on startup. Resuming spends money and runs an agent in a repository, and
doing either because a machine rebooted is a decision nobody made.

### `devplane work finish <id>`

| Flag | What |
|---|---|
| `--remove-worktree` | remove the isolated checkout too |
| `--force` | discard uncommitted changes in it |

Without `--force`, removal refuses to destroy uncommitted or unpushed work.

Finishing records **why** the work counts as finished, and there are four answers: the gates passed,
a reproduction gate reproduced, the project declares no gates, or a person decided it anyway. All
four are legitimate. What cannot happen is a finished piece of work whose record is silent about
which — because then a reviewer cannot tell a checked claim from an unchecked one, and the unchecked
one looks exactly like this product's headline sentence.

### `devplane work export <id>`

The done certificate: what was checked, against which commit, and how to check it yourself.

The point is what it does **not** require. A reviewer with the output needs git and a shell — not
Devplane, not your database, not your machine, and no reason to trust any of them. They clone the
repository it names, check out the commit it names, run the commands it names, and compare the
outcomes.

```console
$ devplane work export w-01a0 > cert.md
```

Paste it into a pull request. With `--json` you get the same facts as an
[in-toto statement](https://in-toto.io/Statement/v1) carrying a `DoneCertificate` predicate, for
another tool to read — including a `verification.steps` array with the commands, a `rederivable`
list naming which fields a reader can check for themselves, and `signed: false`.

**It is deliberately unsigned, and that is the interesting part.** The industry standard for this
shape — in-toto attestations carrying SLSA provenance — has the producer inside the trust boundary
by construction; SLSA's own specification says the build platform *"is trusted to have correctly
performed the operation."* A signature proves an attestation was not altered, never that the claim
inside it is true. This does not ask to be believed at all, which is only possible because a gate is
a handful of commands rather than a build platform: where SLSA must attest because re-running a build
is infeasible, this can instruct, because re-running a gate is a paste.

The certificate states its own limits, in the artifact rather than here, so they survive the paste:
it is evidence that these commands ended as recorded against this commit — not that the work is
correct, that the commands check the right things, or that the record was never altered.

Four things it will tell you that a green tick would hide:

- **A commit nobody else can fetch.** If the commit is on no remote, the instructions cannot work for
  you, and it says so where the instructions are rather than in a footnote.
- **A dirty tree.** Then the commit does not fully describe what was checked, said where the commit
  is shown.
- **Which attempt.** *"Passed on the fourth try"* and *"passed"* are different sentences.
- **Unticked tasks beside a green gate.** Both numbers go in and neither is judged against the other.

Exporting work that is not finished is a fair question with an honest answer: it says where the work
is and what its last gate said, rather than erroring or inventing a certificate.

### `devplane library`

Prompts and skills reused across projects, in the vendors' own formats. Five verbs — `list`, `diff`,
`report`, `install`, `sync` — documented on their own page: [Library](/docs/library/).

## Setup and health

### `devplane trust [path]`

Allow Devplane to start agents in a repository. Required once per repository, because a headless
agent runs that repository's own hooks and MCP servers without asking — and it prints what those are
before it asks you.

```console
$ devplane trust .
  starting an agent here loads this repository's own:

  hook    ./scripts/guard.sh
          runs on every PreToolUse in this repository
  mcp     notes
          `notes-mcp` has no version pin, so starting an agent fetches and
          runs whatever is published at that name today
  skill   Bash
          this skill pre-approves Bash for whoever installs it, so those
          calls are not asked about
  policy  Bash(python:*)
          `python` runs whatever follows `-c`, so this approves `python -c
          '…'` — any code at all. Claude Code reads it the same way

  Trust this repository? [y/N]
```

It **reports and refuses nothing** — a `PreToolUse` hook is a normal thing to ship. It is also
shallow on purpose: it does not read the script a hook names.

A repository that declares none of this says so in one line and asks nothing further.

| Flag | What it does |
|---|---|
| `--dry-run` | Print the same thing and trust nothing. The form to run on somebody else's repository before you clone it. |
| `--yes`, `-y` | Trust without asking. For scripts, and for a directory you wrote. |
| `--json` | The findings as data, with `trusted` and `unreadable`. |

Without `--yes`, a non-interactive stdin is an error rather than a silent yes.

### `devplane check [path]`

Read the repository's `devplane.toml` and say what it will do — and refuse what cannot work. Offline
and daemon-free, so it runs in CI. Exits non-zero on an error.

It also names the three things a person cannot get by reading the file: a rule that covers nothing, a
rule that grants more than it reads as granting, and a path denied for reading that is still
writable. `--json` returns the same read-back the board shows under `,`.

### `devplane agents`

The agents Devplane can drive, and — for those it has started — what each advertised at its
handshake: `resume`, `load`, `list`, whether it declares a mode, whether it needs signing into, with
the date measured. An agent never started has no such line.

### `devplane mcp`

Serve Devplane's read-only surface to an agent over MCP, on stdio. Register it with your agent as a
command MCP server.

*Not listed in `devplane --help`*, along with `devplane statusline` and `devplane hook`: all three
are surfaces for a machine rather than commands anybody types, and a help listing is for a person
working out what the tool does. They work exactly as documented here.

```json
{ "mcpServers": { "devplane": { "command": "devplane", "args": ["mcp"] } } }
```

Four questions, and nothing that acts:

| Tool | Answers |
|---|---|
| `inbox` | What needs a human right now, across every project. Worth asking before your agent asks *you* something you have already been asked. |
| `work` | A piece of work: phase, gate verdicts, the specification it answers. |
| `explain` | What the gate would decide about a call, and which rule decides it — **before** running it, so a refusal costs nothing. |
| `audit` | What Devplane decided and on whose authority. |

**It is read-only because it implements no mutating tool** — not because anything is labelled.
`readOnlyHint` is metadata a client may act on and constrains no server, so it is not what this
rests on. Anything that acts still goes through a person.

Two things worth knowing. Every payload is framed as **a report containing other people's text** —
commands an agent wrote, build output, issue bodies — because this surface is a conduit. And an
`explain` asked here is **recorded** in `devplane audit`: a read-only interrogation is also a way to
probe for a command the rules happen to allow. The CLI's `explain` stays offline and unrecorded — a
person at a terminal is not the party the rules govern.

### `devplane connect claude` · `disconnect claude`

Install or remove hooks and telemetry, in your user settings, with a backup.

| Flag | What |
|---|---|
| `--statusline` | also wrap your status line — the only source of subscription rate limits |

### `devplane serve`

Run the daemon in the foreground. Every other command starts it in the background as needed.

| Flag | What |
|---|---|
| `--port <n>` | default `47831`; `0` asks the OS for any free port |

When the default port is taken by something else, the daemon takes another one and clients follow via
`~/.devplane/daemon.json`. A port you asked for explicitly is never silently swapped.

**A second daemon on the same home is refused, and a crashed one does not lock you out.** A daemon
that was killed leaves its record behind and the pid gets reused, so Devplane checks the pid is
actually a Devplane process before believing it. A stale record is reported and ignored. If the
process table cannot be read at all, it refuses and names the file to delete.

### `devplane stop`

Ask the daemon to stop — over its own API, with the bearer token, reaching the same graceful path as
ctrl-C. **The agents it started are stopped first**, and waited for. It does not signal a pid: a
record left behind by a crash names a pid the operating system may since have given to something
else.
