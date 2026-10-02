+++
title = "Install"
description = "Install Devplane on your platform — installer, npm, cargo or the window — check it works, upgrade it, and remove every trace of it."
weight = 1
[extra]
group = "start"
+++

One binary: host, CLI, hooks and workbench. Nothing runs until you start it; nothing starts at login.

## Platforms

| Platform | Prebuilt binary | How to install |
|---|---|---|
| macOS, Apple Silicon | yes | the installer, `npm install -g devplane`, or `cargo install` |
| Linux x86_64 and arm64 | yes, static (musl) | the installer, `npm install -g devplane`, or `cargo install` |
| Windows x86_64 | yes | `npm install -g devplane`, or the `.zip` from the [releases page](https://github.com/hupe1980/devplane/releases) |
| macOS on Intel, Windows on ARM, anything else | no | `cargo install` (Rust 1.94 or later) |

Tested on macOS and Linux. Windows limits:

- A timed-out gate's child processes are not stopped.
- Gates run through `cmd`, not `sh`.
- No process table: *running a command it started* and lost-run detection do nothing.
- `devplane run attach` starts `claude` and waits instead of replacing itself.
- The test suite does not run on Windows in CI.

## The installer

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
```

Installs to `~/.cargo/bin` (`$CARGO_HOME/bin`); no Rust needed.

> [!IMPORTANT]
> **On macOS, use this, not the releases page.** The binaries are not notarised, and Gatekeeper refuses
> a browser download; `curl` sets no quarantine flag.

## With npm

```sh
npm install -g devplane
```

Fetches the prebuilt binary, with a provenance attestation naming the workflow run and commit.
`npx devplane status` works for a look, but `devplane agent connect` refuses npx's cached binary:
install it before connecting hooks or adding the plugin.

## From source

```sh
cargo install --locked devplane
```

Rust 1.94+. `--locked` uses the release's tested dependency versions.

## The window

```sh
cargo install --locked devplane --features app
devplane up --window
```

See [`devplane up --window`](@/docs/cli.md#devplane-up-window).

- Linux build needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
- No signed app bundle, installer or updater; on macOS `devplane://` links do not open the app.

Settings live in `~/.devplane/app.toml`:

```toml
# ~/.devplane/app.toml
[app]
port = 47831
```

Change the port and run `agent connect` again so telemetry follows. `devplane up --window --port`
overrides the file.

## Inside Claude Code, as a plugin

In a terminal:

```sh
claude plugin marketplace add hupe1980/devplane
claude plugin install devplane@devplane
```

- Adds Devplane's read-only MCP server ([`devplane mcp`](@/docs/cli.md#devplane-mcp)) and the
  `devplane-gate` skill, which runs the gates and reports the exit.
- Installs no hooks (use `devplane agent connect claude`); needs the binary installed first.

## Check it works

```sh
devplane doctor
devplane status
```

`status` lists running Claude Code sessions with no host or configuration. Next:
[the quickstart](@/docs/quickstart.md), or [Troubleshooting](@/docs/troubleshooting.md).

## Finding the `claude` binary

Watching Claude Code and `devplane run attach` need `claude` (the VS Code extension ships its own).
Looked up in order:

1. `$DEVPLANE_CLAUDE_BIN`
2. `PATH`
3. `~/.claude/local/claude`
4. the newest `anthropic.claude-code-*` extension in VS Code, VS Code Insiders or Cursor

Devplane runs without Claude Code; see [Watching sessions](@/docs/observe.md) for other vendors.

## Shell completion

```sh
devplane completions zsh  > ~/.zsh/completions/_devplane
devplane completions bash > ~/.local/share/bash-completion/completions/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

Completing an id asks a running host or reads the store (never starting a host), giving up after
150 ms. zsh and fish describe each id; bash completes the id alone.

## Where it keeps things

`~/.devplane` (mode `0700`, files `0600`):

| File | What |
|---|---|
| `devplane.db` (with `-wal`, `-shm`) | SQLite: events, runs, changes, transcripts, asks, the decision log. Hooks write it directly; the host tails it |
| `devplane.v<n>.<time>.bak` | an older-schema store moved aside ([after an upgrade](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)) |
| `token` | the bearer token for the local API |
| `telemetry-token` | a token that opens only the telemetry routes, written into agents' settings |
| `host.json`, `host.lock` | the running host (port, version, start time, binary and pid) and the lock that keeps it the only one |
| `pending-decisions.jsonl` | decisions a hook took while the store would not open; the next host files them |
| `policy.toml` | optional machine-wide `[policy]` rules |
| `agents.toml` | optional extra agents; see [Driving agents](@/docs/agents.md#adding-an-agent) |
| `app.toml` | optional settings for the window |

Outside it: `~/.claude/settings.json.devplane-backup` (from `agent connect claude`) and each change's
worktree under the repository's `.claude/worktrees/`.

`DEVPLANE_HOME` moves all of it; one host per home, and a second home needs its own port:

```sh
DEVPLANE_HOME=/tmp/vp devplane up --no-browser --port 47832
```

Default port 47831 (`--port` or `DEVPLANE_PORT`); a taken port is refused.

## Upgrading

Install the new version the way you installed the old one, then:

```sh
devplane down                  # the old host, if one runs
devplane agent connect claude  # and codex, copilot: the hooks name the binary's path
devplane doctor
```

- **Re-run `agent connect` whenever the binary moves**; until then `doctor` reports **gate off** or
  **out of date**.
- **A schema change starts a new store**
  ([the old log](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)). The
  [changelog](https://github.com/hupe1980/devplane/blob/main/CHANGELOG.md) says which releases do this.

## Uninstall

```sh
devplane agent disconnect claude   # undoes exactly what connect added
devplane agent disconnect codex
devplane agent disconnect copilot  # deletes the one file it wrote
devplane down                      # stops the host and the agents it started
rm -rf ~/.devplane
```

`agent disconnect` shows the diff and asks (`--yes` in a script). Then remove:

- **The binary**: `rm ~/.cargo/bin/devplane`, `npm uninstall -g devplane`, or `cargo uninstall devplane`.
- **The plugin**: `claude plugin uninstall devplane@devplane`, `claude plugin marketplace remove devplane`.
- `~/.claude/settings.json.devplane-backup`.
- **Per repository**: `.claude/worktrees/` (`git worktree list`), unwanted `change/*` branches, the
  `devplane-gate` entry in `.specify/extensions.yml`, `.claude/skills/devplane-gate/`.
- Completion files, and OpenTelemetry variables exported for Copilot.
