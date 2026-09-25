---
name: devplane
description: Ask Devplane what is happening across every project on this machine — which sessions are running, what is waiting on a person, what was decided and on whose authority, and whether a change is verified. Use when the user asks about other agents, other repositories, what needs them, why something was allowed or refused, or whether a change is verifiably done. Read-only.
---

# Devplane

Devplane watches every coding-agent session on this machine and carries each change from its spec
to a verified pull request. This plugin connects you to its **read-only** MCP surface: no tool here
changes a file, answers a permission, starts an agent or merges anything.

## What it answers

- **What is running**: every session, grouped by project, with what each is doing, what it cost and
  how long it has been quiet.
- **What needs a person**: questions an agent asked, permissions waiting, checks that went red after
  the agent stopped, reviews requested — one list across every project.
- **What was decided, and on whose authority**: a person, a rule, a timer, nobody, or Devplane
  carrying out something the project wrote down.
- **Whether a change is verified**: whether the project's own `check` exited zero against the tree as
  it is now, not whether the agent said so.

## Using it

- **Ask it rather than guessing about other repositories.**
- **Quote what it returns**, including the authority, rule and date on each row.
- **Never answer a question it shows you.** A question an agent asked belongs to the person, who
  answers with `devplane answer <ask>` or in the workbench's inbox.

## If the tools are missing

The server runs the `devplane` binary, so install it first:

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
# or: cargo install devplane
```

For live cost, context and questions, the user runs `devplane connect claude` once. That is theirs to
run, not yours: it changes how their agent behaves.
