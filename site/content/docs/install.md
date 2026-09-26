+++
title = "Install"
description = "Install Devplane on your platform — installer, npm, cargo or the window — check it works, upgrade it, and remove every trace of it."
weight = 1
[extra]
group = "start"
+++

Devplane is one binary: the host, the CLI, the hooks and the workbench. Nothing runs until you start
it, and nothing starts at login.

## Platforms

| Platform | Prebuilt binary | How to install |
|---|---|---|
| macOS, Apple Silicon | yes | the installer, `npm install -g devplane`, or `cargo install` |
| Linux x86_64 and arm64 | yes, static (musl) | the installer, `npm install -g devplane`, or `cargo install` |
| Windows x86_64 | yes | `npm install -g devplane`, or the `.zip` from the [releases page](https://github.com/hupe1980/devplane/releases) |
| macOS on Intel, Windows on ARM, anything else | no | `cargo install` (Rust 1.90 or later) |

macOS and Linux are the platforms Devplane is tested on. On Windows it builds and runs, with these
limits:

- A gate that times out is stopped, but the processes it started are not.
- Gates run through `cmd`, not `sh`, so a `check` written for a POSIX shell may behave differently.
- There is no process table: *running a command it started* and lost-run detection do nothing, and
  every recorded process reads as alive.
- `devplane attach` starts `claude` and waits instead of replacing itself.
- The test suite does not run on Windows in CI.

## The installer

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
```

A prebuilt binary for macOS (Apple Silicon) and Linux, installed to `~/.cargo/bin` (`$CARGO_HOME/bin`).
No Rust toolchain is needed.

> [!IMPORTANT]
> **On macOS, use this rather than the releases page.** The binaries are not notarised, and macOS
> quarantines a file a browser downloaded, so Gatekeeper refuses it. `curl` sets no quarantine flag.

## With npm

```sh
npm install -g devplane
```

The package fetches the prebuilt binary for your platform and carries a provenance attestation
naming the workflow run and commit that built it.

`npx devplane ls` runs it without installing, which is fine for a look. **Do not connect hooks from
npx.** Every hook runs the binary by its full path, and npx's copy lives in a cache npm clears, so
`devplane connect` refuses it. Install it (any way on this page) before `connect` or the plugin.

## From source

```sh
cargo install --locked devplane
```

Rust 1.90 or later. `--locked` builds with the dependency versions the release was tested with.

## The window

```sh
cargo install --locked devplane --features app
devplane app
```

The same host as `devplane serve`, with a native window, a tray item, notifications, one global
shortcut and `devplane://` links. Closing the window keeps hosting. See
[`devplane app`](@/docs/cli.md#devplane-app).

- On Linux the build needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
  librsvg2-dev`.
- There is no signed app bundle, installer or updater, so on macOS `devplane://` links do not open
  the app.
- Without the window, `devplane open` shows the same workbench in your browser.

The window's settings live in `~/.devplane/app.toml`:

```toml
# ~/.devplane/app.toml
[app]
port = 47831
```

The window's host listens on 47831 by default, the same port `devplane serve` uses and `devplane
connect` writes into the telemetry settings. If you change it, run `connect` again so telemetry
follows. `devplane app --port` overrides the file.

## Inside Claude Code, as a plugin

In a terminal:

```sh
claude plugin marketplace add hupe1980/devplane
claude plugin install devplane@devplane
```

The plugin adds two things:

- Devplane's MCP server, with five tools: what needs a person, a change's state and whether it is
  verified, what the gate would decide about a call, what was decided and by whom, and reports
  between projects. It records each `explain` question in the decision log and writes nothing else.
- The `devplane-gate` skill, which runs the repository's gates and reports what they exited with.

It installs no hooks (run `devplane connect claude` for those) and needs the binary installed first.

## Check it works

```sh
devplane doctor
devplane ls
```

`doctor` says whether a host answers, which channels are arriving, and whether the permission gate is
installed **and answering**. `ls` lists running Claude Code sessions with no host and no
configuration. Next: [the quickstart](@/docs/quickstart.md). When something looks wrong:
[Troubleshooting](@/docs/troubleshooting.md).

## Finding the `claude` binary

Watching Claude Code and `devplane attach` need `claude`, which is often not on `PATH` (the VS Code
extension ships its own copy). Devplane looks, in order, at:

1. `$DEVPLANE_CLAUDE_BIN`
2. `PATH`
3. `~/.claude/local/claude`
4. the newest `anthropic.claude-code-*` extension in VS Code, VS Code Insiders or Cursor

Devplane runs without Claude Code: it drives any ACP agent. Which vendors can also be *watched* is on
[Watching sessions](@/docs/observe.md).

## Shell completion

```sh
devplane completions zsh  > ~/.zsh/completions/_devplane
devplane completions bash > ~/.local/share/bash-completion/completions/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

Needs no host. Completing an id (a waiting question, a session, a change) asks a running host, or
else reads the store without starting a host, and gives up after 150 ms. zsh and fish show a sentence
beside each id; bash completes the id alone.

## Where it keeps things

Everything lives in `~/.devplane`, mode `0700`; the files in it are `0600`:

| File | What |
|---|---|
| `devplane.db` (with `-wal`, `-shm`) | SQLite: events, runs, changes, transcripts, asks, reports, the decision log. Hooks write it directly; the host tails it |
| `devplane.v<n>.<time>.bak` | a store from an older schema, moved aside rather than migrated, and never deleted by Devplane (an empty one is). Delete them once you have what you need |
| `token` | the bearer token for the local API |
| `host.json`, `host.lock` | the running host (port, version, start time, binary and pid) and the lock that keeps it the only one |
| `pending-decisions.jsonl` | decisions a hook took while the store would not open; the next host files them |
| `policy.toml` | optional machine-wide `[policy]` rules |
| `agents.toml` | optional extra agents; see [Driving agents](@/docs/agents.md#adding-an-agent) |
| `app.toml` | optional settings for the window |

Outside it: `devplane connect claude` keeps a backup of your settings at
`~/.claude/settings.json.devplane-backup`, and each change's worktree is under the repository's
`.claude/worktrees/`.

`DEVPLANE_HOME` moves all of it, for a throwaway instance beside your real one. Each home has at most
one host, and a second home needs its own port:

```sh
DEVPLANE_HOME=/tmp/vp devplane serve --port 47832
```

The default port is 47831 (`--port` or `DEVPLANE_PORT`). A port another process holds is refused;
the host never moves to another one on its own.

## Upgrading

Install the new version the way you installed the old one, then:

```sh
devplane quit                # the old host, if one runs
devplane connect claude      # and codex, copilot: the hooks name the binary's path
devplane doctor
```

- **Re-run `connect` whenever the binary moves** (a different install method, a new path). Until you
  do, `doctor` reports the gate as **off** for every hook whose binary is gone, or **out of date**
  when the hooks are not the ones this version writes.
- **A schema change starts a new store.** The old file is moved aside as
  `devplane.v<n>.<time>.bak`; `devplane audit` and the Ledger start empty, and the old log stays
  readable with `sqlite3`. The [changelog](https://github.com/hupe1980/devplane/blob/main/CHANGELOG.md)
  says which releases do this.

## Uninstall

```sh
devplane disconnect claude    # undoes exactly what connect added
devplane disconnect codex
devplane disconnect copilot   # deletes the one file it wrote
devplane quit                 # stops the host and the agents it started
rm -rf ~/.devplane
```

Each `disconnect` shows the diff, asks on a terminal (pass `--yes` in a script), and leaves hooks
you configured yourself alone. Then remove the rest:

- **The binary**, the way you installed it: `rm ~/.cargo/bin/devplane` (the installer),
  `npm uninstall -g devplane`, or `cargo uninstall devplane`.
- **The plugin**: `claude plugin uninstall devplane@devplane`, then
  `claude plugin marketplace remove devplane`.
- **`~/.claude/settings.json.devplane-backup`**, once you no longer want the backup.
- **In each repository**: the worktrees under `.claude/worktrees/` (`git worktree list`), the
  `change/*` branches you do not want, the `devplane-gate` entry in `.specify/extensions.yml`, and
  any `.claude/skills/devplane-gate/` you copied.
- **Shell completion files** you wrote with `devplane completions`.
- **OpenTelemetry variables** you exported for Copilot.
