# Vibeplane

**The local-first control plane for AI coding agents.** One binary that **watches** the Claude Code
and GitHub Copilot sessions already running on your machine — terminal, VS Code, desktop — tells you
which ones need you, and **drives** any agent that speaks the
[Agent Client Protocol](https://agentclientprotocol.com): Claude Code, Codex, Copilot, OpenCode and
Gemini out of the box.

**[Documentation → hupe1980.github.io/vibeplane](https://hupe1980.github.io/vibeplane)**

```console
$ vibeplane ls
8 projects · 23 sessions · 5 working · 2 need you · 16 idle · $4.18
17 dormant (never reported) — vibeplane ls --all

saas
  ◆ 7c         vscode     62%   $1.04   3m  Keep the legacy /v1/login route?

core-lib  ·  2 sessions
  ● a1         vscode     88%   $0.41   2s  Bash: cargo test --workspace
  ○ 4f         vscode     12%   $0.02  41m  waiting for a prompt

mobile
  ◆ 04         cli         –    $0.12   8m  Permission: Bash rm -rf node_modules
```

Two lines carry most of the value. **Grouped by project**, because that is the unit you think in —
nine sessions open on one repository are one line of context, not nine rows that differ by a hash.
And **dormant sessions are counted, not listed**: a machine that has been running agents all week
has editor tabs whose processes are still alive. A dormant session that starts asking for something
joins the working set immediately.

> **Status: early, but the whole loop runs** — watching, driving, verified work, declared pipelines,
> resumable sessions, the decision log, and an inbox that measures whether it is worth reading.
> Verified against Claude Code 2.1.270 on a machine with 23 live sessions.

## 🤔 Why

Five VS Code windows, five agents, and no way to know which one is stuck. Claude Code's own
`claude agents` shows background sessions in one directory; the VS Code Agents window shows what VS
Code started; neither spans your terminal, your editor and the desktop app at once, and none of them
knows what *done* means for your project.

Vibeplane watches all of them, on documented interfaces, and stays out of the way.

## 📦 Install

```sh
# macOS and Linux
brew install hupe1980/tap/vibeplane

# any platform
curl -LsSf https://github.com/hupe1980/vibeplane/releases/latest/download/vibeplane-installer.sh | sh

# from source, needs Rust 1.90+
cargo install vibeplane
```

On macOS use one of the first two rather than the releases page: the binaries are not notarised, and
macOS quarantines a file based on what downloaded it — a browser sets that flag and `curl` and
Homebrew do not.

[Install guide →](https://hupe1980.github.io/vibeplane/docs/install/)

## 🔁 The loop, in one screen

```sh
vibeplane ls                 # the working set — works immediately, no setup
vibeplane connect claude     # add live state: hooks + telemetry, into your user settings
vibeplane connect copilot    # the same for GitHub Copilot: one file, and two lines to export
vibeplane inbox              # only what needs a human, most urgent first
vibeplane open               # the board in a browser, updating live

vibeplane trust .            # once per repository, before any agent starts in it
vibeplane work start "fix the flaky login test" --kind bug
vibeplane work show <id>     # where it got to, what it cost, what the checks said
vibeplane audit              # what Vibeplane decided, and on whose authority
vibeplane attention          # whether the inbox is worth reading, per kind
```

`vibeplane ls` works before you connect anything: sessions are discovered from Claude Code's own
roster. Connecting is what adds cost, context usage, blocking and the permission gate.

**One gate, two vendors.** The same `vibeplane.toml` rules govern Claude Code and GitHub Copilot,
evaluated in Vibeplane's own process and never translated into either vendor's configuration — so
`never_auto = ["Read(.env)"]` stops Claude's `Read` and Copilot's `view`, and both name the rule in
`vibeplane audit`. Copilot has no session roster, so there it is connect-first-then-see.

Every command starts the daemon if it is not already running, and every command takes `--json`.

[Quickstart →](https://hupe1980.github.io/vibeplane/docs/quickstart/) ·
[CLI reference →](https://hupe1980.github.io/vibeplane/docs/cli/)

## ✅ Verified done

`work start` makes an isolated checkout at `.claude/worktrees/<slug>` on its own branch, runs your
setup command, copies the files you name, and puts an agent in it. **When the agent says it is
finished, Vibeplane runs your checks.** Green means a human should look; red means the failures go
back to that same session, bounded, and then you are asked — in the inbox, with the same failing
lines the agent was handed.

**Work that stops says why.** A red gate past its budget, a reviewer that kept objecting, a spending
ceiling and a chain that could not continue are four different inbox items, and the one offering to
hand the work back hands back what matches: the failing lines, or the reviewer's findings.

An agent that claims success without earning it reaches `failed`, never `review`.

**Two pieces of work editing the same files is an inbox item, not a merge conflict.** Isolated
checkouts are exactly as isolated as they sound, which is what lets two branches be locally correct
and jointly impossible. Vibeplane compares what each in-flight worktree has touched when work reaches
review, and names the files and the other work.

**A daemon restart does not lose the conversation.** `vibeplane work resume <id>` reconnects to the
agent's own session rather than paying again to rediscover what it already knew — over
`session/resume`, or `session/load` for agents that offer only that.

```toml
# vibeplane.toml — committed, so the definition of done is the project's, not the agent's.
[gates]
check   = ["pnpm typecheck", "pnpm test -- --run"]
on_fail = "feedback"        # feedback | escalate | ignore
max_feedback_rounds = 2

[policy]                    # this repository's rules, not the machine's
auto_allow = ["Read", "Bash(pnpm test *)"]
always_ask = ["Bash(git push *)"]
never_auto = ["Bash(rm -rf *)", "Read(.env)"]

[pipelines.feature]         # agents checking agents
steps = [
  { role = "implement", agent = "claude", prompt = "implement", gate = "check" },
  { role = "review",    agent = "codex",  prompt = "review", findings = { back_to = "implement", max = 2 } },
  { human = "merge" },
]
```

`vibeplane check` reads that file and tells you what it will do — and refuses the mistakes that
otherwise surface several minutes and one model call later: a `back_to` naming a step that does not
exist, a gate nobody declared, an `include` that leaves the repository, a review loop with no check
behind it, and a permission rule that cannot match anything.

[Verified done →](https://hupe1980.github.io/vibeplane/docs/verified-done/) ·
[Pipelines →](https://hupe1980.github.io/vibeplane/docs/pipelines/) ·
[Configuration →](https://hupe1980.github.io/vibeplane/docs/configuration/)

## 🖥️ The board

`vibeplane open` serves one page from the daemon on loopback. Keyboard-first:

| Key | What |
|---|---|
| `j` `k` · `enter` | move · open what a session is saying |
| `1`–`9` | pick one of the answers the agent offered |
| `y` `n` · `r` | allow · deny · reply |
| `?` | why is this here — the decision log for that row |
| `⌘N` | dispatch: prompt, project, kind, and this project's own prompts |
| `⌘K` | jump to any project, session or piece of work by name |

`⌘N` tells you what it will do before it does it, and refuses an untrusted repository with the
command that fixes it. `⌘K` matches by subsequence, so `crlb` finds `core-lib`.

Items that name work you were about to do anyway — a red pull request, a reviewer asking for changes,
a spent feedback budget — carry a `claude-cli://` link that opens an agent in the right repository
with the prompt already typed. It sends nothing: Claude Code fills the box and shows
`Prompt from an external link` until you press enter.

## 📊 Is the inbox worth reading?

A control plane is a filter, so Vibeplane measures its own:

```console
$ vibeplane attention
kind               raised   acted  dismissed  elsewhere   open   acted
permission             41      36          0          4      1     90%
gate_failed             9       8          1          0      0     89%
refused                 3       3          0          0      0    100%
context_high           23       1         17          5      0      4%
stalled                12       0          2         10      0      0%
```

`context_high` is dismissed four times in five — that threshold is wrong, and now you can see it.
`acted` is recorded the moment you answer, so it is counted rather than inferred.

`refused` is the one row about a session that is **not** blocked. A refused agent does not stop, it
tries something else — so a rule that is too tight and a rule that is working look identical on the
board, and the difference only shows up on the bill. Five refusals in one run says which rule keeps
stopping it.

## 🔐 Permissions are Claude Code's, in full

A rule moves between `settings.json` and `vibeplane.toml` by cutting and pasting it — all three
lists, the `:*` form, gitignore paths with all four anchors, MCP server prefixes, and the
allow/deny asymmetries. `Edit(…)` covers every built-in tool that writes files and `Read(…)` every
one that reads them, so two rules cover eight tools.

Rules are resolved **per repository**, evaluated **deny → ask → allow**, and a spelling that cannot
match anything is **refused** rather than carried — because a deny rule that silently matches nothing
reads as protection and is none. A `!` rule is an exception scoped to the file it is written in, as
it is there.

The rules are checked against a *running* Claude Code, not only against its documentation.
`scripts/verify-permissions-diff.sh` **generates** calls, asks both sides for a verdict, and fails on
any disagreement — in either direction, because a rule that is quietly too strict is one people
replace with a broader rule.

Commands are matched the way Claude Code matches them: **per subcommand**, not against the whole
line. `never_auto = ["Bash(rm -rf *)"]` stops `ls && rm -rf /`, and `auto_allow = ["Bash(pnpm test
*)"]` will *not* answer for `pnpm test && rm -rf /` — that one reaches you as a question.

And some commands **no pattern rule may approve**, because Claude Code asks about them whatever the
rules say: an exec wrapper (`watch`, `setsid`, `ionice`, `flock`) that runs whatever follows it,
`find` with `-exec` or `-delete`, and anything past the length its command analysis reads. A rule
naming the exact command still works; `Bash(watch *)` does not, and `vibeplane check` says so rather
than letting it look like protection.

A path rule also reaches **the files a command names** — the target of a redirection and the operands
of the file commands Claude Code recognises — so `Read(.env)` stops `cat .env`, and `Edit(.env)`
stops both `echo pwned > .env` and `echo pwned | tee .env`. And an allow rule covers the command,
not what it writes: `Bash(echo *)` does not answer for `echo x > ~/.ssh/authorized_keys`.

**Symlinks are followed, and the two sides read the pair differently.** A deny applies when *either*
the link or its target matches, so a repository that ships `config/key -> ~/.ssh/id_rsa` does not
walk past `Read(~/.ssh/**)`. An allow applies only when *both* match, so a link pointing out of an
approved directory stops being approved.

```console
$ vibeplane explain 'echo x | tee /etc/hosts'
undecided  Bash
        no rule answers this one, so the provider's own dialog decides and it reaches your inbox
```

`vibeplane explain` answers offline — no daemon, no agent, no bill — which is what you want while
you are still writing the rule. The interesting answer is the `undecided` that looks like an allow:
a rule matched and still did not speak, because the command writes somewhere no rule covers.

**Your prohibitions hold in auto mode**, where a classifier approves routine calls and no permission
prompt ever appears. Denies and asks go out on a hook that fires before every tool call in every
mode; grants stay on the one that fires only when you were going to be asked anyway.

[The rule syntax →](https://hupe1980.github.io/vibeplane/docs/permissions/)

## ⚙️ How it works

```
  agent sessions                           vibeplane serve (the daemon)
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

[Architecture →](https://hupe1980.github.io/vibeplane/docs/architecture/) ·
[Security →](https://hupe1980.github.io/vibeplane/docs/security/)

## 🗂️ Layout

One crate.

| Path | What |
|---|---|
| `src/core/` | Types, the reducer, the attention engine, the permission policy, `vibeplane.toml`. **May not reach the outside world** — no `async fn`, no `.await`, no runtime, no database, no HTTP |
| `src/` | Everything that does: daemon, receivers, HTTP API, protocol client, gates, git, GitHub, SQLite, CLI, and the board (`ui/index.html`) |
| `tests/purity.rs` | Fails the build if `src/core/` ever breaks that rule |
| `site/` | The documentation site (Zola) |
| `scripts/` | Spec fetching and the checks that keep the claims honest |

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

VIBEPLANE_HOME=/tmp/vp just vp ls    # an isolated instance, touching nothing of yours
```

Two checks cost money and need a signed-in Claude Code, so they are not in CI and are run by hand
when permissions change: `just perms-live` (fixed probes) and `just perms` (generated cases, both
sides asked, any disagreement a failure).

The protocol tests drive a real agent process — `examples/echo_agent` — rather than
a vendor's, and the GitHub tests parse captured `gh` output rather than calling GitHub. That is what
keeps them runnable on every commit: a suite that needs a subscription is a suite nobody runs.

`VIBEPLANE_HOME` moves the database, token and daemon record; `CLAUDE_CONFIG_DIR` points `connect`
at a throwaway Claude Code config. Together they let you exercise the whole thing without going near
your own setup — including while your real daemon is running, because a second instance takes
another port rather than refusing to start.

`VIBEPLANE_CLAUDE_BIN` points at a `claude` binary if yours is not on `PATH` — which is common, since
the VS Code extension ships its own copy and installs nothing.

## ⚖️ License

MIT OR Apache-2.0
