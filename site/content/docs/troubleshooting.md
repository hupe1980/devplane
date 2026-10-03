+++
title = "Troubleshooting"
description = "What a symptom means and the command that fixes it: the gate is off, the port is taken, a session is not watched, GitHub is not read, a verified change went stale, the ledger is empty after an upgrade."
weight = 4
[extra]
group = "start"
+++

Start with `devplane doctor`: host, gate (installed **and answering**), what is watched, GitHub, and
channels, on one screen.

## The gate is off

```console
$ devplane doctor
claude code
  gate      off for PreToolUse — its hook runs /Users/you/.npm/_npx/…/devplane, which no longer exists
            run `devplane agent connect claude` from an installed devplane (or pass `--bin <path>`)
```

**Cause.** Hooks run the binary by full path. If it is gone (npx cache cleared, reinstalled, moved),
the hook runs nothing and blocks nothing.

**Fix.** Install `devplane` permanently ([Install](@/docs/install.md)), then connect again:

```sh
devplane agent connect claude  # and codex, copilot, if you use them
devplane doctor                # the gate line should say "answering"
```

`agent connect` refuses a temporary binary (npx cache, App Translocation, disk image, AppImage);
`--bin <path>` names a permanent one.

| `doctor` says | Means | Do |
|---|---|---|
| `INSTALLED AND NOT ANSWERING` | the hook ran and did not answer in time | run the command it prints by hand and read the error |
| `out of date` | the installed hooks are not the ones this version writes | `devplane agent connect claude` |
| `not installed` | nothing was connected | `devplane agent connect claude` |
| `N decision(s) taken while no host was running` | hooks decided while the store was busy; they are enforced | start a host to file them |

**Codex** runs hooks only after you approve them in its dialog; `doctor` cannot tell.

## The port is taken

```console
$ devplane up --no-browser
Error: port 47831 on 127.0.0.1 is taken by another process (python3, pid 4121), so the host did not start. …
```

**Cause.** Another process holds the port. Devplane never moves on its own: `agent connect` wrote this
port and the token into your agents' settings, so a moved host would leave the token going to the
holder.

**Fix.** Stop that process, or choose another port and reconnect:

```sh
devplane up --no-browser --port 47900  # or DEVPLANE_PORT=47900
devplane agent connect claude          # run while that host is up: telemetry follows its port
```

For the window, pin the port in `~/.devplane/app.toml` (`[app] port = 47900`).

*A host is already running (pid …, port …)*: another host owns this home. `devplane down`, or use it.
A second instance needs its own `DEVPLANE_HOME` and port.

## A session is not watched

`devplane status` misses a running session, or shows it without cost and context.

| Cause | Check | Fix |
|---|---|---|
| hooks not installed | `devplane doctor` → `claude code` | `devplane agent connect claude` |
| a session that never reported | `devplane status --all` | it appears once it acts |
| the `claude` binary is not found, so the roster is not read | `devplane doctor` | set `DEVPLANE_CLAUDE_BIN` ([Install](@/docs/install.md#finding-the-claude-binary)) |
| Codex hooks not approved | Codex's own hook dialog | approve them there |
| Copilot publishes no roster | — | a Copilot session appears after its first hook |
| OpenCode is opt-in | `devplane doctor` → `channels` | `export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096` before starting the host |
| Gemini CLI | — | not watchable; drive it: `devplane change start --agent gemini` |

**Cost and context blank**: telemetry goes to the port fixed at `agent connect`. Use 47831, or run
`devplane agent connect claude` while the host is up, then restart the Claude Code session.

The workbench's **Sight** panel lists what this machine cannot see.

## GitHub is not read

The Forge view and `devplane issue list` are empty, or `doctor` says:

```console
github
  github.com   not signed in — devplane auth login
```

**Fix.** Sign in:

```sh
devplane auth login  # enter the shown code at GitHub's device page
devplane doctor      # github.com   signed in as you · repo, read:org
```

Or **Setup → GitHub → Sign in**.

| State | Means |
|---|---|
| no credential store (some Linux) | sign-in refuses rather than write a file |
| *Sign-in expired* | the token was revoked and removed; sign in again |
| *Rate limited* | says when the limit resets |
| *unreachable* | the last lists are kept, marked stale |
| no GitHub app for the host | `gh auth token \| devplane auth login --with-token`, or pipe a fine-grained token |
| **not a GitHub repository** | no remote, or a remote on another host |

`change offer` pushes nothing without `[github] pull_request = true`.

## A verified change went stale

```console
$ devplane change show c-3f9a
  gates      gates passed, stale — the working tree changed after it ran (tree 1a2b3c4 then, 5d6e7f8 now)
```

**Cause.** Any edit since the pass, committed or not, or a changed `check` in `devplane.toml`.

**Fix.** Run the gate again, or let the agent's next turn end:

```sh
devplane change verify c-3f9a
```

A gate that writes the tree (formatter, generator) never verifies; use `cargo fmt --check`,
`prettier --check`. See [Verified done](@/docs/verified-done.md).

## The ledger is empty after an upgrade

`devplane audit`, the Ledger and the change list are empty, and `~/.devplane` has a new
`devplane.v<n>.<time>.bak`.

**Cause.** A new store schema. Devplane does not migrate: it moves the old file aside (never deleting
one that holds anything; an empty one is) and starts fresh.

**Fix.** Nothing is lost. Read the old log:

```sh
sqlite3 ~/.devplane/devplane.v<n>.<time>.bak \
  "select at, authority, action, subject, outcome from decisions order by at desc limit 20"
```

Worktrees and branches are untouched; `devplane change adopt <branch>` brings one back as a change.
Delete a `.bak` once you have what you need.

## Other messages

| You see | Means | Do |
|---|---|---|
| *no host is running — start one with `devplane up` …* | the command starts, steers or stops an agent, which only a host can | `devplane up` in another terminal, or `devplane up --no-browser` |
| *nothing was done: stdin is not a terminal …* | `agent connect` or `agent disconnect` ran from a script or an agent's shell | read the diff it printed, then re-run with `--yes` |
| *not connecting: every hook runs the binary's path …* | you ran `agent connect` from npx or a disk image | install the binary; see [the gate is off](#the-gate-is-off) |
| `change offer` exits `3` | a weakened check in the change is unread | `devplane change review <id>`, then `--seen <path>` for each row |
| *refused: `CLAUDECODE` is set, so this is running inside an agent's session …* | `inbox answer --allow`, `offer`, `finish`, `archive` or `review --seen` ran inside an agent's session | run it from your own terminal |
| every call is asked, and the inbox has `config_broken` | a `devplane.toml` or `~/.devplane/policy.toml` will not load | `devplane config check` names the error |
| `deny … (built in: …)` on an edit to `devplane.toml` | agents may not change the rules that gate them | edit it yourself |
| `devplane gate install --speckit` refuses: *nothing here defines `/devplane-gate`* | `specify` is not on `PATH`, so the hand-written hook was chosen, and its skill file is not where the agent looks | put Spec Kit's `specify` on `PATH` so the extension is installed, or [copy the skill](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow); then re-run |
| macOS: *"devplane" cannot be opened* | a browser download is quarantined | use the `curl` installer |
| the page says *This binary was built without its interface* | it was built from a clone without `ui/dist` | `cd ui && npm install && npm run build`, then rebuild |
| `change start` fails before the agent speaks | the agent is not signed in, or `npx` cannot reach npm | sign the agent in in its own terminal (`claude`, `copilot login`, …); check Node and network |
