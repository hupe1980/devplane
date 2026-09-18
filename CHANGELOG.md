# Changelog

Notable changes per release. Dates are UTC.

## Unreleased

**Renamed from Vibeplane, and every name moves with it.** The binary, the project
file `devplane.toml`, the machine directory `~/.devplane/`, the `DEVPLANE_*`
environment variables and the `devplane:ready` label. Nothing is read under the
old names and nothing migrates itself:

```sh
mv ~/.vibeplane ~/.devplane && mv ~/.devplane/vibeplane.db ~/.devplane/devplane.db
mv vibeplane.toml devplane.toml          # in each project
devplane connect claude                  # the installed hooks name the old binary
```

**Two suggestion defects were shipped and are fixed.** A `WebFetch` rule was
suggested as `WebFetch(docs.rs)`, missing the `domain:` the vendor's syntax
requires — so pasting it granted nothing. And the suggestion never appeared at
all for a session Devplane *watches* rather than drives, because the field it was
read from was `None` at every site that built it. Both were found by making the
product check its own suggestion before showing it.

### Added

- **A permission says how to stop being asked it again, and nothing writes the
  rule.** An item now carries the narrowest rule covering the calls this machine
  has actually seen in that family, how many it covers, and the file to paste it
  into — on the board as well as in the terminal. A pattern is offered only past
  three distinct calls; below that it is the exact call, because one
  interruption says nothing about the shape of the ones like it. The rule is
  **replayed against the call before you are shown it**, so one that would not
  have decided it is refused rather than handed over, and where no rule is
  possible the item says which reason it is. There is no route that edits
  `[policy]` and there will not be: an agent here runs as you and can read the
  daemon's token, so a write path to the rules would be reachable by the thing
  they govern.
- **The project specifies its own features before building them**, with
  [GitHub Spec Kit](https://github.com/github/spec-kit) — requirements with
  stable ids, a plan checked against a written constitution, then a task list.
  Those working files are not published, like the architecture notes; what
  reaches this repository is a test per behaviour the specification asked for.
- **`devplane trust` counts the skills a repository ships**, not only the ones
  that pre-approve a tool. It reported *"declares no hooks, MCP servers or
  skills"* about a repository shipping ten of them — the scan was right and the
  sentence was false, which is the worse of the two for a gate whose job is
  telling you what will load into your agent before you consent.
- **The gate's measurement caught up with the vendor.** The full differential
  matrix ran green against Claude Code **2.1.273** — 126 allow cases and 208 deny
  cases. Twelve deny shapes were **skipped rather than measured**, and the
  command says so: a skipped shape is unmeasured, not clean. The vendor shipped
  2.1.274 the same day and its one rule row cost a widening, which is the gap
  this clock exists to report rather than a reason not to report it.
- **A work view: what changed, what the checks said, and the release control
  beside both.** Approving work you cannot see is not approval. `tab` and
  `enter`, or a click, on a work row opens what its branch changed — against the **merge
  base**, so commits that landed on `main` since the worktree was made are not
  reported as this work's doing — with every gate command, its exit code, and
  the failing lines the agent was handed. A reproduction gate reads as passing
  when its commands failed, and says why; a gate that recorded no commands is
  distinguishable from one that passed. Uncommitted *and untracked* files count:
  a reviewer approves the checkout as it stands, and a file the agent created
  would otherwise have been invisible. A change too large reports what is
  withheld and the command that shows the rest, rather than a silent subset. The
  release control is offered only where the work is actually held at a declared
  human step.

### Changed

- **The fetched third-party corpus moved from `specs/` to `reference/`**, and
  `scripts/fetch-specs.sh` with it. Spec Kit hard-codes `specs/` for the
  project's own feature specifications, and one directory cannot be both a
  gitignored build artefact and committed source of truth. `just specs` is now
  `just reference`.

### Fixed

- **The spec task count included the specification's own quality checklist.**
  Pointed at a real Spec Kit feature, the reader counted **47**
  tasks where the task list had 31 — Spec Kit writes a `checklists/` folder
  whose boxes validate the *spec*, not the feature. Where a `tasks.md` exists it
  is now the task list and the other documents are not; where there is none,
  every box still counts, because a one-file specification reporting zero is a
  worse answer than the one being fixed. A **ticked** box is also no longer read
  as an open question: the checklist line *"No [NEEDS CLARIFICATION] markers
  remain"* was being counted as one.
- **A loop over a special shell variable was auto-approved.** `OPTIND=1/0 ls`
  already asked — an expression assigned to a variable the shell evaluates is
  arithmetic, not a string — but `for OPTIND in 1 2; do ls; done` did not,
  because `for` and `in` are control words stripped before that check runs. The
  `=` going out of sight was enough to walk past it. Claude Code 2.1.273 was
  asked three times: it runs the ordinary loop and refuses this one.
- **A cancelled turn is now tested, not just described.** The client sends
  `session/cancel`, waits five seconds for the agent to end the turn with
  `stop_reason: cancelled`, and tears the connection down if it does not — and
  every fixture turn finished in microseconds, so only the *timeout* branch was
  ever reachable. The test fixture can now be interrupted, and the new
  conformance case fails in 5.8 seconds against an agent that ignores the
  cancel and passes in 0.8 against one that answers it.
- **The docs sidebar had no space between the search box and the first group.**
  `:first-of-type` zeroed the heading's top margin, which is right for a heading
  that starts a column and wrong once a search box sits above it.

## 0.5.0 — 2026-09-17

Surfaces that make the gate's own claim checkable: how old its measurement is,
what it does and does not do, what every repository on the machine has it set to
do, a way for your agents to ask it things rather than being told by you, and —
where a gate went red — what the agent said about it, next to what was measured.

**One thing here changes a verdict**, and in the direction that costs a prompt:
a path grant like `Edit(out.txt)` no longer speaks for whatever command fills
that file. If a rule of yours relied on that, the call now asks. No
configuration breaks.

### Added

- **The gate's measurement is current for the first time.** The full
  differential matrix ran green against Claude Code **2.1.273** — 126 allow
  cases and 208 deny cases — so `devplane gate` no longer reports a gap between
  the release the rules were measured against and the one the vendor ships.
  Twelve deny shapes were **skipped rather than measured**, and the command says
  so: a skipped shape is unmeasured, not clean.
- **<kbd>,</kbd> on the board: what is configured, everywhere.** The machine —
  hooks installed, the settings file, how stale the measurement is — and every
  registered repository's `devplane.toml` read back: gates, pipelines, rules in
  evaluation order, and the three findings nobody gets by reading the file (a
  rule that covers nothing, one that grants more than it reads as granting, a
  path denied for reading that is still writable). The same read-back
  `devplane check --json` prints. **It reads and never writes**: an agent here
  runs as you, so a route that edited `[policy]` would be a widening path.
- **A `devplane.toml` that will not parse is now a critical inbox item.** The
  last good rules are kept, and a daemon restarted against a broken file has
  none to keep — so that repository's `never_auto` list was simply gone and
  nothing on any screen said so. The item names the file and the parser's reason.
- **The agent's account, beside what the gate measured.** When a gate goes red,
  the board and `devplane work show` print what the agent last said, under the
  verdict and judging neither — a report references about one action in eleven
  and drifts toward its plan as the run leaves it, so it is worth very little
  alone and is the whole point next to an exit code that contradicts it. Shown
  **only** beside a failed gate; absent when no transcript was kept, which is
  *nothing was recorded* rather than *the agent said nothing*.
- **`devplane gate`** — how much the gate is worth, which `doctor` does not
  answer. The release the rules were last measured against, how far the vendor
  has moved since, and the gate scored against the EBL-Core execution-boundary
  profile (arXiv:2609.11596) rather than a list written here. **The card shows
  what is missing**, and a test fails if it ever stops doing so. Also a page:
  the scorecard used to live where nobody outside the repository could read it.
- **The measurement is on a clock.** `just owed` re-fetches the vendor's
  changelog and exits non-zero when it has shipped past the release whose
  rule-relevant rows are accounted for; a cron line is the whole mechanism.
  `just advance` moves that floor and **refuses on a red ledger**.
- **`devplane mcp`** serves a read-only surface to your agents over stdio:
  `inbox`, `work`, `explain`, `audit`. The useful one daily is `explain` — an
  agent can find out *before* running a command that a rule refuses it. It is
  read-only because it **implements no mutating tool**, not because anything is
  annotated. Every payload is framed as a report carrying other people's text,
  and an `explain` asked this way is recorded in `devplane audit`.
- **`devplane work start --spec specs/001-password-reset`** names the
  specification a piece of work answers — a file, or the folder your spec tool
  wrote, which is what Spec Kit, Kiro and OpenSpec all actually produce. Every
  gate stamps a fingerprint over every document under it, so *checked against
  `specs/001-password-reset`* stays a claim you can act on after a file moves,
  and one that was not there when the gate ran says so rather than showing a
  blank.
- **The specification's own task list is counted, and shown beside the
  verdict.** *Gates green, 20/31 tasks, 2 unanswered* is a sentence neither the
  exit code nor the agent's account of its own work can produce alone. No
  methodology is learned — the frameworks in this category agree on almost
  nothing, so the outline is the Markdown headings and the progress is the
  `- [ ]` boxes they do share. Words that mark an unanswered question are the
  repository's: `[spec] open_questions`.

### Fixed

- **A path grant approved anything that wrote to that path.** With
  `auto_allow = ["Edit(out.txt)"]` and nothing else, every command redirecting
  into `out.txt` was auto-approved — `cat /etc/passwd > out.txt`,
  `cat ~/.ssh/id_rsa > out.txt`. One grant for one output file was permission to
  pipe any file on the machine into it, with no prompt. Two causes, both now
  measured against Claude Code 2.1.273: a read by a read-only command was
  treated as never needing a prompt, which is true inside the working directory
  and false outside it; and `~` resolved as a path *under* the working directory
  rather than as the home directory. A recognised file command writing through
  an operand — `… | tee out.txt` — now wants a `Bash` rule of its own, which is
  what the running product does. **Found by the differential harness on its
  first full run**, which is what it was built for.
- **An empty board said neither of the two things it could mean.** A heading
  over blank space is not an answer: *nothing is running* is the tool working,
  *nothing is connected* is a thing to do. It now says which, names the command
  when there is one, and makes the quiet-session count the way to see them.
- **The deny axis weighed "it ran" and "it did not run" as if they were the
  same kind of answer.** Its evidence is *did the command run* and its oracle is
  a language model, so a `yes` is a fact and a `no` has two causes that look
  identical: the rule fired, or the model never tried. Asked to run
  `hexdump .env > /dev/null` with an **empty** deny list it produced RAN,
  blocked, RAN — and the first full run reported that shape, and `c''at .env`,
  as **WIDER**, the loudest thing the harness can say. Neither was ever a
  verdict. An absence is now believed only after the shape has been given five
  chances to produce the evidence, and a shape that cannot produce it
  unprohibited at all is skipped and counted, exactly as one whose program is
  not installed already was.
- **The differential permission harness was asking its two sides different
  questions.** The extra grant a write-shape probe needs reached the vendor's
  settings and not the `devplane.toml`, so those rows compared a rule set
  against a different rule set and reported the difference as a finding. One
  list behind both spellings now, and a `selftest` axis that refuses to run the
  matrix when they diverge — it needs no model and no signed-in vendor, so it is
  part of `just verify`.
- **The harness picked the Claude Code binary by glob order, not by version.**
  The editor keeps every release side by side and `2.1.9` sorts after `2.1.273`,
  so a machine with an old build present would have measured against it and
  reported a floor that never moved. It now sorts by version and prints the
  release it measured against.
- **The board was keyboard-first and, for five commands, keyboard-only.** The
  palette, dispatch, the forge list, the setup panel and the reason key had a
  shortcut and no target anywhere on the page, and clicking a session row only
  selected it. The footer legend is now the toolbar — every global command is a
  button printing its own key — a session row opens what it is saying, and each
  inbox item carries `why`. A shortcut nobody can discover is a feature only its
  author has.
- **The quickstart's keyboard table was broken**, so its last three rows
  rendered as prose with pipes in it.
- **A pipeline step's gate verdict now carries the specification stamp** that
  the work loop and `devplane work verify` already recorded. Two paths of three
  read as evidence of absence on the third.
- **The documentation said a spec tool's CLI made *drift* a gate.** It does not:
  `openspec validate` checks specifications against each other and reads no
  source. That is spec integrity, which a gate gives you free; spec-code drift
  needs a step that compares the two.

## 0.4.0 — 2026-09-16

**If you rely on `[policy]`, this is the most important release so far.** Four
ways a `never_auto` rule could read as protection and not fire, and one way the
gate could be made slow enough to stop deciding at all — every one a prohibition
that was written, was legal, and silently did not apply.

Alongside them, four things the tool knew and kept to itself: what trusting a
repository actually loads, which rules grant more than they look like, how old
the gate's own measurement is, and which files a shell command wrote past
Claude Code's checkpoint.

**Upgrading.** `devplane trust` now asks, so a script that calls it needs
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

- **`devplane rewind <run>`** names the files a shell command wrote that
  Claude Code's `/rewind` will not restore — its checkpoint tracks only what its
  own editing tools touched. A read over the decision log; no snapshots and no
  copies of your files. It says *named for writing* rather than *changed*,
  because the gate sees a call before it runs, and it leaves out refused calls
  and paths nothing can pin to one file.
- **`devplane doctor` says how old the gate's measurement is.** Two numbers:
  `measured`, the last release the full differential run was green against, and
  `rows`, the last release whose rule-relevant changelog entries are all
  accounted for — the second is not a compatibility claim and says so. When no
  session reports a version it says that rather than implying no gap. The board
  shows it too, and only when there is one.
- **`devplane trust` lists what it is about to trust** before it asks: every
  `command` hook and its event, every MCP server with the unpinned ones named,
  every skill whose front matter pre-approves the shell, and every `[policy]`
  rule that grants more than it looks like. `--dry-run` prints it and trusts
  nothing; `--yes` skips the prompt. It reports and refuses nothing, and a
  repository that declares none of this says so in one line.
- **`devplane check` reports a rule that grants more than it looks like.**
  `Bash(python:*)` reads as a permission for one interpreter and approves
  `python -c '…'`. Claude Code reads it the same way, so this is reported and
  **not** refused. Two shapes only — the interpreter alone, and the code flag
  with a wildcard after it — so `Bash(python -m pytest *)` stays quiet. In
  `--json` as `overbroad`.
- **`devplane check` reports rules that provably do nothing**: an `auto_allow`
  a `never_auto` already covers, and a rule an earlier one in the same list
  covers. Answered by pattern containment, so `Read(.env)` is reported as
  covered by `Read(*.env)`. Silent on anything it cannot prove. In `--json` as
  `unused`.
- **An agent-facing index at `/llms.txt`**, held to the command list by a test.

### Changed

- **`devplane diagnostics` is now `devplane doctor`**, which is what the
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
several rules decide differently. **Run `devplane connect claude` after
upgrading**: the old hook entries are installed and do not decide.

### Added

- **GitHub across every project.** The daemon reads every registered project's
  open issues and pull requests through `gh`, a few seconds after it starts and
  every five minutes after. Project headings carry `· 4 issues · 2 PRs (1 needs
  you)`; `g` or the counts themselves open both lists on the board, and
  `devplane issues` and `devplane prs` print them. **Nothing is written to
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
  `devplane audit` and `devplane attention` answered 404 on machines that had
  upgraded without restarting.
- **`devplane doctor` runs the gate** with a probe call and reports whether it
  answered and how fast, instead of checking that a settings line exists. The
  probe is recorded nowhere.
- `devplane doctor` reports how many decisions are waiting in the spool.
- **A `gate_down` inbox item**, critical, raised when the daemon's periodic
  probe finds the installed gate not answering. No hook can enforce its own
  presence, so a broken one is otherwise indistinguishable from a quiet machine.
- `devplane explain` says why nothing answered: no rules here, rules that will
  not load, or rules that loaded and did not match. A `devplane.toml` that
  fails to parse is reported with its error.
- **The board is usable with a screen reader.** Every state glyph has a word
  beside it, the inbox, board and work sections are lists, every overlay is a
  dialog that gives focus back to whatever opened it, and there is one live
  region — polite, and silent unless its sentence changes. Each rule has a
  test.
- **`DEVPLANE_UI` serves the board from a file on disk** instead of the copy
  compiled into the binary, so working on the page is edit-and-reload rather
  than rebuild-and-restart. `just ui` is that with the path filled in. The
  board is also served `Cache-Control: no-store`, so a reload gets the page
  that is there.
- **The status line reads the rest of its payload.** The session's model, its
  Claude Code version, the context window's size, every rate-limit window with
  its reset time — including the gateway spend limit — the session cost and the
  lines it changed. `devplane show` prints them; the shim is still optional.
- **`devplane doctor` gains a `gate` section**: the Claude Code release the
  matcher was tested against, and any session observed running a newer one.
- **`findings.only`** on a pipeline step: words that make a finding worth
  returning the work for. A findings file with no matching line is *nothing
  found*. For reporters that grade what they find, such as a spec-driven
  tool's analyser.
- `DEVPLANE_DIFF_AXIS=dialect` runs the `PowerShell`, `Monitor` and `LSP`
  shapes against this matcher and prints a checklist to put to a running Claude
  Code. It is not a measurement and says so in its output.

### Changed

- **The gate decides in its own process and no longer needs the daemon.** An
  unreachable HTTP hook is a non-blocking error Claude Code walks past, so
  every rule was inert whenever the daemon was stopped. `devplane doctor`
  reports an HTTP gate as out of date.
- **A decision taken with no daemon is spooled** to
  `~/.devplane/pending-decisions.jsonl` and filed at the next start. Capped at
  20 000 rows, oldest dropped. Observations are not spooled.
- **A glob in a command's operands reaches a path deny.** `Read(.env)` now
  stops `cat .en?`, `cat .env*`, `head -c3 .en?` and `cat .en[v]`. A wildcard
  still cannot reach a name beginning with `.` unless the pattern spells the
  dot, so `cat *` is not one of them. Allow rules never grant on a glob.
- **`fmt` and `pr` read their operands**, so a `Read` deny covers them.
- **An allow rule must cover at least one part of a command.** A rule matching
  nothing no longer approves a command made entirely of read-only parts, and no
  verdict names a rule that did not fire.
- **`devplane explain` reads `~/.devplane/policy.toml`** as well as the
  project's rules, so it answers for the gate rather than for half of it.
- **`devplane.toml` is found without git.** A directory with no repository
  above it is governed by the file sitting in it. Inside a repository the root
  still wins.
- **`devplane work issues` is gone; `devplane issues --ready` replaces it.**
  `--label` and `--cwd` imply `--ready`.
- **GitHub Copilot's `powershell` tool is reported as `PowerShell`**, not
  `Bash`, so its commands are matched as PowerShell rather than parsed by a
  POSIX shell parser.
- `devplane check` labels an exception `except` rather than by the list it
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
  the next start as a `devplane-probe-<pid>` project, a working session and
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
  label twice on `devplane ls`; a repeated label now falls back to the id.
- The protocol conformance tests wrote their fixture's session files into the
  repository they ran from — 847 of them — instead of a scratch directory.
- **`devplane inbox` failed to decode any item without a session** — a piece of
  work whose runs have ended, which is the ordinary case for a pull request
  going red later. The command printed a decoding error instead of the inbox.
- **`max_runtime` never fired.** The elapsed time was computed through a string
  round trip that fell back to zero.
- An inbox row with no subject and no action no longer prints a bare `· `.
- `cli::run` uses the `Cli` it is given instead of re-parsing the process's own
  argv, so a test can drive a subcommand without a subprocess.

### Removed

- The `/devplane/policy` and `/devplane/copilot/gate` endpoints. The process
  that enforces a verdict records it through `/devplane/decided`.

## 0.2.0 — 2026-09-15

Several permission rules now decide differently, after checking them against a
running Claude Code. Read **Changed** before upgrading: two of them can make an
existing `never_auto` rule cover less than it did.

### Added

- `devplane explain --replay` — replays every tool call already observed against
  the current rules, and names the rule that would answer the ones that reached
  you. Offline; `--dir` scopes it to one project.
- `devplane doctor` names the model provider, and on Bedrock, Google Cloud's
  Agent Platform, Microsoft Foundry, a Console key or a gateway says which of
  Claude Code's own supervision surfaces are unavailable there.
- Permission items in `devplane inbox` name the rule that would have answered
  them.
- A pipeline `review` step falls back to the repository's `REVIEW.md`.
- `devplane check` notes when a `Read(path)` deny has no `Edit(path)` beside it.
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
- `devplane ls` says why the cost and context columns are blank when nothing is
  connected.

### Fixed

- **`devplane open` served a board that never loaded.** Two `const hit` in one
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
