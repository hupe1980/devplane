+++
title = "Troubleshooting"
description = "What a symptom means and the command that fixes it: the gate is off, the port is taken, a session is not watched, GitHub is not read, a verified change went stale, the ledger is empty after an upgrade."
weight = 4
[extra]
group = "start"
+++

Start with `devplane doctor`. It answers most of the questions below on one screen: whether a host
answers, whether the permission gate is installed **and answering**, what is watched, whether
GitHub is read, and which channels are arriving.

## The gate is off

```console
$ devplane doctor
claude code
  gate      off for PreToolUse — its hook runs /Users/you/.npm/_npx/…/devplane, which no longer exists
            run `devplane connect claude` from an installed devplane (or pass `--bin <path>`)
```

**Cause.** Every hook runs the `devplane` binary by its full path. When that file goes (npx's cache
was cleared, you reinstalled another way, the binary moved), the hook runs nothing, and a hook that
runs nothing never blocks: the agent carries on with no prohibitions.

**Fix.** Install `devplane` somewhere permanent ([Install](@/docs/install.md)), then connect again
from that binary:

```sh
devplane connect claude            # and codex, copilot, if you use them
devplane doctor                    # the gate line should say "answering"
```

`connect` refuses to write hooks from a binary that lives somewhere temporary (npx's cache, a macOS
App Translocation path, a mounted disk image, an AppImage). `--bin <path>` names a permanent one.

Related lines on the same screen:

| `doctor` says | Means | Do |
|---|---|---|
| `INSTALLED AND NOT ANSWERING` | the hook ran and did not answer in time | run the command it prints by hand and read the error |
| `out of date` | the installed hooks are not the ones this version writes | `devplane connect claude` |
| `not installed` | nothing was connected | `devplane connect claude` |
| `N decision(s) taken while no host was running` | hooks decided while the store was busy; they are enforced | start a host to file them |

**Codex** runs a hook only after you approve it in Codex's own dialog. Until then nothing is watched
or gated there, and `doctor` cannot tell.

## The port is taken

```console
$ devplane serve
Error: port 47831 on 127.0.0.1 is taken by another process (python3, pid 4121), so the host did not start. …
```

**Cause.** Something else listens on the port. Devplane will not move to another port on its own:
`devplane connect` wrote this port and the bearer token into your agents' settings, so a host
elsewhere would leave every session sending the token to whatever holds the port.

**Fix.** Stop that process, or choose another port and point the settings at it:

```sh
devplane serve --port 47900        # or DEVPLANE_PORT=47900
devplane connect claude            # run while that host is up: telemetry follows its port
```

For the window, pin the port in `~/.devplane/app.toml` (`[app] port = 47900`).

*A host is already running (pid …, port …)* is different: another Devplane host owns this home.
`devplane quit` stops it, or use it as it is. A second, independent instance needs its own
`DEVPLANE_HOME` and port.

## A session is not watched

`devplane ls` does not show a session you know is running, or shows it without cost and context.

| Cause | Check | Fix |
|---|---|---|
| hooks not installed | `devplane doctor` → `claude code` | `devplane connect claude` |
| a Claude Code session that has never reported anything | `devplane ls --all` | nothing: it appears once it does something |
| the `claude` binary is not found, so the roster is not read | `devplane doctor` | set `DEVPLANE_CLAUDE_BIN` ([Install](@/docs/install.md#finding-the-claude-binary)) |
| Codex hooks not approved | Codex's own hook dialog | approve them there |
| Copilot publishes no roster | — | a Copilot session appears after its first hook |
| OpenCode is opt-in | `devplane doctor` → `channels` | `export DEVPLANE_OPENCODE_URL=http://127.0.0.1:4096` before starting the host |
| Gemini CLI | — | not watchable; start it with `devplane change start --agent gemini` to drive it |

**Cost and context are blank** while everything else arrives: telemetry is not reaching the host.
Telemetry goes to one port, fixed when you ran `connect`; a host on another port (a `port` or
`--port` you chose) receives none. Use the default port, 47831, or run
`devplane connect claude` again while the host is up. Restart the Claude Code session afterwards: it
reads its settings when it starts.

The workbench's **Sight** panel lists what this machine cannot see. An empty Sessions list is not
proof that nothing runs.

## GitHub is not read

The Forge view and `devplane forge issues` are empty, or `doctor` says:

```console
github
  github.com   not signed in — devplane login github
```

**Fix.** Sign in — Devplane reads GitHub with its own sign-in, and the Forge view says *not signed in
to GitHub* rather than showing an empty list:

```sh
devplane login github          # enter the shown code at GitHub's device page
devplane doctor                # github.com   signed in as you · repo, read:org
```

Or **Setup → GitHub → Sign in** in the workbench. The token is kept only in the operating system's
credential store (Keychain, Credential Manager, Secret Service); on a Linux without one, sign-in
refuses rather than write it to a file. *Sign-in expired* means GitHub no longer accepts the token —
it was revoked — and it has been removed; sign in again. *Rate limited* says when the limit resets;
*unreachable* keeps the last lists, marked stale.

A running host reads at once after a sign-in, and again every five minutes. No GitHub app
registered for the host? Sign in with a token instead: `gh auth token | devplane login github
--with-token`, or pipe a fine-grained personal access token to the same command. `doctor` also lists each project that is **not a
GitHub repository**, with the reason (no remote, or a remote on another host).

Without `[github] pull_request = true` in the repository's `devplane.toml`, `devplane change offer`
pushes nothing and prints the `git push` line and the address that opens the pull request.

## A verified change went stale

```console
$ devplane change show c-3f9a
  gates      gates passed, stale — the working tree changed after it ran (tree 1a2b3c4 then, 5d6e7f8 now)
```

**Cause.** *Verified* means the latest `check` passed against the tree exactly as it stands. Any
edit since, in the worktree, committed or not, makes it stale. So does changing the `check` commands
in `devplane.toml`.

**Fix.** Run the gate again, or let the agent's next turn end:

```sh
devplane change verify c-3f9a
```

A gate that edits the tree it checks (a formatter that writes, a code generator) can never verify:
the digest differs before and after. Make that command check instead of write (`cargo fmt --check`,
`prettier --check`). See [Verified done](@/docs/verified-done.md).

## The ledger is empty after an upgrade

`devplane audit` prints nothing, the Ledger and the change list are empty, and
`~/.devplane` holds a new `devplane.v<n>.<time>.bak`.

**Cause.** The new version uses a different store schema. Devplane does not migrate: it moves the
old file aside, keeps the three newest such files, and starts a new store.

**Fix.** Nothing is lost. Read the old log directly:

```sh
sqlite3 ~/.devplane/devplane.v10.<time>.bak \
  "select at, authority, action, subject, outcome from decisions order by at desc limit 20"
```

Worktrees and branches are untouched; `devplane change adopt <branch>` brings one back as a change.
Delete a `.bak` once you have what you need.

## Other messages

| You see | Means | Do |
|---|---|---|
| *no host is running — start one with `devplane open` …* | the command starts, steers or stops an agent, which only a host can | `devplane open` in another terminal, or `devplane serve` |
| *nothing was written: stdin is not a terminal …* | `connect` or `disconnect` ran from a script or an agent's shell | read the diff it printed, then re-run with `--yes` |
| *not connecting: every hook runs the binary's path …* | you ran `connect` from npx or a disk image | install the binary; see [the gate is off](#the-gate-is-off) |
| `change offer` exits `3` | a weakened check in the change is unread | `devplane change review <id>`, then `--seen <path>` for each row |
| *refused: `CLAUDECODE` is set, so this is running inside an agent's session …* | `answer --allow`, `offer`, `finish`, `archive` or `review --seen` ran inside an agent's session | run it from your own terminal |
| every call is asked, and the inbox has `config_broken` | a `devplane.toml` or `~/.devplane/policy.toml` will not load | `devplane check` names the error |
| `deny … (built in: …)` on an edit to `devplane.toml` | agents may not change the rules that gate them | edit it yourself |
| `devplane speckit` refuses: *nothing here defines `/devplane-gate`* | the skill file is not where the agent looks | [copy the skill](@/docs/specs.md#the-gate-inside-spec-kit-s-workflow), then re-run |
| macOS: *"devplane" cannot be opened* | a browser download is quarantined | use the `curl` installer |
| the page says *This binary was built without its interface* | it was built from a clone without `ui/dist` | `cd ui && npm install && npm run build`, then rebuild |
| `change start` fails before the agent speaks | the agent is not signed in, or `npx` cannot reach npm | sign the agent in in its own terminal (`claude`, `copilot login`, …); check Node and network |
