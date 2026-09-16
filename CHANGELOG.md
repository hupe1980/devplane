# Changelog

Notable changes per release. Dates are UTC.

## 0.4.0 — 2026-09-16

**If you rely on `[policy]`, this is the most important release so far.** Four
ways a `never_auto` rule could read as protection and not fire, and one way the
gate could be made slow enough to stop deciding at all — every one a prohibition
that was written, was legal, and silently did not apply.

Alongside them, four things the tool knew and kept to itself: what trusting a
repository actually loads, which rules grant more than they look like, how old
the gate's own measurement is, and which files a shell command wrote past
Claude Code's checkpoint.

**Upgrading.** `vibeplane trust` now asks, so a script that calls it needs
`--yes`; without it, a non-interactive stdin is an error rather than a silent
yes. Your rules are unchanged but may prompt where they did not — that is the
point of **Fixed**. The decision log gains a column and keeps its rows.

### Fixed

- **A deny rule with a wildcard did not meet an operand with a wildcard.** The
  matcher asked whether either pattern matched the other *as text*, when the
  question is whether any filename satisfies both. `never_auto = ["Read(*.env)"]`
  did not stop `cat conf*`, though the shell expands it onto `conf.env`; the same
  held for `Read(*.pem)` against `cat server*` and `Read(*.key)` against
  `cat id_*`. It is now a real pattern intersection, and the property is
  brute-forced against every name over a small alphabet in the test suite.
- **A protected file could be pushed past the analysis bounds.** The parser
  stopped collecting after 64 files or four levels of nesting and said nothing,
  so `cat f1 … f80 .env` and a substitution nested deeply enough both reached
  *undecided* under `Read(.env)`. The bounds are higher and, more importantly,
  reaching one is now reported: a deny rule treats the part nobody read as
  though it could be anything.
- **Quoting got past a deny.** A shell removes quotes before choosing the
  program, so `r''m -rf /` runs `rm` — and a `Bash(rm *)` deny matched against
  the text as written did not see it. Deny and ask rules are now matched against
  the unquoted form as well. Allow rules are not: removing quotes can only make
  more text match, which on that side would approve a spelling nobody wrote a
  rule for. This is the quote-removal class from the **GuardFall** study of
  eleven coding agents' command guards, ten of which had it.
- **A long command line could make the gate slow enough to stop deciding.**
  Every rule re-parsed the whole command, so a forty-rule policy parsed it forty
  times — 97 ms on a long line, on the synchronous hook your session is blocked
  on, where a hook that reaches its timeout renders no decision and the call
  proceeds. A command is now parsed once however many rules ask about it, and a
  line past the 10,000 characters the analysis reads is answered without being
  parsed at all. Worst case measured: **0.11 ms**.

### Added

- **`vibeplane rewind <run>`** names the files a shell command wrote that
  Claude Code's `/rewind` will not restore — its checkpoint tracks only what its
  own editing tools touched. A read over the decision log; no snapshots and no
  copies of your files. It says *named for writing* rather than *changed*,
  because the gate sees a call before it runs, and it leaves out refused calls
  and paths nothing can pin to one file.
- **`vibeplane doctor` says how old the gate's measurement is.** Two numbers:
  `measured`, the last release the full differential run was green against, and
  `rows`, the last release whose rule-relevant changelog entries are all
  accounted for — the second is not a compatibility claim and says so. When no
  session reports a version it says that rather than implying no gap. The board
  shows it too, and only when there is one.
- **`vibeplane trust` lists what it is about to trust** before it asks: every
  `command` hook and its event, every MCP server with the unpinned ones named,
  every skill whose front matter pre-approves the shell, and every `[policy]`
  rule that grants more than it looks like. `--dry-run` prints it and trusts
  nothing; `--yes` skips the prompt. It reports and refuses nothing, and a
  repository that declares none of this says so in one line.
- **`vibeplane check` reports a rule that grants more than it looks like.**
  `Bash(python:*)` reads as a permission for one interpreter and approves
  `python -c '…'`. Claude Code reads it the same way, so this is reported and
  **not** refused. Two shapes only — the interpreter alone, and the code flag
  with a wildcard after it — so `Bash(python -m pytest *)` stays quiet. In
  `--json` as `overbroad`.
- **`vibeplane check` reports rules that provably do nothing**: an `auto_allow`
  a `never_auto` already covers, and a rule an earlier one in the same list
  covers. Answered by pattern containment, so `Read(.env)` is reported as
  covered by `Read(*.env)`. Silent on anything it cannot prove. In `--json` as
  `unused`.
- **An agent-facing index at `/llms.txt`**, held to the command list by a test.

### Changed

- **`vibeplane diagnostics` is now `vibeplane doctor`**, which is what the
  documentation has always called it. Both spellings still work.
- **The board reads as a table again.** Costs and context percentages sat behind
  a cell with no width, so one session on `cli` rather than `claude-vscode`
  shifted every number after it. Rows are also denser — twenty sessions fit on a
  screen — and quieter: the session id is no longer the brightest thing on a row,
  and a run whose state wants a person carries the same left accent bar as the
  inbox card above it.
- **The board works on a narrow screen**, where it used to scroll sideways.
  Below 46rem the scanning columns give way and the summary takes its own line.
- **Accessibility: the page declares a language, Tab stays inside an open
  dialog, and a dialog dims the page behind it in dark mode as well as light.**
- **Findings wrap** instead of handing the terminal one long line whose
  continuation lands under the label.
- **The gate is stricter in four places and looser in none.** Each fix above
  costs at most a prompt on a call that used to run unasked. If a rule of yours
  starts prompting where it did not, that is a call it was always meant to cover.

## 0.3.0 — 2026-09-15

The permission gate runs as a `command` hook instead of reaching the daemon, and
several rules decide differently. **Run `vibeplane connect claude` after
upgrading**: the old hook entries are installed and do not decide.

### Added

- **GitHub across every project.** The daemon reads every registered project's
  open issues and pull requests through `gh`, a few seconds after it starts and
  every five minutes after. Project headings carry `· 4 issues · 2 PRs (1 needs
  you)`; `g` or the counts themselves open both lists on the board, and
  `vibeplane issues` and `vibeplane prs` print them. **Nothing is written to
  GitHub** — every action is a link.
- **What GitHub is waiting on you for is in the inbox**: an issue assigned to
  you, a review requested from you, and your own pull request that is red,
  contested or approved-and-unmerged. Snoozable per project, and always normal
  urgency, so none of them raises a desktop notification. A draft of your own
  asks nothing — though a review requested of you, or changes requested on your
  own, still reaches you through one.
- **A project GitHub could not be read for keeps its last good numbers** and is
  marked `stale`, rather than showing them as fresh. One with no GitHub remote
  is ruled out and asked again an hour later. `doctor` gains a `github` section
  naming whose `gh` this is, when it last read, and what was ruled out and why.
- **A client restarts a daemon older than itself.** `/healthz` names the
  daemon's version; every command compares it to its own and restarts a stale
  one instead of hitting routes it does not have. Two releases of
  `vibeplane audit` and `vibeplane attention` answered 404 on machines that had
  upgraded without restarting.
- **`vibeplane doctor` runs the gate** with a probe call and reports whether it
  answered and how fast, instead of checking that a settings line exists. The
  probe is recorded nowhere.
- `vibeplane doctor` reports how many decisions are waiting in the spool.
- **A `gate_down` inbox item**, critical, raised when the daemon's periodic
  probe finds the installed gate not answering. No hook can enforce its own
  presence, so a broken one is otherwise indistinguishable from a quiet machine.
- `vibeplane explain` says why nothing answered: no rules here, rules that will
  not load, or rules that loaded and did not match. A `vibeplane.toml` that
  fails to parse is reported with its error.
- **The board is usable with a screen reader.** Every state glyph has a word
  beside it, the inbox, board and work sections are lists, every overlay is a
  dialog that gives focus back to whatever opened it, and there is one live
  region — polite, and silent unless its sentence changes. Each rule has a
  test.
- **`VIBEPLANE_UI` serves the board from a file on disk** instead of the copy
  compiled into the binary, so working on the page is edit-and-reload rather
  than rebuild-and-restart. `just ui` is that with the path filled in. The
  board is also served `Cache-Control: no-store`, so a reload gets the page
  that is there.
- **The status line reads the rest of its payload.** The session's model, its
  Claude Code version, the context window's size, every rate-limit window with
  its reset time — including the gateway spend limit — the session cost and the
  lines it changed. `vibeplane show` prints them; the shim is still optional.
- **`vibeplane doctor` gains a `gate` section**: the Claude Code release the
  matcher was tested against, and any session observed running a newer one.
- **`findings.only`** on a pipeline step: words that make a finding worth
  returning the work for. A findings file with no matching line is *nothing
  found*. For reporters that grade what they find, such as a spec-driven
  tool's analyser.
- `VIBEPLANE_DIFF_AXIS=dialect` runs the `PowerShell`, `Monitor` and `LSP`
  shapes against this matcher and prints a checklist to put to a running Claude
  Code. It is not a measurement and says so in its output.

### Changed

- **The gate decides in its own process and no longer needs the daemon.** An
  unreachable HTTP hook is a non-blocking error Claude Code walks past, so
  every rule was inert whenever the daemon was stopped. `vibeplane doctor`
  reports an HTTP gate as out of date.
- **A decision taken with no daemon is spooled** to
  `~/.vibeplane/pending-decisions.jsonl` and filed at the next start. Capped at
  20 000 rows, oldest dropped. Observations are not spooled.
- **A glob in a command's operands reaches a path deny.** `Read(.env)` now
  stops `cat .en?`, `cat .env*`, `head -c3 .en?` and `cat .en[v]`. A wildcard
  still cannot reach a name beginning with `.` unless the pattern spells the
  dot, so `cat *` is not one of them. Allow rules never grant on a glob.
- **`fmt` and `pr` read their operands**, so a `Read` deny covers them.
- **An allow rule must cover at least one part of a command.** A rule matching
  nothing no longer approves a command made entirely of read-only parts, and no
  verdict names a rule that did not fire.
- **`vibeplane explain` reads `~/.vibeplane/policy.toml`** as well as the
  project's rules, so it answers for the gate rather than for half of it.
- **`vibeplane.toml` is found without git.** A directory with no repository
  above it is governed by the file sitting in it. Inside a repository the root
  still wins.
- **`vibeplane work issues` is gone; `vibeplane issues --ready` replaces it.**
  `--label` and `--cwd` imply `--ready`.
- **GitHub Copilot's `powershell` tool is reported as `PowerShell`**, not
  `Bash`, so its commands are matched as PowerShell rather than parsed by a
  POSIX shell parser.
- `vibeplane check` labels an exception `except` rather than by the list it
  subtracts from.
- A rule is suggested for every command tool, not only `Bash`.

- **A closed editor tab is no longer a lost session.** Reconciliation marked
  every live run whose process had gone as `lost`, which is *critical*. On the
  development machine that made **twelve of the inbox's thirteen items**
  sessions nobody had touched for two days. One observation — the process is
  not there — now has two readings, and the reducer picks from what the run was
  doing: working or being asked something is a loss; idle or just-announced is
  a session that ended.
- **The board is the working set again.** A run counted as in play if it had
  *ever* reported, so a machine with one live session showed **thirty-eight
  rows**, twenty-five of them editor tabs reading "waiting for a prompt" since
  Tuesday. A session that is working or asking is always listed; everything
  else is listed while it is still today's business (six hours) and counted
  afterwards. `--all` lists them, and the counter now says "quiet" rather than
  "dormant (never reported)", which is what it now means.
- **The numbers above the board partition it.** `38 sessions · 1 working ·
  0 need you · 25 idle` alongside `10 dormant` double-counted ten sessions and
  left twelve failed ones unmentioned. `working + need you + idle + failed +
  quiet` is now the total, on the board page as well as the CLI, and failed
  sessions are printed when there are any.

### Fixed

- **A `PowerShell` rule resolves command names to their cmdlet and ignores
  case**, as Claude Code does. `never_auto = ["PowerShell(Remove-Item *)"]`
  stopped `Remove-Item` and let `rm`, `del`, `ri`, `rd` and `erase` through.
- **A `Bash(…)` rule reaches the `Monitor` tool** and **a `Read(…)` rule reaches
  `LSP`** — both named in Claude Code's rule-format table, neither reached
  before.
- **A `Read` deny reaches a path inside a git revision**, so `Read(.env)`
  refuses `git show HEAD:.env`.
- A path rule on a reader is told to become a `Read` rule rather than an `Edit`
  rule.
- `Monitor(npm *)` is reported as a rule that cannot work instead of being
  accepted and never consulted.
- The permissions page said `!` exceptions were not implemented while another
  section documented them. The refused-rules table is now checked against the
  gate by `cargo test`.
- **"No activity for 519 min" about a session the board showed as busy.** The
  quiet clock ran for any session the roster had given a status to — but
  without hooks installed there is no channel carrying activity, so the clock
  was measuring the installation rather than the session. A stall is now raised
  only for a session that has produced activity at least once.
- **A review requested from any team counted as a review requested from you.**
  `a && b || c` grouped as `(a && b) || c`, so every team's request matched —
  and a pull request's `reviewRequests` cannot say which teams you belong to
  anyway. GitHub is asked instead, once per pass, with `review-requested:@me`,
  which it resolves against your actual team membership.
- **A lost session could not be dismissed.** It was critical, offered `open`
  only — `focus` and `attach` have nothing to reach once the process is gone —
  and had no snooze. It now carries what the run was doing rather than
  overwriting that with "process not found at startup", and can be snoozed.
- **The gate's own probe could reach the board.** The hook declines to report
  the probe call `doctor` and the daemon's timer make, but the daemon did not
  decline to *file* one — so a probe spooled by an earlier build arrived at
  the next start as a `vibeplane-probe-<pid>` project, a working session and
  two audit rows. Both receivers now drop the probe session, and a start
  forgets any rows an earlier build left.
- **Every value the board prints is escaped.** Session names, branch names,
  pull request titles and permission options come from repositories and from
  models, and eighteen of the page's 149 interpolation sites did not escape
  them. No exploitable path was found. Two tests now enforce it — one reading
  the page, one rendering it with an `<img onerror=...>` in every field a
  person reads.
- **Run rows written by an earlier build were dropped from the board** when a
  later build added a field to the run's totals — fourteen sessions on the
  development machine, reported only by `doctor`. The totals now default any
  field a row lacks.
- **`stalled` fired for sessions that had never reported.** Without hooks a
  roster row emits no activity, so its idle clock measured nothing and every
  long turn on a machine that had not run `connect` was a stall. A stall is
  now raised only for a session that reports.
- Two sessions of one project with the same short name printed the same
  label twice on `vibeplane ls`; a repeated label now falls back to the id.
- The protocol conformance tests wrote their fixture's session files into the
  repository they ran from — 847 of them — instead of a scratch directory.
- **`vibeplane inbox` failed to decode any item without a session** — a piece of
  work whose runs have ended, which is the ordinary case for a pull request
  going red later. The command printed a decoding error instead of the inbox.
- **`max_runtime` never fired.** The elapsed time was computed through a string
  round trip that fell back to zero.
- An inbox row with no subject and no action no longer prints a bare `· `.
- `cli::run` uses the `Cli` it is given instead of re-parsing the process's own
  argv, so a test can drive a subcommand without a subprocess.

### Removed

- The `/vibeplane/policy` and `/vibeplane/copilot/gate` endpoints. The process
  that enforces a verdict records it through `/vibeplane/decided`.

## 0.2.0 — 2026-09-15

Several permission rules now decide differently, after checking them against a
running Claude Code. Read **Changed** before upgrading: two of them can make an
existing `never_auto` rule cover less than it did.

### Added

- `vibeplane explain --replay` — replays every tool call already observed against
  the current rules, and names the rule that would answer the ones that reached
  you. Offline; `--dir` scopes it to one project.
- `vibeplane doctor` names the model provider, and on Bedrock, Google Cloud's
  Agent Platform, Microsoft Foundry, a Console key or a gateway says which of
  Claude Code's own supervision surfaces are unavailable there.
- Permission items in `vibeplane inbox` name the rule that would have answered
  them.
- A pipeline `review` step falls back to the repository's `REVIEW.md`.
- `vibeplane check` notes when a `Read(path)` deny has no `Edit(path)` beside it.
- `just rows` checks every rule-relevant row of Claude Code's changelog against
  a committed ledger; `just perms-allow` and `just perms-deny` run one half of
  the differential harness.

### Changed

- **`Read(path)` deny no longer covers a shell redirect or `touch`.** It still
  covers a recognised file command that writes, such as `tee`. To protect a file
  from a shell, write both `Read(path)` and `Edit(path)`.
- **`mv` operands are writes.** `mv` removes its source, so an `Edit` deny stops
  it. `cp` is unchanged.
- Deny rules reach further: option values (`grep -f.env`), `git diff`/`git grep`
  operands, everything under a directory a `grep -r` or `cp -r` walks, and
  whatever `env` or `sudo` runs.
- Many more reader commands are covered — `awk`, `sort`, `od`, `strings`, `jq`,
  `base64`, `wc`, `diff` and others. `xxd`, `zcat`, `join`, `less`, `more` and
  `truncate` are **not**: Claude Code does not recognise them either.
- Symlinks resolve from both ends, so a rule naming `/tmp` also covers
  `/private/tmp`.
- A leading assignment that runs something — `DIRSTACKSIZE=$(id) ls`,
  `OPTIND=1/0 ls` — is no longer treated as a read-only command.
- No allow rule approves a command behind `env`, `eval`, `sudo`, `doas` or
  `exec`. Deny rules see through them, which is stricter than Claude Code and
  deliberate.
- `vibeplane ls` says why the cost and context columns are blank when nothing is
  connected.

### Fixed

- **`vibeplane open` served a board that never loaded.** Two `const hit` in one
  block scope is a `SyntaxError`, so the whole script failed to parse: the page
  rendered its chrome, said "connecting", and fetched nothing. The board's
  script is now parsed by the test suite.
- A multi-byte character in an option value crashed the permission matcher, and
  with it every tool call waiting on the hook.
- `sed -n 1p .env` and `grep -f pats.txt .env` named no file, so a deny rule on
  it did nothing.
- An allow rule naming an exact compound command approved nothing.
- `Bash(rule) trailing text` is reported as text after the closing bracket rather
  than as a missing one.

## 0.1.0 — 2026-09-14

First release. Watching Claude Code sessions, driving any Agent Client Protocol
agent, verified-done with project gates, declared pipelines, a per-repository
permission gate, and the decision log.
