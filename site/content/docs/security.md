+++
title = "Security"
description = "The trust model: what stays on your machine, what guards the local API, where the permission gate fails closed, and what Devplane does not defend against."
weight = 24
[extra]
group = "reference"
+++

Devplane refuses and defers tool calls, runs commands your repository committed, and pushes
branches. This page is the trust model, including what it does **not** defend against.

## Nothing leaves the machine

No account, no cloud relay, no usage analytics, no crash reporting. The only network egress is
the GitHub host you configured (github.com unless `[github] host` says otherwise), and only once you
have signed in to it, npm when `npx` first fetches a built-in agent's
pinned adapter, and the agents themselves, which talk to their providers as they do without
Devplane.

## Loopback plus a bearer token

The host binds `127.0.0.1`, which any process running as you can reach. What separates it is the
bearer token in `~/.devplane/token`, required in the `Authorization` header on every `/api` and
`/devplane` request; a token in a URL is not accepted. `~/.devplane` is mode `0700` and the files in
it `0600`.

- `/healthz` is open: it names the version and is the liveness test. The workbench's static files are
  open too, and carry no data.
- **A taken port is a refusal.** The agents' settings send the token to one port; a host that quietly
  moved elsewhere would leave the token going to whatever holds it. So the host does not start, and
  says which process holds the port.
- The telemetry receiver needs a token too, because a forged observation corrupts the record — but
  **not the one above.** Claude Code hands its settings' `env` block to every tool call, so whatever
  `connect` writes there the agent can read. It writes a **telemetry-only token**, derived one way
  from the real one and kept in `~/.devplane/telemetry-token`: it is accepted on the telemetry
  routes and refused everywhere else, so an agent holding it can report telemetry and nothing more.
  `devplane connect claude` writes the endpoint and `OTEL_EXPORTER_OTLP_HEADERS` together;
  `disconnect` removes both.
- The workbench is served with a Content-Security-Policy of `'self'` only, `no-referrer` and
  `nosniff`: the page renders agent-written text, and a script that got in could reach no other
  origin.
- `devplane open` hands the token to the workbench once in the URL; the page removes it from the
  address bar and sends it as a header from then on.

To point Claude Code at Devplane by hand, set both:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT="http://127.0.0.1:47831/devplane/otel"
export OTEL_EXPORTER_OTLP_HEADERS="Authorization=Bearer $(cat ~/.devplane/telemetry-token)"
```

## The trust gate

```sh
devplane trust --dry-run ~/src/someone-elses-repo   # show, trust nothing
devplane trust .                                    # show, then ask
devplane trust --yes .                              # a directory you wrote
```

A headless agent runs **the repository's own hooks and MCP servers** with no dialog, so `devplane
change start` refuses an untrusted repository. `trust` first shows every `command` hook and its
event, every MCP server (unpinned ones named), every skill that pre-approves the shell, and every
allow rule in the repository's agent settings that grants more than it looks like. A worktree
inherits the trust of the checkout that owns it. Untrusted projects are still observed.

## Where the permission gate fails closed

The rules are in [Permissions](@/docs/permissions.md). For security:

- **Evaluation cannot fail open.** It is synchronous and in-process in the `devplane hook` command
  hook; there is no "policy service unreachable" path, and no host has to be running.
- **A broken policy file fails closed.** A `devplane.toml` or `~/.devplane/policy.toml` that will not
  load sends every gated call to a person and raises a critical item until it loads.
- **Auto mode is covered.** Prohibitions ride `PreToolUse`, which fires before every call in every
  mode.
- **The matcher is linear**, and a line past 65,536 characters is asked about rather than skipped.
- **Nothing writes the rules.** No API route or button edits `[policy]`.
- **Devplane never approves.** It refuses or defers.
- **On GitHub Copilot** the HTTP hook fails open, so the gate there is a `command` hook too.

**No hook can enforce its own presence**: a timed-out or uninstalled hook does not block. So
`devplane doctor` probes the installed gate, and a running host does the same on a timer and raises a
critical item when it stops answering. Codex skips a hook you have not approved in its own dialog;
`devplane connect codex` says so.

## An agent cannot approve itself — mostly

`devplane answer --allow` is refused inside an agent session: when `CLAUDECODE`,
`CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`, `DEVPLANE_RUN`, or a Codex or Copilot session
variable (`CODEX_SANDBOX`, `CODEX_THREAD_ID`, `COPILOT_CLI`, …) is set. `--deny` still works.
`change offer`, `change finish`, `change archive` and `change review --seen` are refused the same
way.

Beneath every rule you write, a built-in prohibition keeps an agent from editing Devplane's files,
reading its token, or editing its repository's `devplane.toml` — by a file tool or a shell command.
[Permissions](@/docs/permissions.md#devplane-s-own-files-are-protected) has the detail.

**The limit:** the token in `~/.devplane/token` is readable by the agent's user, which is you. The
built-in rule reads the command line a gated tool call runs; a script the agent runs that opens the
file itself is not stopped, and neither is an agent that unsets those variables. A `never_auto` rule
on `Bash(devplane answer *)` narrows the gap; it does not close it.

## Gates and setup are project-authored

Gates (`[gates]`) and setup (`[workspace] setup`) are read from the `devplane.toml` in the checkout
that **owns** the worktree, never from the agent's branch. They run as children of the host, never
through the agent, each in its own process group so a timeout kills the whole tree (on Unix; on
Windows a timed-out gate's children are not killed). The review leads
with any check the change weakened. See [Verified done](@/docs/verified-done.md).

## Untrusted text is data

Every agent-produced string, hook payload and issue body is untrusted: never executed, never
interpolated into a shell, rendered as text. An issue body reaches an agent marked as a report from
someone else that may be wrong, and bounded in size. Strings passed to AppleScript for a
notification are escaped.

## `.worktreeinclude` cannot leave the repository

A fresh worktree copies the ignored files the committed `.worktreeinclude` names. Refused: a path
outside the checkout, a symlink pointing out of the repository (`config/local.env ->
~/.ssh/id_rsa`), and writing through a symlink already at the destination.

## Telemetry stays local and redacted

Devplane never sets `OTEL_LOG_USER_PROMPTS`, `OTEL_LOG_ASSISTANT_RESPONSES` or
`OTEL_LOG_TOOL_CONTENT`, and drops account and email attributes at ingest. `connect` shows the
environment block before writing it and does not override a collector you configured.

## Transcripts

What a **driven** agent says is stored in `~/.devplane/devplane.db` and pruned after 30 days by the
sweep a host runs when it starts. A repository can opt out with `[transcripts] keep = false`. Watched sessions carry no prose.

## Agent supply chain

The built-in agents launched through `npx` are pinned to exact versions; nothing auto-updates.
OpenCode runs the `opencode` on your `PATH`. Agents you add in `~/.devplane/agents.toml` run exactly
the command you wrote.

## What this is not

- **Not a sandbox.** Agents run as you. Use the vendors' sandboxes; Devplane adds worktree isolation
  and accountability. A `Bash` rule reads command text and cannot follow a program that decides at
  run time what to execute.
- **Not tamper-evident.** The [decision log](@/docs/decisions.md) is append-only in a local file, not
  hash-chained. Anything with write access to your home directory can edit it.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on
[the repository](https://github.com/hupe1980/devplane). No external security review has been done;
this page is a threat model, not an audit.
