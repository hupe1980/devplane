+++
title = "Pipelines"
description = "Declared chains of agent runs: one implements, another reviews, a gate decides, and a person approves where the project says so."
weight = 13
[extra]
group = "guide"
+++

“Start Claude to implement; when it is done, start an agent to check it” is worth having only if the
chain is written down in the repository rather than improvised per session — the same chain, on every
piece of work of that kind, with the human in the places the project chose.

```toml
[pipelines.feature]
steps = [
  { role = "implement", prompt = "implement", gate = "check" },
  { role = "review", agent = "codex", prompt = "review",
    findings = { back_to = "implement", max = 2 } },
  { role = "verify", prompt = "write-tests", gate = "check" },
  { human = "merge" },
]
```

The **kind** of work chooses the pipeline, which is why `--kind` is the only thing you have to
decide:

```sh
vibeplane work start "add rate limiting" --kind feature
```

## The cursor lives on the work item

Which step, how many times each has been entered, and what the last reviewer found are columns in
SQLite on the work row — not state in the process running it. A daemon that dies between *review*
and *verify* comes back and resumes at *verify*, rather than paying for *implement* twice.

The roles are **copied onto the work when it starts**, so editing `vibeplane.toml` mid-flight cannot
renumber a chain that is already running and send it to a step nobody agreed to. A role that has
since been deleted is an error rather than a guess.

## A reviewer reports by writing a file

Not in prose. Prose has to be parsed, and a parser that is wrong is wrong invisibly — it either
invents findings that send real work backwards or misses them and waves a bad change through.

```toml
# a fragment of a reviewing step
findings = { back_to = "implement", max = 2,
             file = ".vibeplane/findings.md" }
```

The reviewing step writes `.vibeplane/findings.md` in the worktree. Vibeplane reads it, hands the
contents to the step named, and deletes it so the next round starts clean. **An empty file and a
missing one both mean “nothing found”** — a reviewer should never have to invent a complaint to fill
a file.

Vibeplane appends the instruction naming that path to a reviewing step's prompt itself, so the
mechanism cannot be broken by a project's template forgetting it.

## Humans are steps

```toml
# a fragment of a pipeline
steps = [{ human = "merge" }]
```

Suspends the chain into phase `human` and puts a `human_step` item in the inbox — at *normal* level,
because the chain stopped exactly where the project asked it to, and an expected pause is not an
alarm. The agents are retired while it waits: a chain can sit at a human step for days, and holding
a model open for all of them is memory and money for nothing.

```sh
vibeplane work approve <id>
```

## Choosing a reviewer

Naming a second vendor for review is cheap, because both are protocol agents. It is not
automatically an improvement — **a reviewer weaker than the implementer removes correct code**, and
one below a capability floor changes nothing at all while doubling the bill. Pick for precision
rather than for how much a model flags.[^review]

Vibeplane enforces the structural half:

> [!IMPORTANT]
> **A step that can send work back must have a gate behind it.** `vibeplane check` and `work start`
> refuse the configuration otherwise. With a gate, a reviewer can only *propose*; the project's
> checks decide.

[^review]: On 116 controlled tasks, Claude reviewing Codex drafts raised the pass rate 71.6&nbsp;% →
89.7&nbsp;%, while Codex reviewing Claude drafts lowered it 91.4&nbsp;% → 82.8&nbsp;%
([arXiv:2607.21656](https://arxiv.org/abs/2607.21656)). In an execute–review–revise study the
weakest reviewer changed zero of a hundred answers while doubling token cost, and a mid-tier
cross-family reviewer raised accuracy 52&nbsp;% → 64&nbsp;% with none damaged
([arXiv:2609.04270](https://arxiv.org/abs/2609.04270)).

## Bounds

Loops burn tokens by design, so they are bounded three ways:

| Bound | Stops | Where |
|---|---|---|
| `findings.max` | a reviewer and an implementer circling | per findings loop |
| `max_feedback_rounds` | an agent and a test suite arguing | per gate |
| `[budget]` | one turn going a long way on its own | per work item |

**Two rounds is the default because that is where the curve flattens**: most of what iterative
self-repair recovers, it recovers in the first two
([arXiv:2604.10508](https://arxiv.org/abs/2604.10508)). A larger bound mostly buys tokens.

The bound is a safety property as well as an economic one — reward hacking under iterative
refinement is a documented failure mode, which is why the gate runs as a child of the daemon and the
feedback says *fix the cause, do not disable the check*.

Exhaustion asks a person. It never loops for ever.

## Prompts

`prompt = "implement"` names a template at `.vibeplane/prompts/implement.md`, committed on the
branch like the gates. When no such file exists the string is used literally, so a one-line pipeline
does not require creating a directory of files first.

A `prompt` is looked up in one order, the same for every agent:

1. `.vibeplane/prompts/<name>.md` — the portable form, committed on the branch like the gates.
2. `.claude/skills/<name>/SKILL.md`, project then personal — **your Claude Code Skills work as
   pipeline prompts**, so a project that already keeps its prompts there needs no second copy. Only
   the body is used; markdown is portable, so this works for a step running on Codex too.
3. `REVIEW.md` at the repository root, **for a step that reports findings only** — the file Claude
   Code's Code Review reads. A project that has one has already written down what it wants flagged,
   at what severity, with a verification bar, and every sentence of that is one a review prompt
   would otherwise have to invent. It is markdown addressed to a reviewer, so it reaches a reviewer
   running on Codex too.
4. The literal text, so a one-line pipeline needs no files at all.

`REVIEW.md` is offered only to a step with a `findings` block. Handing a reviewer's brief to the step
that is *writing* the code is a different instruction wearing the same words.

A skill's frontmatter — `allowed-tools`, `context: fork`, `model` — are directives the Claude
harness applies when *it* loads the skill by name. Inlining the body takes the instructions without
them.

Templates substitute `{title}`, `{task}`, `{role}` and `{findings}`. A template that never mentions
`{findings}` would silently drop them and the loop would spend its whole budget re-reviewing the same
code, so they are appended instead.

## Spec-driven development

Vibeplane knows nothing about `.specify/`, `openspec/` or Kiro's steering files — three third-party
layouts to track, for no capability `[gates]` and `[pipelines]` do not already have. It contributes
the part the category leaves out: **every spec tool ships a consistency checker and none of them
decides.** Spec Kit's `/speckit.analyze` is **"STRICTLY READ-ONLY"** and closes with a
*recommendation*; `/speckit.converge` appends the unbuilt work to `tasks.md` rather than failing.

**If your spec tool has a CLI, drift is already a gate.** OpenSpec has one:

```toml
[gates]
check = ["openspec validate --strict", "pnpm test -- --run"]
```

A red drift check behaves exactly like a red test: the lines go back to the same session, bounded by
`max_feedback_rounds`, and the work never reaches `review`.

**An analyser that runs inside a session reports in prose and grades it.** A step that returns the
work for a `LOW` terminology note spends its budget on style, so `findings.only` says which grades
stop it:

```toml
[pipelines.feature]
steps = [
  { role = "implement", agent = "claude", prompt = "speckit.implement", gate = "check" },
  { role = "drift", agent = "claude", prompt = "speckit.analyze",
    findings = { back_to = "implement", max = 2, only = ["CRITICAL", "HIGH"] } },
  { human = "merge" },
]
```

Matching is per line and case-insensitive; a findings file with no matching line is **nothing
found**. The words are yours — `CRITICAL` and `HIGH` are Spec Kit's vocabulary, and Vibeplane holds
no table of anyone's severity names.

**The phases are a pipeline.** `specify → plan → tasks → implement` is a chain with sign-off between
the steps, so declare it as one and use your own prompts — a Spec Kit command, a Skill, a file in
`.vibeplane/prompts/`. What Vibeplane adds is the gate between the steps, the human step that
suspends until somebody releases it, the cursor that survives a crash, and the row in
`vibeplane audit` saying why each step proceeded.

## What `check` refuses

```sh
vibeplane check
```

| Refused | Why it is not merely untidy |
|---|---|
| `back_to` names a step that is not in the pipeline, or is not *earlier* | a loop forwards is a chain that never ends |
| `back_to` names a step with no `gate` | a reviewer that can move code with nothing re-checking it can move it backwards |
| a step asks for a gate no `[gates.named]` declares | a missing gate that reads as success is the failure this layer exists to prevent |
| `[gates.named.x] run = []` | an empty gate is not a pass |
| two steps share a name | steps are addressed by name, so two of a name is an ambiguity resolved silently and wrongly |
| a pipeline with no steps | work of that kind would finish before it started |

Warnings rather than refusals: `max = 0` on a findings loop, an empty `[gates] check` beside a
pipeline that asks for `gate = "check"`, and any `[budget]` at all — because a guard that may not
fire has to say so.

A malformed **step** says which step and what is wrong with it (`step 'review' has no 'prompt'`)
rather than one sentence about an enum, because a person reading a forty-line file learns nothing
from that.
