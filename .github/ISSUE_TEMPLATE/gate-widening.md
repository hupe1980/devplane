---
name: The gate let something through that a rule should have stopped
about: A never_auto / always_ask rule did not fire, or a verdict named a rule that did not match
title: "gate: <rule> did not stop <call>"
labels: gate, widening
---

This is the report that matters most. Every gate defect found so far was silent — no error, no log
line — so a report with the exact call is worth more than any feature request.

**The rule** (from `vibeplane.toml` or `~/.vibeplane/policy.toml`):

```toml
[policy]
never_auto = ["..."]
```

**The call** — the tool and its input exactly as the agent made it:

```
Bash: cat .env*
```

**What Vibeplane decided** — paste the output of:

```sh
vibeplane explain '<the command>'        # what the rule table says
vibeplane audit --limit 5                # the row that was written, if any
```

**What the vendor did** with the same call and rule (Claude Code / Copilot / Codex), if you can tell.

**Versions:** `vibeplane --version`, `claude --version` (or `copilot`, `codex`), OS.
