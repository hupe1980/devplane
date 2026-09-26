---
name: devplane
description: Ask Devplane what is happening across every project on this machine — what is waiting on a person, what was decided and on whose authority, whether a change is verified, and what reports projects filed about each other. Use when the user asks about other agents, other repositories, what needs them, why something was allowed or refused, or whether a change is verifiably done. Read-only.
---

# Devplane

Devplane watches the coding-agent sessions it can see on this machine (`devplane doctor` says which)
and carries each change from its spec to a verified pull request. This plugin connects you to its
**read-only** MCP surface: no tool here changes a file, answers a permission, starts an agent or
merges anything. The one thing written is a record that an `explain` question was asked.

## What it answers

- **What needs a person**: questions an agent asked, permissions waiting, checks that went red after
  the agent stopped, reviews requested — one list across every project.
- **What was decided, and on whose authority**: a person, a rule, a timer, nobody, or Devplane
  carrying out something the project wrote down.
- **Whether a change is verified**: whether the project's own `check` exited zero against the tree as
  it is now, not whether the agent said so, and whether its diff weakened a check.
- **What the permission gate would decide** about one call, and which rule decides it.
- **Reports between projects**: what was filed against a project and from it, and what became of
  each.

It does not list running sessions. For that, the user runs `devplane ls` or opens the workbench.

## Using it

- **Ask it rather than guessing about other repositories.**
- **Quote what it returns**, including the authority, rule and date on each row.
- **Never answer a question it shows you.** A question an agent asked belongs to the person, who
  answers with `devplane answer <ask>` or in the workbench's inbox.

## If the tools are missing

The server runs the `devplane` binary, so install it first:

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
# or: npm install -g devplane, or: cargo install --locked devplane
```

For live cost, context and questions, the user runs `devplane connect claude` once. That is theirs to
run, not yours: it changes how their agent behaves.
