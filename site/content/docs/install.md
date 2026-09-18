+++
title = "Install"
description = "Install Devplane, and check it found the agent binaries on your machine."
weight = 1
[extra]
group = "start"
+++

Devplane is a single static binary. It contains the daemon, the CLI, the hook shims and the web
board — there is no service to run and nothing to configure before it is useful.

## The path that is meant for you

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
```

Fetches a prebuilt binary for macOS (Apple Silicon), Linux and Windows. No Rust toolchain.

> [!IMPORTANT]
> **On macOS, use this rather than downloading from the releases page.**
> The binaries are not notarised — the release tooling signs Windows artifacts and has no macOS
> signing support at all — and macOS applies `com.apple.quarantine` based on *what downloaded the
> file*. A browser sets it, so a `.tar.gz` you click will be refused by Gatekeeper. `curl` does not
> set it, so a binary that arrives that way just runs.
>
> The artifacts on the releases page are what this command fetches. They are not a second install
> path, and saying so is more useful than letting you find out.

Intel Macs are not covered: the release builds `aarch64-apple-darwin` only. Build from source
there.

## From source

```sh
cargo install devplane
```

Requires Rust 1.90 or later, and builds from source — a few minutes the first time. This is the
path for contributors and for platforms the release matrix does not cover, not the front door.

## Check it works

```sh
devplane ls
```

If Claude Code sessions are running, you will see them immediately — discovery needs no
configuration at all. If nothing appears, Devplane will tell you which of the two reasons it is.

## Finding the `claude` binary

Session discovery runs `claude agents --json`, and `claude` is routinely **not** on your `PATH`: the
VS&nbsp;Code extension ships its own copy and installs nothing globally. Devplane looks in four
places, in order:

1. `$DEVPLANE_CLAUDE_BIN`
2. `PATH`
3. `~/.claude/local/claude`
4. the newest `anthropic.claude-code-*` extension in VS&nbsp;Code, VS&nbsp;Code Insiders or Cursor

If yours lives somewhere else:

```sh
export DEVPLANE_CLAUDE_BIN=/path/to/claude
```

> [!NOTE]
> Devplane runs perfectly well with no Claude Code at all — it drives any agent that speaks the
> Agent Client Protocol. The binary is only needed for *watching* Claude Code sessions and for
> `devplane attach`.

## Where it keeps things

Everything lives in one directory, `~/.devplane`:

| File | What |
|---|---|
| `devplane.db` | SQLite: events, runs, work, transcripts, the decision log |
| `token` | the bearer token for the local API, mode `0600` |
| `daemon.json` | the running daemon's pid and the port it actually bound |
| `policy.toml` | optional machine-wide permission rules |
| `agents.toml` | optional extra agents, by name |

`DEVPLANE_HOME` moves all of it, which is how you run a throwaway instance beside your real one:

```sh
DEVPLANE_HOME=/tmp/vp devplane ls
```

A second instance takes another port rather than refusing to start, so it never interferes with the
daemon you are actually using.

## Uninstall

```sh
devplane disconnect claude    # removes the hooks and telemetry it installed
devplane disconnect copilot   # deletes the one file it wrote
devplane stop                 # stops the daemon, and the agents it started
rm -rf ~/.devplane
```

Then remove the binary the way you installed it — `cargo uninstall devplane`, or deleting it from
`~/.local/bin`.

`disconnect claude` removes exactly the entries `connect` added, leaving the hooks you configured
yourself alone. `disconnect copilot` deletes `~/.copilot/hooks/devplane.json`, which is the whole
of what it wrote — any OpenTelemetry variables you exported are yours to remove.
