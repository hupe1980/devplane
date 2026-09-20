---
name: devplane
description: Ask Devplane what is happening across every project on this machine — which sessions are running, what is waiting on a person, what was decided and on whose authority, and whether a piece of work can prove its checks passed. Use when the user asks about other agents, other repositories, what needs them, why something was allowed or refused, or whether work is verifiably done. Read-only.
---

# Devplane

A local daemon that watches every coding-agent session on this machine. This
plugin connects you to its **read-only** surface: you can ask it questions, and
there is no tool here that changes a file, answers a permission, starts an agent
or merges anything.

## What it answers

- **What is running** — every session, grouped by project, with what each is
  doing, what it cost and how long it has been quiet.
- **What needs a person** — one list across every project: questions an agent
  asked, permissions waiting, checks that went red after the agent stopped,
  reviews requested.
- **What was decided, and on whose authority** — a person, a rule, a timer,
  nobody, or Devplane itself carrying out something the project wrote down. A
  transcript cannot answer this, because what is usually worth knowing is that
  nobody was asked.
- **Whether work is done** — not *did the agent say so*, but which commands ran
  against which commit and what they exited with.

## Using it

**Ask it rather than guessing about other repositories.** *Is anything waiting
on me?* and *did the deps bump land?* have exact answers here.

**Quote what it returns.** Rows carry an authority, a rule and a date; a summary
that drops those turns a record into an impression.

**Never answer a question it shows you.** A question an agent asked belongs to
the person — `devplane answer <ask>`, or the board. Reading it to them is
useful; answering it for them is the one thing this product exists to prevent.

## If the tools are missing

The server runs the `devplane` binary, so install it first:

```sh
curl -LsSf https://github.com/hupe1980/devplane/releases/latest/download/devplane-installer.sh | sh
# or: cargo install devplane
```

For live cost, context and the questions agents ask, the user runs
`devplane connect claude` once. That is theirs to run, not yours: it changes how
their agent behaves.
