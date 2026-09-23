# Devplane

**Devplane records who decided, when nobody asked you** — a person, a rule, a classifier, a timer, or
nobody — across every project and every coding agent on your machine.

**Devplane never approves a tool call.** Your agent's own permission system does that, in its own
settings, where it is authoritative. Devplane can *prohibit* and *defer*, because being stricter than
your agent is free — and it shows you the request and who needs to answer it. There is no way to
express an approval: the type does not exist.

Your agents now run in modes that decide for you. A classifier reviews the call, a timer answers the
plan, a question gets dropped when the agent moves on — each one a deliberate trade of your attention
for throughput, and none of them leaves you a list. This is that list: one page, every project, what
was decided in your name and on whose authority, plus the questions nobody ever put in front of you,
answerable once from one place.

It **watches** the sessions already running on your machine — terminal, VS Code, desktop — and
**drives** any agent that speaks the
[Agent Client Protocol](https://agentclientprotocol.com).

Those are two different lists:

| | Watched — you started it | Driven — Devplane started it |
|---|---|---|
| **Claude Code** | yes: hooks, telemetry, roster, status line | yes |
| **GitHub Copilot** | hooks and telemetry, **not yet proved end to end** | yes |
| **Codex · OpenCode · Gemini** | not yet | yes |

Driving is the easy half — an agent Devplane started reports through the protocol by construction.
Watching a session you opened yourself is fully demonstrated for Claude Code only. `devplane doctor`
says what is watched here, per vendor and per channel.

And a session ends while the work does not. When a check goes red two hours after the agent stopped
there is no session left to show it, so the unit here is a **Work**: the branch, the project's own
checks, the pull request, the cost — and a finished one leaves a certificate whose commands a reviewer
can re-run without trusting this tool.

One binary. Local-first, loopback only, no account and no cloud relay.

|  | |
|---|---|
| 📋 | **One page, every project** — what needs you, ordered by what is waiting rather than by repository |
| ⚖️ | **Every decision on the record** — a person, a rule, a classifier, a timer, or nobody |
| 🙋 | **Questions nobody put in front of you** — including the ones an agent asked and walked away from |
| 🚫 | **Never approves a tool call** — it can prohibit and defer, and the type cannot express an allow |
| ✅ | **Verified done** — the project's own checks, with a certificate a reviewer can re-run |
| 🔌 | **Any ACP agent** — Claude Code, Codex, Copilot, OpenCode, Gemini |
| 🔒 | **Nothing leaves the machine** — no CDN, no web font, no analytics, no telemetry |

**[Documentation → hupe1980.github.io/devplane](https://hupe1980.github.io/devplane)**

```console
$ devplane ls
8 projects · 23 sessions · 5 working · 2 need you · 4 idle · $4.18
12 quiet (nothing heard for hours) — devplane ls --all

saas  ·  4 issues · 2 PRs (1 needs you)
  ◆ 7c         vscode     62%   $1.04   3m  Keep the legacy /v1/login route?

core-lib  ·  2 sessions  ·  1 PR
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace
  ○ 4f         vscode     12%   $0.02  41m  waiting for a prompt

mobile
  ◆ 04         cli         –    $0.12   8m  Permission: Bash rm -rf node_modules
```

Two lines carry most of the value. **Grouped by project**, because that is the unit you think in —
nine sessions open on one repository are one line of context, not nine rows that differ by a hash.
And **quiet sessions are counted, not listed**: a machine that has been running agents all week has
editor tabs whose processes are still alive. A session that is *doing* something or *asking* you
something is always listed, however long it has been at it; everything else is on the board while it
is still today's business and counted afterwards. A quiet session that starts asking for something
joins the working set immediately. The numbers add up — every session is in exactly one of them.

**GitHub is on the same board.** Half of what is waiting on you is not a session — it is the issues
and pull requests on your repositories. Devplane reads them through your own `gh` for every
registered project: the heading carries the counts, the board's **issues and pull requests** surface
lists them across every project, `devplane issues` and `devplane prs` print the same two lists, and an
issue assigned to you or a review requested from you is an inbox item. A draft of your own is not —
you already said it is unfinished. Nothing is ever written to GitHub from a list; every action is a
link.

> **Status: early, but the whole loop runs** — watching, driving, verified work, exportable done
> certificates, declared pipelines,
> resumable sessions, the decision log, and an inbox that measures whether it is worth reading.
> Built against Claude Code 2.1.273 on a machine with 23 live sessions.

## 🤔 Why

Five VS Code windows, five agents, and no way to know which one is stuck. Claude Code's own
`claude agents` shows background sessions in one directory; the VS Code Agents window shows what VS
Code started; neither spans your terminal, your editor and the desktop app at once, and none of them
knows what *done* means for your project.

Devplane watches all of them, on documented interfaces, and stays out of the way.

## 📦 Install

```sh
# a prebuilt binary: macOS (Apple Silicon and Intel), Linux, Windows
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh

# or run it without installing anything, if you have Node
npx devplane ls

# from source, needs Rust 1.90+
cargo install devplane
```

An `npx` run and an installed binary share `~/.devplane/` and produce one daemon — the same Devplane,
not two.

Inside Claude Code, as a **read-only** plugin — the MCP surface and a skill that explains what to ask
it, with nothing in it that changes anything:

```sh
claude plugin marketplace add hupe1980/devplane
/plugin install devplane@devplane
```

On macOS use the installer rather than the releases page: the binaries are not notarised, and macOS
quarantines a file based on what downloaded it — a browser sets that flag and `curl` does not.

[Install guide →](https://hupe1980.github.io/devplane/docs/install/)

## 🔁 The loop, in one screen

```sh
devplane ls                 # the working set — works immediately, no setup
devplane connect claude     # add live state: hooks + telemetry, into your user settings
devplane connect copilot    # the same for GitHub Copilot: one file, and two lines to export
devplane inbox              # only what needs a human, most urgent first
devplane issues             # every open issue across your projects, what needs you first
devplane prs                # every open pull request, the same way
devplane open               # the board in a browser, updating live

devplane trust .            # shows the hooks, MCP servers and skills you are about to allow
devplane work start "fix the flaky login test" --kind bug
devplane work show <id>     # where it got to, what it cost, what the checks said
devplane rewind <run>       # files a shell command wrote past Claude Code's checkpoint
devplane audit              # what Devplane decided, and on whose authority
devplane attention          # whether the inbox is worth reading, per kind
devplane explain --replay   # which rule to write so it stops asking
devplane rules 'Bash(curl:*)'  # which of your repositories is missing that rule
devplane library diff       # which of your six copies of a skill drifted
devplane dispatch --to a,b,c "bump deps"   # one prompt, three repositories
```

`devplane ls` works before you connect anything: sessions are discovered from Claude Code's own
roster. Connecting is what adds cost, context usage, blocking and the permission gate.

**`devplane --help` sorts those thirty-five commands into the five errands people arrive with** —
see what is happening · what needs you and what happened without you · start and steer work · set up
a project · the daemon.

**And when nothing needs you, the inbox says what the day came to** rather than rendering an empty
box. How many decisions were taken in your name and by what authority, how many questions waited and
for how long, what will want you next — and a last line that says the daemon keeps watching, so
closing the tab costs nothing. Where something *would* stop without you, it says that instead.

```console
$ devplane inbox
since you last looked · 16h

Clear.

  14 decisions taken in your name today — 2 by you, 9 by a rule, 3 by nobody.
  2 questions waited for you, the longest for 4h.

The daemon keeps watching and the pipelines keep running. Nothing repeats if it
restarts, so closing this costs nothing.
```

The hairline is how long since you **read** it — a poll is not a look — and items raised inside that
gap are marked `new`, as a word rather than a colour.

**And a long inbox folds rather than scrolls.** Above twelve rows, kinds whose members are
interchangeable — issues assigned, stalled sessions, context warnings — collapse into one row naming
the kind, the project and the count; and where one item is a *named* consequence of another, the
consequence is counted on the cause's row. **Nothing is ever hidden without a count**: rendered plus
summarised plus counted-on-a-cause equals raised, asserted over every kind. A question, an abandoned
question, a permission and a human step are **never** folded — each needs an answer only you can
give, and a summary row is a question nobody saw with a number beside it.

There is no rate in it and there never will be: *you answered 4 of 17* is one word away from a
performance metric about you.

**One gate, two vendors.** The same `devplane.toml` rules govern Claude Code and GitHub Copilot,
evaluated in Devplane's own process and never translated into either vendor's configuration — so
`never_auto = ["Read(.env)"]` stops Claude's `Read` and Copilot's `view`, and both name the rule in
`devplane audit`. Copilot has no session roster, so there it is connect-first-then-see.

**One prompt, six repositories, one row to review.** `dispatch --to` sends the same intent to several
projects and reports every refusal — untrusted, dirty worktree, unparseable config — **before
anything is written**, never as one failure after three successes. Cost is stated in runs, not
dollars.

```console
$ devplane dispatch --to api,web,jobs,billing --mode gate "bump deps and run gates"
  refused   web   uncommitted changes — commit or stash first

  draft
  draft was chosen for you: more than 3 targets
  `--mode gate` was not honoured, for the reason above

  3 projects × one run
```

Above three targets **draft is chosen for you** — each project opens with the prompt typed and not
sent — and you are told, rather than discovering it. `devplane batch` shows the fan-out as one row
with one outcome per target, questions first, **with no percentage or pass rate anywhere**.

**No position merges, and none weakens a permission**: each accepted target goes through the same
single-target dispatch, so the same rules decide it and the same rule is credited.

**One library, every project.** The prompts and skills you reuse live in a directory you own, in the
vendors' own formats, unmodified. `devplane library diff` says which copies drifted and which way,
which projects lack one, and which frontmatter fields are a documented hard error on Anthropic's own
distribution paths. It invents no format, rewrites no artefact and grades nothing.

```console
$ devplane library diff review-findings
  DRIFT      payments-api    the project's copy was edited
  STALE      billing         the library moved; this copy is the one you installed
  UNRECORDED api             a copy is here that Devplane did not install
  ok         infra
```

Every command starts the daemon if it is not already running, and every command takes `--json`.

[Quickstart →](https://hupe1980.github.io/devplane/docs/quickstart/) ·
[The library →](https://hupe1980.github.io/devplane/docs/library/) ·
[CLI reference →](https://hupe1980.github.io/devplane/docs/cli/)

## ✅ Verified done

`work start` makes an isolated checkout at `.claude/worktrees/<slug>` on its own branch, runs your
setup command, copies the files you name, and puts an agent in it. **When the agent says it is
finished, Devplane runs your checks.** Green means a human should look; red means the failures go
back to that same session, bounded, and then you are asked — in the inbox, with the same failing
lines the agent was handed.

**Work that stops says why.** A red gate past its budget, a reviewer that kept objecting, a spending
ceiling and a chain that could not continue are four different inbox items, and the one offering to
hand the work back hands back what matches: the failing lines, or the reviewer's findings.

An agent that claims success without earning it reaches `failed`, never `review`.

**Two pieces of work editing the same files is an inbox item, not a merge conflict.** Isolated
checkouts are exactly as isolated as they sound, which is what lets two branches be locally correct
and jointly impossible. Devplane compares what each in-flight worktree has touched when work reaches
review, and names the files and the other work.

**A daemon restart does not lose the conversation.** `devplane work resume <id>` reconnects to the
agent's own session rather than paying again to rediscover what it already knew — over
`session/resume`, or `session/load` for agents that offer only that.

```toml
# devplane.toml — committed, so the definition of done is the project's, not the agent's.
[gates]
check   = ["pnpm typecheck", "pnpm test -- --run"]
on_fail = "feedback"        # feedback | escalate | ignore
max_feedback_rounds = 2

[policy]                    # this repository's rules, not the machine's
always_ask = ["Bash(git push *)"]   # Devplane defers these to you
never_auto = ["Bash(rm -rf *)", "Read(.env)"]   # and refuses these outright
# There is no allow list here. Approving a call would mean claiming your agent
# would have approved it too — a claim about somebody else's code that goes
# stale every release. Put grants in your agent's own settings, where they are
# enforced by the thing that owns them.

[pipelines.feature]         # agents checking agents
steps = [
  { role = "implement", agent = "claude", prompt = "implement", gate = "check" },
  { role = "review",    agent = "codex",  prompt = "review", findings = { back_to = "implement", max = 2 } },
  { human = "merge" },
]
```

`devplane check` reads that file and tells you what it will do — and refuses the mistakes that
otherwise surface several minutes and one model call later: a `back_to` naming a step that does not
exist, a gate nobody declared, an `include` that leaves the repository, a review loop with no check
behind it, and a permission rule that cannot match anything.

**And it lints the grants you wrote for your agent, which is where grants belong.** A rule that
**grants more than it looks like** — `Bash(python:*)` reads as a narrow permission for one
interpreter and approves `python -c '…'`, which is any code at all — is reported rather than
refused, because it is your agent that will honour it and yours to narrow. In a scan of 3,171 public
agent setups, **3.1 %** carried a grant of exactly that shape.

Reading a rule to tell you what it does costs nothing and cannot go stale. *Enforcing* one would
mean mirroring your agent's semantics for ever, which this project tried and stopped doing.

[Verified done →](https://hupe1980.github.io/devplane/docs/verified-done/) ·
[Configuration →](https://hupe1980.github.io/devplane/docs/configuration/)

## 📐 Spec-driven, with no format to adopt

**Spec-driven development gets the one thing the category leaves out.** Every spec tool ships a
consistency checker and none of them decides: Spec Kit's `/speckit.analyze` is *"STRICTLY
READ-ONLY"*, grades its findings and closes with a recommendation. If your tool has a CLI, drift is
already a gate —

```toml
[gates]
check = ["openspec validate --strict", "pnpm test -- --run"]
```

That gives you spec **integrity** — the tool checking its own artefacts — for free. Spec-*code* drift
needs something that reads both, which is a pipeline step with a severity threshold. A red drift
check then behaves exactly like a red test: the lines go back to the same session, and the work never
reaches `review`.

And a work item can name what it answers — a file, or the folder your tool wrote:

```sh
devplane work start "password reset" --kind feature --spec specs/001-password-reset
```

Every gate stamps a fingerprint over every document under it, so *checked against
`specs/001-password-reset`* stays a claim you can act on after a file moves — and counts the task
list, which is the one thing these tools spell the same way:

```
password reset flow   human  ✓implement › ✓drift › ▸merge   specs/001-password-reset
                                              20/31 tasks, 2 unanswered   gates green
```

**That pair is the point.** An agent's own account of its work references about one action in eleven,
so *gates green* beside *eleven boxes still open* is a sentence neither the exit code nor the agent
can produce alone. No methodology is learned: the outline is the Markdown headings, the progress is
the `- [ ]` boxes, and nothing here knows what a requirement is.

### 🧾 The done certificate

`done` is a claim, and a claim only you can check is one everybody else takes on trust. So it ships
as a portable artifact instead:

```sh
devplane work export <id> > cert.md     # paste into the pull request
devplane --json work export <id>        # an in-toto statement, for another tool
#                                       # or open the work on the board: one button, same markdown
```

It names the repository, the commit, the gate commands, each command's outcome, and the digest and
size of each command's output — then it names the steps:

```
## Check this yourself

    git clone git@github.com:acme/widgets.git && cd widgets
    git checkout 4f2a9c1e8b3d7a5069fe2c14b8d93a70e5c6f182
    cargo test
```

**A reviewer runs that, and nothing in it passes through Devplane.** Which is also why it is
unsigned: the standard for this shape — in-toto attestations carrying SLSA provenance — keeps the
producer inside the trust boundary, and its own spec says the build platform *"is trusted to have
correctly performed the operation."* This does not ask to be believed. That is only possible because
a gate is a handful of commands rather than a build platform: where SLSA must attest because
re-running a build is infeasible, this can instruct, because re-running a gate is a paste.

It also says the things a green tick would hide — the commit is on no remote so you *cannot* check
it, the tree was dirty so the commit is not what ran, it passed on the fourth attempt — and it states
its own limits in the artifact, so they survive the paste: evidence that these commands ended as
recorded against this commit, not that the work is correct.

[Pipelines →](https://hupe1980.github.io/devplane/docs/pipelines/)

## 🖥️ The board

`devplane open` serves one page from the daemon on loopback, and **it opens on what needs you**.

![What needs you: one list across every project, each row naming its project and how long it has waited](https://raw.githubusercontent.com/hupe1980/devplane/main/site/static/inbox.png)

One list, every project, ordered by what is waiting rather than by which repository it belongs to. A
project with six idle sessions and nothing to decide ranks below a project with none running and a
red check from last night — not because anything is weighted, but because idle sessions raise nothing
to answer.

Three empty states, because they are three different facts: *nothing needs you* is the tool working,
*Devplane has not answered recently* means what is on screen is not current, and *some projects could
not be read* means the list is narrower than it looks.

Sessions are still there — **Sessions**, under *see what is happening* in the sidebar. Every watcher in
this category can show you those — Claude Code ships `claude agents` itself and does it better — so no
effort goes into making them prettier than a terminal table.

The sidebar is grouped under the same four errands `devplane --help` uses: the errand is the heading,
and each item under it is a noun.

**It works on a phone**, over Tailscale or any private network, because *what needs me* is a question
people ask away from their desk. Everything is served from the one binary — no CDN, no web font, no
analytics — so nothing has to load, and the layout at phone width is a different arrangement rather
than the same one shrunk.

<img src="https://raw.githubusercontent.com/hupe1980/devplane/main/site/static/narrow.png" alt="The same page at phone width: the sidebar laid out as a scrolling strip of short labels across the top" width="330">

**Every action is a button**, and there are no keyboard shortcuts.

A permission also carries the rule that stops it being asked again — the narrowest one covering the
calls this machine has seen, and the file to paste it into, which is your agent's own
`settings.json`. Nothing writes it for you.

**Start work** tells you what it will do before it does it, and refuses an untrusted repository with
the command that fixes it. It produces a **draft per project** — the vendor's own window, opened with
the prompt typed and not sent — because a fan-out that half fires is the one failure a control plane
cannot take back. Starting agents outright is `devplane dispatch --to a,b,c --apply`, in a terminal
that can show you what happened.

**Library** on the board is the same thing as a matrix — *what is installed where*: one row per prompt
or skill, one column per repository, and a word in every cell. *Edited here* and *library moved on* are the same yes-or-no and
opposite instructions, so nothing here is a tick.

Items that name work you were about to do anyway — a red pull request, a reviewer asking for changes,
a spent feedback budget — carry a `claude-cli://` link that opens an agent in the right repository
with the prompt already typed. It sends nothing: Claude Code fills the box and shows
`Prompt from an external link` until you press enter.

## 📊 Is the inbox worth reading?

A control plane is a filter, so Devplane measures its own:

```console
$ devplane attention
kind               raised   acted  dismissed  elsewhere   open   acted
permission             41      36          0          4      1     90%
gate_failed             9       8          1          0      0     89%
refused                 3       3          0          0      0    100%
context_high           23       1         17          5      0      4%
stalled                12       0          2         10      0      0%
```

`context_high` is dismissed four times in five — that threshold is wrong, and now you can see it.
`acted` is recorded the moment you answer, so it is counted rather than inferred.

And the other half — **which rule to write so it stops**:

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

1104 of the 1239 calls Devplane leaves to your agent · paste into permissions.allow in your agent's settings
```

Offline, like the rest of `explain` — no daemon, no agent, no bill. Each suggestion is in the
vocabulary that tool's rules use: a command prefix, a directory glob, a domain. A rule is offered
only where one really covers the set, and only after a command has interrupted you three times —
which is what stops a list like this recommending `Bash(rm -rf node_modules)`.

`refused` is the one row about a session that is **not** blocked. A refused agent does not stop, it
tries something else — so a rule that is too tight and a rule that is working look identical on the
board, and the difference only shows up on the bill. Five refusals in one run says which rule keeps
stopping it.

## ⏳ A question outlives the agent that asked it

An agent asks you something. You are not at the desk; the machine reboots; the daemon stops. **The
question is still there** — still in the inbox, still answerable — and answering it resumes the
session the agent left behind and delivers what you chose.

```console
$ devplane asks
7c3f9a21  Keep the legacy /v1/login route?
    question · it waits — nothing answers this but you
    devplane answer 7c3f9a21 --option 'Keep it'
```

The id is the ask's own token, not a session id: an answer addressed to a session is undeliverable
the moment the session is gone, which is what every restart leaves behind. The answer is written
down **before** anything is delivered, so a crash between *answered* and *acted* replays as answered
rather than as ask-again.

**Nothing ends an unanswered ask unless your project asks for that.** No default timeout, no
"proceed", and no re-asking — re-asking would be Devplane composing a prompt in your name.

```toml
[questions]
deadline = "4h"   # never (the default) | 90s | 30m | 4h
```

When it fires the agent is told *no*, and the audit row names **a clock** with the duration and the
file that set it.

**And a question the agent gave up on is on the record.** When a session Devplane watches asks you
something and then moves on without an answer, that is a row — the question as it was written, what
it was choosing between, and what the agent did instead — rather than a row that quietly disappears.
It offers no answer, because the call is over; it offers the session.

**And Devplane reports the clocks it did not set.** `devplane modes` names your agent's own question
timer per session — the duration, where it came from, and whether you chose it — including the
`CLAUDE_AFK_TIMEOUT_MS` environment variable, which overrides the setting and turns auto-continue on
even where your own settings say `never`. A session whose questions close immediately says so in
words and sorts to the top. **The clock only runs while Devplane is up**: a question that waited overnight with
the daemon stopped comes back with its whole window ahead of it, because a deadline bounds how long a
question waits for somebody who could have answered it.

What became of an ask is a sentence rather than a status — *delivered to the waiting agent*,
*delivered into a resumed session*, *a clock refused it after 4h — set in devplane.toml*, *nobody
answered*. No two read alike, because telling them apart without opening a transcript is the point.

## 🚫 Devplane never approves

It can **prohibit** a call and **defer** one to you. It cannot approve one — `Verdict` has no `Allow`
variant, so the type cannot express it.

Approving would mean claiming your agent would have approved it too: a claim about somebody else's
code that goes stale every release. Keeping it honest here meant mirroring Claude Code's rule
semantics, three compatibility floors, a differential harness and a release clock. It produced
**thirty-three** occasions when the mirror was wrong in the dangerous direction, at a measured
**$1,756–$3,511 a month** in probe spend. Prohibiting and deferring claim nothing about anyone, cost
nothing, and cannot decay.

So there are two lists, and they use **Claude Code's own syntax** — a prohibition moves between
`settings.json` and `devplane.toml` by cutting and pasting it:

```toml
[policy]
never_auto = ["Bash(rm -rf *)", "Read(.env)", "mcp__*"]
always_ask = ["Bash(git push *)"]
```

Rules are resolved **per repository**, from the checkout that *owns* the worktree — so a rule an
agent adds to its own branch changes nothing about what it may do. A spelling that cannot match
anything is **refused** rather than carried, because a deny rule that silently matches nothing reads
as protection and is none.

Commands are matched **per subcommand**, not against the whole line, so `never_auto = ["Bash(rm
-rf *)"]` stops `ls && rm -rf /`. It also looks **through the wrappers that run something else** —
`sudo`, `doas`, `exec`, `env`, `watch` — and matches a program's **name as well as its path**, so
`sudo rm -rf /` and `/bin/rm -rf /` meet a rule naming `rm`. Claude Code does neither; being stricter
is free when you cannot approve.

**And when it cannot read the line, it asks you.** `rm$IFS-rf x`, `$(echo rm) -rf x`,
`eval "rm -rf x"`, `sh -c "…"` and `curl … | sh` hide what runs behind something no matcher can
resolve. They are put in front of you with the reason, on Devplane's own authority rather than
credited to a rule that did not decide them — and only in a project whose rules could have applied,
so an inbox does not fill with questions nobody asked for.

A path rule also reaches **the files a command names** — the
operands of `grep`, `awk`, `jq`, `git diff` and twenty more, a path hidden in an option value like
`grep -f.env x`, everything under a directory a `grep -r` walks, and the target of a redirection.
That list is measured against a running Claude Code rather than transcribed: `xxd`, `zcat` and `less`
are **not** recognised by it and so are not in it.

**And a prohibition carries five more writers the vendor does not.** `cp`'s and `install`'s and
`rsync`'s and `ln`'s destination, every operand of `truncate`, and `dd`'s `of=` reach a protected file
through a command a keyword filter did not think of. Mirroring the vendor here meant
`never_auto = ["Edit(secrets/**)"]` refused `tee secrets/k` and `echo x > secrets/k` and allowed
`cp /tmp/a secrets/k` — so on the deny side, where the worst case is a prompt, it does not mirror.

**`Read` and `Edit` are two halves and you want both.** `Read(.env)` stops `cat .env` and
`echo x | tee .env`; it does *not* stop `echo x > .env` or `touch .env`, which are `Edit` business.
`devplane check` prints a note when only one half is present.

**Symlinks are followed from both ends.** A prohibition applies when either the link or its target
matches, and the rule's own path is resolved too: `/tmp` is a symlink on macOS, so `Read(//tmp/**)`
has to stop `cat /private/tmp/x` as well.

```console
$ devplane explain 'echo x | tee /etc/hosts'
undecided  Bash
        no rule answers this one, so the provider's own dialog decides and it reaches your inbox
```

`devplane explain` answers offline — no daemon, no agent, no bill — which is what you want while you
are still writing the rule.

**Your prohibitions hold in auto mode**, where a classifier approves routine calls and no permission
prompt ever appears: they go out on a hook that fires before every tool call in every mode.

**And two commands tell you what that means for you.** `devplane attention` opens with one line —
*you answered 3 of the 619 decisions taken in your name* — whose denominator is everything that ran,
not just what you were asked about. It is a count. There is no threshold for that ratio alone: the
published criterion it comes from is over residual risk and needs an error rate nothing here can
observe, so the paper is cited and never restated as a verdict.

**And `devplane modes` tells you which sessions are running that way.** `auto` is the built-in
starting mode on Pro, Max and Team; with six repositories open, each session knows its own mode and
nothing collects them.

```console
$ devplane modes
devplane
  devplane-d1           auto  seen 2026-09-19T18:34:4…
  devplane-ba           not reported yet

3 of 18 live session(s) decide without you.
```

A mode this build does not recognise prints as a question rather than as *supervised*, and a session
that has not reported one says so instead of hiding — the hook that fires on every tool call does not
carry the mode, so silence is common and is not a finding. Read, never set: Devplane does not change
a permission mode, for the same reason it writes no rule.

Grants belong in your agent's own settings, where its own permission system enforces them — and
`devplane explain --replay` tells you which one to write.

[The rule syntax →](https://hupe1980.github.io/devplane/docs/permissions/)

## 🌍 Which world is this machine in?

Claude Code's own availability matrix splits cleanly: everything it ships to **run** an agent works
on every provider, and everything it ships to **supervise, schedule, review and audit** one needs a
claude.ai sign-in. On Amazon Bedrock, Google Cloud's Agent Platform, Microsoft Foundry, a Console API
key or a corporate gateway, the vendor's whole supervision layer is off — and hooks, OpenTelemetry,
workflows, skills and sandboxing all still work, which is exactly Devplane's substrate.

```console
$ devplane doctor
provider
  Amazon Bedrock  (CLAUDE_CODE_USE_BEDROCK is set)
  Devplane is the only gate on this machine.
  off here   Remote Control · Routines · ultrareview · Code Review · Channels · analytics
  partial    auto mode — fewer models, and sessions start in Manual
  still on   hooks · OpenTelemetry metrics · workflows · skills · sandboxing · MCP servers
```

[Which surfaces →](https://hupe1980.github.io/devplane/docs/cli/#devplane-doctor)

## 🔍 Trust is a decision, so it shows you the evidence

`devplane trust` is the one deliberate act here: it lets headless agents start in a directory, and a
headless agent runs **that repository's own hooks and MCP servers** with no dialog of its own. So it
prints what those are before it asks.

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

  Trust this repository? [y/N]
```

In a scan of 3,171 public agent setups, **16.0 % carried a confirmed defect** of one of these kinds.
`--dry-run` prints the list and trusts nothing — the form to run on somebody else's repository before
you clone it.

It **reports and refuses nothing**: a `PreToolUse` hook is a normal thing to ship. It is shallow on
purpose, too — it does not open the script a hook names.

## 🤖 Your agents can ask it things

```json
{ "mcpServers": { "devplane": { "command": "devplane", "args": ["mcp"] } } }
```

Four questions — `inbox`, `work`, `explain`, `audit` — and nothing that acts. The useful one daily is
`explain`: an agent finds out *before* running a command that a rule refuses it, instead of burning a
turn. It is read-only because it **implements no mutating tool**, not because anything is labelled.
[MCP →](https://hupe1980.github.io/devplane/docs/cli/#devplane-mcp)

## ⚙️ How it works

```
  agent sessions                           devplane serve (the daemon)
  ──────────────                           ───────────────────────────
  terminal ─┐                          ┌─ hooks     → lifecycle, blocking, policy
  VS Code  ─┼─ hooks + OpenTelemetry ─►├─ OTLP      → cost, tokens, entrypoint
  desktop  ─┘                          ├─ roster    → discovery, background state
  claude --bg ── claude agents --json ─┤
  copilot ──── hooks + OTel traces ────┼─ SQLite    → events, runs, work, search
                                       ├─ messages  → what driven agents said
                                       └─ decisions → why anything was allowed
                                             │
                                    board · inbox · SSE
                                             │
                                    CLI  ·  browser  ·  your scripts
```

State is a pure reduction over observed events, so the board is rebuildable and the inbox is derived
rather than stored. The one thing that is *not* a reduction is the decision log: an observation can
be re-derived from the provider, but “this command ran because rule X allowed it” and “this pull
request exists because these checks passed” cannot be re-derived from anything, so they are appended
and never pruned.

Every row in it carries **on whose authority** — and there are five, because three of them are
things no other tool records:

| | |
|---|---|
| `person` | you were asked, and you answered |
| `rule` | one of your own `never_auto` or `always_ask` rules matched, and the rule is on the row |
| `timer` | **a clock decided**, because nobody answered in time |
| `nobody` | **asked, never answered, and the moment passed** — the run ended under the question, or the daemon stopped while it was waiting |
| `daemon` | Devplane doing what your project told it to: a gate ran, a pipeline advanced, a pull request opened |

There is no `classifier` — Devplane cannot attribute an individual call to a model's approval, and
`devplane modes` tells you which sessions run under one instead. There is no `unknown` either: a row
whose authority cannot be established is a row Devplane does not write.

[Architecture →](https://hupe1980.github.io/devplane/docs/architecture/) ·
[Security →](https://hupe1980.github.io/devplane/docs/security/)

## 🗂️ Layout

One crate.

| Path | What |
|---|---|
| `src/core/` | Types, the reducer, the attention engine, the permission policy, `devplane.toml`. **May not reach the outside world** — no `async fn`, no `.await`, no runtime, no database, no HTTP |
| `src/` | Everything that does: daemon, receivers, HTTP API, protocol client, gates, git, GitHub, SQLite, CLI |
| `ui/` | The interface — Svelte, built to a bundle the binary embeds. A surface is a directory under `ui/src/surfaces/` |
| `tests/purity.rs` | Fails the build if `src/core/` ever breaks that rule |
| `site/` | The documentation site (Zola) |
| `scripts/` | Fetching the third-party reference docs, and the checks that keep the claims honest |

That one rule is why the board is rebuildable, the inbox is derived rather than stored, and the
permission policy cannot fail open. A second crate would enforce it more strongly, by simply not
linking `tokio`, `sqlx` or `reqwest` — at the price of a second manifest and a second publish for
one binary. One crate and a check covers every way the rule realistically breaks.

## 🛠️ Development

Everything is a [`just`](https://just.systems) recipe — `just` on its own lists them.

```sh
just check                   # what CI runs: fmt, clippy, build, test
just verify                  # that, plus the claim and dependency ledgers and the site
just open                    # the board in a browser
just site                    # the documentation site, at http://127.0.0.1:1111
just ui                      # the interface from ui/dist on disk — rebuild the bundle, reload

DEVPLANE_HOME=/tmp/vp just vp ls    # an isolated instance, touching nothing of yours
```

`just make-og` and `just make-board` redraw the two images the site and this page use — the Open
Graph card from `scripts/og-card.html`, and the board screenshot from a throwaway daemon fed through
the real hook endpoints. Both carry the product's name, so both were wrong after the rename and no
check could read either.

`just channels` fails the build until every row of Claude Code's changelog that touches a channel
Devplane actually uses — hooks, permission modes, the settings deciding whether a hook is consulted
at all — is written down as covered or declined with a reason. Devplane mirrors nobody's permission
semantics, so a changed rule shape is the vendor's business and a changed hook contract is ours.

The protocol tests drive a real agent process — `examples/echo_agent` — rather than
a vendor's, and the GitHub tests parse captured `gh` output rather than calling GitHub. That is what
keeps them runnable on every commit: a suite that needs a subscription is a suite nobody runs.

`DEVPLANE_HOME` moves the database, token and daemon record; `CLAUDE_CONFIG_DIR` points `connect`
at a throwaway Claude Code config. Together they let you exercise the whole thing without going near
your own setup — including while your real daemon is running, because a second instance takes
another port rather than refusing to start.

`DEVPLANE_CLAUDE_BIN` points at a `claude` binary if yours is not on `PATH` — which is common, since
the VS Code extension ships its own copy and installs nothing.

`DEVPLANE_UI` points the daemon at a built `ui/dist` on disk, so an interface change is
`npm run build` and a reload rather than a rebuild and a restart — `just ui` is that with the path
filled in. The bundle compiled into the binary is what ships.

`tests/ui_bundle.rs` holds the interface: that every route a surface calls is one the daemon serves,
every token it uses is one the bundle defines, and nothing is fetched from outside the machine.
`ui/tests/render.ts` renders each surface with Svelte's server renderer and asserts on the result —
no test runner, because the alternative was three dependencies to check strings `render()` already
returns.

## 📓 Changes

[CHANGELOG.md](CHANGELOG.md) — breaking changes are called out at the top of each entry.

**Coming from Vibeplane**: the binary, `devplane.toml`, `~/.devplane/`, the `DEVPLANE_*` variables
and the `devplane:ready` label all renamed, and nothing migrates itself. The 0.6.0 entry has the
three commands.

## ⚖️ License

MIT OR Apache-2.0
