+++
title = "Install"
description = "Install Devplane on your platform — the desktop app, the installer, npm or cargo — check it works, upgrade it, and remove every trace of it."
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

The desktop app is the same program with a window: it hosts while it is open, and its executable
also runs every command above. Each release carries it for three platforms, beside the command
line's archives on the [releases page](https://github.com/hupe1980/devplane/releases):

| Platform | Package | Install |
|---|---|---|
| macOS, Apple Silicon | `Devplane_<version>_aarch64.dmg` | open it, drag Devplane to Applications, open it from there |
| Windows x86_64 | `Devplane_<version>_x64-setup.exe` | run it; it installs for you alone, without administrator rights |
| Linux x86_64 (Ubuntu 24.04 and Debian-based) | `Devplane_<version>_amd64.deb` | open it in your software installer |
| Linux x86_64 (other distributions) | `Devplane_<version>_amd64.AppImage` | mark it executable and open it |

- **Linux needs the WebKitGTK runtime** (`libwebkit2gtk-4.1-0`). The `.deb` declares it, so your
  package manager installs it; the AppImage carries its own.
- **The macOS app is for Apple Silicon Macs.** On an Intel Mac, build it from source (below).
- **Move Devplane to Applications before connecting an agent.** Opened from the disk image or from
  Downloads, macOS runs a temporary copy, and `agent connect` refuses to write a path that
  disappears when it quits.
- **An AppImage connects by its own file**, so moving or renaming it later turns the gate off;
  `devplane doctor` says so, and connecting again from the new place fixes it. It registers
  `devplane://` links the first time it runs; the other packages register them when installed.
- **Checksums**: each package has a `.sha256` beside it, and the release notes list every package
  with its checksum and whether it is signed.
- **Signed or not**: a signed release's macOS app is notarised and its Windows installer names its
  publisher. A package built without the signing credentials says `-unsigned` in its file name and
  the release notes, and your system warns before opening it. An unsigned release never replaces a
  signed one as the default download, and is never offered as an update.

Started from Finder, the Start menu or a launcher, the app opens its window. Typed in a terminal,
the same executable is the command line (`devplane` alone runs `status`). The command line installed
above stays separate: it links no graphical toolkit, and each copy's `devplane doctor` says which
one it is.

### Update checks

Off until you answer. Setup asks whether Devplane may check, and says exactly what it fetches:
`https://github.com/hupe1980/devplane/releases/latest/download/latest.json`, with nothing about
you, this machine or your projects. Your answer is one line in your own `~/.devplane/app.toml`,
shown before it is written:

```toml
# ~/.devplane/app.toml
[app]
updates = "on"   # or "off"; absent means not asked, and nothing is checked
```

With checks on, the app checks once when it starts, never on a timer, and downloads nothing until
you choose **Install and restart**. The update is verified against the release key the app was built
with before anything is replaced, and a restart says first which agents it would stop. A `.deb`
installation, or an app run from its disk image, is told where to get the new version instead of
replacing itself. The command line never checks for updates.

### Removing the app

- **macOS**: drag Devplane from Applications to the Bin.
- **Windows**: *Settings → Apps → Installed apps → Devplane → Uninstall*.
- **Linux**: remove the `devplane` package (`sudo apt remove devplane`), or delete the AppImage.

Each leaves `~/.devplane` (your history and settings) in place: the Windows uninstaller and the
`.deb` say so as they finish, and on macOS and with an AppImage removal is deleting one file. Run
`agent disconnect` first (see [Uninstall](#uninstall)), or `devplane doctor` from another install
reports the hooks as **gate off**.

### From source

```sh
cargo install --locked devplane --features app
devplane up --window
```

See [`devplane up --window`](@/docs/cli.md#devplane-up-window). A Linux build needs
`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`. A source build is
not a bundle: on macOS `devplane://` links do not open it.

Settings live in `~/.devplane/app.toml`:

```toml
# ~/.devplane/app.toml
[app]
port = 47831
```

Change the port and run `agent connect` again so telemetry follows. `devplane up --window --port`
overrides the file.

### Building signed packages

The release workflow (`.github/workflows/release.yml`) builds every package on each `v*` tag. It
signs with these secrets of the repository's `release` environment, which a pull request cannot
read; a platform whose secrets are absent is built and labelled `-unsigned` instead, with the
missing names in the job summary.

| Secret | What it is |
|---|---|
| `APPLE_CERTIFICATE` | the Developer ID Application certificate, as a base64 `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12`'s password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: <name> (<team id>)` |
| `APPLE_ID` | the Apple ID notarisation runs as |
| `APPLE_PASSWORD` | an app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | the ten-character team id |
| `WINDOWS_SIGN_COMMAND` | a command that signs the file `%1` in place with the code-signing certificate |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | optional: passed to a cloud signing tool `WINDOWS_SIGN_COMMAND` runs |
| `TAURI_SIGNING_PRIVATE_KEY` | the release key that signs updates (`cargo tauri signer generate`) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | its password |

The release key's public half is committed as `plugins.updater.pubkey` in `tauri.conf.json`; a build
without it never checks for updates. Every signed package is verified with the platform's own tools
(`codesign`, `spctl`, `stapler`, `signtool`) before it is uploaded, and a failure fails the release.

## Inside Claude Code, as a plugin

In a terminal:

```sh
claude plugin marketplace add hupe1980/devplane
claude plugin install devplane@devplane
```

- Adds Devplane's MCP server ([`devplane mcp`](@/docs/cli.md#devplane-mcp)) and the
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

- **The binary**: `rm ~/.cargo/bin/devplane`, `npm uninstall -g devplane`, or `cargo uninstall devplane`;
  the desktop app as in [Removing the app](#removing-the-app).
- **The plugin**: `claude plugin uninstall devplane@devplane`, `claude plugin marketplace remove devplane`.
- `~/.claude/settings.json.devplane-backup`.
- **Per repository**: `.claude/worktrees/` (`git worktree list`), unwanted `change/*` branches, the
  `devplane-gate` entry in `.specify/extensions.yml`, `.claude/skills/devplane-gate/`.
- Completion files, and OpenTelemetry variables exported for Copilot.
