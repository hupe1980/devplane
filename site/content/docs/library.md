+++
title = "The library"
description = "Prompts and skills you reuse across projects, in the vendors' own formats — with one command that says which of your copies drifted, which projects lack one, and what a distribution path will reject."
weight = 15
[extra]
group = "guide"
+++

You have a `review-findings` skill. It is in four of your six repositories, two of those copies have
been edited since you put them there, and one repository has a copy you did not put there at all.

Nothing on your machine can tell you that.

```sh
devplane library diff review-findings
```

## Devplane owns verbs, and none of the nouns

`SKILL.md` has forty-six readers and a six-field portable core, so the library invents no format of
its own. An artefact here is exactly what its vendor wrote, byte for byte, and everything Devplane
knows about it lives in a separate file beside it.

```
~/.devplane/library/
  skills/
    review-findings/
      SKILL.md          ← the vendor's format, byte-for-byte
      .devplane.toml    ← ours: where it came from, its digest, when, who
  prompts/
    bump-deps.md        ← portable plain text, for agents that are not Claude Code
```

The split is the whole trick. The artefact stays byte-identical to what its vendor expects, and the
provenance sits next to it where `cat` can read it. The sidecar is **never part of the artefact** —
it does not reach the digest, so installing something does not immediately report it as changed.

## `devplane library diff` — which copy moved, and which way

```console
$ devplane library diff review-findings
review-findings

  DRIFT      payments-api    the project's copy was edited
  STALE      billing         the library moved; this copy is the one you installed
  CONFLICT   web             both moved — nobody here will pick
  MISSING    jobs            installed, and no longer there
  UNRECORDED api             a copy is here that Devplane did not install
  ok         infra

  suppressed: payments-api/.DS_Store (ignored by policy, not by accident)

Documented failures only. Fields a tool silently ignores are not covered and are not absent.
```

**Six outcomes, because there are six situations.** The two that matter most are `DRIFT` and
`STALE`: they are the same boolean and opposite instructions. One means somebody edited the project's
copy; the other means the source moved and the copy is *stale rather than wrong*. A tool that
collapsed them into "different" would have told you nothing you could act on.

`CONFLICT` offers **no resolution**. Both sides moved, and picking one for you means throwing away
work somebody did.

`UNRECORDED` answers a question nothing else asks: *which of my projects has a skill I did not put
there?*

**Exit code is always `0`.** This is a report. A non-zero exit would make it a gate, and nothing here
grades anything.

### Suppressed, not hidden

A `.DS_Store` beside your skill would otherwise report all six projects as drifted on a Mac, which is
the fastest way to teach somebody to stop reading this command. So there is a short ignore list —
`.DS_Store`, `.git/`, `*.swp`, `*~` — and **every suppression is printed**. An ignore list you cannot
see is somewhere to hide a change.

### What a distribution path will reject

```
leaving Claude Code:
  ERROR  argument-hint   unexpected key on claude.ai, the Skills API, package_skill.py
                         allowed: allowed-tools, compatibility, description, license, metadata, name
```

Anthropic's own distribution paths raise a **hard error** on a frontmatter field outside the
specification's six. That is documented, so Devplane reports it — against the **paths**, never
against a vendor. There is no `--to cursor`: what a third-party reader does with an unknown key is
not written down anywhere, and inventing an answer would be a compatibility claim over somebody
else's moving surface.

The constraints the specification *does* state are checked by counting: `name` at most 64 characters
and slug-shaped, `description` at most 1024 and non-empty, `compatibility` at most 500.

**And the caveat is printed every time**: a field a tool ignores *silently* raises nothing anywhere,
and nothing here can see it. An empty finding list is not a clean bill of health. If the frontmatter
could not be read at all, the report says `unread` rather than showing you nothing.

## `devplane library report` — what it will be allowed to do

```console
$ devplane library report review-findings
review-findings                 ~/.devplane/library/skills/review-findings

  from      github:acme/skills#review-findings
  fetched   2026-09-19T14:02:11Z, digest c3f1…, asked for by hupe

  allowed-tools   Bash
                  this skill pre-approves those for whoever installs it

3.8% of 3,171 public agent setups carry a shell-granting skill.

Devplane does not tell you this is safe. It tells you what it will be allowed to do.
```

Only the **shell-granting** entries are listed, and only bare ones. `Bash(npm test *)` is a scoped
grant and is the author doing the right thing; reporting it would make the common good case noisy and
teach you to ignore this line. A skill that lists a bare `Bash` is handing the shell to whoever
installs it, which is the class the percentage below is about.

**No verdict, ever.** Not *safe*, not *risky*, not a tick, not a score. A green tick in front of the
one case that needed a person is worse than no tick at all — and the public skill registries have
between ninety and 1.9 million entries and a malware problem, so the thing they lack is not
distribution. It is provenance.

## `devplane library install` — every refusal before the first byte

```console
$ devplane library install review-findings --to api,web,jobs,billing
  refused   billing    not trusted — devplane trust /repos/billing
  conflict  web        a different copy is already there (--force to replace)

Nothing was written. 2 of 4 targets would proceed.
```

The preflight runs over **every** target first. A refusal is never one failure after three
successes, because by then two repositories have a file in them and you have to work out which.

`--force` answers a collision and **never** an untrusted target: the flag exists for a copy somebody
edited, not for a repository nobody has looked at. Each overwrite is reported individually — five
quiet successes and one lost file is how people learn to stop reading output.

Re-installing bytes that are already there is **not** a collision. Refusing it would teach you to
pass `--force` by reflex, which is how a flag stops meaning anything.

### Only where a vendor says

| Path | Documented by |
|---|---|
| `.claude/skills/<name>/SKILL.md` | Claude Code, project scope |
| `~/.claude/skills/<name>/SKILL.md` | Claude Code, personal scope |
| `~/.copilot/skills/<name>/SKILL.md` | Copilot CLI, personal scope |
| `.devplane/prompts/<name>.md` | Devplane's own portable form |

Copilot documents project skills taking precedence over personal ones of the same name. Devplane
**does not resolve that precedence** — it reports both copies, because which one wins is the vendor's
business and guessing it is the kind of claim this refuses to make.

## `devplane library sync` — one copy, one direction you named

```console
$ devplane library sync review-findings --from library --to payments-api
  would replace  .claude/skills/review-findings   library → project
  run with --apply
```

Nothing is written without `--apply`. A two-sided divergence is refused with a sentence that says a
person must pick, and does not pick.

**`sync` is not a daemon.** No timer, no watcher, no background process. Something that edited files
in six repositories on a schedule would produce a commit nobody wrote in a repository nobody was
looking at. It is a command and a button, and it is one keystroke from undone because the repository
is a git repository.

## Two things it will never do

**It will never rewrite your artefact.** The copy is byte-identical to the source, and the digest is
what proves it. The moment this starts templating somebody's skill, it owns a format.

**It will never translate between vendors.** *One skill, correct everywhere* requires deciding what
`context: fork` means on a product with no subagents, and it does not mean anything. You get a report
of what a target will reject instead: a converter guesses, a report tells you what you are losing.

## Spec Kit comes along for free

Spec Kit ships its commands **as Agent Skills** — `.claude/skills/speckit-*/SKILL.md`. So the library
installs them, drift-checks them and reports their provenance with no code in Devplane that knows
what a specification is.

## The API

| Route | Returns |
|---|---|
| `GET /api/library` | artefacts, with coverage per project |
| `GET /api/library/{name}` | drift outcomes, provenance, portability findings |

**Read-only. There is no write route**, because an agent on this machine runs as the same user and
can read the token that page uses.
