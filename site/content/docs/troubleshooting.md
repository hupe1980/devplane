+++
title = "Troubleshooting"
description = "What a symptom means and the command that fixes it: the gate is off, the port is taken, a session is not watched, GitHub is not read, a verified change went stale, the ledger is empty after an upgrade."
weight = 4
[extra]
group = "start"
+++

Start with `devplane doctor`: the host, the gate (installed **and answering**), what is watched,
GitHub, and notifications, on one screen.

## The gate is off

```console
$ devplane doctor
claude code
  gate off Claude Code — its hooks run /Users/you/.npm/_npx/…/devplane, which no longer exists
            connect again from where Devplane is now: devplane agent connect claude
```

**Cause.** Hooks run the binary by full path. If it is gone (npx cache cleared, reinstalled, moved),
the hook runs nothing and blocks nothing.

**Fix.** Install `devplane` permanently ([Install](@/docs/install.md)), then connect again:

```sh
devplane agent connect claude  # and codex, copilot, if you use them
devplane doctor
```

`agent connect` refuses a temporary binary (npx cache, App Translocation, a disk image, an AppImage's
mount); `--bin <path>` names a permanent one.

| `doctor` says | Means | Do |
|---|---|---|
| `INSTALLED AND NOT ANSWERING` | the hook ran and did not answer in time | run the command it prints by hand and read the error |
| `out of date` | the installed hooks are not the ones this version writes | `devplane agent connect claude` |
| `not installed` | nothing was connected | `devplane agent connect claude` |
| `N decision(s) taken while no host was running` | hooks decided while the store was busy; they were enforced | start a host to file them |

**Codex** runs hooks only after you approve them in its dialog; `doctor` cannot tell.

## The port is taken

```console
$ devplane up --no-browser
Error: port 47831 on 127.0.0.1 is taken by another process (python3, pid 4121), so the host did not start. …
```

**Cause.** Another process holds the port. Devplane never moves: `agent connect` wrote this port and
the token into your agents' settings.

**Fix.** Stop that process, or choose another port and reconnect while that host is up:

```sh
devplane up --no-browser --port 47900  # or DEVPLANE_PORT=47900
devplane agent connect claude          # telemetry follows the running host's port
```

For the window, set `[app] port = 47900` in `~/.devplane/app.toml`.

*a host is already running (pid …, port …)*: another host owns this home. Use it, or `devplane down`.
A second instance needs its own `DEVPLANE_HOME` and port.

## A session is not watched

`devplane status` misses a running session, or shows it without cost and context.

| Cause | Check | Fix |
|---|---|---|
| hooks not installed | `devplane doctor` → `claude code` | `devplane agent connect claude` |
| a session that never reported | `devplane status --all` | it appears once it acts |
| `claude` not found, so the roster is not read | `devplane doctor` | set `DEVPLANE_CLAUDE_BIN` ([Install](@/docs/install.md#finding-the-claude-binary)) |
| Codex hooks not approved | Codex's own hook dialog | approve them there |
| Copilot publishes no roster | — | a Copilot session appears after its first hook |
| OpenCode is opt-in | `devplane doctor` → `watched` | `export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096` before starting the host |
| Gemini CLI | — | not watchable; drive it: `devplane change start --agent gemini` |

**Cost and context blank**: telemetry goes to the port fixed at `agent connect`. Use 47831, or run
`devplane agent connect claude` while the host is up, then restart the Claude Code session.

## GitHub is not read

The Forge view and `devplane issue list` are empty, or `doctor` says:

```console
github
  github.com   not signed in — devplane auth login
```

**Fix.** `devplane auth login` (enter the code it shows at GitHub's device page), or **Setup → GitHub →
Sign in**. `doctor` then says *signed in as you*.

| State | Means |
|---|---|
| no credential store (some Linux) | sign-in refuses rather than write the token to a file |
| *sign-in expired* | GitHub no longer accepts the token, so it was removed; sign in again |
| *rate limited* | says when the limit resets |
| *unreachable* | the last lists are kept, marked stale |
| no GitHub app for the host | `gh auth token \| devplane auth login --with-token`, or pipe a fine-grained token |
| **not a GitHub repository** | no remote, or a remote on another host |

`change offer` pushes nothing without `[github] pull_request = true`.

## A verified change went stale

```console
$ devplane change show c-019a
  gates      gates passed, stale — the working tree changed after it ran (tree 1a2b3c4 then, 5d6e7f8 now)
```

**Cause.** An edit since the pass, committed or not, or a changed `check` in `devplane.toml`.

**Fix.** `devplane change verify <id>`, or let the agent's next turn end. A gate that writes the tree
(a formatter, a generator) never verifies; use `cargo fmt --check`, `prettier --check`.

## The ledger is empty after an upgrade

`devplane audit`, the Ledger and the change list are empty, and `~/.devplane` has a new
`devplane.v<n>.<time>.bak`.

**Cause.** A new store schema. Devplane does not migrate: it moves the old file aside (an empty one is
deleted) and starts fresh.

**Fix.** Nothing is lost. Read the old log:

```sh
sqlite3 ~/.devplane/devplane.v<n>.<time>.bak \
  "select at, authority, action, subject, outcome from decisions order by at desc limit 20"
```

Worktrees and branches are untouched; `devplane change adopt <branch>` brings one back as a change.
Delete a `.bak` once you have what you need.

## Notifications do not arrive, or carry no buttons

`devplane doctor` prints a `notices` line from the running app:

| It says | Do |
|---|---|
| *actions and reply* | nothing; a notification held by Do Not Disturb or Focus appears when it ends |
| *not permitted* | macOS: System Settings › Notifications › Devplane, allow notifications |
| *plain, no actions* | a build run from source or from `target/`, or a service that reports no actions: install the app |
| *no notification service answered* (Linux) | start a notification daemon; the tray count and the window still show every item |
| *plain — the app is not running* | `devplane up --window` for answerable notifications |

`DEVPLANE_NOTIFY=0` turns them off.

## Other messages

| You see | Means | Do |
|---|---|---|
| *no host is running — start one with `devplane up` …* | the command starts, steers or stops an agent, which only a host can | `devplane up` in another terminal |
| *nothing was done: stdin is not a terminal …* | `agent connect` or `disconnect` ran from a script or an agent's shell | read the diff it printed, then re-run with `--yes` |
| *not connecting: every hook runs the binary's path …* | you ran `agent connect` from npx or a disk image | install the binary; see [the gate is off](#the-gate-is-off) |
| `change offer` exits `3` | a weakened check in the change is unread | `devplane change review <id>`, then `--seen <path>` for each row |
| *refused: `CLAUDECODE` is set, so this is running inside an agent's session …* | a command an agent may not run ran inside an agent's session | run it from your own terminal |
| every call is asked, and the inbox has `config_broken` | a `devplane.toml` or `~/.devplane/policy.toml` will not load | `devplane config check` names the error |
| `deny … (built in: …)` on an edit to `devplane.toml` | agents may not change the rules that gate them | edit it yourself |
| `gate install --speckit` refuses: *nothing here defines `/devplane-gate`* | `specify` is not on `PATH`, so the hand-written hook was chosen, and its skill is missing | put `specify` on `PATH`, or [copy the skill](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow); then re-run |
| *trusting a repository is a person's decision, and this request cannot show it came from the Devplane window* | you pressed **Trust** in a browser tab, which cannot prove it is you | `devplane project trust <path>` in your own terminal |
| macOS: *"devplane" cannot be opened* | a browser download is quarantined | use the `curl` installer |
| the page says *built without its interface* | built from a clone without `ui/dist` | `cd ui && npm install && npm run build`, then rebuild |
| `change start` fails before the agent speaks | the agent is not signed in, or `npx` cannot reach npm | sign the agent in in its own terminal (`claude`, `copilot login`, …); check Node and network |
