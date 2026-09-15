+++
title = "Security"
description = "The trust model for software that approves tool calls, runs project commands and pushes to repositories."
weight = 24
[extra]
group = "reference"
+++

Vibeplane is privileged software. It approves tool calls, runs commands your repository committed,
and pushes branches. This page is the trust model, stated plainly, including what it does **not**
defend against.

## Nothing leaves the machine

No account, no cloud relay, no service to run, and no telemetry of our own — no usage analytics and
no crash reporting.

The daemon's only network egress is: GitHub through the `gh` CLI (opt-in, per repository), and the
agent processes themselves, which talk to their own providers exactly as they do without Vibeplane.

## Loopback is not an access control

Every listener binds `127.0.0.1`. That is necessary and not sufficient: **any process running as you
can reach the port.** What actually separates Vibeplane from everything else on the machine is the
bearer token in `~/.vibeplane/token`, mode `0600`, required on every request.

Two deliberate exceptions:

- `/healthz` — proves the port is ours without revealing what is on it.
- the telemetry endpoints — the exporter cannot be given a per-signal credential without sending it
  to every other collector you configure. They accept **observations only, never commands**.

The board is served unauthenticated because it is a static page containing no data; it cannot fetch
any without the token your browser holds. `vibeplane open` hands that token over once in the URL and
the page strips it from the address bar, so it does not end up in a screenshot or a bookmark.

## The trust gate

```sh
vibeplane trust .
```

`vibeplane work start` and `vibeplane dispatch` both refuse an untrusted repository, and both go
through the same check — a check one caller can skip is not a check.

The reason is specific: a headless agent runs **that repository's own hooks and MCP servers** with no
dialog of its own. The decision to allow that has to be one somebody made deliberately, once, for
that directory. A worktree inherits the trust of the checkout that owns it, because trusting a
project and then being asked again for each of its checkouts teaches people to say yes without
reading.

Untrusted projects are observe-only, which is still the whole of the observer.

## Where the policy can fail open, and where it cannot

Prohibitions are enforced through **two** synchronous hooks, not one. `PermissionRequest` fires only
when Claude Code is about to ask a human; `PreToolUse` fires before every tool call in every mode,
which is the only way a `never_auto` rule reaches a session in **auto mode**, where a classifier
approves routine calls and no prompt — and so no `PermissionRequest` — ever happens. `PreToolUse`
carries a prohibition or nothing, never a grant: an allow there would skip the classifier as well as
the prompt.

**Evaluation itself cannot fail open.** It is total, synchronous and in-process: there is no “policy
service unreachable” path, and the build fails if anything in the pure half grows one. Given the
call, the answer is always a verdict.

**And the gate does not need the daemon.** It is a `command` hook — the `vibeplane` binary, reading
the call on stdin and answering on stdout in about 26 ms — so a daemon that is stopped, crashed or
not yet started costs you the *record* of a decision, never the decision. A decision taken with no
daemon listening is appended to `~/.vibeplane/pending-decisions.jsonl` and written into the log at
the next start, marked as filed late.

**No hook can enforce its own presence**, and that is the vendor's design: a timed-out hook does not
block, and the reference says plainly *"don't count on a stalled hook to act as a gate."* So
detection is the defence. `vibeplane doctor` **runs** the installed gate with a probe call rather
than checking a line exists in a settings file, and the daemon does the same on a timer and raises a
critical `gate_down` item when it stops answering.

*The rules cannot fail open; a channel can, and the channel is a local process.*

`never_auto` cannot be overridden by `auto_allow`, in either direction.

The harder half is **silence**. A deny rule that matches nothing reads as protection and provides
none, and nothing errors. So the rule syntax implements the published specification row by row, and
the spellings that cannot work — a path rule on `Write`, an `mcp__` rule with brackets, a bare
wildcard in `auto_allow` — are **refused** by `vibeplane check` rather than carried. And because
reading a specification is not the same as agreeing with the product, `scripts/verify-permissions-diff.sh`
**generates** calls and fails on any disagreement with a running Claude Code.

It asks on two axes: an `auto_allow` list answering *did Claude Code run it?*, and a `never_auto`
list answering *did Claude Code refuse?*. The oracle is a model, so a disagreement is re-asked once
and reported only if it reproduces.

A third axis, `VIBEPLANE_DIFF_AXIS=dialect`, covers the tools that are not shells — `PowerShell`,
`Monitor`, `LSP` — which cannot use that oracle, since two run no command and the third needs a
Windows host. It runs **this matcher alone** and prints a checklist to put to a running product, and
says in its own output that it is not a measurement.

Its last full run, against Claude Code 2.1.270, was clean on both: 176 deny cases and 126 allow
cases, with no reproducible disagreement. **It has not been re-run since the current matcher
shipped**, and a harness result is true only of the code it ran against.
See [Permissions](/docs/permissions/).

A malformed `vibeplane.toml` keeps the rules it had. Because a process starting fresh has none to
keep, both `vibeplane doctor` and `vibeplane explain` name the project whose rules are not in force
rather than answering as though it simply had no rules.

### On GitHub Copilot the same rules ride a different hook

Copilot's HTTP `preToolUse` hook **fails open** — a timeout or a non-2xx reply falls through to its
default permission flow — so the gate there runs as a `command` hook, which fails *closed*.

Two consequences, both deliberate:

- **If the daemon is not running, the hook says nothing and exits zero.** A fail-closed hook that
  errored would deny every tool call on your machine while Vibeplane is stopped.
- **A hook timeout is fail-open on every Copilot event**, administrator policy hooks included. A slow
  Vibeplane there is not one that blocks the session; it is one that was not consulted.

## The policy cannot be made slow by its subject

The pattern is yours; the text it matches is a command an agent chose. Glob matching is **linear
rather than backtracking**, because a matcher that goes exponential on `Bash(a*a*a*b)` is a denial of
service against the synchronous hook a session is blocked on. Measured under 50 ms for a
2 000-character command against ten wildcards.

## Gates and setup commands are project-authored

They come from the committed `vibeplane.toml`, never from an agent. They run as **children of the
daemon**, never through the agent — letting the thing being checked choose the check is the one
mistake this layer exists to avoid.

Each runs in its own process group, so a timeout reaches the whole tree: killing only the shell would
leave the second half of `a && b` running.

## Untrusted text is data

Every agent-produced string is untrusted: never executed, never interpolated into a shell, rendered
as text. The same goes for hook payloads and issue bodies.

An issue's body reaches an agent **marked plainly as a report from someone else that may be wrong**,
and bounded, so a thousand-line log cannot spend the context window before the agent has read any
code. Text written by anyone on the internet is arriving at something that can run commands.

Strings that reach AppleScript for a notification are escaped, so a tool name cannot become script.

## `[workspace] include` cannot leave the repository

`include` takes repository-relative paths. The file naming them is **committed**, so it arrives with
somebody else's code — from a fork, a pull request, a vendored dependency.

Three ways out, all refused:

- A path that spells its way out: `../../.ssh/id_rsa`, or any absolute path.
- A **symlink** pointing out of the repository. Git tracks symlinks, so `config/local.env ->
  ~/.ssh/id_rsa` is a file a repository can ship. The source is resolved, not just read.
- Writing **through** a symlink already at the destination, which would reach any path on your disk.

A symlink that stays inside the repository works normally: the rule is containment, not a ban on
symlinks.

## Telemetry stays local and redacted

The receiver binds loopback only. Vibeplane never sets `OTEL_LOG_USER_PROMPTS`,
`OTEL_LOG_ASSISTANT_RESPONSES` or `OTEL_LOG_TOOL_CONTENT`, so prompt and response text stays
redacted. Account and email attributes are dropped at ingest.

The environment block `connect` writes is shown before it is written and removed by `disconnect`. If
a collector is already configured, `connect` refuses to override it.

## Transcripts

What a *driven* agent says is written to the same SQLite file as everything else, on your machine,
behind the same `0600` token, and pruned on the same retention sweep. It is kept by default because
such a run has no other window — and because the protocol streams the text to Vibeplane either way,
so discarding it was never a privacy measure.

It is still a repository's decision: `[transcripts] keep = false` writes nothing down, including the
prompts Vibeplane itself sent. Sessions Vibeplane merely watches are unaffected, because the
documented channels carry no prose at all.

## Agent supply chain

The five built-in agents are pinned to exact versions, and so is every `npx` package. Nothing
auto-updates.

Agents beyond those four are launch commands **you** write in `~/.vibeplane/agents.toml`. Vibeplane
runs what is in that file and pins nothing on your behalf — a smaller trust surface than fetching a
registry, but the pin is in your hands.

## What this deliberately is not

The decision log is **not tamper-evident**. Hash-chaining defends against an adversary with write
access to the same machine as the agents themselves, which is not a threat model this product has —
and claiming it would be worse than not having it.

Vibeplane does **not** sandbox agents. The vendors' own sandboxes and worktree isolation are what is
used. And nothing here governs what an agent does: no software can make an external agent's `rm -rf`
at-most-once from outside the process that runs it.

## Reporting something

Open an issue on [the repository](https://github.com/hupe1980/vibeplane). For anything you would
rather not post publicly, use GitHub's private vulnerability reporting there.

No external security review has happened. This page is a threat model, not an audit.
