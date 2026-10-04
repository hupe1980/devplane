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

Tested on macOS and Linux. On Windows:

- A timed-out gate's child processes are not stopped.
- Gates run through `cmd`, not `sh`.
- No process table: *running a command it started* and lost-run detection do nothing.
- `devplane run attach` starts `claude` and waits instead of replacing itself.
- CI does not run the test suite on Windows.

## The installer

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
```

Installs to `$CARGO_HOME/bin` (`~/.cargo/bin`); no Rust needed.

> [!IMPORTANT]
> **On macOS, use this, not the releases page.** The binaries are not notarised, and Gatekeeper refuses
> a browser download; `curl` sets no quarantine flag.

## With npm

```sh
npm install -g devplane
```

Fetches the prebuilt binary; the package carries a provenance attestation naming the workflow run and
commit. `npx devplane status` works for a look, but `devplane agent connect` refuses npx's cached
binary: install it before connecting hooks or adding the plugin.

## From source

```sh
cargo install --locked devplane
```

Rust 1.94+. `--locked` uses the release's tested dependency versions.

## What a release contains

Each `v*` tag publishes, on the [releases page](https://github.com/hupe1980/devplane/releases):

| File | What |
|---|---|
| `devplane-installer.sh` | the installer above |
| `devplane-<target>.tar.xz` (Windows: `.zip`) | the command line, for `aarch64-apple-darwin`, `aarch64-unknown-linux-musl`, `x86_64-unknown-linux-musl`, `x86_64-pc-windows-msvc` |
| `Devplane_<version>_…` | the desktop app's packages ([below](#the-window)) |
| `<file>.sha256` | the SHA-256 of each archive and package |

The [CI action](@/docs/ci.md) installs `devplane-<target>.tar.xz` and checks it against its `.sha256`
before unpacking.

## The window

The desktop app is the same program with a window: it hosts while it is open, and its executable
also runs every command. Started from Finder, the Start menu or a launcher, it opens its window; typed
in a terminal, it is the command line (`devplane` alone runs `status`). The command line installed
above links no graphical toolkit; each copy's `devplane doctor` says which it is.

| Platform | Package | Install |
|---|---|---|
| macOS, Apple Silicon | `Devplane_<version>_aarch64.dmg` | open it, drag Devplane to Applications, open it from there |
| Windows x86_64 | `Devplane_<version>_x64-setup.exe` | run it; it installs for you alone, without administrator rights |
| Linux x86_64, Debian and Ubuntu | `Devplane_<version>_amd64.deb` | open it in your software installer |
| Linux x86_64, other distributions | `Devplane_<version>_amd64.AppImage` | mark it executable and open it |

- **Linux needs WebKitGTK** (`libwebkit2gtk-4.1-0`). The `.deb` declares it; the AppImage carries its own.
- **On an Intel Mac**, build the app from source ([below](#the-window-from-source)).
- **Move Devplane to Applications before connecting an agent.** Opened from the disk image or from
  Downloads, macOS runs a temporary copy, and `agent connect` refuses a path that disappears.
- **An AppImage connects by its own file**: move or rename it and `devplane doctor` reports the gate
  off until you connect again. It registers `devplane://` links on first run; the other packages
  register them when installed.

### Signed and unsigned packages

A package built without the signing credentials, or without the release key in `tauri.conf.json`
(`plugins.updater.pubkey`, empty in this repository), is unsigned:

- its file name says so before the extension: `Devplane_<version>_aarch64-unsigned.dmg`;
- the release notes list it as unsigned, and your system warns before opening it;
- the app never checks for updates, and the release never replaces a signed one as the default download.

A signed release's macOS app and disk image are notarised, its Windows installer names its publisher,
and its update packages and `.deb` carry a `.sig` from the release key. The release notes list every package with its
platform, whether it is signed, and its SHA-256.

### Update checks

Off until you answer. Setup asks, naming the one address it fetches
(`https://github.com/hupe1980/devplane/releases/latest/download/latest.json`) and that it sends
nothing about you, this machine or your projects. Your answer is one line in your own file, shown
before it is written:

```toml
# ~/.devplane/app.toml
[app]
updates = "on"   # or "off"; absent means not asked, and nothing is checked
```

With checks on, the app checks once at start and downloads nothing until you choose **Install and
restart**. The update is verified against the release key before anything is replaced, and the restart
says first which agents it would stop. A `.deb` install, or an app run from its disk image, is told
where to get the new version instead. The command line never checks.

### Removing the app

- **macOS**: drag Devplane from Applications to the Bin.
- **Windows**: *Settings → Apps → Installed apps → Devplane → Uninstall*.
- **Linux**: `sudo apt remove devplane`, or delete the AppImage.

Each leaves `~/.devplane` in place. Run `agent disconnect` first ([Uninstall](#uninstall)), or another
install's `devplane doctor` reports the hooks as **gate off**.

### The window from source

```sh
cargo install --locked devplane --features app
devplane up --window
```

A Linux build needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
A source build is not a bundle: on macOS `devplane://` links do not open it. See
[`devplane up --window`](@/docs/cli.md#devplane-up).

The window's port is `[app] port` (default 47831); `--port` overrides it. Run `agent connect` again
after changing it, so telemetry follows:

```toml
# ~/.devplane/app.toml
[app]
port = 47831
```

### Building signed packages

`.github/workflows/release.yml` builds every package on a `v*` tag, after CI passed on that commit. It
signs with these secrets of the repository's `release` environment, which a pull request cannot read.
A platform missing any it needs is built unsigned, and the job summary names what was missing.

| Secret | Platform | What it is |
|---|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | macOS | the Developer ID Application certificate as a base64 `.p12`, and its password |
| `APPLE_SIGNING_IDENTITY` | macOS | `Developer ID Application: <name> (<team id>)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS | the Apple ID notarisation runs as, an app-specific password for it, the team id |
| `WINDOWS_SIGN_COMMAND` | Windows | a command that signs the file `%1` in place |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | Windows, optional | passed to a cloud signing tool the command runs |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | every platform | the release key that signs updates (`cargo tauri signer generate`) |

Its public half goes in `plugins.updater.pubkey` in `tauri.conf.json`. Signed packages are checked
with `codesign`, `spctl`, `stapler` and `signtool` before upload; a failure fails the release.

## Inside Claude Code, as a plugin

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

Watching Claude Code and `devplane run attach` need `claude`. Looked up in order:

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
| `host.json`, `host.lock` | the running host (port, version, start time, binary, pid) and the lock that keeps it the only one |
| `pending-decisions.jsonl` | decisions a hook took while the store would not open; the next host files them |
| `policy.toml` | optional machine-wide `[policy]` rules and your [grants](@/docs/permissions.md#grants-your-own-rule-per-project) |
| `agents.toml` | optional extra agents ([Driving agents](@/docs/agents.md#adding-an-agent)) |
| `app.toml` | optional settings for the window and GitHub |
| `trusted.toml` | the repositories you trusted |

Outside it: `~/.claude/settings.json.devplane-backup` (from `agent connect claude`) and each change's
worktree under the repository's `.claude/worktrees/`.

`DEVPLANE_HOME` moves all of it. One host per home; a second home needs its own port:

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
  ([reading the old one](@/docs/troubleshooting.md#the-ledger-is-empty-after-an-upgrade)); the
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
  the app as in [Removing the app](#removing-the-app).
- **The plugin**: `claude plugin uninstall devplane@devplane`, `claude plugin marketplace remove devplane`.
- `~/.claude/settings.json.devplane-backup`.
- **Per repository**: `.claude/worktrees/` (`git worktree list`), unwanted `change/*` branches, the
  `devplane-gate` entry in `.specify/extensions.yml`, `.claude/skills/devplane-gate/`.
- Completion files, and OpenTelemetry variables exported for Copilot.
