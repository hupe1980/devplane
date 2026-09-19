# Devplane

**Where work that outlives a session lives.** One binary, one page, every project on your machine —
what landed, what is red, what a reviewer is waiting on, and which finished work can *prove* it is
done.

A session ends; the work does not. When a check goes red two hours after the agent stopped there is
no session left to show it, and that is the gap this fills. Devplane's unit is a **Work**: the
branch, the project's own checks, the pull request, the cost. It **watches** the sessions already
running on your machine — terminal, VS Code, desktop — and **drives** any agent that speaks the
[Agent Client Protocol](https://agentclientprotocol.com): Claude Code, Codex, Copilot, OpenCode and
Gemini out of the box.

**Devplane never approves a tool call.** Your agent's own permission system does that, in its own
settings, where it is authoritative. Devplane can *prohibit* and *defer*, because being stricter than
your agent is free — and it shows you the request and who needs to answer it.

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
registered project: the heading carries the counts, `g` on the board opens every issue and pull
request across every project, `devplane issues` and `devplane prs` print the same two lists, and an
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
# a prebuilt binary: macOS (Apple Silicon), Linux, Windows
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh

# from source, needs Rust 1.90+
cargo install devplane
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
```

`devplane ls` works before you connect anything: sessions are discovered from Claude Code's own
roster. Connecting is what adds cost, context usage, blocking and the permission gate.

**One gate, two vendors.** The same `devplane.toml` rules govern Claude Code and GitHub Copilot,
evaluated in Devplane's own process and never translated into either vendor's configuration — so
`never_auto = ["Read(.env)"]` stops Claude's `Read` and Copilot's `view`, and both name the rule in
`devplane audit`. Copilot has no session roster, so there it is connect-first-then-see.

Every command starts the daemon if it is not already running, and every command takes `--json`.

[Quickstart →](https://hupe1980.github.io/devplane/docs/quickstart/) ·
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

### The done certificate

`done` is a claim, and a claim only you can check is one everybody else takes on trust. So it ships
as a portable artifact instead:

```sh
devplane work export <id> > cert.md     # paste into the pull request
devplane --json work export <id>        # an in-toto statement, for another tool
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

Sessions are still there, below it and behind a rail click. Every watcher in this category can show
you those — Claude Code ships `claude agents` itself and does it better — so no effort goes into
making them prettier than a terminal table.

**It works on a phone**, over Tailscale or any private network, because *what needs me* is a question
people ask away from their desk. One HTML file, no CDN and no web font, so nothing has to load — and
where there is no keyboard the key legend is not shown, because it would be describing keys nobody
can press.

<img src="https://raw.githubusercontent.com/hupe1980/devplane/main/site/static/narrow.png" alt="The same page at phone width: the rail laid out horizontally, each item wrapping, no keyboard legend, and the five tools as tappable buttons" width="330">

Keyboard-first:

| Key | What |
|---|---|
| `j` `k` · `enter` | move · open what a session is saying |
| `tab` · `enter` on a work row | what it changed — diff, gate commands, the agent's account |
| `1`–`9` | pick one of the answers the agent offered |
| `y` `n` · `r` | allow · deny · reply |
| `?` | why is this here — the decision log for that row |
| `,` | what is configured — this machine, and every repository's `devplane.toml` read back |
| `g` | every open issue and pull request, across every project |
| `⌘N` | dispatch: prompt, project, kind, and this project's own prompts |
| `⌘K` | jump to any project, session or piece of work by name |

A permission also carries the rule that stops it being asked again — the narrowest one covering the
calls this machine has seen, and the file to paste it into, which is your agent's own
`settings.json`. Nothing writes it for you.

`⌘N` tells you what it will do before it does it, and refuses an untrusted repository with the
command that fixes it. `⌘K` matches by subsequence, so `crlb` finds `core-lib`.

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
-rf *)"]` stops `ls && rm -rf /`. A path rule also reaches **the files a command names** — the
operands of `grep`, `awk`, `jq`, `git diff` and twenty more, a path hidden in an option value like
`grep -f.env x`, everything under a directory a `grep -r` walks, and the target of a redirection.
That list is measured against a running Claude Code rather than transcribed: `xxd`, `zcat`, `less`
and `truncate` are **not** recognised by it and so are not in it.

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

[Architecture →](https://hupe1980.github.io/devplane/docs/architecture/) ·
[Security →](https://hupe1980.github.io/devplane/docs/security/)

## 🗂️ Layout

One crate.

| Path | What |
|---|---|
| `src/core/` | Types, the reducer, the attention engine, the permission policy, `devplane.toml`. **May not reach the outside world** — no `async fn`, no `.await`, no runtime, no database, no HTTP |
| `src/` | Everything that does: daemon, receivers, HTTP API, protocol client, gates, git, GitHub, SQLite, CLI, and the board (`ui/index.html`) |
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
just ui                      # the board served from ui/index.html — edit, reload, no rebuild

DEVPLANE_HOME=/tmp/vp just vp ls    # an isolated instance, touching nothing of yours
```

`just make-og` and `just make-board` redraw the two images the site and this page use — the Open
Graph card from `scripts/og-card.html`, and the board screenshot from a throwaway daemon fed through
the real hook endpoints. Both carry the product's name, so both were wrong after the rename and no
check could read either.

`just channels` fails the build until every row of Claude Code's changelog that touches a channel
Devplane actually uses — hooks, permission modes, the settings deciding whether a hook is consulted
at all — is written down as covered or declined with a reason. The rule ledger that used to sit
beside it is gone with the approval path: Devplane no longer mirrors anybody's permission semantics,
so a changed rule shape is the vendor's business and a changed hook contract is still ours.

The protocol tests drive a real agent process — `examples/echo_agent` — rather than
a vendor's, and the GitHub tests parse captured `gh` output rather than calling GitHub. That is what
keeps them runnable on every commit: a suite that needs a subscription is a suite nobody runs.

`DEVPLANE_HOME` moves the database, token and daemon record; `CLAUDE_CONFIG_DIR` points `connect`
at a throwaway Claude Code config. Together they let you exercise the whole thing without going near
your own setup — including while your real daemon is running, because a second instance takes
another port rather than refusing to start.

`DEVPLANE_CLAUDE_BIN` points at a `claude` binary if yours is not on `PATH` — which is common, since
the VS Code extension ships its own copy and installs nothing.

`DEVPLANE_UI` points the daemon at `ui/index.html` on disk, so editing the board is a browser reload
rather than a rebuild and a restart — `just ui` is that with the path filled in. The copy compiled
into the binary is what ships.

`tests/ui_contract.rs` holds the page: that it serves every field it reads, escapes every value it
prints, keeps its overlays dialogs that give focus back, and renders at all. That last one runs the
page's script against a stub DOM with an `<img onerror=...>` in every readable field and checks what
lands in the document; it needs `node`, and skips where there is none.

## 📓 Changes

[CHANGELOG.md](CHANGELOG.md) — breaking changes are called out at the top of each entry.

**Coming from Vibeplane**: the binary, `devplane.toml`, `~/.devplane/`, the `DEVPLANE_*` variables
and the `devplane:ready` label all renamed, and nothing migrates itself. The unreleased entry has the
three commands.

## ⚖️ License

MIT OR Apache-2.0
