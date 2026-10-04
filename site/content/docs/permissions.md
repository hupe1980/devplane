+++
title = "Permissions"
description = "Devplane approves nothing on its own. It refuses a tool call (deny) or puts one in front of you (ask), in Claude Code's rule syntax, per repository, and allows only what you wrote in your own grant — failing closed when the rules will not load."
weight = 22
[extra]
group = "reference"
+++

**Devplane approves nothing on its own.** It *refuses* (`deny`) or *defers to a person* (`ask`), and
allows only what you wrote yourself: a [grant](#grants-your-own-rule-per-project) in your own
`~/.devplane/policy.toml`, never in a repository.

```toml
# devplane.toml
[policy]
deny = [
  "Bash(rm *)",
  "Read(.env)",
  "mcp__*",
]
ask = [
  "Bash(git push *)",
]
```

- `~/.devplane/policy.toml` holds a machine-wide `[policy]` of the same shape.
- The same rules answer watched sessions' hooks (no host needed), driven runs' permission requests,
  and Devplane's own actions. Every verdict lands in the [decision log](@/docs/decisions.md) with the
  rule named.

Test before you commit:

```sh
devplane config explain 'pnpm test && rm -rf /'
devplane config explain --tool Read .env
devplane config explain --replay  # every observed call against the current rules
devplane config check             # does devplane.toml parse; is any rule refused
```

## The syntax is Claude Code's

A prohibition copies between `settings.json` and `devplane.toml` unchanged. `devplane doctor` prints
the baseline: rule syntax modelled on Claude Code 2.1.273. What a rule *reaches* is wider here, only
toward a prompt ([stricter](#where-devplane-is-stricter-than-claude-code)).

### Tools

| You write | It matches |
|---|---|
| `Read`, `Bash`, `Bash(*)` | every use of that tool |
| `mcp__github`, `mcp__github__*` | every tool from the `github` MCP server |
| `mcp__github__get_*` | that server's `get_` tools |
| `mcp__github__create_issue` | one tool |
| `mcp__*`, `*` | a glob over the whole tool name |

### Commands

For `Bash`, `PowerShell` and `Monitor` the specifier matches **tokens, not text**. The line splits into
simple commands on `&&`, `||`, `;`, `|`, `&` and newlines, quotes removed as the shell would. A rule
is a token prefix: program, then flags **as a set**, then operands.

| You write | Matches | Does not match |
|---|---|---|
| `Bash(npm run build)` | `npm run build` | `npm run build --watch` |
| `Bash(npm run *)` | `npm run build`, `npm run test --watch`, `npm run` | `npm install` |
| `Bash(npm run:*)` | the same — `:*` is the form the permission dialog writes | |
| `Bash(ls *)` | `ls -la`, `ls` | `lsof` |
| `Bash(rm *)` | `rm x`, `rm -rf /`, `/bin/rm -rf /`, `RM -rf /`, `r''m -rf /` | `rmdir x` |
| `Bash(rm -rf /)` | `rm -rf /`, `rm -r -f /`, `rm -fr /` | `rm -rfv /` |

- `:*` counts only at the end; in `Bash(git:* push)` the colon is literal.
- A `Bash(…)` rule also governs `Monitor`.
- `config check` warns when a rule's second token is a flag cluster; `Bash(rm *)` is robust.
- **PowerShell is literal tokens.** `PowerShell(Remove-Item *)` stops `Remove-Item x`; aliases (`rm`,
  `del`) are not resolved and go to a person under a `PowerShell(…)` rule.

### A prohibition reaches every simple command on the line

A `deny` fires when **any** simple command matches. `Bash(rm *)` refuses all of these:

| Shape | Example |
|---|---|
| a compound line | `ls && rm -rf /` |
| a subshell, backtick or `sh -c` (four levels deep) | `( cd /x && rm -rf . )`, `` echo `rm -rf /` ``, `sh -c 'rm -rf /'` |
| a wrapper and its flags | `sudo --user root rm -rf /`, `env -P /bin rm x`, `timeout 30 rm x`, `nohup`, `flock`, `busybox`, `builtin` |
| a function body or coprocess | `function f { rm -rf /; }`, `coproc rm -rf /` |
| a brace expansion | `{rm,-rf,/}` |
| a here-string given to a shell | `bash <<< 'rm -rf /'` |
| a program named by its path or in another case | `/usr/bin/rm -rf /`, `./rm x`, `RM -rf /` |
| a globbed program name | `/bin/r? -rf x`, `/bin/r[m] -rf x` — the shell picks the program, so a person is asked |
| a long flag abbreviated | `rm --rec --for /` reads as `rm -rf /` |
| a wrapper the reader does not know | `pkexec rm -rf /`, `unbuffer rm x`, `taskset 1 rm x` — a program that may run its arguments, one of which a rule prohibits, is put to a person |

Quoted text is data: `echo "a; rm -rf x"` meets no `rm` rule; `grep -e 'rm -rf' f` is a pattern.

> [!WARNING]
> A `Bash` rule reads the line; it cannot follow a program that decides at run time what to execute.
> For containment, use a sandbox.

### Paths

For `Read` and `Edit` the specifier is a **gitignore pattern** (`*` within a segment, `**` across).
`Edit(…)` covers every built-in writing tool; `Read(…)` every reading one, `Grep`, `Glob` and `LSP`
included. A `Read` deny also blocks writing that path with a file tool, except `NotebookEdit` (needs
an `Edit` deny).

| You write | Anchored at | Example |
|---|---|---|
| `//path` | the filesystem root | `Read(//tmp/**)` |
| `~/path` | your home directory | `Read(~/.ssh/**)` |
| `/path` | the file the rule is written in | `Read(/secrets/**)` in a `devplane.toml` means that repository's `secrets` |
| `path`, `./path` | the agent's working directory | `Edit(src/**)` |

A bare filename matches at any depth (`Read(.env)` is `Read(**/.env)`). Symlinks resolve at both ends:
`config/key -> ~/.ssh/id_rsa` does not get past `Read(~/.ssh/**)`.

### Path rules reach shell commands

A `Read` or `Edit` rule also governs the files a shell command **names**:

| Command | Reached by |
|---|---|
| `cat .env`, `grep TOKEN .env`, `head .env`, `less .env`, `base64 < .env` | `Read` — the operands of known readers, and input redirections |
| `cp .env /tmp/b`, `mv .env x`, `rm .env` | `Read` — never looking at a file also means never moving or replacing it |
| `cp /tmp/a .env`, `tee .env`, `touch .env`, `dd of=.env`, `ln -s x .env` | `Edit` and `Read` |
| `echo x > .env` | `Edit` only — an output redirection is `Edit` business |
| `grep -r key secrets` | a recursive command reaches everything under its directory |
| `cat .en?`, `cat conf*` | a glob operand that could expand onto the protected file |

To protect a file from a shell, write both halves (`config check` notes a missing one):

```toml
[policy]
deny = ["Read(.env)", "Edit(.env)"]
```

Not reached: a program that opens the file itself (a script, a build), a path inside an option value
(`grep -f.env x`), and `cat *` (does not expand onto dotfiles).

### Exceptions, with `!`

```toml
[policy]
deny = ["Bash(git *)", "!Bash(git status *)"]
```

- An exception applies **per simple command**: `git status && git push --force` is refused.
- It is scoped to its own file, so a project cannot cancel a machine-wide prohibition.

### Hosts and parameters

```toml
[policy]
deny = [
  "WebFetch(domain:evil.example)",
  "Agent(isolation:worktree)",
]
```

`domain:` matches the parsed host, not a substring. `Tool(param:value)` matches a top-level input field
(`*` wildcard); an omitted parameter never matches.

## Devplane's own files are protected

A built-in prohibition beneath your rules: an agent may not edit Devplane's files in `~/.devplane`
(`policy.toml`, `agents.toml`, the store, `token`, `host.json`), read the token, or edit its
repository's `devplane.toml`.

- It covers file tools and shell commands that write, truncate, move, remove or redirect into those
  files, or remove, move or chmod `~/.devplane`, also after `cd`, `pushd` or `git -C` on the line.
- The same on a directory above them (`rm -rf ~`, `rm -rf .` in the project) is asked about.

```console
$ devplane config explain 'echo x > devplane.toml'
deny  Bash
        by writes `/home/you/code/saas/devplane.toml` (built in: an agent may not change Devplane's own rules, records or credentials, nor read its token)
```

- An unknown program is refused `~/.devplane` files and asked about for `devplane.toml` (`git add
  devplane.toml` is ordinary, `sed -i` is not).
- A name the reader cannot resolve (variable, glob, untraceable `cd`) is asked about. A spelling that
  never names them (`~/.dev*/tok*`), or a script opening the file itself, is not seen.
- Reading the rest stays allowed. Edit `devplane.toml` yourself and commit it.

## Order: deny wins

1. `deny`, across **both** files
2. `ask`
3. *unresolved*: the line hides what it runs, so a person is asked
4. your own [grant](#grants-your-own-rule-per-project), where a person was about to be asked
5. *undecided*: no answer; your agent asks as usual

Specificity does not change the order; neither file cancels the other's prohibitions. The governing
`devplane.toml` is at the root of the checkout the command runs in; a worktree inherits the rules and
gates of its owning checkout, so relaxing a rule on the agent's branch changes nothing.

## Rules that cannot work are refused

`devplane config check` and `devplane change start` report these before an agent starts:

| Refused | Why |
|---|---|
| `Write(src/**)`, `Glob(src/**)`, `NotebookEdit(x)` | file permissions are checked only against `Read(…)` and `Edit(…)`; these are never consulted |
| `mcp__github(create_issue)` | an `mcp__` rule with brackets is skipped on load |
| `Bash(command:rm *)` | ignored because a compound command bypasses it; write `Bash(rm *)` |
| `Agent(researcher)` | that tool has no field for a bare specifier to match |
| `Bash(rm -rf *` | the bracket is never closed |

And one **warning**, because the list behind it is a snapshot of Claude Code's tool reference:

| Warned about | Why |
|---|---|
| `Bahs(rm *)`, `Stop Task` | not a tool name Claude Code documents, so the rule matches nothing. The name shown in a transcript is not always the rule name — `Stop Task` is written `TaskStop` |

`Cd(<path>)` rules govern a slash command, not a tool call; `config check` says so. Keep them in
`settings.json`.

## Rules that do nothing

`devplane config check` reports a valid rule an earlier rule in the same list already covers:

```console
$ devplane config check
  policy    2 deny, 0 ask
            deny   Bash(rm *)
            deny   Bash(rm -rf /tmp/build)
  unused    `Bash(rm -rf /tmp/build)` does nothing: `Bash(rm *)` above it already covers
            every call it names
```

- Decided by pattern containment; silent when unprovable; lists with a `!` exception are skipped.
- It also names allow rules in `.claude/settings.json` that grant more than they look like
  (`Bash(python:*)` approves `python -c` with any code). It writes nothing.

## Where Devplane is stricter than Claude Code

Always toward a prompt: wrappers and `-c` are looked through, program names match through the path
case-folded, flags are a set even in an exact rule, more writers meet `Edit` (`cp`, `install`,
`rsync`, `ln`, `truncate`, `dd of=`), and an unreadable line is asked about.

## When a line cannot be read, a person is asked

```console
$ devplane config explain 'rm$IFS-rf node_modules'  # deny = ["Bash(rm *)"]
ask — unreadable  Bash
        because the program is named by a variable the shell expands; `Bash(rm *)` constrains what Bash may run
```

| Shape | Example |
|---|---|
| a program name the shell builds | `rm$IFS-rf x`, `$(echo rm) -rf x`, `X=rm; $X -rf /` |
| a command substitution anywhere on the line | `echo $(ls)`, `` echo `date` `` |
| an argument the shell rewrites that could make a rule match | `rm $F /` under `Bash(rm -rf /)`; `cat $F` under `Read(.env)` |
| a variable that names something run later | `GIT_SSH_COMMAND=… git fetch`, `LD_PRELOAD=… ls` |
| a command line built from input | `xargs rm -rf`, `eval "$cmd"` |
| code in a string, file or stdin | `python -c "…"`, `node -e "…"`, `python3 script.py`, `curl … \| sh` |
| a command run somewhere else | `ssh host …`, `docker run …`, `su`, `chroot` |
| `find` that runs or deletes | `find . -delete`, `find . -exec …` |
| a wrapper flag the reader does not know | `sudo --made-up x rm …` |
| more than 8 wrappers before the command | `sudo sudo … sudo ls` |
| a line past 65,536 characters | — |

- Only where the project has rules for the tool; a rule that answers the call always wins.
- The audit outcome is `unresolved`, under the authority of the rules that made it a question.
- A heredoc fed to a non-executing program (`cat`, `tee`, `grep`, `jq`, …) is data; fed to `bash`,
  `| sudo bash` or `| python3` it is read.

## When the file is broken, every call is asked

A `devplane.toml` that will not load **fails closed**: every gated call in that repository goes to a
person, naming the file and error. A broken `~/.devplane/policy.toml` does so machine-wide. A critical
inbox item stays until it loads.

```console
$ devplane config explain 'cat .env'
ask — unreadable  Bash
        because devplane.toml is not valid: … unknown field `polcy` …; until it loads, a person decides every call
```

## Auto mode

Auto mode never prompts, so `PermissionRequest` never fires there. `devplane agent connect claude`
installs two synchronous hooks:

| Hook | Fires | Carries |
|---|---|---|
| `PreToolUse` | before every tool call, in every mode | a prohibition, or nothing |
| `PermissionRequest` | when a person was going to be asked | the same, plus the request to show you |

`PreToolUse` never answers *allow* — an allow there would skip Claude Code's own checks — and
`PermissionRequest` does only with your own grant. `devplane status --modes` shows which sessions run without prompts;
`devplane doctor` probes the gate and prints the round trip.

## Answering a watched session's permission

A permission in a session **you** started can be answered elsewhere if the project sets a hold:

```toml
[questions]
hold = true        # or "45s"
```

- A call your `ask` rules matched becomes an ask; the hook waits for the hold.
- Answer in the Inbox or with `devplane inbox answer <ask> --allow` (or `--deny`); first answer wins,
  authority **you**. Unanswered: authority **nobody**, and your agent shows its own dialog.
- A prohibition applies first and is never held.
- An agent cannot answer its own permission ([Security](@/docs/security.md#an-agent-cannot-approve-itself-mostly)).
- With the app running, the hold's notification carries **Allow** and **Deny**
  ([below](#answering-from-the-notification)).

## Answering from the notification

With `devplane up --window` running, a permission or a question that can be answered gets one
notification, whatever the window is doing:

- **A permission** carries **Allow** (this one call) and **Deny** — nothing wider. An agent's own
  *always allow* is answered in the window, where its scope is shown in full. A watched session's
  permission carries them only while its hold lasts; without a hold, the notification opens the item.
- **A question** with one field carries every option as a numbered action (`1. …`, `2. …`), or none:
  never some of them. Too many options, several questions at once, or free text where the platform
  has no reply box: the body says how many options wait, and one click opens the answer window on that
  question.
- **A click** on the notification opens the answer window on that item — not the topmost one.
- **Pressing an action** answers as you, through the same short hold as the window's answers: the
  answer window can still take it back (`z`) for a few seconds. Dismissing, closing or letting it time
  out answers nothing.
- **Answered elsewhere**, the notification is taken back where the platform allows it; one the
  platform kept answers nothing and opens the item, saying who answered it, from where and when.
- Quitting the app takes every notification back. Nothing retries a notification, raises its
  interruption level, or plays a sound of its own: Do Not Disturb and Focus hold it like any other, and
  the tray count still counts the item.

What each platform offers is in [the window](@/docs/workbench.md#the-window).

### Where an answer came from

Every answer names who gave it — **you** — and the path, in the decision log, `devplane inbox --all`
and the answer window:

| Path | Recorded when |
|---|---|
| `window` | the app's own window answered: it proves that per question with a key the app hands its window in memory, never served, logged or stored |
| `notification` | you pressed an action on the app's notification; handled inside the app, so no request can claim it |
| `cli` | anything else holding the control token: `devplane inbox answer`, a script, **a browser tab on `devplane up`** |

Neither a request's `Origin` header nor anything in its body changes the path. What this cannot
prove, and does not claim: a program driving your desktop (scripting, accessibility) can press a
notification or a window button as you, and a process running as you can read the window's key out of
the app's memory.

## Grants: your own rule, per project

A grant is an allow you wrote, for one project, in your own file — your answer given in advance:

```toml
# ~/.devplane/policy.toml
[grants]                  # when one is offered (each at least 2)
offer_after = 5
offer_again_after = 5

[projects."/home/you/code/saas"]
allow = [
  "Bash(cargo test)",
  { rule = "Bash(cargo clippy --all-targets)", until = "2026-12-01" },
]
```

- **Offered, not asked for.** After you answer the same call the same way from the window or a
  notification `offer_after` times — never a refusal among them — its permission item offers the
  exact rule, the answers it comes from, and the lines it adds to the file. Nothing is written until
  you confirm. Answers from the command line do not count: any holder of the token can send one.
- **Only after every prohibition.** Deny, ask and an unreadable line all win; the order has no
  setting. A grant answers a driven run's request with the agent's own *allow once*, and a Claude
  Code `PermissionRequest`; never `PreToolUse`, Codex or Copilot.
- **Narrower than an allow list.** It allows a line only when every command on it is granted, no
  argument is rewritten by the shell, nothing is redirected to a file and no wrapper (`sudo`, `env`)
  stands in front. A call whose input never arrived is never granted. A path grant reaches only
  files inside the project.
- **Refused by name**, by `devplane config check` and every load: a machine-wide `[policy] allow`
  (there is no global switch), a wildcard where the command goes (`Bash(*)`), a program that runs any
  code (`Bash(python *)`), a forge write (`git push`, `gh`), any tool but `Bash`, `Read` and `Edit`, a
  path outside the project, and a threshold below 2. In a repository's `devplane.toml`, any
  `[policy] allow`, `[grants]`, `[projects]` or threshold is refused: a repository cannot grant.
- **What it may write** is what the project declares — the change's worktree and the caches
  `[workspace] share` sets up. Devplane does not know what the command itself touches, and the offer
  says so.
- **Recorded and named.** Every call a grant allows is a `rule` decision naming the grant. Where the
  agent's own settings would have asked, the record, the offer, `devplane config check` and
  `devplane doctor` say the grant is *widening the agent*, by count and by rule.
- **Revoking lists what it did.** `devplane config grants` shows each grant, the calls it decided
  and, where a gate later failed on a change it decided on, how many were reversed — absent where
  nothing could be observed. `devplane config revoke 'Bash(cargo test)'` removes it and lists every
  decision it made. A grant whose `until` passed answers nothing, and the item says it lapsed.
- The built-in rule above keeps agents from reading or editing `policy.toml`. That narrows the hole
  and does not close it: a process running as you can still write the file.

## Tuning the rules

- **Asked too often:** an inbox permission carries the narrowest allow rule and the `settings.json`
  for it; `devplane config explain --replay` suggests allow rules from past calls.
- **A rule too tight:** five refusals in one run raise a `refused` item naming the rule.
