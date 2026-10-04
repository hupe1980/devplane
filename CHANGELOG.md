# Changelog

Notable changes per release, in the style of [Keep a Changelog](https://keepachangelog.com/). Dates are UTC.

## Unreleased

### Added

- Grants: your own allow rule, per project, in your own `~/.devplane/policy.toml` — never in a
  repository. After you answer the same permission the same way several times from the window or a
  notification, its inbox item offers the exact rule as the lines it adds to the file, with the
  answers it comes from and what the project declares it may write, and writes it only when you
  confirm. A grant answers a driven run's request and a Claude Code `PermissionRequest` only after
  every deny, ask and unreadable line, never `PreToolUse`; it allows a line only when every command
  on it is granted and nothing on it is rewritten or redirected. Every allow is a `rule` decision
  naming the grant, and one broader than the agent's own settings is said to widen the agent.
  `devplane config grants` lists them with what they decided; `devplane config revoke` removes one
  and lists every decision it made. A machine-wide `[policy] allow`, a wildcard command, a forge
  write, a threshold below 2, and any grant key in `devplane.toml` are refused by name. The store's
  schema version is 18.

- Answer from the notification. With the app running, a permission carries **Allow** and **Deny**,
  and a question with one field carries every option as a numbered action — or none, never some of
  them — plus a typed reply on macOS and Windows; anything else is one click from the answer window,
  which now opens on that very item. A press answers as you, through the same few-second hold the
  window's answers wait in. Each item is announced once, also across a restart, within a second
  rather than on the fifteen-second sweep, and the notification is taken back when the item is
  answered elsewhere or the app quits. The record names the path an answer came from — `window`,
  `notification` or `cli` — and no request can claim the first two; `board` is no longer written.
  `devplane doctor` says what the app's notifications can do. The notifications are the platform's
  own, driven from the app's process: Tauri's notification plugin is gone. A store from before is
  set aside.

- A change's pull request now says who approved it: GitHub's review list and the base branch's
  rules are read when the pull request changes and once more when it merges, and one sentence says,
  by kind, how many of the required approvals came from a person's account, a model a vendor
  documents as its reviewer (Copilot code review), or a bot account — with dismissed approvals and
  approvals on an older commit said as such, and GitHub's own review decision beside it. The same
  sentence is in the certificate (`predicate.devplane.mergeApprovals`, with the `curl` that
  re-reads the reviews) and, with `devplane verify --approvals` or the action's `approvals` input,
  in the job summary. A model's approval reads *could not be attributed* until the record has a
  word for a model. Read-only: nothing is written to the pull request, no merge is blocked, and no
  decision is recorded. `verify` makes no network request without `--approvals`.
- A session you started yourself in Claude Code now records who decided each permission, from
  Claude Code's own telemetry, with its word kept beside the authority: `person` for an answer at
  the terminal CLI's prompt, `rule` only when Devplane finds and names the matching rule in your
  Claude Code settings. Every other reported call — decided under auto mode, by a mode that allows
  it, by settings that do not say how, by another hook, in an SDK or `-p` session, or with no or an
  undocumented source — is listed by `devplane audit --without-me` under *Not attributed*, with its
  reason and no authority. `devplane doctor` names any source Claude Code sends that its
  documentation does not define. Nothing is recorded as a model's decision. The store's schema
  version is 17.

- `devplane change draft <name> -- <intent>` drafts a specification with the project's own tool:
  the agent is sent Spec Kit's `/speckit-specify` (spelled the way that agent invokes it, from
  `.specify/integration.json`) or OpenSpec's installed propose workflow, followed by your words and
  nothing else, in the change's own worktree. Spec Kit is told the folder Devplane numbered and not
  to record an active feature. The drafted folder is reviewed as the change's diff and accepted or
  rejected (`draft-accept`, `draft-reject`, or on the change); Devplane writes no line into it. The
  New change window drafts too, and shows the exact text it will send before anything is made.
- Each line of an open change's specification carrying one of `[spec] open_questions` is its own
  inbox item, naming file and line — no longer one item per project. A bracketed marker is answered
  in place as a one-line diff you confirm (`devplane change clarify`, or in the inbox); an unbracketed
  one is sent to the change's agent as your message. After a Spec Kit draft with no words declared,
  the change offers Spec Kit's own marker as a diff to `devplane.toml`.
- The diff a driven agent reported for an edit is shown on the decision that allowed or refused it,
  in the change's ledger and in the Ledger, labelled as the agent's own report and quoted as text; a
  permission request carries the proposed diff before you answer. A clipped diff says how many bytes
  were not kept, a call with no diff says so, and a project that keeps no transcripts says no diff
  was kept. The worktree, not the report, is what the gates read. The store's schema version is 15.
- `devplane change resume <change> --agent <id> "<what to do>"` continues a change with a different
  agent in the same worktree, told only your words — never the earlier agent's transcript. Both runs
  stay on the change, the switch is recorded as yours, and it is refused while a run is still live.
- An agent that needs signing in shows its own sign-in methods on the change; `devplane change login
  <change> --method <id>` runs a terminal login in your terminal and starts the change again, or asks
  the agent to sign itself in. Devplane never asks for, reads or stores a credential.
- A driven agent's notices to you are shown on the change; a stop records whether the session was
  closed over the protocol; a change says when the agent was not given Devplane's tools; and the
  agent's own session list is read page by page.
- A change whose specification folder is in your checkout but not yet on the base branch starts:
  the folder is declared to the agent as an additional directory, and an agent that cannot take one
  is never prompted, saying why. An agent answering another protocol version is not driven.
- Asked about its own change, Devplane's MCP server now includes the change's task lines.

- A driven session's permission mode is stated on the change, in the agent's own words: when you set
  no mode, Devplane asks the agent for its own ask-first mode before the first prompt (Codex starts
  every session in *Auto review*, where a reviewer model answers) and says so; a mode you set is kept,
  and one that decides without you is said whole. `mode` in `agents.toml` chooses one for Devplane to
  ask for. Asking writes no decision.
- A tool call reported in pieces is joined before a rule reads it: an update carrying only what
  changed, or a permission request naming the call by its id alone, no longer hides the command from
  a project's deny rule, and an untitled call reads *untitled call ‹id›*, never blank. Where a rule
  about commands is in force and an agent sends no command at all (Gemini CLI 0.62.0), a person
  decides and the inbox says why.
- The adapters are repinned to the ACP registry read on 2026-10-02 (claude-agent-acp 0.85.1,
  codex-acp 2.1.1, Copilot 1.0.91, Gemini CLI 0.62.0), each beside a replay of its transcript; `just
  pins` says when a pin falls behind.
- GSD is read as a fourth specification layout: detected by `.planning/STATE.md`, every phase folder
  under `.planning/phases/` listed (inserted, prefixed and bracket-convention phases included,
  workstreams not), and no progress claimed — GSD keeps no task file, so no count, percentage or bar
  appears and the surfaces say why. A change's worktree says when a gitignored `.planning/` is not
  in it and names `.worktreeinclude`.
- OpenSpec is read OpenSpec's way: `config.yml` is recognised beside `config.yaml`; a folder that only
  wraps changes is not listed and a dot-named change is; task boxes are counted as `openspec list`
  counts them (every line of `tasks.md`, `[ x]` done, `[~]`, `[-]` and `[]` open), so the counts
  equal `openspec list --json`; a change under a custom schema shows no count and says so; and a
  store-only project says its changes live in the named store instead of showing an empty list.
- Kiro is detected by `.kiro/specs/`, not `.kiro/`, so a repository where `openspec init --tools
  kiro` wrote `.kiro/skills/` reads as OpenSpec only.
- The weakened group reads what governs the agent and what the build trusts: an edit to the agent's
  hook and settings files, `CLAUDE.md`/`AGENTS.md`, skills or MCP configuration (*agent controls*), to
  `CODEOWNERS` or branch protection as code (*ownership*), or to a lockfile or a manifest's
  dependencies (*supply chain*) is a row read before the green, counted by kind beside *verified*, in
  the certificate and in `devplane verify`'s first section, and holds the offer until a person has
  read it. A lockfile row says which packages were added, removed or changed; no row says the edit is
  good or bad, and none changes an exit code.
- The desktop app as packages: a disk image for Apple Silicon Macs, a per-user Windows installer, a
  `.deb` that declares its WebKitGTK dependency and an AppImage that carries its own, each with a
  `.sha256` and registering `devplane://`. Signed and notarised when the release has the signing
  credentials; otherwise every file says `-unsigned`, no update manifest is published, and the release
  never displaces a signed one as the default download. Pull requests bundle all four, unsigned. The
  command line's archives and installers are unchanged.
- Update checks in the app, off until you answer in Setup or the first-run checklist: one request per
  launch to a fixed address, nothing sent about you, nothing downloaded before **Install and restart**,
  and the download verified against the release key before anything is replaced. The command line
  never checks.
- Started from Finder, the Start menu or a launcher, the app's executable opens the window; typed in a
  terminal it is the command line. Opened while another host serves the home, it says so and offers
  that host's page instead of exiting unseen.
- `devplane doctor` says whether this binary carries the window, whether it is a signed build, and
  whether update checks are on, and reports *gate off* by agent for Codex and Copilot as for Claude
  Code. Hooks written from an AppImage are recognised as Devplane's.
- The certificate ends with the change's `devplane://` link and `devplane change show <id>`, as plain
  text.

- A driven agent can say the specification and the tests conflict: `spec_and_tests_conflict` on
  Devplane's MCP server records its claim on the change — quoted, attributed to the agent and run,
  read after the weakened checks and before the gates — stops the run, and runs `check` once with
  nothing handed back. The claim never changes *verified* and is not a failure. It is the server's
  one write besides `explain`'s audit row, bounded, and only about the caller's own run. Every
  implementation prompt and feedback round now names the tool. `devplane change claim-read` marks a
  claim read.
- A first-run checklist in Setup, opened first while a step is left: sessions already running, update
  checks, connect an agent, trust a repository, declare a check, the spec dialect, and a first change
  that reads *verified*. Each step is read from the machine, can be skipped and brought back, and
  writes only a diff the person confirmed.
- Declare a check from the window: commands the repository already has (`cargo test`, the test script
  with its package manager, `justfile` recipes, CI steps) offered with their file and line, and
  `openspec validate --all --strict` as a named gate, never part of `check`; the change to
  `devplane.toml` is shown as a diff and written only on confirm.
- Connect and disconnect an agent, and trust a repository after reading what it would allow, from the
  window.
- The review reads in one scroll: every hunk of every file in one column, in review order, each
  file's facts a strip that stays in view; `j`/`k` cross files.
- Review comments: press a line number or `f`, and the comment waits in a pending review kept by the
  host; **Send N comments** (or `⌘Enter`) sends them all as one turn to the change's agent, each tied
  to the tree and hunk it was written against, resuming the session or starting a run when none is
  live. After the agent's next turn each comment says whether its hunk changed.
- The line that weakened a check is marked in the diff's gutter with the gate that ran over it, and
  the Gates view links back to it.
- The offer names the files nobody opened, by path (*2 files not opened: …*), in the window and
  `devplane change offer`; it never refuses for them.

### Changed

- Telemetry no longer clears a waiting permission, even one naming the waiting call: only the
  hook's or the agent protocol's own answer does. Claude Code's telemetry event names written as the
  documented bare `event.name` (`tool_decision`) are now read.
- *Seen* is kept by the host for every hunk, keyed by its content: the window, `devplane change
  review --seen <path>`, the inbox and the offer read the same marks, and a hunk the agent rewrites is
  unread again. A weakened row is read when its hunks are. The marks in the browser are gone.
- A change's header is one line, with the rest under **More**; at 1280×800 the review's first diff
  line is above 300 px. Each count is said once; the address names the hunk under the cursor.
- The store's schema is version 13: an older store is set aside, not migrated.
- A failed read says what it means for the page, names `devplane doctor`, offers Retry and keeps the
  host's raw answer behind a disclosure; no loading skeleton stays beside it.
- An empty list on a host no session has reported to says what it cannot see, so it never reads like
  an empty one.
- No count is shown while a list is loading (Specifications).
- The forge offers *Sign in* when the host's error is a missing sign-in, says *not read yet* before
  its first read, and names a project whose read failed; *No open issue* only after a read.
- New change says, before anything is created, when the project declares no `check`.
- A gate command that is not installed reads *`<program>` is not on PATH*, not a failing check.
- An uncommitted `devplane.toml` no longer makes a checkout dirty for a new change: it governs from
  the checkout.

## 0.12.0 — 2026-09-27

The verdict runs the base's tests against the change and runs in CI; the command line is a set of nouns; offering
commits the tree the gates passed on.

### Breaking

- Commands are noun first. `ls`/`ps` → `status` (`ls` stays an alias); `show`, `watch`, `attach` → `run
  show|watch|attach`; `open`, `serve`, `app` → `up`, `up --no-browser`, `up --window`; `quit` → `down`; `answer`,
  `snooze` → `inbox answer|snooze`; `forge prs|issues` → `pr list`, `issue list`; `speckit` → `gate install
  --speckit`; `check`, `explain` → `config check|explain`; `trust` → `project trust`; `agents`, `connect`,
  `disconnect` → `agent list|connect|disconnect`; `login`, `logout` → `auth login|logout`. An old name is refused
  with the new one.
- `--json` is one global flag. Every change, run and ask id accepts a unique prefix.
- `devplane.toml` keys: `[policy] never_auto`/`always_ask` → `deny`/`ask`; `[project] default_agent` → `[agent]
  default`; `[policy] max_parallel_runs`/`stall_timeout` → `[agent] max_parallel`/`stall_timeout`; `[transcripts]
  keep` → `[agent] keep_transcripts`; `[gates.named.X]` → `[gates.X]`; `[budget] usd` → `max_usd`. `[gates]
  on_fail` and `[github] draft` are gone. Old keys are refused by name.
- `change offer` commits the tree the last passing `check` ran against to the change's branch, then pushes; it
  refuses if the tree moved. `archive` after an offer needs no `--discard-uncommitted`.
- `devplane gate` reports `passed`, not `verified`; only a change is *verified*.
- `gate install --speckit` refuses a repository with no gate, unless `--anyway`.
- `POST /api/projects/trust` refuses a request that carries only the control token.
- A binding on a chord the browser keeps (`⌘W`, `⌘T`, `⌘N`, `⌘L`, `⌘R`, `⌘Q`, `⌘Tab`) fails the build.
- Store schema 12. An older store is moved aside; a newer one is refused.
- Minimum Rust is 1.94.

### Added

- `devplane verify [--base <ref>] [--summary <path>] [--statement <path>] [--no-base-checks]`: runs the base's
  `check` on this checkout with no host, store or network, and writes a summary led by the weakened checks (to
  `$GITHUB_STEP_SUMMARY` in CI). Exit 0 passed, 1 failed or tree moved, 2 could not run. `action.yml` runs it as a
  composite action.
- Base checks: the base's tests run on the change, and the change's new tests run on the base, in a throwaway
  checkout. A base test failing on the change is shown with the weakened checks and holds the offer; new tests
  that pass without the change are named. They run on offer, on `change verify --base-checks`, from the Gates view,
  in `verify`, or after every passing `check` with `[gates] base_checks = "auto"`.
- Test counts beside *verified*, read from the runner's summary (cargo, nextest, pytest, jest, vitest, go).
- `[gates] junit` names the JUnit files `check` writes, so tests are named, not only counted.
- The workbench leads a change with its evidence, has one keyboard cursor on every list, keeps the open view in
  the address, and fits narrow windows.
- The inbox from the keyboard: `1`–`9` choose an option, `t` types an answer and `⌘↵` sends it, `a`/`d` allow or
  deny a permission Devplane holds, `w` reaches a session the vendor owns (its VS Code link, or its resume command
  copied), `s` snoozes, `o` opens, `r` reviews. Each key is printed on its control, and a key that does not apply
  says why. The answer window takes the same keys; Search and Setup take the list keys.
- An answer from the window is held for 5 seconds before it reaches the agent, and `z` takes it back with nothing
  recorded. Answering another item sends the held one at once. The host holds it, so closing the page still sends
  it; quitting drops it, says so first, and leaves the question open. `devplane inbox answer` is never held.
- `?` lists the keys for the item first, then the surface's, then those that work everywhere.
- `s` snoozes an inbox item until its session (or change) next reports activity, not for an hour; **for an hour**
  sits beside it. Snoozed items are counted at the foot of the list and listed with a way back each.
  `devplane inbox snooze <id> --until-activity` does the same from a terminal. A snooze that would hide a question
  or a permission is refused, naming it.
- One count: the inbox badge, the status bar's *need you* and the tray and dock badge show the same number — the
  whole inbox, without a held answer. Asks that outlived their sessions keep their own noun.
- `⌘K` over an inbox item lists that item's actions first, with their keys.

### Security

- A change could read *verified* with no weakened check shown: through `assume-unchanged`, a `-diff` attribute, an
  uncommitted `.gitignore`, an embedded repository, `info/exclude`, a truncated diff, or runner configuration such
  as `.cargo/config.toml`. The review now diffs the digested tree and reads the whole change.
- The policy reader no longer lets a prohibited command through `git config alias`, `tar --to-command`, GNU `sed
  e`, `zip -TT`, `less`, `man -P`, `tmux`, or a script run by path.
- Agents are denied reading or writing Devplane's home on every vendor's hook.
- An answer is recorded as the window's only when it carries a proof under a key the app hands its own webviews in
  memory; every other holder of the token is recorded as the command line. The `Origin` header and the request body
  are no longer read for it. The CLI proves the host holds the token before sending it.
- Trusting a repository over HTTP needs the window's proof; `devplane project trust` writes it directly and is refused
  inside an agent session.
- CI judges a pull request with the base's `action.yml`, never the pull request's own. The action installs a pinned
  release after checking its `.sha256`, and pipes nothing into a shell.
- Opening a worktree in an editor or terminal no longer goes through a shell, so a path containing `&`, `;` or `|`
  cannot run a command.
- The certificate's subject is the checked tree, and names the commit only when the commit is that tree. With no
  recorded tree there is no in-toto statement, and the export says why, instead of a statement with an empty subject.

### Fixed
- An Alt shortcut typed inside a text field (`⌥3` for `£`) switched the view and dropped the draft.
- The command palette opened without focus in its field, and listed some commands twice.
- *Offer* was enabled while a weakened check was unread; it is held, with the reason beside it.
- A `devplane://` link opened with the app closed was rejected on Linux and Windows; a second launch now hands its
  link to the running app.
- On Linux, opening a terminal reported success when none had started.

- `change show <prefix>` failed where other commands took the prefix.
- An older run snapshot could overwrite a newer one and lose events.
- A file named by a task sent to the agent was reported as not asked for.
- The change timeline printed dates in the browser's locale.

### Removed

- Reports between projects (`report`, `[reports]`), `rewind`, `modes` (now `status --modes`) and `attention`
  (now in `doctor`).

## 0.11.0 — 2026-09-26

Devplane signs in to GitHub itself, and this release closes the ways an agent could steer its own
verdict.

### Breaking

- `gh` is no longer used. Sign in with `devplane login github` (OAuth device flow, token in the OS credential
  store); `DEVPLANE_GH` is gone. Without `[github] pull_request`, `change offer` prints `git push` and the compare URL.
- A task is never called *verified*: the count is *seen by a passing check* (`seen_by_pass` on the wire).
- A change is *verified* only while its project declares a `check` and `devplane.toml` loads.
- `change offer` is refused (exit `3`, HTTP `409`) while a weakened-check row is unseen.
- Commands: `gate run` → `gate`, `speckit install` → `speckit`, `issues`/`prs` → `forge issues`/`forge prs`,
  `asks` → `inbox --all`; `focus` and `rules` removed.
- `--json` only on commands that print JSON; a refused command prints `{"error": …}` and exits non-zero.
- `connect`, `disconnect` and `change stop` refuse without `--yes` when stdin is not a terminal.
- `answer`: `--option` excludes `--allow`/`--deny`; `--field` is for questions only.
- Store schema 11. The old store is moved aside, not migrated.
- Telemetry settings carry a new token: run `devplane connect` again.

### Security

- The tree digest hashes every file as it is on disk. A clean filter planted in `.git/config` could make edited code
  digest as verified code, and ran in the host.
- Vendor settings carry a telemetry-only token (`~/.devplane/telemetry-token`). Claude Code hands its settings'
  environment to every tool call, so the old header gave agents the token that answers permissions.
- On Codex, an *ask* is a *deny*: Codex runs a call its hook asks about. A crash while deciding refuses on every vendor.
- A prohibition cannot be walked past through a wrapper the reader does not know (`pkexec`, `unbuffer`, `taskset`, …),
  `flock -c`, or an abbreviated long flag (`rm --rec --for`).
- Host-run git never starts `core.fsmonitor`, never prompts, and times out; the offer pushes with `--no-verify`.
- Built-in prohibition: an agent may not edit Devplane's files, read its token, or edit its repository's
  `devplane.toml`. `change offer|finish|archive` and `review --seen` are refused from an agent's session.
- A line whose effect cannot be read statically (a variable naming the program, `GIT_SSH_COMMAND`, `LD_PRELOAD`,
  more than eight wrappers) is put to a person.
- The workbench is served with a `'self'`-only Content-Security-Policy, `no-referrer` and `nosniff`.
- The GitHub token lives only in the OS credential store; a token GitHub rejects is deleted.
- The host refuses a taken port instead of moving; the token is accepted only in the `Authorization` header;
  `~/.devplane` is `0700` and its files `0600`.
- An unreadable diff holds the offer. Trusting a repository is recorded in the ledger.

### Added

- **GitHub:** `devplane login github [--host] [--with-token]`, `logout github`, Setup → GitHub in the workbench,
  Enterprise hosts (`[github] host`, `[github.hosts."<host>"] client_id`), and a GitHub section in `doctor`.
- The Forge view and `devplane forge` name each failure — not signed in, expired, rate limited, unreachable, not a
  GitHub repository — with its fix, and say how many items lie past the page.
- *Verified* is always shown with its qualifier (*verified · 1 check weakened*) on every surface.
- `change review <id> --seen <path>` marks a weakened row read; the certificate says when.
- The review also flags: a test added with no assertion, an assertion removed or made trivial, a removed test, a
  changed tolerance, lint and type suppressions, a test file renamed out of the suite, a new `.gitignore` pattern,
  edits to build, lint, coverage and CI configuration or to a script a gate runs, and masked failures (`|| true`, …).
- `doctor` reports a hook whose binary is gone as *gate off*.
- Driven agents get Devplane's read-only MCP server, start in their worktree, and are closed with `session/close`.
- `cargo install devplane --features app` builds the desktop app from crates.io.
- Docs: Troubleshooting, a platform table, an upgrade section, and a quickstart from nothing to a verified change.

### Changed

- A forge poll is one GraphQL request per repository plus one search per host, not two `gh` processes per project.
- The inbox never offers a change; *ready to decide* leads with Review.
- `snooze` is capped at 30 days.
- The workbench keeps its state through the poll, pauses while hidden, never shows a count before its list loads,
  says when a read failed or is stale, and shows progress on long writes.
- Keys: `Alt+` shortcuts work on macOS, `a`/`d` allow and deny in the inbox, and Enter opens an item.
- The pure core no longer reads the clock or the disk.

### Removed

- The `offer` inbox action, the review's *accept* mark, and the OTLP metrics receiver.

### Fixed

- Spec Kit, OpenSpec and Kiro repositories without a `devplane.toml` listed no specifications.
- The certificate's re-run steps checked out a commit without the verified work.
- An answer to something the agent did not ask closed the question and stranded the agent.
- Concurrent opens of an old-schema store could leave the retired copy empty.
- Retention deleted the runs of open changes.
- A report's issue could be filed twice.
- A late telemetry event could clear a waiting permission or revive an ended run.
- An agent that could not start ended silently.
- Offering a branch that already had a pull request failed on every retry.
- *Needs you* missed assigned issues past the first page and repositories whose name differs in case.
- The inbox could read *Clear.* while items waited; marking one hunk marked a whole file's weakened checks.
- The window's host used a random port, so telemetry reached nothing.
- `trust` needed a running host; `disconnect` damaged a status line ending in a quote.
- `.worktreeinclude` failed on file systems without hard links.

## 0.10.0 — 2026-09-25

### Breaking

- No prebuilt binary for Intel Macs: macOS is Apple Silicon only. `cargo install devplane` still
  builds on Intel.
- `devplane dispatch`, `batch`, `say`, `tail` and `library` are removed. Use `devplane change start "…" [--project X]…`
  (one prompt to several projects, every refusal reported before anything is created), `devplane change prompt`
  and `devplane watch [RUN]`.
- Work is now Change everywhere, with no alias: `devplane change …`, `/api/changes…`, ids `c-…`, table `changes`.
  A change's state (`drafted`, `isolated`, `in flight`, `verified`, `offered`, `archived`) is computed on every read.
- Removed: pipelines (`[pipelines]`, roles, human steps, findings, `.devplane/prompts/`), change kinds, per-kind
  budgets, reproduction gates (`[gates.named.*] expect`), batches, the prompt/skill library and its sidecars, and
  `change approve`. A config still carrying an old key is refused by name, with a sentence saying what to do.
- The budget is one `usd` plus `max_turns` and `max_runtime`.
- `[workspace] include` is replaced by the repository's own `.worktreeinclude` (`.gitignore` syntax, ignored files only).
- The authority `daemon` is renamed `devplane`; the event source is `host`. `~/.devplane/daemon.json` is `host.json`.
- Branches are named `change/<slug>`.
- Store schema version 9. The old store is moved aside, not migrated.
- Hooks write straight to the store; nothing auto-starts a host. `/devplane/hook`, `/decided`, `/hold` and
  `/statusline` are gone. Re-run `devplane connect claude`. Read commands fall back to the store when no host answers.
- `devplane stop` is `devplane quit` (`POST /api/quit`), and says what it ends first.
- A second `devplane serve` is refused by asking the recorded port's `/healthz`, not by trusting a pid. A port given
  with `--port` or `DEVPLANE_PORT` is bound or the start fails.
- Nothing opens a pull request by itself: `change finish` records the basis and removes nothing
  (`--remove-worktree` is gone). Use `change offer`.
- `change archive` keeps the branch unless `--delete-branch`; `--discard-uncommitted` is separate from `--force`.
- API: `/api/dispatch*`, `/api/batch*`, `/api/library`, `/api/changes/{id}/approve`, `/api/changes/{id}/diff` and
  `/api/runs/{id}/events` are removed. `POST /api/changes/preflight` is added.
- Exact command rules with no wildcard require the same flag set: `Bash(git status)` no longer covers
  `git status --short`. PowerShell alias canonicalisation is gone.
- `devplane connect` and `disconnect` show the diff and ask; `--yes` writes without asking.
- `GET /api/setup` reports `declares_gates` instead of `verified`; `GET /api/attention` drops `oversight`.

### Added

- The workbench UI: a title bar with the ⌘K command palette and **New change**, an activity bar with counts, sidebar
  lists, preview tabs, a bottom panel (Activity, Sight; ⌘J) and a status bar. Light and dark themes.
- A change opens as a document with Overview, Tasks, Review, Gates, Ledger and Agent tabs.
- Review: file tree plus unified or side-by-side diff, keys `j`/`k`/`n`/`p`/`s`/`a`/`f`/`v`. "Checks weakened or
  changed" (skip markers, deleted tests, gate or CI config edits) leads the review and is named in the certificate.
- `devplane change review <id> [--by intent]` reads a change in `[review] roles` order, each file with its role,
  covering test, decisions and task.
- Sessions, ledger, reports, forge and search are sortable, groupable grids.
- The New change dialog runs a live preflight, including install-cost notes (`[workspace] setup`, lockfiles).
- The certificate names a digest of the gate commands and says when the change altered a check.
- Every dispatched prompt tells the agent it may stop and say the specification cannot be satisfied.
- `devplane change start --spec <folder> --task <selector>`: send chosen task lines (a requirement token, or
  `file:line` whatever the notation); a selector matching nothing refuses before anything is created.
- A change naming a specification reports *ticked* and *verified* counts separately.
- `devplane change drift <id> --run <run> --tell|--accept` when the specification moved under a run.
- `[spec] tokens` declares requirement-id shapes; `GET /api/specs` carries `trace`, `layouts` and `no_layout`.
- Spec Kit `[USn]` and Kiro nested `_Requirements:` citations are read; OpenSpec's legacy `project.md` marker is
  detected; timestamped Spec Kit folders are accepted.
- `devplane change adopt <branch>`, `change archive`, `change offer` (pushes and opens a PR only with
  `[github] pull_request = true`, otherwise prints the commands), `change prompt` and `change stop`.
- `devplane report file|ls|show|start|reject|defer|fixed|open`: file a finding about another project; it reaches that
  project's person, and a GitHub draft is sent only by `report open`. `[reports] deliver_from` opts a live run in.
- `devplane app` (cargo feature `app`): the same host in a window, with notifications, a tray badge, a global
  shortcut (`[app] shortcut`), `devplane://` deep links and open-in-editor/terminal.
- `devplane connect codex`, through `~/.codex/hooks.json`; Codex runs the hooks only after you approve them in it.
- Copilot's hooks are all `command` hooks and decide without a host.
- `[workspace] share = ["cargo"]` shares one `CARGO_TARGET_DIR` between a project's isolated changes.
- The inbox is six bands, and `devplane inbox` prints how many items are snoozed.
- `devplane doctor` opens with a `host` section and dates each vendor row.
- One host per home, held by `~/.devplane/host.lock`.
- `connect --statusline` and `--yes` are accepted after the vendor name.
- The event stream sends a `resync` event.

### Changed

- *Verified* means the latest `check` gate passed against the working tree as it is now, digested (untracked files
  included) before and after the run, so uncommitted work can be verified. Named gates never verify.
- A stale pass reads *gates passed, stale* with both tree digests wherever a change is shown.
- Gates, pull-request settings and budgets are read from your own checkout, never from the agent's branch.
- `events.source` names which vendor's shim wrote a hook row (`hook`, `copilot_hook`, `codex_hook`).
- A host killed mid-run is recovered by the next one; its runs read as ended by the host.

### Fixed

- *Verified* was unreachable without a commit, and a passing named gate after a failing `check` read as verified.
- Gate output byte counts and digests were wrong over 32 KB; the gate stamp was taken after the commands ran.
- Hook events became invisible after a retention pass (`events.seq` is never reused); projection replay double-counted.
- `host.json` and the token are written atomically, the token created `0600`.
- Upserting a project on the hook path no longer resets its trust.
- Review listed files twice and dropped lines starting `--- ` or `+++ `.
- "Formatter-only" no longer hides Python or YAML re-indentation.
- Decisions were matched to files by substring path.
- `change offer` refuses a dirty or empty branch and no longer passes an invalid `--repo` to `gh pr create`.
- Merge checks use the configured base branch, and a merged PR counts as merged.
- A spec folder missing from the base branch is refused at start.
- `change drift --tell` carries the changed text.
- In-place changes are gated.
- The UI's POSTs were sent without a JSON content type (HTTP 415).
- A directory reached through a symlink (macOS `/tmp`) became a second project.
- The host registered its own working directory as a project named `.`.
- Decision rows now carry their project.
- A refused call no longer reads as the session's current work.

### Security

- A `devplane.toml` or `policy.toml` that fails to load now makes every gated call ask; before, it disabled every rule.
- Holding hooks are installed with a timeout above the longest hold.
- `devplane answer --allow` is refused inside an agent session. Limit: the host's bearer token is still readable by
  the agent.
- The command reader sees through functions, `coproc`, brace expansion and globs in the program word, here-strings and
  `<` into a shell, `builtin`, and wrapper flags.
- The matcher is a quote-aware reader: flags compare as a set (`-rf` = `-fr` = `-r -f`), `sh -c`/`eval`/`su -c` are
  read inside, and a line it cannot read is *unresolved* (a question), never silently undecided.
- Exception rules apply per simple command; exact rules compare short flags as a set.
- Ask answers are compare-and-set.
- The governing project is the longest canonical registered root; a worktree's `.git` file is honoured only when
  git's back-reference verifies it.
- `base_branch` is validated as a git ref, and every user-supplied ref follows `--end-of-options`.
- A text tool's `-e`/`-c` (`sed`, `grep`, …) is read as a pattern, not a command line.

## 0.9.0 — 2026-09-24

### Breaking

- The telemetry endpoints require the bearer token. `devplane connect` writes `OTEL_EXPORTER_OTLP_HEADERS`; re-run it.
- `GET /api/library/{name}` is removed.

### Added

- **Plans** surface and `GET /api/specs`: each project's specification, outline, open boxes and unanswered lines.
- `[spec] plans`: where a repository keeps its specifications (no default).
- Finished work shows unticked boxes, open questions or a missing specification beside its approval.
- Drift: work records its specification's fingerprint at start and shows when it changed.
- A line matching `[spec] open_questions` in an in-flight specification reaches the inbox.
- `[questions] hold`: hold an `always_ask` permission for up to 120 s so it can be answered from anywhere.
- Portable prompts in `.devplane/prompts/` may use a closed set of placeholders (`{project}`, `{gate.failures}`, …).
- The launcher offers the project's existing prompts, with `argument-hint` as a field.
- OpenCode is read over its event feed (`DEVPLANE_OPENCODE_URL`), read-only.
- `devplane inbox --project <name>` and `--needs-you`, and `#inbox/<project>` on the board.
- `devplane completions <bash|zsh|fish>`.
- `devplane audit --otel` writes the decision log as OpenTelemetry GenAI records.
- `devplane check` reads `[spec]` back.

### Changed

- Hidden commands are no longer offered by shell completions.
- The certificate's spec stamp and the Plans page read from the same source.
- A specification with no task list can no longer render as complete.

### Fixed

- The Plans page never finished loading when several plans had no work (duplicate list keys).
- A request that never returned left a page loading forever; requests now time out at 10 s.
- The specification reader counted backticked markers and `checklists/` as open questions.

### Security

- A prohibition no longer stops applying to a heredoc fed to an unrecognised program (`| sudo bash`, `| python3`, …).
- The telemetry endpoints accepted OTLP records from any local process or web page.
- An answer naming an option nobody offered was recorded as an `allow`.

## 0.8.0 — 2026-09-23

### Breaking

- A command the analysis cannot read now asks instead of being denied.
- Every UI surface was renamed; Search became a sidebar field and Changes is reached from its Work.
- `POST /api/asks/{id}/answer` rejects unknown fields; a permission answer with neither decision nor option is a
  bad request.

### Added

- `devplane dispatch --template <name>` warns which frontmatter fields the distribution paths reject.
- A driven run is enriched from the agent's `session/list` (titles only, never state).
- `devplane doctor` says `started from …` when the running daemon is a different binary.
- Every Work is reachable from the work surface; a library matrix surface; `POST /api/dispatch/preflight`.
- `/api/forge` rows carry `needs_you`; `/api/modes` is typed and includes `unreported`.

### Fixed

- A session that ended while asking you stayed "waiting for you" forever.
- A session you quit was recorded as `completed`; it is now `stopped`.
- A late end-of-turn revived a run that was over.
- `devplane modes` could claim every session asks you when none had reported.
- Three UI surfaces were wired to nothing; the certificate had no copy control.
- Pressing *allow* on the board denied the call (the body sent `choice`, which the route does not read).
- Every inbox action is wired; a watched session's permission says why it has no yes/no.
- A prohibition refused every long Bash command; heredoc bodies are now parsed and excluded from the length.
- A stalled command was truncated to 80 characters at record time.
- `devplane doctor` reported OpenCode as publishing nothing; new `unbuilt` and `not checked` states.
- `devplane answer <typo>` printed a serde error and the port; it now names the command that lists open asks.
- The help screen listed its commands twice; the inbox row shows project and age.
- Checkbox styling, channel-table alignment and two flaky timing tests.

### Removed

- `GET /api/decisions/pane` and its HTML rendering.

## 0.7.0 — 2026-09-21

### Breaking

- `ui/legacy.html` is gone; `DEVPLANE_UI` points at a built `ui/dist` directory.
- `GET /api/work/{id}/changes` no longer returns `html`.
- The interface has no keyboard shortcuts.
- `devplane mcp` and `devplane statusline` are hidden from `--help` (both still work).
- Store schema version 3; no migration.

### Added

- Surfaces are addressable as `#<surface>/<focus>`.
- Snooze can be undone from its confirmation; an answer says it cannot be recalled.
- A changes surface renders a Work's diff file by file, never as HTML.
- `devplane doctor` lists what is watched per vendor and channel: `read`, `unproved` or `not published`.
- The board marks a session close to compaction, using `context_high_percent`.
- `devplane rules`: which repositories are missing a rule. Read-only.
- The inbox folds interchangeable kinds above twelve rows, always with a count; questions and permissions never fold.
- `devplane attention` reports how often folding was right.
- The done certificate is shown on the page with a copy button, and carries `gen_ai.evidence.origin`.
- All four ways a Work reaches done render as a sentence, including *no gate was declared*.
- `devplane gate run --name <gate>` runs one named gate.
- An empty inbox summarises what was decided without you.
- MCP tool-call audit rows show the server's source (`plugin`, `sdk`, `user`, `project`).
- `devplane --help` groups commands by errand.
- `CLICOLOR_FORCE` enables colour; `NO_COLOR` still wins.
- An unreadable command line is `unresolved` and asks you.
- A question an agent moved past is recorded as `question_abandoned` and listed by `devplane asks`.
- `devplane modes` reports the per-session question timer, including `CLAUDE_AFK_TIMEOUT_MS`.

### Changed

- The interface is rebuilt in Svelte 5 on Vite and embedded in the binary; it opens on what needs you.
- The project's tagline now says what it records.

### Fixed

- `devplane ls` and the board no longer claim nothing is running on a machine they cannot fully see.
- The diff surface printed no file count.
- Copilot's `notification` event is mapped.
- The decision log could return rows in the wrong order (ties broken by `rowid`).
- New inbox items are marked `new`.
- The board shows the question clock; `never` is no longer reported as a timer; the clock line names its file.
- Aligned columns collapsed when colour was on.
- `devplane speckit install` refuses when no `/devplane-gate` skill is reachable (`--anyway` overrides).
- The `typescript` feature did not compile.
- A prohibition now sees through `sudo`, `doas`, `exec`, `env`, `watch` and absolute paths.
- `cp`, `truncate`, `dd`, `install`, `rsync` and `ln` meet `Edit(…)` prohibitions.
- `devplane explain --replay` ignored machine-wide rules.
- A question's deadline no longer counts time Devplane was not running.

### Removed

- `core::diff::render`, the `html` field, `in_force`, and `AcpEvent::PermissionExpired`.

### Internal

- The built interface is committed and CI checks it is reproducible.
- The repository's own gate runs the interface checks; `agent-client-protocol` 2.2 (schema 1.9.1).

## 0.6.0 — 2026-09-20

### Breaking

- New run state `interrupted`: Devplane stopped the run while shutting down.
- Renamed from Vibeplane: binary, `devplane.toml`, `~/.devplane/`, `DEVPLANE_*` and the `devplane:ready` label.
  Nothing migrates; move the files and re-run `devplane connect claude`.
- Devplane no longer approves a tool call. `auto_allow` fails to load; `devplane explain --replay` composes the
  grants to move into your agent's settings.
- `devplane decide` is removed; `devplane answer <ask>` covers permissions and questions
  (`POST /api/asks/{id}/answer`, `GET /api/asks`).
- No built-in ten-minute permission timeout; set `[questions] deadline` if you want one.
- The decision log records `authority` (`person`, `rule`, `timer`, `nobody`, `daemon`) instead of `actor`.
- Store schema version 1; any other version is moved aside.

### Added

- `npx devplane`, published with npm provenance.
- `devplane agents` shows each agent's advertised capabilities.
- `devplane modes` reports an ACP agent's self-declared mode and whose clock can answer a question for you.
- `devplane gate run` runs the repository's gates; `devplane speckit install` registers it as a Spec Kit hook.
- Asks are durable: a question survives a daemon restart and is answerable later.
- Intel Mac release builds.
- A Claude Code plugin with the read-only MCP surface and a skill.
- `devplane asks` and `devplane audit --without-me`.
- `devplane library` (`diff`, `report`, `install`, `sync`), `devplane dispatch --to` and `devplane batch`.
- Leaked agents from a killed daemon are reported, not killed.
- The page opens on what needs you.
- A done certificate a reviewer can check, as Markdown or JSON (unsigned).
- A permission suggests the narrowest rule to paste into your agent's `settings.json`.
- A work view with the branch's changes against the merge base and every gate result.
- `devplane trust` counts the skills a repository ships.

### Changed

- A driven run reads as `interrupted` after a restart, and its question stays answerable.
- The interface is a Svelte project embedded at compile time.
- The fetched reference corpus moved from `specs/` to `reference/`.

### Fixed

- A clean shutdown recorded interrupted work as `completed`.
- A process you may not signal was reported as dead.
- A stale `daemon.json` stopped the daemon starting.
- A session running a background command read as waiting for you.
- The board followed a session without hooks only once; a roster `waiting` status is now an inbox item.
- The suggested rule was missing `domain:` for `WebFetch`, and was TOML for a JSON file.
- The spec task count included Spec Kit's `checklists/`.
- `devplane inbox` passed an option's label instead of its value.

### Removed

- The permission mirror and its harness, the version-gap warning, and the old `devplane gate` report.
- Source maps from the shipped binary.

## 0.5.0 — 2026-09-17

### Added

- A setup surface (`,` on the board) reading back every repository's `devplane.toml`. Read-only.
- A `devplane.toml` that will not parse is a critical inbox item.
- When a gate goes red, the agent's last message is shown beside it.
- `devplane gate`: how current the gate's measurement is, scored against a published profile.
- `devplane mcp`: a read-only MCP server with `inbox`, `work`, `explain` and `audit`.
- `devplane work start --spec <path>` stamps the specification's fingerprint on every gate.
- The specification's task list is counted beside the verdict; `[spec] open_questions` sets the question words.

### Fixed

- A path grant like `Edit(out.txt)` approved any command writing to that path.
- An empty board now says whether nothing is running or nothing is connected.
- Every board command has a visible control.
- A pipeline step's gate verdict carries the specification stamp.

## 0.4.0 — 2026-09-16

`devplane trust` now asks; scripts need `--yes`.

### Fixed

- A wildcard deny now meets a wildcard operand (`Read(*.env)` stops `cat conf*`).
- A protected file could be pushed past the analysis bounds; reaching a bound is now treated as unknown.
- Quoting got past a deny (`r''m -rf /`).
- A long command could make the gate time out; a command is now parsed once.

### Added

- `devplane rewind <run>`: files a shell command wrote that Claude Code's `/rewind` will not restore.
- `devplane doctor` reports how old the gate's measurement is.
- `devplane trust` lists hooks, MCP servers, skills and broad rules before asking; `--dry-run` and `--yes`.
- `devplane check` reports overbroad rules (`overbroad`) and rules that provably do nothing (`unused`).
- `/llms.txt`.

### Changed

- `devplane diagnostics` is now `devplane doctor` (both work).
- The board aligns as a table, fits a narrow screen, and improves dialog accessibility.

## 0.3.0 — 2026-09-15

Run `devplane connect claude` after upgrading: the gate is now a `command` hook.

### Added

- GitHub issues and pull requests across every project (`devplane issues`, `devplane prs`); read-only.
- Issues assigned, reviews requested, and your red or approved PRs reach the inbox.
- A client restarts a daemon older than itself.
- `devplane doctor` probes the gate, reports the spool, and gains `github` and `gate` sections.
- A critical `gate_down` inbox item.
- `devplane explain` says why nothing answered.
- Screen-reader support on the board.
- `DEVPLANE_UI` serves the board from disk.
- The status line reads model, version, context, rate limits, cost and lines changed.
- `findings.only` on a pipeline step.

### Changed

- The gate decides in its own process; decisions without a daemon are spooled.
- Globs in operands reach a path deny; `fmt` and `pr` read their operands.
- An allow rule must cover part of a command.
- `devplane explain` reads `~/.devplane/policy.toml`.
- `devplane.toml` is found without git.
- `devplane work issues` is replaced by `devplane issues --ready`.
- Copilot's `powershell` tool is reported as `PowerShell`.
- A closed editor tab is a session that ended, not a lost one; the board shows the working set.

### Fixed

- `PowerShell` rules resolve aliases; `Bash(…)` reaches `Monitor`, `Read(…)` reaches `LSP` and `git show HEAD:path`.
- Stall and review-request false positives; lost sessions can be snoozed.
- Every value on the board is escaped.
- `max_runtime` never fired.
- `devplane inbox` failed on items without a session.

### Removed

- The `/devplane/policy` and `/devplane/copilot/gate` endpoints.

## 0.2.0 — 2026-09-15

### Added

- `devplane explain --replay` replays observed calls against the current rules.
- `devplane doctor` names the model provider and what it lacks.
- Permission items name the rule that would have answered them.
- A pipeline `review` step falls back to `REVIEW.md`.
- `devplane check` notes a `Read(path)` deny with no `Edit(path)`.

### Changed

- `Read(path)` deny no longer covers a shell redirect or `touch`; write both `Read` and `Edit`.
- `mv` operands are writes.
- Deny rules reach option values, `git diff`/`git grep`, recursive walks, and commands behind `env` or `sudo`.
- More reader commands are covered; symlinks resolve from both ends.

### Fixed

- `devplane open` served a board that never loaded.
- A multi-byte character crashed the matcher.
- `sed -n 1p .env` and `grep -f pats.txt .env` escaped deny rules.

## 0.1.0 — 2026-09-14

First release: watching Claude Code sessions, driving any Agent Client Protocol agent, verified-done with project
gates, declared pipelines, a per-repository permission gate, and the decision log.
