# Changelog

Notable changes per release, in the style of [Keep a Changelog](https://keepachangelog.com/). Dates are UTC.

## Unreleased

## 0.10.0 — 2026-09-25

### Breaking

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
