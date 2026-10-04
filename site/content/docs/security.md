+++
title = "Security"
description = "The trust model: what stays on your machine, what guards the local API, where the permission gate fails closed, and what Devplane does not defend against."
weight = 24
[extra]
group = "reference"
+++

Devplane refuses and defers tool calls, runs commands your repository committed, and pushes branches.
This is its trust model, including what it does **not** defend against.

## Nothing leaves the machine

No account, cloud relay, analytics or crash reporting. Network traffic is only:

- the GitHub host you signed in to (`github.com` unless `[github] host` in `~/.devplane/app.toml`);
- npm, when `npx` first fetches a built-in agent's pinned adapter;
- the desktop app's update check, once per start, only if you turned it on
  ([Update checks](@/docs/install.md#update-checks));
- the agents themselves, talking to their providers.

## Loopback plus a bearer token

The host binds `127.0.0.1`, reachable by any process running as you. Every `/api` and `/devplane`
request needs the bearer token from `~/.devplane/token` in the `Authorization` header, never in a URL.
`~/.devplane` is `0700`, its files `0600`.

- `/healthz` (the version) and the workbench's static files are open and carry no data.
- **A taken port is a refusal**: agents' settings send the token to one port, so the host never moves
  ([Troubleshooting](@/docs/troubleshooting.md#the-port-is-taken)).
- **Telemetry has its own token.** Claude Code hands its settings' `env` block to every tool call, so
  `agent connect` writes a **telemetry-only token** (`~/.devplane/telemetry-token`, derived one way
  from the real one) that opens the telemetry routes and nothing else. `agent disconnect` removes it.
- The workbench is served with CSP `'self'`, `no-referrer` and `nosniff`.
- `devplane up` passes the token once in the URL; the page strips it and sends a header from then on.

To point Claude Code at Devplane by hand:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT="http://127.0.0.1:47831/devplane/otel"
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer $(cat ~/.devplane/telemetry-token)"
```

## The trust gate

```sh
devplane project trust --dry-run ~/src/someone-elses-repo  # show, trust nothing
devplane project trust .                                   # show, then ask
devplane project trust --yes .                             # a directory you wrote
```

A headless agent runs **the repository's own hooks and MCP servers** without a dialog, so `change
start` refuses an untrusted repository. `project trust` first shows every `command` hook and its
event, every MCP server (unpinned ones named), every skill that pre-approves the shell, and every allow
rule in the repository's agent settings that grants more than it looks like. A worktree inherits its
checkout's trust. Untrusted projects are still observed.

## Where the permission gate fails closed

Rules: [Permissions](@/docs/permissions.md).

- **Evaluation cannot fail open**: synchronous and in-process in `devplane hook`; no service to be
  unreachable, no host needed.
- **A broken policy file fails closed**: a `devplane.toml` or `~/.devplane/policy.toml` that will not
  load sends every gated call to a person and raises a critical item.
- **Auto mode is covered**: prohibitions ride `PreToolUse`, which fires in every mode.
- **The matcher is linear**; a line past 65,536 characters is asked about.
- **Devplane allows only through your grants.** Otherwise it refuses or defers. A
  [grant](@/docs/permissions.md#grants-your-own-rule-per-project) is a rule in your own
  `policy.toml`, consulted after every prohibition; a repository cannot declare one.
- **No route or button edits `[policy]`.** Devplane writes a grant only when you confirm its offer in
  the desktop window; the token alone is refused.
- **On GitHub Copilot** the HTTP hook fails open, so the gate is a `command` hook there too.

**No hook can enforce its own presence**: a timed-out or uninstalled hook does not block. `devplane
doctor` probes the gate, and a running host does so on a timer, raising a critical item when it stops
answering. Codex skips hooks you have not approved in its dialog.

## An agent cannot approve itself — mostly

Refused inside an agent session: `devplane inbox answer --allow` (and `--option` without `--field`),
`change offer`, `change finish`, `change archive` and `change review --seen`. `--deny` still works. A
session is detected by any of `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`,
`DEVPLANE_RUN` (set on every agent Devplane starts), or a Codex or Copilot session variable
(`CODEX_SANDBOX`, `CODEX_THREAD_ID`, `COPILOT_CLI`, …).

A built-in prohibition beneath your rules stops an agent editing Devplane's files, reading its token,
or editing its repository's `devplane.toml`, by file tool or shell
([details](@/docs/permissions.md#devplane-s-own-files-are-protected)).

**Where an answer came from is the host's word.** The desktop window holds a key in memory, handed
only to its own pages and never written anywhere. An answer is recorded from the `window` only with
that window's proof for that one ask, and from a `notification` only by the app's own handler; every
other holder of the token is recorded `cli`, whatever its headers or body say
([the paths](@/docs/permissions.md#where-an-answer-came-from)). Trusting a repository and writing a
grant over HTTP need the same proof. `devplane project trust` writes `~/.devplane/trusted.toml`
itself, and is refused inside an agent session; the built-in rule keeps agents from reading or writing
that file.

**The limit:** the token is readable by the agent's user, which is you. The built-in rule reads the
gated command line; a script that opens the file itself, or an agent that unsets those variables or
calls the host with the token, is not stopped. A `deny` on `Bash(devplane inbox answer *)` narrows the
gap without closing it. The window's proof narrows it too: an agent that can drive your desktop
(scripting, accessibility) can press its buttons, and a process running as you can read the key from
the app's memory.

## Gates and setup are project-authored

`[gates]` and `[workspace] setup` are read from the `devplane.toml` in the checkout that **owns** the
worktree, never the agent's branch. They run as children of the host, never through the agent, each in
its own process group so a timeout kills the tree (Unix only; on Windows children survive).

## Untrusted text is data

Agent output, hook payloads and issue bodies are never executed or interpolated into a shell, and are
rendered as text. An issue body reaches an agent marked as someone else's report, size-bounded.
AppleScript notification strings are escaped.

## `.worktreeinclude` cannot leave the repository

A fresh worktree copies the ignored files the committed `.worktreeinclude` names. Refused: a path
outside the checkout, a symlink out of the repository (`config/local.env -> ~/.ssh/id_rsa`), and
writing through a symlink at the destination.

## Telemetry stays local and redacted

`OTEL_LOG_USER_PROMPTS`, `OTEL_LOG_ASSISTANT_RESPONSES` and `OTEL_LOG_TOOL_CONTENT` are never set;
account and email attributes are dropped at ingest.

## Transcripts

A **driven** agent's output is stored in `~/.devplane/devplane.db` and pruned after 30 days.
`[agent] keep_transcripts = false` opts a repository out. Watched sessions carry no prose.

## Agent supply chain

Built-in `npx` agents are pinned to exact versions; nothing auto-updates. OpenCode runs the `opencode`
on your `PATH`. `~/.devplane/agents.toml` entries run exactly the command written.

## What this is not

- **Not a sandbox.** Agents run as you; use the vendors' sandboxes. A `Bash` rule reads command text
  and cannot follow a program that decides at run time what to execute.
- **Not tamper-evident.** The [decision log](@/docs/decisions.md) is append-only in a local file, not
  hash-chained.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on [the repository](https://github.com/hupe1980/devplane).
No external security review has been done; this page is a threat model, not an audit.
