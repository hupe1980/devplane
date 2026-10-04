---
name: devplane
description: Ask Devplane what is happening across every project on this machine — what is waiting on a person, what was decided and on whose authority, and whether a change is verified. Use when the user asks about other agents, other repositories, what needs them, why something was allowed or refused, or whether a change is verifiably done. Changes no file and answers no permission.
---

# Devplane

Devplane puts in front of a person the evidence for deciding whether an agent's change is safe to
merge, and keeps the record of how they know. This plugin connects you to its MCP server. No tool
here changes a file, answers a permission, starts an agent or merges anything. It writes two things:
a record of each `explain` question, and — only from an agent Devplane started for a change — a
claim that the change's specification and tests conflict, which stops that run.

## What it answers

- **What needs a person**: questions an agent asked, permissions waiting, checks that went red after
  the agent stopped, reviews requested — one list across every project.
- **What was decided, and on whose authority**: a person, a rule, a timer, nobody, or Devplane
  carrying out something the project wrote down.
- **Whether a change is verified**: whether the project's own `check` exited zero against the tree as
  it is now, not whether the agent said so, and whether its diff weakened a check.
- **What the permission gate would decide** about one call, and which rule decides it.

It does not list running sessions. For that, the user runs `devplane status` or opens the workbench.

## Using it

- **Ask it rather than guessing about other repositories.**
- **Quote what it returns**, including the authority, rule and date on each row.
- **Never answer a question it shows you.** A question an agent asked belongs to the person, who
  answers with `devplane inbox answer <ask>` or in the workbench's inbox.

## If the tools are missing

The server runs the `devplane` binary, so install it first:

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
# or: npm install -g devplane, or: cargo install --locked devplane
```

For live cost, context and questions, the user runs `devplane agent connect claude` once. That is theirs to
run, not yours: it changes how their agent behaves.
