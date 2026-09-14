+++
title = "Install"
description = "Install Vibeplane, and check it found the agent binaries on your machine."
weight = 1
[extra]
group = "start"
+++

Vibeplane is a single static binary. It contains the daemon, the CLI, the hook shims and the web
board — there is no service to run and nothing to configure before it is useful.

## The two paths that are meant for you

```sh
# macOS and Linux
brew install hupe1980/tap/vibeplane

# any platform
curl -LsSf https://github.com/hupe1980/vibeplane/releases/latest/download/vibeplane-installer.sh | sh
```

Both fetch a prebuilt binary. Neither needs a Rust toolchain.

> [!IMPORTANT]
> **On macOS, use one of these two rather than downloading from the releases page.**
> The binaries are not notarised — the release tooling signs Windows artifacts and has no macOS
> signing support at all — and macOS applies `com.apple.quarantine` based on *what downloaded the
> file*. A browser sets it, so a `.tar.gz` you click will be refused by Gatekeeper. `curl` and
> Homebrew do not set it, so a binary that arrives either way just runs.
>
> The artifacts on the releases page are what these two commands fetch. They are not a third
> install path, and saying so is more useful than letting you find out.

## From source

```sh
cargo install vibeplane
```

Requires Rust 1.90 or later, and builds from source — a few minutes the first time. This is the
path for contributors and for platforms the release matrix does not cover, not the front door.

## Check it works

```sh
vibeplane ls
```

If Claude Code sessions are running, you will see them immediately — discovery needs no
configuration at all. If nothing appears, Vibeplane will tell you which of the two reasons it is.

## Finding the `claude` binary

Session discovery runs `claude agents --json`, and `claude` is routinely **not** on your `PATH`: the
VS&nbsp;Code extension ships its own copy and installs nothing globally. Vibeplane looks in four
places, in order:

1. `$VIBEPLANE_CLAUDE_BIN`
2. `PATH`
3. `~/.claude/local/claude`
4. the newest `anthropic.claude-code-*` extension in VS&nbsp;Code, VS&nbsp;Code Insiders or Cursor

If yours lives somewhere else:

```sh
export VIBEPLANE_CLAUDE_BIN=/path/to/claude
```

> [!NOTE]
> Vibeplane runs perfectly well with no Claude Code at all — it drives any agent that speaks the
> Agent Client Protocol. The binary is only needed for *watching* Claude Code sessions and for
> `vibeplane attach`.

## Where it keeps things

Everything lives in one directory, `~/.vibeplane`:

| File | What |
|---|---|
| `vibeplane.db` | SQLite: events, runs, work, transcripts, the decision log |
| `token` | the bearer token for the local API, mode `0600` |
| `daemon.json` | the running daemon's pid and the port it actually bound |
| `policy.toml` | optional machine-wide permission rules |
| `agents.toml` | optional extra agents, by name |

`VIBEPLANE_HOME` moves all of it, which is how you run a throwaway instance beside your real one:

```sh
VIBEPLANE_HOME=/tmp/vp vibeplane ls
```

A second instance takes another port rather than refusing to start, so it never interferes with the
daemon you are actually using.

## Uninstall

```sh
vibeplane disconnect claude    # removes the hooks and telemetry it installed
vibeplane disconnect copilot   # deletes the one file it wrote
vibeplane stop                 # stops the daemon, and the agents it started
rm -rf ~/.vibeplane
```

Then remove the binary the way you installed it — `brew uninstall vibeplane`, `cargo uninstall
vibeplane`, or deleting it from `~/.local/bin`.

`disconnect claude` removes exactly the entries `connect` added, leaving the hooks you configured
yourself alone. `disconnect copilot` deletes `~/.copilot/hooks/vibeplane.json`, which is the whole
of what it wrote — any OpenTelemetry variables you exported are yours to remove.
