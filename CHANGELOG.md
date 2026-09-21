# Changelog

Notable changes per release. Dates are UTC.

## 0.7.0 — 2026-09-21

### Breaking

- **`ui/legacy.html` is gone, and `DEVPLANE_UI` points at a directory.** It took
  a path to the single HTML file; it now takes a built `ui/dist`. The interface
  is a bundle the binary embeds, and a build without one serves a page saying so
  rather than falling back.

- **`GET /api/work/{id}/changes` no longer returns `html`.** It returns
  `changes` only. The rendered form existed for the deleted page; anything
  reading the field should render the structured change set.

- **The interface has no keyboard shortcuts.** Every action is a control.

- **`devplane mcp` and `devplane statusline` are hidden from `--help`.** Both
  still exist and both still work — an agent runs the first and `devplane
  connect claude` writes the shim that calls the second. Anything parsing the
  help listing to discover commands will not see them.

- **The store schema is version 3.** `attention_log` gained `folded_at`. There
  is no migration: delete `~/.devplane/devplane.db` or let it be recreated.

### Added

- **A surface can be opened *about* something.** The shell reads
  `#<surface>/<focus>` and hands the second half to the surface, which is the
  only thing that makes a detail view addressable. It does not know what the
  focus means and must not.

  Three surfaces had no way in. *Why this is here* rendered "open a row and this
  shows what was decided" on every visit — with no row to open and nothing able
  to give it one. The work view read whichever Work happened to be first. Board
  rows link to their decision log now, as anchors rather than click handlers, so
  they work from the keyboard and open in a new tab.

- **Undo where it works, and a plain sentence where it cannot.** The inbox has
  one reversible action and one irreversible one, and until now it treated them
  the same: silently.

  Snooze can be taken back — `minutes=0` un-snoozes and the daemon has always
  accepted it, with a comment calling it *"the only way back"*. **No surface
  ever offered it.** So the one thing on the page a person could undo was the
  one thing they could not: they waited out the hour, or restarted the daemon.
  It is offered now, attached to the confirmation of the action that created it,
  and cleared by every path that is not reversible — a stale undo button offers
  to take back something else.

  Answering is the opposite case. It reaches the agent and nothing this page can
  send recalls it, so there is no control; the sentence says so at the moment it
  happens. Where an action cannot be taken back, saying so is the honest
  substitute for a button that could not keep its promise.

- **A surface that shows what a Work actually changed.** `ui/src/surfaces/changes/`
  reads the structured change set the daemon already served and renders it: file
  by file, hunk by hunk, with the four answers kept apart — hunks, a binary
  file's size, a body deliberately not shown with the reason, and a branch that
  changed nothing, which is a **finding** rather than an empty state because a
  gate that passed over no change verified nothing.

  **It renders the parsed shape, never HTML.** A diff is the densest
  concentration of somebody else's text this product shows — file names, commit
  content, whatever an agent wrote — so it is the worst possible place for a
  surface to insert markup a server composed. Every line carries `+`, `−` or a
  space, because red and green are the one pair that cannot be separated under
  deuteranopia and a diff is exactly where that matters.

  It was added as a **directory**, with no edit to any existing surface, no line
  in the shell and no route table — which is what the shell rebuild was for, now
  demonstrated rather than asserted.

- **`devplane doctor` says what is watched here, per vendor and per channel.**
  The product's headline sentence is about every agent on your machine, and that
  covers **two different lists**: an agent Devplane *drives* reports through the
  protocol by construction, and an agent somebody started *themselves* is
  readable only as far as that vendor publishes channels. Collapsing the two is
  how a claim becomes half true without anybody lying — and it had been
  collapsed, in the README.

  Three states, and the middle one is the point. `read` means demonstrated end
  to end. `not published` means the vendor offers nothing, so an empty surface
  for that vendor means *Devplane cannot see it* rather than *nothing is
  happening*. **`unproved`** is the honest description of a channel that is
  built and has never been run; collapsing it into either neighbour is the lie.

  Every row carries the date it was checked against the vendor's own
  documentation, and a guard fails when an agent that can be driven has no row
  saying whether it can be watched.

- **The board marks a session close to compaction, and the number comes from the
  daemon.** The context percentage was plain text at every value, so the one
  thing on this board worth catching early — a window about to compact — looked
  exactly like a window at three percent.

  It was the **last guard the rebuild owed**, carried on the ledger through
  every pass since the port began, and it was owed because there was nothing to
  hold: no surface coloured a gauge. It is closed by building the thing rather
  than by reclassifying it.

  The threshold is `context_high_percent`, read from the board payload the
  daemon already served and nothing read. A figure written into the page would
  disagree with the one `devplane ls` uses the moment somebody changed it, and
  the two surfaces would call the same session crowded and fine. **No threshold
  means no opinion**: a default invented in the page would be this surface
  deciding what "crowded" means on its own, which is the failure the guard
  exists to prevent arriving by the back door. The mark carries a word, not only
  a colour.

  With it the ledger over the deleted page closes: **35 carried, 17 dropped, 0
  owed.**

- **`devplane rules` — which of your repositories is missing a rule.** The
  fleet half of *answer once*: `devplane explain` answers for one call on one
  machine, this asks across every registered project.

  Both files are read and never conflated — a `devplane.toml` prohibition is
  what Devplane refuses, a `permissions.deny` entry is what the agent refuses.
  Four states per file: **has it**, **covered** by a named wider rule,
  **missing**, or **unreadable**, whose fix is `devplane check` rather than a
  paste. Coverage uses the containment procedure that already finds redundant
  rules; a pair it cannot decide is reported as missing rather than guessed at.

  With no argument, the rules some projects hold and others do not.

  **It writes nothing and offers no apply-to-all**, proved over the source and
  mutation-tested. Adding instruction files helped 27.7% of 148 measured
  projects and hurt 26.35% — what separated them was what the rules said.

- **The inbox folds instead of growing without bound.** Above twelve rows,
  kinds whose members are interchangeable to you — issues assigned, reviews
  requested, stalled sessions, context warnings — collapse into one row naming
  the kind, the project and the count. And where one raised item is a *named*
  consequence of another (a configuration that will not parse explains the
  refusals in that project; a gate that is not answering explains calls with no
  verdict; a leaked agent explains the sessions it holds) the consequence is
  counted on the cause's row.

  **Nothing is ever hidden without a count.** Rendered + summarised +
  counted-on-a-cause == raised, asserted over a set spanning every kind, with
  every id reachable. An inbox short enough to read renders exactly as it did
  before, with no summary rows at all.

  **Four kinds are never folded** — a question, an abandoned question, a
  permission, a human step. Each needs an answer only you can give, and a
  summary row is a question nobody saw with a number beside it. The foldable
  set is enumerated in the code, so a kind added later is unfoldable until
  somebody decides otherwise.

  The reason is a measurement rather than taste: oversight modelled as a finite
  attention budget is an inverted U, and at a reviewer capacity of 50,
  escalating **100 %** of actions lets **39 %** of danger through against 22 %
  at 72 %. A list that grows without bound stops being read exactly when it
  matters.

- **`devplane attention` reports whether folding was right.** Two columns per
  kind: how often it was folded, and how often it was folded **and then acted
  on once opened**. A kind always folded and never acted on is one nobody
  needed as a row; a kind folded and then acted on is one the summary was
  standing in front of. Counted on a read, never on a poll — the board fetches
  the inbox every couple of seconds, and counting folds per render would
  measure polling.

- **The done certificate is on the page.** It existed only behind
  `devplane work export` — a command nobody types — while the board showed the
  gate that produced it and never what it proves. Opening a piece of work now
  shows the basis, the commands with their outcomes, where the evidence came
  from, and one button that puts the whole certificate on the clipboard as
  markdown: the same bytes the command writes. Two clicks from a finished Work
  to a pull request body.

  Every sentence is composed by the daemon. The page renders and words nothing,
  held by an absence check — a certificate described twice is one that can
  disagree with itself, and nothing would notice.

- **The certificate says where its predicate came from.** `gen_ai.evidence.origin`,
  the name the OpenTelemetry GenAI conventions have open for it, on the in-toto
  statement, in the markdown and on the page:

  | Value | What carries it |
  |---|---|
  | `externally_observed` | the gate transcript — commands this tool ran, and the codes they ended on |
  | `self_reported` | the agent's own account, carried as a claim and never as the predicate |
  | *absent* | not known — **never defaulted** |

  The third state is the point: the only value anybody would default to is the
  flattering one, and a certificate that quietly promotes *the agent said so* to
  *a check observed it* is the failure the document exists to prevent. This had
  been recorded as adopted in four places and implemented nowhere.

- **All four ways a Work reaches done render as a sentence**, and *no gate was
  declared* is one of them rather than an empty block — a blank reads as
  *nothing to show* where it means *this project never said what done means*. A
  Work that finished before Devplane kept a record says **that**, which is
  neither *unfinished* nor an empty certificate.

- **`devplane gate run --name <gate>` runs one gate from `[gates.named]`.** A
  named gate could be declared, validated by `devplane check` and listed by it —
  and the only thing that could *run* one was a pipeline step. So a person who
  wrote one down had no way to try it before wiring a pipeline around it, which
  is a configuration key with no reader for its commonest use.

  With no `--name` it runs `check`, exactly as before. `expect = "fail"` is
  honoured, so a gate that did what it was asked to do is not reported as a
  failure. A name nothing declares exits non-zero and prints the names that are
  declared, rather than passing silently — an empty gate that reads as success
  is the failure that layer exists to prevent.

- **An empty inbox says what the day came to.** It printed *"Nothing needs
  you"* and stopped. Now it says what was decided without you, by whom, and what
  will want you next — the close is the one moment this product has something
  good to report, and an absence of rows is not it.

- **An audit row for an MCP tool call says where that server came from.**
  Claude Code began carrying the server's name and a **source** — `plugin`,
  `sdk`, `user`, `project` — on five hook events on 2026-09-18, with the
  instruction to base trust on `source` rather than on the name or the
  `mcp__<server>__` prefix. Devplane read neither, and an audit row for a call
  into a server a cloned repository defined read exactly like one into a server
  the person installed themselves.

  It is now recorded and shown. **Nothing is derived from it**, and an absence
  check over the policy holds that no verdict may read the field: deciding which
  provenances are acceptable is a judgement the owner makes in their own
  settings. A value this build has never seen is printed as received rather than
  mapped to a guess, and rows that cannot have a provenance say nothing rather
  than reserving a blank.

- **`devplane --help` groups its thirty-five commands by errand.** See what is
  happening · what needs you and what happened without you · start and steer
  work · set up a project · the daemon. A test asserts every visible command
  belongs to exactly one group, so adding one without filing it fails the build.

  Nothing was renamed, merged or removed: the list is long because the product
  does a lot, and the five *seat* surfaces each answer a question the others
  cannot.

- **`CLICOLOR_FORCE` turns colour on where it would otherwise be off** — for a
  pager, or a CI log that renders escapes. `NO_COLOR` still wins.

- **When Devplane cannot read a command line, it asks you** instead of saying
  nothing. `rm$IFS-rf x`, `$(echo rm) -rf x`, `eval "…"` and `curl … | sh` hide
  what runs behind something no matcher can resolve, and reporting *no rule
  answers this* would be true and misleading.

  The verdict is `unresolved`, on Devplane's own authority rather than credited
  to a rule that did not decide it, and it carries the reason. Only in a project
  whose rules could have applied, so an inbox does not fill with questions
  nobody asked for.

- **A question an agent asked and moved past no longer disappears.** When a session Devplane
  *watches* asks you something and then starts another tool call without an answer, the question used
  to leave the inbox exactly as if you had answered it — the two produced identical state and nothing
  anywhere recorded that nobody had. It is now a `question_abandoned` item naming the question as the
  agent wrote it, the options it was choosing between, and what it did instead; and it is in
  `devplane asks` as *nobody answered*, the same word the durable asks use.

  **No answer action, deliberately**: the tool call is over, so a button there would offer something
  no route can deliver. The item offers the session instead.

  **And an empty list says which vendors it cannot speak for.** The derivation is Claude Code's hook
  events and no other vendor documents an equivalent, so *no question was abandoned* and *this cannot
  be seen for Copilot* are printed as two different sentences.

  One thing it deliberately does not claim: a question closed by your agent's own auto-continue
  timer. That **submits**, so the tool succeeds and it is indistinguishable from an answer from
  outside — `devplane modes` is the surface that covers that half.

- **`devplane modes` reports the question timer per session, including the one that overrides your
  settings.** `CLAUDE_AFK_TIMEOUT_MS` takes precedence over `askUserQuestionTimeout` and turns
  auto-continue on **even where your settings say `never`**; `0` closes each question immediately
  rather than turning the timeout off. Devplane reads it from the environment each session was
  started in — its `SessionStart` hook runs as a child of that session — and shows it under that
  session, with the machine-wide setting it overrode.

  **This closes a hole in which the surface could say the opposite of the truth.** Before it,
  `devplane modes` read two settings files and nothing else, so a session running with
  `CLAUDE_AFK_TIMEOUT_MS=0` — every question ended by nobody the instant it was asked — showed no
  timer at all, and the summary line read *Every session that has reported asks you*.

  Three states, and no two of them read alike: a clock, **no clock**, and **not read** — a session
  that started before `devplane connect` has no environment reading, is counted separately, and is
  never rendered as *nothing is set*.

### Changed

- **The interface is rebuilt, and `ui/legacy.html` is deleted.** Svelte 5 on
  Vite, built to a bundle the binary embeds — one artefact, nothing fetched, and
  the output stays readable so the page can be followed without this repository.

  **It opens on what needs you**, and the rest is a sidebar in three bands:
  what is asking for you, what you are doing, how the machine is set up. Nine
  surfaces whose names are sentences do not fit across the top without becoming
  a menu bar you read left to right.

  **Adding a surface is adding a directory.** `import.meta.glob` resolves
  `ui/src/surfaces/*/index.ts`; no shared file names a surface, so a new one is
  not a merge conflict.

  **A session row leads with its state**, what it is doing is the subject, and
  the numbers are metadata against the right edge. The row links to its decision
  log, so a detail surface can be opened *about* something rather than hoping
  the right thing is first.

  **Escaping is structural rather than remembered.** Svelte escapes by
  construction, so the rule is that no surface uses the one construct that opts
  out — which matters most on the diff surface, the densest concentration of
  somebody else's text the product renders.

  **There are no keyboard shortcuts.** Every action is a control, so the focus
  ring and a skip link are the whole keyboard story.

  The switch was one change: the two interfaces were never live together. The 52
  guards over the page it replaces are each carried by a named guard or dropped
  with a reason, and the sixteen controls it offered are accounted for the same
  way.

- **The property ratchet was rebuilt, because a floor that moves when it is
  inconvenient is not a floor.** The constant had been re-seated twice in one day
  — once for the keyboard removal, once at the switch — and neither could be
  attributed afterwards, because the counter was fixed in the same window.

  The floor is no longer a constant anybody edits. It is the measured peak less
  the properties listed in `REMOVED_WITH_FEATURE`, each with a count and a
  reason, so deleting guards fails the check until the deletion is **written
  down** — and the write-down is a diff a reviewer sees rather than a number that
  quietly got smaller. The accounting now happens at the moment of removal, by
  the person who knows what they removed.

- **The property counter was wrong a second time, in the same direction.** It
  had been fixed once — it matched `fail(` only at the start of a line and
  missed every `if (…) fail("…")`, reporting 60 against a true 99. The fix
  matched `fail("`, which misses every message written with a backtick: eight
  of them, all the ones that name a surface.

  It no longer matches messages at all. It counts the calls, then asserts that
  each one it counted is one it knows how to read, so a third quote style fails
  the check instead of quietly lowering the number. A counter that measures the
  wrong thing is worse than none, because it is believed — and this one was
  being read to decide whether switching was safe.

- **Devplane says what it records, instead of naming a category four other
  projects lead with.** Every published surface opened on *the local-first
  control plane for AI coding agents* — a six-word noun phrase held by
  `builderz-labs/mission-control` (6,247★), `loopx-project/loopx` (5,906★),
  `mixpeek/amux` (481★) and `preloop/preloop`, with the field's largest project
  at 12,213★ describing the same thing as *supervising* coding agents. A first
  line a reader compares against somebody with six thousand stars is not one to
  write.

  The phrase is gone from `Cargo.toml`, `site/zola.toml`, `site/static/llms.txt`,
  the plugin marketplace description, `devplane --help` and the rustdoc, and all
  six now lead with what is actually recorded:

  > Devplane records who decided, when nobody asked you — a person, a rule, a
  > classifier, a timer, or nobody — across every project and every coding agent
  > on your machine.

  `README.md` leads with the same sentence and **`Devplane never approves a tool
  call` moves from paragraph three to paragraph two**, which is where the
  strongest thing this project does belongs.

### Fixed

- **The diff surface printed no file count.** `plural(n, one, many)` returns the
  word and the call passed two arguments, so the line read `· file` with no
  number. `svelte-check` had been reporting it; nothing ran `svelte-check`.

- **The board's empty state made a claim about your machine that it could not
  check.** It said *"No agent session is running on this machine"*. On a machine
  running three Codex sessions that is false — Devplane cannot see those at all —
  and it is false in the reassuring direction, on the surface people trust to
  tell them nothing needs them.

  `devplane ls` had always got this right: *no **Claude Code** sessions are
  running*. One surface was honest and its twin was not, which is what happens
  when two surfaces compose the same sentence in two places.

  Both read one table now. The board says what it watches, names the vendors
  whose channels are read but unproved as a separate sentence — folding them in
  would be the same overstatement in a smaller font — and names the ones that
  appear **only when Devplane starts them**, which is the gap a person has no
  other way to discover.

- **A message shipped with a newline and nine spaces in the middle of a
  sentence**, and the guard that exists to catch exactly that waved it through.

  `no_message_carries_a_collapsed_line_continuation` exempted any run of spaces
  under ten that followed a `\n`, reasoning that a two- or four-space
  continuation indent is deliberate. The reasoning is sound and the exemption
  was useless: the check only fires on a run of **six or more**, so a deliberate
  indent never reaches it. All the exemption could do was wave through runs of
  six to nine — and it did.

  Measured before removing it: every `\n`-plus-spaces run in `src/` is two or
  four spaces, or a `.join("\n   ")` whose spaces end at the closing quote and
  was already excluded. Nothing legitimate depended on it. With the exemption
  gone the check immediately found the one real instance and nothing else.

- **Copilot's `notification` event was never mapped** — the one that says
  somebody is waiting on you. Eleven of GitHub's fourteen hook events were read;
  the missing one carries `notification_type: "permission_prompt"` in the same
  vocabulary Claude Code uses, so a Copilot session with a dialog open produced
  no block on the surface the product is named for.

  Twelve are mapped now. `permissionRequest` is silent by decision: it fires
  before the permission service runs, so it says a decision is about to be taken
  rather than what it was, and recording it as a block would mark every
  auto-allowed call as waiting on a person. A guard fails when the vendor's
  event count moves.

- **A `doctor` column was one character out of line.** `render::pad` guarantees
  at least one space, so a string exactly as long as the column comes back one
  wider — correct for a table whose columns must not touch, and wrong when the
  caller adds its own separator on top. The gap is part of the column now.

- **The decision log could come back in the wrong order.** `decisions` sorted by
  timestamp alone, and `at` ties constantly — a gate finishing and the run it was
  about ending share a second — so SQLite was free to return either first.

  It surfaced as a test that passed alone and failed under parallel load, which
  is the mild version. The real cost is the audit page showing two decisions in
  the wrong sequence, in the one table whose entire purpose is saying what
  happened in what order.

  The tie-break is `rowid`, which is the append order of an append-only log. The
  row's own id cannot serve: it is a uuid v7 whose head is a millisecond and
  whose tail is random, so two rows written in the same millisecond would sort
  by the random part — which is not an order at all.

- **Nothing ever marked the items you had not seen.** The inbox has shown
  *since you last looked · 16h* since 0.6.0, and the response has carried the
  moment it was measured from so a surface could compare — and neither surface
  ever did. Items raised inside the gap looked exactly like the ones that were
  already there.

  They are marked `new` now, on `devplane inbox` and on the board, as a **word**
  rather than a colour: red and amber cannot be separated under deuteranopia at
  any usable lightness, so nothing here is distinguishable by colour alone. The
  daemon decides which rows are new, so the two surfaces cannot draw the
  boundary in different places, and nothing is marked before your first look —
  there is no boundary yet to be on the far side of.

- **The board said nothing about the question clock.** `devplane modes` has
  reported what can answer a question on this machine without you; the board
  did not carry it at all, so the two surfaces disagreed about a fact about the
  machine. The board now shows the same sentence, composed in the same place —
  and **only where a clock actually answers**. A clock set to `never` is
  somebody having written down that nothing may answer for them, which is true,
  reassuring, and not what a header is for.

- **An audit row could not tell *no server* from *a server whose origin nobody
  reported*.** Both printed nothing, so an MCP tool call from a vendor whose
  channel has no provenance field read exactly like `Bash(ls)` — a blank that
  taught you *ordinary tool* when it meant *unknowable here*. There are three
  states now, and the middle one says so.

- **`devplane modes` called the setting's own default a timer answering in your
  name.** `askUserQuestionTimeout` takes one of four values — `60s`, `5m`,
  `10m`, or **`never`, which is its default** — and every string in the settings
  file was read as a duration. So a person who had explicitly set `never` was
  told:

  ```text
  you set a never timer on your own questions — after that, whatever is
  selected is submitted
  ```

  False, about the most likely value in any file, and in the direction of alarm.
  It was worst where it mattered most: an administrator deploying `never` in
  managed settings is **hardening** the machine, and Devplane reported it as
  somebody taking the person's attention away.

  `never` is now a value rather than a missing case. The line stays — somebody
  wrote it down — and says the questions wait. A clock that does not answer is
  told apart from one that does, and from nothing being set at all, by a flag on
  the wire rather than by comparing a rendered string.

- **The clock line never said where the clock was set.** The settings path was
  read as `file`; the field had been renamed `where_set` two releases earlier,
  and the reader fell back to an empty string — so a person was told a clock was
  answering their questions and never told where to change it. Every test
  passed, because none of them asserted on the location and `""` is a valid
  string.

  The CLI now deserialises the same type the API serialises, so a rename is a
  compile error. An absence check keeps the clock path from going back to
  reading its own API key by key.

- **Every aligned column collapsed when colour was on.** `{:<10}` pads to a
  string's character count, and a painted string carries ten characters of escape
  code that occupy no columns — so a ten-wide column holding a coloured `failed`
  measured sixteen, padded to nothing, and the next field began immediately after
  it:

  ```text
  failedcargo clippy --all-targets --all-features -- -D warningsexit 101
  ```

  Uncoloured, the same code was correct — and **every test in this repository
  captures stdout, which is not a terminal**, so every test had only ever seen
  the version that worked. Fixed by padding to *visible* width at eleven call
  sites across five surfaces, with a column now a minimum rather than a promise,
  so a field that overruns still separates from the next one.

- **`devplane speckit install` refuses to register a hook nothing can run.** The
  entry it writes is `optional: false`, which means the agent is told it may not
  skip it — so registering it where no `/devplane-gate` skill is reachable put a
  step in the workflow that cannot be performed. It now checks, names the two
  ways to fix it, and takes `--anyway`.

- **The `typescript` feature did not compile.** `AbandonedQuestion` carried a
  `Vec<Choice>` and `Choice` had no export, so `cargo build --features
  typescript` failed on a tree where everything else was green.

- **A prohibition no longer walks past `sudo`, `exec`, `env` or an absolute
  path.** With `never_auto = ["Bash(rm *)"]` set, `rm -rf x` was refused and
  `sudo rm -rf x`, `doas rm -rf x`, `exec rm -rf x`, `env FOO=1 rm -rf x`,
  `watch rm -rf x` and `/bin/rm -rf x` were **allowed without a word** — while
  `nohup rm -rf x` and `timeout 5 rm -rf x` were refused, because those two
  wrappers happened to be on a different list.

  A restrictive rule now looks through the wrappers that run something else
  under another user, environment or process image, and matches a program's file
  name as well as the path it was spelled with.

  **Claude Code does neither**, and this module spent its life agreeing with it —
  correctly, while Devplane could still *approve*, because a matcher broader than
  the vendor's would have approved calls the user's own settings refuse. Devplane
  has not been able to approve since the permission gate was deleted, so the only
  thing a broader match can do now is refuse more, and refusing more is free. The
  premise changed and the matcher did not.

- **Five writers no longer reach a protected file past an `Edit` rule.**
  `never_auto = ["Edit(secrets/**)"]` refused `tee secrets/k.txt` and
  `echo x > secrets/k.txt` and allowed `cp /tmp/a secrets/k.txt`,
  `truncate -s 0 secrets/k.txt`, `dd of=secrets/k.txt`,
  `install -m 600 /tmp/a secrets/k.txt`, `rsync /tmp/a secrets/k.txt` and
  `ln -s /tmp/a secrets/k.txt`. Claude Code does not apply file rules to those
  commands, and this table mirrored it.

  On the restrictive side it no longer does. The direction is kept, so
  `cp secrets/k.txt /tmp/b` is a **read** of the protected file and meets a
  `Read(…)` rule rather than an `Edit(…)` one.

- **`devplane explain --replay` no longer ignores your machine-wide rules.** It
  built its evaluator from the projects' files alone, so every call
  `~/.devplane/policy.toml` forbids was reported as *no rule here*. The
  constructor it used says in its own documentation that it is for tests and that
  `devplane explain` using it was a bug; the fix had reached `explain` and not
  `explain --replay`.

- **One evaluator, not two.** `PolicyCache::evaluate` and
  `PolicyCache::restrictive` had identical bodies — left over from the days when
  one of them could return `allow` — and the hook chose between them by event,
  under a comment explaining a difference that was not there.

- **A question's deadline no longer counts the time Devplane was not running.**
  A project setting `[questions] deadline` had every waiting question expired
  within a minute of the next daemon start: the sweep measured wall-clock
  seconds from the moment the question was asked, so closing a laptop at 17:00
  with a `10m` deadline produced, at 09:00 the next morning, an audit row reading
  *"a clock refused it after 10m"* about ten minutes nobody was ever given.

  A deadline bounds how long a question waits for somebody who **could** have
  answered it. While the daemon is down there is no board, no inbox and no
  notification, so the clock is not running; it now starts at the later of *when
  it was asked* and *when Devplane last started*. A restart can only ever lengthen
  a wait, never shorten one.

  This also restores a guarantee that was silently false for every project with a
  deadline set: *a daemon that was stopped leaves the question open and
  answerable after the restart*.

### Documentation

- **`devplane modes` names one thing it cannot see.** The `CLAUDE_AFK_TIMEOUT_MS` environment
  variable overrides the `askUserQuestionTimeout` setting and turns question auto-continue on even
  where the setting says `never`, so the absence of a timer line does not prove there is no timer.
  The CLI reference now says so and says how to check. Reading it per session is specified as
  `019-the-clock-on-this-session`.

### Removed

- **`core::diff::render` and the `html` field beside it.** The change-set route
  served rendered HTML for `ui/legacy.html` to insert. That page is deleted, and
  after the switch the function was read by nothing but its own tests while the
  route went on composing markup no client asked for.

  `esc` went with it — an HTML escaper whose doc comment said *the only way text
  leaves this module*, which is now true of nothing, because no text leaves that
  module as markup. The surface escapes by construction, which is a stronger
  guarantee than a function everybody has to remember to call.

  **Every claim the deleted tests made survives, relocated to where it is now
  true.** The hostile-input test no longer asserts escaping — it asserts the
  parser carries hostile text **verbatim**, because a parser that sanitises has
  changed the diff it was asked to report, and a reviewer approving the
  sanitised version is approving something nobody wrote.

- **`in_force` and the `InForce` type.** They existed to answer *what did this
  session's environment override?*, had no caller in the product, and could not
  have worked: `never` — the value the question is entirely about — had no
  representation, so the field meant to carry it could not. `devplane modes`
  already prints the machine's clock and each session's own, and the session
  sentence says in words that it overrides the files.

- **`AcpEvent::PermissionExpired`.** Orphaned when the hard-coded ten-minute
  permission timeout was deleted in 0.6.0: the variant stayed declared and
  handled while nothing could construct it, and its handler wrote a decision row
  reading *"nobody answered within ten minutes"* about a rule that no longer
  exists. Nothing observable changes — the handler could never run. An expiry now
  comes only from a project's own deadline.

- **`devplane mcp` and `devplane statusline` are no longer listed in `--help`.**
  Both are surfaces for a machine — one goes in an agent's configuration file, the
  other is invoked by Claude Code — and neither is a command anybody types, which
  is the same reason `devplane hook` has been hidden all along. **Both still work
  exactly as before** and both are still in the CLI reference.

### Internal

- **This repository's own gate was weaker than its CI.** `devplane.toml`
  declared four checks; CI ran three more over the interface — a build, a
  `svelte-check` and the wire-type export. So `devplane gate run` could pass and
  the build fail ten minutes later, which it did: an extraction left an unused
  import, a dead prop and eight orphaned CSS selectors.

  A product that ships a definition of done, whose own definition of done is
  narrower than the thing that actually gates its merges, has the defect it
  exists to surface — pointed inward. The gate runs the interface checks now.

- **The crate would have published with no interface.** `ui/dist/` is generated
  and gitignored, so a clean checkout has none — and `include` in `Cargo.toml`
  matches nothing rather than failing. Both jobs that run `cargo publish` did so
  without building it, so `cargo install devplane` would have produced a binary
  whose one page says it was built without an interface.

  The guard that exists for this asked whether `release.yml` *mentions*
  `npm run build`. It does — in the job that builds the release binaries, which
  is not the job that publishes. It checks per job now.

- **The render harness could pass on its own prose, and did.** A check that
  greps a surface for the construct it forbids finds the comment explaining why
  the construct is forbidden — so it passes for ever afterwards, including once
  the thing it guards has been deleted.

  This has now caught the repository **five times**, twice within the hour the
  diff surface and the undo contract were written. It is no longer something to
  remember: `source()` strips comments and is the only way the harness reads a
  surface, and a guard fails if any check opens one directly. Both halves
  mutation-verified.

- **The property counter was corrected a third time, and this time it caught
  itself.** It reads `fail(` calls and asserts every one carries a message it
  can parse; two new calls put their message on the next line after the
  formatter wrapped them, and the assert refused to undercount rather than
  quietly reporting a smaller number. The quote is looked for past any
  whitespace now. The count is **123**, up from 104.

- **The README's command count is guarded.** It has drifted twice in opposite
  directions — once counted by eye as thirty-six and corrected only by piping
  `--help` through `wc`, once left at thirty-five when a thirty-sixth command
  landed. It is checked against `COMMAND_GROUPS` now.

- **A latency budget was being measured by the test suite rather than by the
  gate.** `the_policy_gate_answers_fast_enough_to_be_invisible` asserted on the
  **worst** of twenty runs — the right statistic for *must not be felt*, and the
  wrong one to measure inside `cargo test`, where fifteen binaries run in
  parallel and each spawns processes. It passed for months on luck and started
  failing at **598ms** the day another test began spawning a process, while the
  same test run alone measured well inside budget.

  A budget that fails for reasons unrelated to what it measures gets raised
  until it stops failing, and then it is not a budget. The **median** carries it
  now — a real regression moves every run, scheduler noise moves the tail — with
  a loose five-second ceiling on the worst run, set where only a hang can reach
  it. That catches a gate that answers instantly nineteen times and hangs once,
  which the median alone would not.

- **`agent-client-protocol` 2.1 → 2.2**, schema 1.7.0 → **1.9.1**. Clean —
  nothing this product uses changed, and session notices are still not
  advertised.

- Three public functions with no reader are gone (`api::content_type_of`,
  `github::Issue::has_label`) or are now `#[cfg(test)]`-gated
  (`store::open_in_memory`, which was a test fixture compiled into every shipped
  binary). The guard that finds them read `src/core/` only and now reads the
  whole crate.

## 0.6.0 — 2026-09-20

A question an agent asked you now outlives the process that asked it; Devplane
stops answering *yes* on your agent's behalf; and the project is renamed from
Vibeplane. Each of those changes something you have already configured, so the
breaking list below is the part to read.

**And Devplane stopped telling itself one lie.** Work that was interrupted by a
clean shutdown was recorded as `completed` — *done*, written by the tool, over
work nobody finished. There is an `interrupted` state for it now, `devplane stop`
waits for the endings it causes to be written, and a process you are not allowed
to signal is no longer reported as dead.

### Breaking

- **`state` can now be `interrupted`.** A new run state, in `devplane ls --json`,
  `/api/board` and every other surface that carries one. Anything matching on the
  set of states needs the new arm; it means **Devplane stopped this run** because
  the daemon was shutting down, as distinct from `stopped` (you did) and `lost`
  (a process was expected and not found).

- **Renamed from Vibeplane**: the binary, the project file `devplane.toml`, the
  machine directory `~/.devplane/`, the `DEVPLANE_*` variables and the
  `devplane:ready` label. Nothing is read under the old names and nothing
  migrates itself:

  ```sh
  mv ~/.vibeplane ~/.devplane && mv ~/.devplane/vibeplane.db ~/.devplane/devplane.db
  mv vibeplane.toml devplane.toml          # in each project
  devplane connect claude                  # the installed hooks name the old binary
  ```
- **Devplane no longer approves a tool call.** `Verdict` has no `Allow` variant,
  so the type cannot express an approval, and a `devplane.toml` containing
  `auto_allow` fails to load and names the line. Calls those rules used to answer
  *yes* for are answered by your agent's own permission system, exactly as if
  Devplane had said nothing; `devplane explain --replay` composes the grants to
  move into its settings.
- **`devplane decide` is deleted.** `devplane answer <ask>` covers a permission
  and a question alike, because the ask records which it is. `POST
  /api/asks/{id}/answer` and `GET /api/asks` replace `/api/runs/{id}/decide` and
  `/api/runs/{id}/answer`.
- **Nothing refuses a permission after ten minutes any more.** That clock was a
  constant nobody chose and no surface reported. A permission and a question both
  wait, and the only clock that ends one is a project's own:

  ```toml
  [questions]
  deadline = "4h"   # never (the default) | 90s | 30m | 4h
  ```

  When it fires the agent is told **no**, and the audit row names *a clock*, the
  duration and the file that set it. A value that will not parse fails
  `devplane check` and the wait stays unbounded.
- **The decision log records `authority`, not `actor`**, with five values:
  `person`, `rule`, `timer`, `nobody`, `daemon`. Three of those were spelled
  `daemon`, so *a clock refused a call* and *a question died unanswered* were
  indistinguishable from *Devplane ran a gate* in the one column the table exists
  to be filtered by.
- **There is no database migration.** The store schema is **version 1** — the
  count starts here, because nothing before this release describes a database
  anybody has. A file stamped with any other version is moved aside to
  `devplane.v<n>.bak` and a fresh one takes its place. Everything but the
  decision log is re-derivable, which is why the old file is moved, never
  deleted.

### Removed

- **Devplane's own ten-minute clock**, and `devplane decide` with it.
- **Three functions the permission gate's deletion had orphaned**, including one
  whose job — *never compose a rule for a command whose quoting this matcher
  cannot read* — turned out to still have a home: the rule Devplane offers you
  to paste into your agent's settings is now refused for such a command, because
  a suggestion that widens silently arrives with this product's name on it.
- **The permission mirror, and everything built to keep it honest.** Answering
  *yes* on the vendor's behalf was a claim about somebody else's code. Keeping it
  true needed three compatibility floors, a differential harness and a release
  clock, and it produced **thirty-three** occasions when the mirror was wrong in
  the dangerous direction — at a measured **$1,756–$3,511 a month** in probe
  spend. Deleted: `Verdict::Allow`, the harness scripts, the rule ledger, the
  conformance dialect axis and the release cadence. What is left prohibits and
  defers, which claims nothing about anyone and cannot decay.
- **`devplane gate`.** The command reported how current the mirror's measurement
  was; there is no mirror to measure.
- **The version-gap warning.** `devplane ls` and `doctor` counted how many
  releases a running Claude Code was past the release the rules were measured
  against. It measured the decay of a claim Devplane stopped making. `doctor`
  now states the release the rule syntax was modelled on, and nothing else.
- **Source maps from the shipped binary.** Four times the size of the bundle,
  and readable output makes them unnecessary.

### Added

- **`npx devplane`.** The release publishes an npm package alongside the shell
  installer, so a machine with Node can run Devplane without installing it. An
  `npx` run and an installed binary share `~/.devplane/` and produce one daemon.
  Published through npm trusted publishing, so the package carries a provenance
  attestation and no publishing token is stored in this repository.

- **`devplane agents` shows what each agent advertised when it started** —
  `resume`, `load`, `list`, whether it declares a mode, whether it needs signing
  into — with the date it was measured:

  ```console
  claude     Claude Code    npx @zed-industries/claude-code-acp
             resume · load · modes · measured 2026-09-20
  ```

  An agent you have never started has **no line at all**, rather than a row of
  crosses: *not probed* and *not supported* are different facts.

- **`devplane modes` reports the mode an ACP agent declares for itself**, so
  *which projects are deciding without you* is answerable beyond Claude Code:

  ```console
  saas
    7c                    plan-only (the agent's own mode)   seen 2026-09-20T08:14
  ```

  Shown in the agent's own word, never mapped onto
  `default`/`acceptEdits`/`plan`/`bypassPermissions`, and uncoloured: it does not
  say whether a person is asked. The mode a session **starts** in is reported,
  not only changes to it. `session/set_mode` is not sent; changing a mode is a
  mutation and this reads.

- **The board carries the number the inbox keeps about itself** — *You answered
  0 of the 49 decisions taken in your name.* `devplane attention` had printed it;
  the board had not.

  A **count, not a grade**: the contrast fires only when nothing at all reached
  you out of a non-zero total, and a week in which nothing happened shows
  nothing. The citation sits beside it, marked as somebody else's result.

- **A verdict for the workflows that only report.** `devplane gate run` runs this
  repository's `[gates]` and says what they exited with; `devplane speckit install`
  registers it as a Spec Kit extension hook, so a `/speckit-implement` run gets a
  verdict from outside the agent. Four outcomes and only `verified` exits 0 —
  *no checks declared* and *configuration unreadable* are distinct from each other
  and from success, because a workflow reading the exit code alone would treat an
  empty `devplane.toml` as a green build. It decides on exit codes: no
  specification is read and no prose is graded.
- **A question an agent asked you outlives the process that asked it.** An ask is
  a row with its own id, answerable from any surface however long afterwards. Stop
  the daemon with a question waiting and it is still there: still in the inbox,
  still answerable, and answering it resumes the session the agent left behind and
  delivers what you chose. It was held in memory on a live connection, so a
  restart took the run, the agent and the question together.
- **Intel Macs are a release target.** `x86_64-apple-darwin` cross-compiles from
  the same runner that builds the ARM one, so nothing needs to build from source
  on macOS.
- **A Claude Code plugin.** `claude plugin marketplace add hupe1980/devplane`,
  then `/plugin install devplane@devplane`, adds Devplane's **read-only** MCP
  surface and a skill that says what to ask it. It ships no hooks: those carry a
  machine-specific port and token, so `devplane connect claude` still writes
  them into your own settings where you can read them back.
- **`devplane modes` says whose clock can answer a question in your name.**
  Claude Code's `askUserQuestionTimeout` is `user or managed` scope, so an
  administrator can set one and the vendor's own settings UI hides the row while
  they have. Devplane reports the duration and the file — in yellow when you did
  not choose it — and never writes the setting.
- **`devplane asks`** — everything an agent has asked you and what became of
  each one. Open first and oldest first, because it is a queue of what is owed
  to you rather than a feed; settled ones carry the sentence that ended them,
  and no two of those read alike.
- **`devplane audit --without-me`** — only what was decided *instead of* you: a
  rule, a clock, or nobody. Your own answers and Devplane running a gate are
  left out, because those are the rows you already know about.
- **`devplane check` says what happens to a question nobody answers**, because
  that is a decision the file takes on your behalf.
- **`devplane library report` says where a copy would go and who documents that
  path**, which is the line that answers *why this directory and no other*.
- **`devplane library`** — prompts and skills reused across projects, in the
  vendors' own formats, unmodified. `diff` says which copies drifted and which
  way, which projects lack one, and which frontmatter fields are a documented
  hard error on Anthropic's distribution paths; `report` says what a skill will
  be allowed to do and where it came from; `install` copies byte-for-byte into
  vendor-documented paths only, naming every refusal before the first write;
  `sync` writes nothing without `--apply`. No verb grades anything, and none
  translates between vendors.
- **`devplane dispatch --to`** — one prompt to several repositories, as one
  reviewable row. `--mode draft|gate|pr`; draft is chosen for you above three
  targets and says so. Every refusal is named before anything is written, and a
  project name matching nothing stops the whole dispatch. No position merges and
  none weakens a permission.
- **`devplane batch`** — a fan-out with one outcome per target, questions first.
  No percentage, no pass rate, no colour on the batch itself.
- **Leaked agents are found and reported.** A daemon killed rather than stopped
  leaves its agents running, unreachable and still spending. The next daemon
  finds them from the process table and raises a critical inbox row with the
  command that ends one. It does not kill them: one may be part-way through
  writing what it was last asked to do.
- **The page opens on what needs you.** One list across every project, ordered
  by what is waiting on a person — work and requests together, because a red
  check on yesterday's branch and a permission asked two minutes ago are the
  same question. Sessions sit below it. Three empty states, because they are
  three different facts: *nothing needs you*, *Devplane has not answered
  recently*, and *some projects could not be read*.
- **A done certificate a reviewer can check without trusting this tool.** The
  repository, the commit, the commands, their outcomes, and how to re-run them —
  as Markdown or JSON. It is deliberately **unsigned**: the standard for this
  shape keeps the producer inside the trust boundary, and this does not ask to be
  believed. It also states what a green tick would hide — the commit is on no
  remote, the tree was dirty, it passed on the fourth attempt — inside the
  artefact, so those survive the paste.
- **A permission says how to stop being asked it again, and nothing writes the
  rule.** An item carries the narrowest rule covering the calls this machine has
  seen in that family, how many it covers, and the file to paste it into — which
  is your agent's own `settings.json`, in that file's own JSON. A pattern is
  offered only past three distinct calls. The rule is **replayed against the call
  before you are shown it**, so one that would not have decided it is refused
  rather than handed over.
- **A work view: what changed, what the checks said, and the release control
  beside both.** A work row opens what its branch changed — against the **merge
  base**, so commits that landed on `main` since are not reported as this work's
  doing — with every gate command, its exit code and the failing lines the agent
  was handed. Uncommitted *and untracked* files count, and a change too large
  names what is withheld rather than showing a silent subset.
- **A navigable shell, and two themes that were measured rather than eyeballed.**
  Every contrast pair is computed by a test against the surface it sits on, in
  both themes, so a token that fails is a failing build rather than something
  somebody notices later.
- **`devplane trust` counts the skills a repository ships**, not only the ones
  that pre-approve a tool. It reported *"declares no hooks, MCP servers or
  skills"* about a repository shipping ten of them — the scan was right and the
  sentence was false, which is the worse of the two for a gate whose job is
  telling you what will load into your agent before you consent.
- **The project specifies its own features before building them**, with
  [GitHub Spec Kit](https://github.com/github/spec-kit) — requirements with
  stable ids, a plan checked against a written constitution, then a task list.
  Those working files are not published, like the architecture notes; what
  reaches this repository is a test per behaviour the specification asked for.

### Changed

- **A run Devplane drove no longer reads as `working` after a restart.** Its
  connection died with the daemon that held it, so it is `interrupted` — and a
  question it was holding is now **still waiting for you**, rather than recorded
  as answered by nobody. The two cases are told apart by who ended the agent: a
  turn that ended on its own leaves a question nobody can answer any more, and a
  daemon that was stopped leaves one that is perfectly answerable.
- **The interface is a Svelte project.** One binary still, with the built assets
  embedded at compile time and nothing fetched at runtime; the served output is
  readable rather than minified. Building it needs node; `cargo build` does not.
- **The fetched third-party corpus moved from `specs/` to `reference/`**, and
  `scripts/fetch-specs.sh` with it. Spec Kit hard-codes `specs/` for the
  project's own feature specifications, and one directory cannot be both a
  gitignored build artefact and committed source of truth. `just specs` is now
  `just reference`.
- **The changelog ledger covers channels rather than rules.** `just channels`
  fails the build until every row of Claude Code's changelog touching a channel
  Devplane uses — hooks, permission modes, the settings deciding whether a hook
  is consulted — is covered or declined with a reason. A changed *rule* shape is
  the vendor's business now; a changed *hook contract* is still ours.

### Fixed

- **The live process-table test no longer depends on a bash builtin.** It gave
  its probe a recognisable name with `sh -c 'exec -a <name> sleep 30'`, which is
  a bashism: on Debian and Ubuntu `/bin/sh` is dash, `exec -a` is not a thing,
  and the probe became a zombie reading `[sh] <defunct>`. The test failed on
  Linux for a reason unrelated to what it checks. It now executes a symlink, so
  the process is really named by the path it was started from — no shell
  involved.

- **Stopping Devplane no longer records interrupted work as `completed`.** A
  driven run that was working when the daemon stopped came back reading
  `completed`, with whatever it was waiting on cleared. The new `interrupted`
  state says **Devplane stopped it** — distinct from `stopped` (you did) and
  `lost` (a process was expected and not found) — and keeps what the run was
  waiting on. The branch and worktree are untouched.

  ```console
  saas
    ⊘ 7c         vscode      –    $1.04   3m  interrupted — the daemon stopped this
  ```

  Two fixes came with it: a duplicate session-ended event that could overwrite
  the ending just recorded, and `devplane stop` returning before the endings it
  caused were written.

- **A process you may not signal is no longer reported as dead.** `kill(pid, 0)`
  fails two ways that mean opposite things — `ESRCH` is *no such process*,
  `EPERM` is *it exists and is not yours* — and Devplane compared the return
  code to zero, so every process owned by another user or by root read as gone.
  Reconciliation could mark a live run `lost` because of it.
- **A stale `daemon.json` no longer stops the daemon starting.** The guard asked
  only whether the recorded pid was alive, so after a daemon was killed and the
  pid reused, `devplane serve` refused to start — naming somebody else's process,
  with no hint that the repair was deleting a file. It now checks the pid is
  actually a Devplane process: a stale record is reported and ignored, and if the
  process table cannot be read it refuses and names the file to delete.

- **The `timer` row now names the file that set the deadline, not just its
  name.** It recorded `devplane.toml` for every project on the machine, which
  tells somebody with six repositories to go and look in six places — from a
  conditional whose two branches computed the same string. It now records the
  project's own path, so the row points at the file you would edit.
- **The `nobody` row no longer blames a clean shutdown.** Its reason read *"the
  daemon stopped while the question was waiting"*, which is the one case that
  does **not** produce it: stopping Devplane leaves the question open and
  answerable. The row is for a daemon that was killed, and it now says so.


- **A session waiting on a background job no longer reads as waiting for you.**
  Providers report such a session as `idle`, and the board turned that into
  *waiting for a prompt* — which says you are the blocker, wrongly, for as long
  as the suite runs. Devplane reads the process table and says `running a command
  it started`. It is not in the inbox, because nothing is owed, and it never ages
  off the board.
- **The board follows a session that has no hooks**, instead of freezing at the
  first thing it saw. Without `devplane connect` the session roster is the only
  channel there is, and it was read once per session and then ignored: one first
  seen idle stayed idle through every turn it ran afterwards.
- **A permission the roster reports now reaches the inbox.** Session `status` has
  three documented values — `busy`, `waiting`, `idle` — and `waiting` was being
  read as idle, so a session its own vendor reported as blocked on a person showed
  as *waiting for a prompt*. It carries the vendor's words for what it is waiting
  on, and no Allow or Deny, because that session belongs to Claude Code.
- **`devplane doctor` printed a section called `gate` twice**, from two code
  paths, opening with the same sentence. It is one section now, and where the
  running daemon was built against a different release from the binary you are
  holding, it says so instead of rendering the difference as a repeat.
- **The command `devplane inbox` printed for a question could not work.** It
  passed the option's *label* where the protocol wants its *value* — equal in
  the captured fixture and in nothing else, so it worked in the tests and failed
  against any agent that spells the two differently.
- **`devplane library --help` advertised blueprints, which this command does not
  have, and called its five verbs four.**
- **Two generated interface types silently overwrote each other.** Rust has
  modules and the wire has one namespace, so two `Kind`s and two `Finding`s each
  exported one file; whichever generated last won, and a page could have
  compiled against a type describing something else entirely.
- **A 404 from a stale daemon now says so.** A route this binary asks for is a
  route this binary has, and between releases the version number cannot tell two
  builds apart — so *404 Not Found* used to send people looking for a feature
  that was right there.

- **Two suggestion defects were shipped.** A `WebFetch` rule was suggested as
  `WebFetch(docs.rs)`, missing the `domain:` the vendor's syntax requires — so
  pasting it granted nothing. And the suggestion never appeared at all for a
  session Devplane *watches* rather than drives, because the field it was read
  from was `None` at every site that built it.
- **The suggested rule was TOML for a JSON file.** After the destination moved to
  `.claude/settings.json`, the text kept its old shape, so pasting it broke the
  file somebody was editing to be interrupted less. The page was fixed from a
  screenshot; the terminal was not, because nothing read it. Both now render the
  same line from one function, and a test parses it as the file it names.
- **The key legend described keys the row would not answer.** It listed nine
  fixed shortcuts on every screen, when `y`/`n` only answers a permission and
  `f`/`a` need a session behind the row. It is derived from the same `actions`
  array the buttons and the key handler read, so it cannot name a key that does
  nothing — and on a touch device it is not shown at all, where six rows of key
  caps sat above two items.
- **`devplane check` showed inert rules in green.** An `auto_allow` rule decides
  nothing, and a green `allow` badge beside it told somebody a protection was in
  force when it was not.
- **`explain --replay` called every undecided call an interruption.** Most were
  answered silently by the agent's own settings, so the count argued for writing
  rules that were never needed.
- **The spec task count included the specification's own quality checklist.**
  Pointed at a real Spec Kit feature, the reader counted **47** tasks where the
  task list had 31 — Spec Kit writes a `checklists/` folder whose boxes validate
  the *spec*, not the feature. A **ticked** box is also no longer read as an open
  question: the checklist line *"No [NEEDS CLARIFICATION] markers remain"* was
  being counted as one.
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
