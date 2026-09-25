+++
title = "Install"
description = "Install Devplane — installer, npx, cargo or the window — check it works, and know where it keeps its files."
weight = 1
[extra]
group = "start"
+++

Devplane is one binary: the host, the CLI, the hooks and the workbench. Nothing runs until you start
it, and nothing starts at login.

## The installer

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
```

A prebuilt binary for macOS (Apple Silicon and Intel), Linux and Windows. No Rust toolchain.

> [!IMPORTANT]
> **On macOS, use this rather than the releases page.** The binaries are not notarised, and macOS
> quarantines a file a browser downloaded, so Gatekeeper refuses it. `curl` sets no quarantine flag.

## Without installing

```sh
npx devplane ls
```

Fetches the same prebuilt binary and runs it, against the same `~/.devplane/`. The npm package carries
a provenance attestation naming the workflow run and commit that built it.

## From source

```sh
cargo install devplane
```

Rust 1.90 or later.

## The window

```sh
cargo install devplane --features app
devplane app
```

The same host as `devplane serve`, with a native window, a tray item, notifications, one global
shortcut and `devplane://` links. Closing the window keeps hosting. See
[`devplane app`](@/docs/cli.md#devplane-app).

- On Linux the build needs `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev
  librsvg2-dev`.
- Not built: a signed app bundle, installer or updater. The `devplane://` scheme is registered by a
  bundle, so on macOS those links do not open the app.
- Without the window, `devplane open` shows the same workbench in your browser.

The window's settings live in `~/.devplane/app.toml`:

```toml
# ~/.devplane/app.toml
[app]
port = 47900
```

`port = 0` (the default) picks a free one; `devplane app --port` overrides the file.

## Inside Claude Code, as a plugin

```sh
claude plugin marketplace add hupe1980/devplane
/plugin install devplane@devplane
```

The plugin adds Devplane's read-only MCP server (what is running, what needs a person, what was
decided and by whom, whether a change is verified) and the `devplane-gate` skill, which runs the
repository's gates and reports what they exited with. It installs no hooks (run `devplane connect
claude` for those) and needs the binary installed first.

## Check it works

```sh
devplane doctor
devplane ls
```

`doctor` says whether a host answers, which channels are arriving, and whether the permission gate is
installed **and answering**. `ls` lists running Claude Code sessions with no host and no
configuration. Next: [the quickstart](@/docs/quickstart.md).

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
devplane completions bash > /usr/local/etc/bash_completion.d/devplane
devplane completions fish > ~/.config/fish/completions/devplane.fish
```

Needs no host. Completing an id (a waiting question, a session, a project) asks the running host,
gives up after 150 ms, and is silent when none runs. zsh and fish show a sentence beside each id;
bash completes the id alone.

## Where it keeps things

Everything lives in `~/.devplane`:

| File | What |
|---|---|
| `devplane.db` | SQLite: events, runs, changes, transcripts, asks, reports, the decision log. Hooks write it directly; the host tails it |
| `devplane.v<n>.bak` | a database from another schema, moved aside rather than migrated. Delete it once you have what you need |
| `token` | the bearer token for the local API, mode `0600` |
| `host.json` | the running host: port, version, start time, binary and pid |
| `pending-decisions.jsonl` | decisions a hook took while the store would not open; the next host files them |
| `policy.toml` | optional machine-wide `[policy]` rules |
| `agents.toml` | optional extra agents; see [Driving agents](@/docs/agents.md#adding-an-agent) |
| `app.toml` | optional settings for the window |

`DEVPLANE_HOME` moves all of it, for a throwaway instance beside your real one. Each home has at most
one host, and a second home needs its own port:

```sh
DEVPLANE_HOME=/tmp/vp devplane serve --port 47832
```

The default port is 47831 (`--port` or `DEVPLANE_PORT`).

## Uninstall

```sh
devplane disconnect claude    # removes exactly the hooks and telemetry connect added
devplane disconnect codex
devplane disconnect copilot   # deletes the one file it wrote
devplane quit                 # stops the host and the agents it started
rm -rf ~/.devplane
```

Then remove the binary the way you installed it: `cargo uninstall devplane`, or delete it from
`~/.local/bin`. Each `disconnect` shows the diff first unless you pass `--yes`, and leaves hooks you
configured yourself alone. OpenTelemetry variables you exported for Copilot are yours to remove.
