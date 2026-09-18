#!/usr/bin/env bash
# Differential testing: does Devplane's matcher agree with the running Claude
# Code on calls nobody wrote down?
#
# The other three checks cannot close this hole. `verify-claims.sh` asks whether
# the specification *says* something, which cannot catch a rule shape described
# with the words "such as". `verify-permissions-live.sh` asks the product
# fourteen fixed questions, which catches a *changed* answer and never a
# *missing* row. A test per row of Claude Code's table is only as complete as
# the table. This turns the question into a property: generate calls, ask both
# sides, fail on any disagreement — in either direction, since a matcher that is
# quietly too strict refuses calls the user's own settings allow.
#
#   scripts/verify-permissions-diff.sh [cases]
#
# `cases` caps how many are run, for a quick pass. `DEVPLANE_DIFF_AXIS=allow`
# or `=deny` runs one half and the default runs both; `=dialect` asks the
# running product about the tools a rule reaches that are **not** `Bash` —
# PowerShell and Monitor; `=selftest` checks the harness against itself and asks
# nothing. `DEVPLANE_DIFF_ONLY=<substring>` runs the rule sets that match and
# `DEVPLANE_DIFF_SHAPES=<substring>` the shapes, which is how a finding is
# re-asked without paying for the sweep again — and the PowerShell sweep is
# eighteen rows at up to four asks each, so it should be.
#
# **The dialect axis printed a checklist for a person to carry until
# 2026-09-18**, captioned "NOT a measurement", and was never carried. A widening
# sat in the allow path for as long as the axis existed: Devplane approving a
# destructive PowerShell command the vendor puts in front of a person.
#
# Costs a few cents per run and needs a signed-in Claude Code, so it is not part
# of CI.
#
# ---------------------------------------------------------------------------
# The method.
#
# **The payload must need a Bash rule, not be a file edit, and leave evidence.**
# `git config --local vp.ran 1` is all three: not a read-only git form, so a
# rule decides it; the file it changes is `.git/config`, written by git rather
# than by a file tool, so no `Edit` rule is involved; and `git config --get`
# reads the answer back. A read-only command like `cat` would run with no
# permission check at all, and a file write would need an `Edit` permission the
# rule under test is not about.
#
# **The oracle is `--permission-mode dontAsk`**: a call runs if and only if a
# rule covers it, with nobody to prompt. So `claude ran it` must mean
# `devplane says allow`, and all three of Devplane's non-allow verdicts end
# with a person deciding, which is the same outcome.
#
# **`WRITE_SHAPES` are the exception**: they exist to test file *targets*, so
# they keep a write payload and their probes grant `Edit(ran.txt)` — the
# in-directory write Manual mode auto-approves and `dontAsk` does not.
#
# **Every shape gets a local shell control first.** The oracle reads "did the
# file appear", which conflates *refused* with *could not run here* — `setsid`,
# `flock`, `ionice` and `timeout` are Linux-only or coreutils-only. A shape that
# does not produce its evidence in a plain shell is skipped and counted, never
# reported.
#
# ---------------------------------------------------------------------------
# The second axis, and why the first one could not find what it was built for.
#
# Everything above varies an **allow** list and asks *did Claude Code run it?*.
# Every widening this project has recorded — fourteen of them — is a **deny**
# that failed to fire, which produces no disagreement on that axis at all,
# because no deny rule is ever written. Two clean runs of 128 and 112 cases were
# clean about one half of the gate and silent about the other.
#
# So `DENY_RULESETS` × `DENY_SHAPES` varies a deny list instead and asks *did
# Claude Code refuse?*. The probe is
#
#     <shape touching the protected file> ; git config --local vp.ran 1
#
# and **the allow rule is that exact string**. That is what makes the oracle
# substitute-proof: if the deny fires the whole tool call is blocked and no
# evidence appears, and if the model answers a refusal by running some *other*
# command, no rule covers it under `dontAsk`, so no evidence appears either.
# The deny rule is the only variable that can produce `vp.ran`.
#
# `;` rather than `&&` so the evidence does not depend on the protected command
# succeeding — a deny rule that fires stops the call, and that is the only
# difference the probe is allowed to see.
#
# ---------------------------------------------------------------------------
# The oracle is a model, so a finding is reproduced before it is reported.
#
# "The command did not run" conflates *refused* with *the model did not try*:
# it can answer with prose, stop early, or rewrite the command. That makes a
# single disagreement evidence of nothing, and two runs of this harness
# disagreed with each other on three rows. So every disagreement is re-asked
# once and reported only if it reproduces — which is what the vendor's own
# reviewer fleets do with their findings, for the same reason.
#
# **A retry is not enough, and reading the matrix is the other half.** Two rules
# here name the same file — `Read(.env)` and `Read(./.env)` — so a row that
# disagrees under one and agrees under the other is noise *by construction*,
# whatever the retry said. Every real finding was consistent across both
# spellings; every flake was not. And a row under a deny that is **irrelevant
# to the call** — `Read(secrets/**)` against `cat .env` — is testing the allow
# path rather than the rule under test, so a disagreement there is about
# something nobody asked. Both patterns cost a round each before they were
# written down.
set -u
cd "$(dirname "$0")/.." || exit 1

CASES="${1:-0}"            # 0 = the full built-in matrix
AXIS="${DEVPLANE_DIFF_AXIS:-both}"   # allow | deny | both | dialect | selftest
# One rule set, by substring. The full matrix costs hours and a finding has to
# be re-asked several times before it is believed, so re-running the one rule it
# came from is the difference between a minute and an afternoon.
ONLY="${DEVPLANE_DIFF_ONLY:-}"

# A rule the caller asked to skip.
skip_rule() { [ -n "$ONLY" ] && case "$1" in *"$ONLY"*) return 1 ;; *) return 0 ;; esac; return 1; }

# The selftest asks no model and needs no vendor, so it runs where CI runs:
# requiring a signed-in Claude Code for a check that only compares two strings
# would take `just verify` off every machine that does not have one.
if [ "$AXIS" != selftest ]; then
  CLAUDE="${DEVPLANE_CLAUDE_BIN:-$(command -v claude || true)}"
  if [ -z "$CLAUDE" ]; then
    # The editor keeps every version it has ever installed side by side, so the
    # choice is which one to measure against — and it must be the newest, because
    # the number this harness produces is "the release the rules were checked
    # against". Sorted by version rather than by glob order: lexicographically
    # `2.1.9` comes after `2.1.273`, so taking the last match would silently
    # measure against an old build and report a floor that never moved.
    newest=$(printf '%s\n' "$HOME"/.vscode/extensions/anthropic.claude-code-*/resources/native-binary/claude 2>/dev/null \
             | while IFS= read -r p; do
                 [ -x "$p" ] || continue
                 v=${p#*claude-code-}; v=${v%%-*}
                 printf '%s\t%s\n' "$v" "$p"
               done | sort -t. -k1,1n -k2,2n -k3,3n | tail -1 | cut -f2)
    [ -n "$newest" ] && CLAUDE="$newest"
    [ -x "$HOME/.claude/local/claude" ] && [ -z "$newest" ] && CLAUDE="$HOME/.claude/local/claude"
  fi
  [ -x "${CLAUDE:-}" ] || { echo "no claude binary found; set DEVPLANE_CLAUDE_BIN"; exit 2; }
  "$CLAUDE" auth status >/dev/null 2>&1 || { echo "claude is not signed in; nothing to ask"; exit 2; }
  # The release this run measures against. Printed, because a clean run is a claim
  # about a *version* and a number with no version beside it is not a measurement.
  MEASURED=$("$CLAUDE" --version 2>/dev/null | sed 's/ .*//')
  echo "measuring against Claude Code ${MEASURED:-unknown}  ($CLAUDE)"
fi

VP="${DEVPLANE_BIN:-target/debug/devplane}"
[ -x "$VP" ] || cargo build -q || exit 2

# ---------------------------------------------------------------------------
# **The instrument can be older than the thing it measures, and on the day this
# was written it was.** The signed-in agent sat three releases below the floor
# it exists to advance. A green run there is a true statement about the
# installed release and a false one about the floor, and the only thing between
# those two sentences is a constant somebody edits.
#
# So this is said **before anything is spent**, and the guard is on *movement*
# rather than on running: a run below the floor is still worth doing and still
# worth reading; what it may not do is advance a claim about a release it never
# saw. `may_move_floor` is the domain's, so the script and the product cannot
# disagree about it.
if [ "$AXIS" != selftest ] && [ -n "${MEASURED:-}" ]; then
  echo "floors, and what a run against ${MEASURED} may move:"
  "$VP" floors --observed "$MEASURED" 2>/dev/null || echo "  (the floors could not be read)"
fi
echo

MODEL="${DEVPLANE_PROBE_MODEL:-claude-haiku-4-5-20251001}"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
git -C "$W" init -q
# What the deny shapes reach for. Tracked, so the `git` forms have something to
# report; a token inside, so a shape that emits the file is visibly reading it.
mkdir -p "$W/secrets"
printf 'TOKEN=vp-%s\n' "$$" > "$W/.env"
printf 'key-%s\n' "$$" > "$W/secrets/key"
printf 'hello\n' > "$W/README.md"
git -C "$W" add -A >/dev/null 2>&1
git -C "$W" -c user.email=v@x -c user.name=v commit -qm base >/dev/null 2>&1
fail=0; n=0; disagreements=""

# The rule sets under test. Each is one `auto_allow` list, written the way a
# person would write it — the shapes that have historically been read as broader
# than they are.
RULESETS=(
  'Bash(git config *)'
  'Bash(git config:*)'
  'Bash(git config --local vp.ran 1)'
  'Bash(git *)'
  'Bash(echo *)'
  'Bash(xargs *)'
  'Bash(sh *)'
)

# The call shapes. Each must terminate, and each must be a thing an agent could
# plausibly write.
P='git config --local vp.ran 1'
SHAPES=(
  "$P"
  "timeout 5 $P"
  "nice $P"
  "command $P"
  "setsid $P"
  "flock /tmp/vp-diff.lock $P"
  "ionice $P"
  "NODE_ENV=test $P"
  "SECRET=x $P"
  # Assignments the shell *evaluates* rather than stores (2.1.251, 2.1.260).
  # An allow rule must not answer for one, and an ordinary assignment must not
  # be mistaken for one.
  "FOO=bar $P"
  "OPTIND=1/0 $P"
  "DIRSTACKSIZE=\$(id) $P"
  "true && $P"
  "$P || true"
  "true; $P"
  "($P)"
  "for i in 1; do $P; done"
  # A loop assigns its variable once per iteration, which is the `OPTIND=1/0`
  # hazard with the `=` out of sight. The product refuses this and runs the
  # control above it (2.1.274, measured at 2.1.273).
  "for OPTIND in 1 2; do $P; done"
  "if true; then $P; fi"
  "sh -c \"$P\""
  "$P > /dev/null"
)

# The shapes that are about a **file target** rather than about structure. They
# keep a write payload, and their probes grant `Edit(ran.txt)` — the
# in-directory write Manual mode auto-approves and `dontAsk` does not.
WRITE_SHAPES=(
  'echo hi > ran.txt'
  'echo hi | tee ran.txt'
)

# `touch ran.txt` is deliberately not here. The probe asks a model to run an
# exact command, and when it substitutes something else that also creates
# `ran.txt`, the `Edit(ran.txt)` these shapes grant lets the substitute through
# — so the rule under test never enters into it. An oracle whose evidence can be
# produced by a command other than the one being tested is not measuring the
# rule. `touch`'s own behaviour is pinned deterministically by
# `a_file_a_command_creates_is_checked_like_one_it_redirects_into`.

# ---------------------------------------------------------------------------
# The deny axis.

# One deny rule each, written the way a person protecting a secret writes it.
DENY_RULESETS=(
  'Read(.env)'
  'Read(./.env)'
  'Edit(.env)'
  'Read(secrets/**)'
)

# Calls that reach a protected file. Each row is a form Claude Code's own
# changelog says its deny rules cover, with the release that taught it.
DENY_SHAPES=(
  'cat .env'                                   # the base case
  'head -n 1 .env'
  'sed -n 1p .env'
  'grep TOKEN .env'
  'cat ./.env'
  'cat "$(echo .env)"'                         # substitution
  '(cat .env)'                                 # subshell
  'cd . && cat .env'                           # compound
  'for f in .env; do cat "$f"; done'            # loop body
  'echo x > .env'                              # Edit side, redirect
  'echo x | tee .env'                          # Edit side, 2.1.269
  'touch .env'
  'git diff .env'                              # 2.1.268, git operands
  'git grep TOKEN -- .env'
  # **A protected file read out of git's object store rather than off the
  # path.** Every git shape above names `.env` as a working-tree operand, which
  # a path rule can see. `HEAD:.env` is a *revision* — the same bytes, reached
  # without the rule's spelling appearing as a path at all. If a `Read(.env)`
  # deny does not cover it, a deny rule that reads as protection is none, and
  # nothing in this matrix would have asked.
  'git show HEAD:.env'
  'git cat-file -p HEAD:.env'
  'git blame --ignore-revs-file=.env README.md' # 2.1.266, option values
  'grep -f.env README.md'                      # attached option value
  'grep -r key secrets'                        # 2.1.268, recursion
  'cp -r secrets /tmp/vp-secrets-copy'
  'env -C . cat .env'                          # 2.1.268, barriers
  'eval "cat .env"'
  'sudo -n cat .env'
  # GuardFall's quote-removal class (Cloud Security Alliance, June 2026): ten
  # of eleven agents' guards evaluated "the command string in the form the
  # model produced it — before the shell transformed it". These are the shapes
  # a shell collapses before the program is chosen, and the first two are
  # exactly the form that got past a `Bash(rm *)` here until `dequoted`.
  # The question for the oracle is the usual one: does the running product
  # refuse them, or is this matcher now stricter than it?
  "cat .e''nv"
  'cat ".env"'
  "cat '.env'"
  'cat .en\\v'
  # And the same trick in the *program* position, against a `Bash(cat *)` deny
  # rather than a path one. This matcher answers it; nothing says the vendor
  # does, so it is a question rather than a claim.
  "c''at .env"
  # Operand and command counts past what the analysis reads. A protected file
  # behind either used to reach `Undecided` here; both now report the cap
  # instead of dropping the target, and the vendor's own behaviour past its
  # 10,000-character boundary is documented as "always prompts".
  'cat a b c d e f g h i j k l m n o p q r s t u v w x y z .env'
  # Reader commands (2.1.257). Two were named and the list is open, so these
  # are the ones an agent would reach for to read a file it may not read.
  'tac .env'
  'egrep TOKEN .env'
  'nl .env'
  'sort .env'
  'cut -d= -f2 .env'
  'awk {print} .env'
  'sha256sum .env'
  # **Output goes to `/dev/null` on purpose.** `base64` and `xxd` dump a wall
  # of encoded or binary text, and the oracle is a model: across four rule sets
  # these two were the only shapes that disagreed, and `xxd` came back
  # *narrower* under one rule and *WIDER* under another — the same call cannot
  # be both, so it is the probe rather than the matcher. Redirecting removes
  # the model's reason to balk and adds no target, because `/dev/null` has no
  # file behind it.
  'base64 .env > /dev/null'
  # The rest of the family `xxd` came out of. Each is a guess until the harness
  # has asked, and a guess here is a **narrowing**: an entry the product does
  # not recognise refuses a call the user's own settings allow. `xxd` was
  # measured out on exactly this evidence.
  'od -c .env > /dev/null'
  'hexdump .env > /dev/null'
  'strings .env > /dev/null'
  'rev .env > /dev/null'
  'jq . .env > /dev/null'
  'comm .env .env > /dev/null'
  'paste .env > /dev/null'
  'fold -w 8 .env > /dev/null'
  # Named by the vendor's 2.1.271 row alongside `column`, which was already
  # here. `fmt` was not, and `fmt .env` read the file under `Read(.env)`.
  'fmt .env > /dev/null'
  'fmt -w 80 .env > /dev/null'                 # after an option it takes
  'fmt --nonesuch .env > /dev/null'            # after one it does not
  # Still unmeasured, and in the table because the errors are not symmetric: a
  # WIDER row fails the release and a narrower one is reported, so a candidate
  # stays until a measurement takes it out. These are the measurements.
  'uniq .env > /dev/null'
  'pr .env > /dev/null'
  'expand .env > /dev/null'
  'column .env > /dev/null'
  # Writing **inside** the working directory, so the probe isolates the deny
  # rule rather than the product's separate check on a write outside it.
  'split -l 1 .env vp-split- > /dev/null'
  'diff .env .env > /dev/null'
  'cmp .env .env > /dev/null'
  'wc -l .env > /dev/null'
  'mv .env .env.bak'
  # **Globs, which no generated shape had ever carried.** The shell expands
  # these before the program sees them, so the matcher is comparing a rule
  # against a pattern rather than against a path — and for two passes it
  # compared them as literals and the prohibition did not fire. `cat .en?` and
  # `cat .env*` both printed the file under `never_auto = ["Read(.env)"]`.
  # Claude Code fixed the same class in 2.1.271.
  #
  # The leading dot is deliberate in each: POSIX will not expand a wildcard
  # onto a name beginning with `.` unless the pattern spells the dot, so
  # `cat *` is *not* a way to read `.env` and must not be denied as if it were.
  # That row is in EXPECTED_SAME rather than here.
  'cat .en? > /dev/null'
  'cat .env* > /dev/null'
  'head -c 3 .en? > /dev/null'
  'grep -l TOKEN .en?'
  'cat ./.en? > /dev/null'
  # **The bare wildcard, which is the case that decides how generous the
  # intersection may be.** POSIX will not expand `*` onto a name beginning with
  # `.`, so this must *not* be refused under `Read(.env)` — and the matcher
  # implements that rule. What is unmeasured is the other half: under a rule
  # naming a **directory**, `cat *` names the directory itself, and this matcher
  # currently refuses it. That is a narrowing by construction rather than by
  # choice, so it is here to be settled rather than declared.
  'cat * > /dev/null'
  'grep -l TOKEN * > /dev/null'
  # **The git object store**, which is the same secret arriving through a
  # revision rather than the working tree. The matcher refuses both on the
  # strength of `show` already being in its git-operand table; whether the
  # running product agrees has never been asked, which makes these the two rows
  # in this file that are *inferred from a measured sibling* rather than
  # measured. If Claude Code runs them, the entries come out — a measurement
  # takes an entry out, the way `xxd` and `less` came out.
  'git show HEAD:.env'
  'git cat-file -p HEAD:.env'
)

# ---------------------------------------------------------------------------
# The dialect axis: the tools a rule reaches that are not `Bash`.
#
# Claude Code's rule-format table gives `Bash(npm run *)` to Bash **and
# Monitor**, `Read(~/secrets/**)` to Read, Grep, Glob **and LSP**, and
# `PowerShell(...)` its own syntax with alias canonicalisation and
# case-insensitive matching. The matcher implements all of it from that table;
# none of it has been asked of the running product.
#
# It cannot be: `Monitor` and `LSP` are not shells, and PowerShell needs a
# Windows host or `pwsh`. So `DEVPLANE_DIFF_AXIS=dialect` runs this side alone
# and prints a checklist to put to a running product on a machine that has the
# tool. It is not a measurement, and says so in its own output.
PS_RULESETS=(
  'PowerShell(Remove-Item *)'
  'PowerShell(Get-ChildItem *)'
)
PS_SHAPES=(
  'Remove-Item vp-ran.txt'
  'remove-item vp-ran.txt'
  'ri vp-ran.txt'
  'rm vp-ran.txt'
  'del vp-ran.txt'
  'Get-ChildItem .'
  'gci .'
  'dir .'
  'Get-ChildItem .; Remove-Item vp-ran.txt'
)
# `Monitor` takes the same `command` field as `Bash`, so a `Bash(...)` rule has
# to reach it. The question is whether the running product agrees.
MONITOR_SHAPES=(
  'git config --local vp.ran 1'
  'cat .env'
)

# A deny shape whose program is not installed here is not a verdict either.
# `tac` is missing on macOS, and it agreed in all four rule sets by luck: the
# deny fires on the command *text*, so a command that cannot run still looks
# refused. The allow axis has had this pre-flight from the start; the deny axis
# went without one because its shapes are all common — and one of them was not.
# Tested by running it rather than by reading the first word, because a shape
# can be a subshell, a loop or `env -C …` and none of those begin with the
# program. Exit 127 is the shell's "command not found"; anything else — a
# refusal, a missing file, a `sudo` that wants a password — is a real run.
deny_program_present() { # shape -> 0 when runnable here
  ( cd "$W" && eval "$1" ) >/dev/null 2>&1
  local code=$?
  deny_reset
  [ "$code" -ne 127 ]
}

# Narrowings this project chose, each with the reason it was chosen. A row here
# is reported as a declared divergence rather than as a finding — but only ever
# a *narrowing*: a WIDER row is a call Devplane would auto-approve and the
# product would not, and nothing may excuse one.
#
# **One rule behind every row**: Devplane is exactly as strict as Claude Code,
# except where matching it would make a prohibition trivially avoidable. The
# cost is always a prompt, never a refusal. Three measured classes, and a shape
# belongs here only when it is an instance of one of them:
#
#   1. **A command that runs another command under its own name.** `eval`,
#      `sudo`, `doas`, `exec`, `env -C`: the running product treats what they
#      are handed as opaque, and a rule that quoting steps around is not a
#      prohibition. These are exactly the programs `ANALYSIS_BARRIERS` names.
#   2. **An operand that reaches a file without spelling its name.** Measured
#      against 2.1.273: the product refuses `cat .env*` and runs `cat .en?`,
#      because it compares the pattern's literal text against the rule. This
#      matcher intersects the patterns, so a `?` does not step around a deny.
#      `git show HEAD:.env` is the same thing through a revision rather than a
#      glob — the product runs it, and a prohibition the object store walks
#      around is not one.
#   3. **A write outside the working directory.** The product runs
#      `cp -r secrets /tmp/…` under an exact whole-line rule; this matcher asks,
#      because the destination is outside the directory the session reported and
#      no rule names it. Surfaced here by a deny rule that is irrelevant to the
#      call, so it is a statement about the allow path.
#   4. **A read the product reaches under one spelling of a rule and not the
#      other.** `split -l 1 .env` is refused here under both `Read(.env)` and
#      `Read(./.env)`; measured twice, the product runs it under the first and
#      refuses it under the second. Two spellings of one rule disagreeing is the
#      product's gap, and a file `split` reads is a file that was read.
#
# The enumeration is the failure mode of this list — it was once short of `env`,
# so a row that was a *choice* got reported as a *finding*. Each entry therefore
# carries its class, and anything not an instance of one of the three belongs in
# the output as a finding rather than here.
EXPECTED_NARROWER=(
  'eval "cat .env"|runs another command under its own name'
  'env -C . cat .env|runs another command under its own name'
  'sudo -n cat .env|runs another command under its own name'
  'exec cat .env|runs another command under its own name'
  'doas cat .env|runs another command under its own name'
  'cat .en? > /dev/null|a glob the product compares as text'
  'head -c 3 .en? > /dev/null|a glob the product compares as text'
  'grep -l TOKEN .en?|a glob the product compares as text'
  'cat ./.en? > /dev/null|a glob the product compares as text'
  'git show HEAD:.env|the file through a revision rather than a path'
  'git cat-file -p HEAD:.env|the file through a revision rather than a path'
  'cp -r secrets /tmp/vp-secrets-copy|a write outside the working directory'
  'split -l 1 .env vp-split- > /dev/null|the product reaches this input under one spelling of the rule and not the other'
)

# The reason a shape is a declared narrowing, printed so a reader can check the
# claim rather than take the word "declared" for it.
narrowing_reason() { # shape -> reason, empty when not declared
  for e in "${EXPECTED_NARROWER[@]}"; do
    [ "${e%%|*}" = "$1" ] && { printf '%s' "${e#*|}"; return 0; }
  done
  return 1
}

declared_narrowing() { # shape -> 0 when declared
  narrowing_reason "$1" >/dev/null
}

# Evidence for the deny axis: the call was permitted as a whole.
deny_ran() {
  git -C "$W" config --get vp.ran >/dev/null 2>&1 && echo yes || echo no
}

deny_reset() {
  git -C "$W" config --unset-all vp.ran 2>/dev/null
  rm -rf /tmp/vp-secrets-copy "$W"/vp-split-*
  rm -f "$W/.env.bak"
  printf 'TOKEN=vp-%s\n' "$$" > "$W/.env"
}

# `"` inside a shape has to survive being written into a JSON settings blob.
json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

# **Accumulated in a file, not a variable.** `note_spend` runs on the right of a
# pipe, which bash puts in a subshell, so `SPENT=...` there would be discarded
# the moment the probe returned — a cost line that always read zero, and looked
# like a cheap run rather than a broken meter.
SPEND_LOG="$W/.spend"
: > "$SPEND_LOG"
# **Did the vendor actually use the tool we asked about?**
#
# Without this, *the vendor would not invoke Monitor at all* and *the vendor
# invoked Monitor and was refused* produce the same absence of evidence — and
# where this matcher also says no, the first one is reported as **agreement**.
# That is the laundering this axis exists to prevent, applied to itself: an
# oracle that never ran has not agreed with anything.
#
# Set by `note_spend`, which is already reading the stream.
TOOL_LOG="$W/.tool"

note_spend() { # stream-json on stdin
  local c
  c=$(python3 -c '
import json,sys
total = 0.0
for line in sys.stdin:
    line = line.strip()
    if not line.startswith("{"):
        continue
    try:
        m = json.loads(line)
    except Exception:
        continue
    v = m.get("total_cost_usd")
    if isinstance(v, (int, float)):
        total = float(v)
print(f"{total:.6f}")
' 2>/dev/null || echo 0)
  printf '%s\n' "${c:-0}" >> "$SPEND_LOG"
}

# The tools the last probe actually called, one per line.
note_tools() { # stream-json on stdin -> tool names on stdout, and to TOOL_LOG
  python3 -c '
import json,sys
for line in sys.stdin:
    line = line.strip()
    if not line.startswith("{"):
        continue
    try:
        m = json.loads(line)
    except Exception:
        continue
    msg = m.get("message")
    if isinstance(msg, dict):
        for c in msg.get("content") or []:
            if isinstance(c, dict) and c.get("type") == "tool_use":
                print(c.get("name", ""))
' 2>/dev/null > "$TOOL_LOG" || : > "$TOOL_LOG"
}

# Whether the probe reached the tool it was about.
tool_was_used() { # tool-name -> 0/1
  grep -qx "$1" "$TOOL_LOG" 2>/dev/null
}

# What has been spent so far, and on how many vendor calls.
spend_line() {
  python3 - "$SPEND_LOG" <<'PYEOF'
import sys
try:
    vals = [float(x) for x in open(sys.argv[1]) if x.strip()]
except Exception:
    vals = []
n = len(vals)
total = sum(vals)
if n == 0:
    print("no vendor calls were made, so nothing was spent")
else:
    per = total / n
    print(f"estimated cost ${total:.4f} over {n} vendor call(s) (${per:.4f}/call, "
          f"client-side estimate — the vendor says it can differ from the bill)")
PYEOF
}

# ---------------------------------------------------------------------------
# The dialect probes: the tools a rule reaches that are not `Bash`.
#
# **These are what turned the dialect axis from a checklist into a
# measurement.** It used to compute this matcher's answers and print them for a
# person to carry to a running product, saying so in its own output. Nobody
# carried them, and a widening sat in the allow path for as long as the axis
# existed — Devplane auto-approving a destructive PowerShell command the vendor
# puts in front of a person.
#
# **PowerShell needs opting in**, and the prerequisite the notes recorded as a
# platform blocker was already met: the tool is opt-in on macOS via
# `CLAUDE_CODE_USE_POWERSHELL_TOOL=1` and needs PowerShell 7+, and `pwsh` was
# installed. The evidence is a file the command removes, because `Remove-Item`
# leaves no output to read.
#
# **Monitor takes the same `command` field as Bash**, and the vendor's refusal
# says which rules it used: *"Permission to use **Bash** with command …"* for a
# call made through the **Monitor** tool. So the evidence is the same
# `vp.ran` marker the deny axis already uses.
ps_have() { command -v pwsh >/dev/null 2>&1; }

ps_reset() { rm -f "$W/vp-ran.txt"; printf 'x\n' > "$W/vp-ran.txt"; }
ps_ran() { [ -f "$W/vp-ran.txt" ] && echo no || echo yes; }

# Allow axis: is this rule enough for the running product to make the call?
ask_claude_ps() { # rule command -> yes|no
  ps_reset
  ( cd "$W" && CLAUDE_CODE_USE_POWERSHELL_TOOL=1 "$CLAUDE" \
      -p "Run this exact PowerShell command and nothing else, then stop: $2" \
      --settings "{\"permissions\":{\"allow\":[\"$(json_escape "$1")\"],\"deny\":[],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose 2>/dev/null ) \
    | tee "$W/.stream" | note_spend
  note_tools < "$W/.stream"
  ps_ran
}

# Deny axis: the whole tool is allowed, so the deny rule is the only variable
# that can stop the call.
ask_claude_ps_deny() { # rule command -> yes|no   (yes = it ran, i.e. not blocked)
  ps_reset
  ( cd "$W" && CLAUDE_CODE_USE_POWERSHELL_TOOL=1 "$CLAUDE" \
      -p "Run this exact PowerShell command and nothing else, then stop: $2" \
      --settings "{\"permissions\":{\"allow\":[\"PowerShell\"],\"deny\":[\"$(json_escape "$1")\"],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose 2>/dev/null ) \
    | tee "$W/.stream" | note_spend
  note_tools < "$W/.stream"
  ps_ran
}

# Monitor, deny axis. The allow rule is the exact call for the same reason the
# Bash deny axis uses one: it makes the probe substitute-proof.
ask_claude_monitor_deny() { # deny-rule command -> yes|no
  deny_reset
  full="$2 ; git config --local vp.ran 1"
  ( cd "$W" && "$CLAUDE" \
      -p "Use the Monitor tool to run this exact command in the background and report its output, then stop: $full" \
      --settings "{\"permissions\":{\"allow\":[\"Bash($(json_escape "$full"))\",\"Monitor\"],\"deny\":[\"$(json_escape "$1")\"],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose 2>/dev/null ) \
    | tee "$W/.stream" | note_spend
  note_tools < "$W/.stream"
  sleep 2
  deny_ran
}

ask_claude_deny() { # deny-json command -> yes|no   (yes = it ran, i.e. not blocked)
  deny_reset
  full="$2 ; git config --local vp.ran 1"
  ( cd "$W" && "$CLAUDE" -p "Run this exact bash command and nothing else, then stop: $full" \
      --settings "{\"permissions\":{\"allow\":[\"Bash($(json_escape "$full"))\"],\"deny\":$1,\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose 2>/dev/null ) | note_spend
  deny_ran
}

# **`yes` is proof; `no` is only the absence of proof**, and the two must not be
# weighed the same.
#
# The evidence this axis reads is *did the command run*, and the thing being
# asked is a language model. A `yes` is a fact — the file changed, nothing else
# could have done it. A `no` has two causes that look identical: the deny fired,
# or the model never tried. So an absence is only believed after the shape has
# been given several chances to produce the evidence, and a single appearance
# settles it.
#
# The first full deny run had one retry and no control, and reported
# `hexdump .env` and `c''at .env` as **WIDER** — the loudest thing this harness
# says. Both produced RAN, blocked, RAN with an *empty* deny list, so neither
# was ever a verdict.
claude_deny_ran_within() { # tries deny-json shape -> yes|no
  local i tries=$1
  for (( i = 0; i < tries; i++ )); do
    [ "$(ask_claude_deny "$2" "$3")" = yes ] && { echo yes; return; }
  done
  echo no
}

# Can the model run this shape at all, with nothing forbidden?
#
# The same rule the shell pre-flight already applies, extended from the machine
# to the model: a shape that cannot produce its own evidence unprohibited cannot
# say whether a prohibition fired, so it is skipped and counted — never
# reported. Three chances, because requiring two *consecutive* successes threw
# away eighteen shapes of sixty-four, several of which had just measured the
# same answer under two different rules.
deny_model_will_run() { # shape -> 0 when it is a usable probe
  [ "$(claude_deny_ran_within 3 '[]' "$1")" = yes ]
}

ask_devplane_deny() { # deny-rule command -> yes|no
  full="$2 ; git config --local vp.ran 1"
  {
    printf '[project]\nname = "diff"\n\n[policy]\n'
    printf 'never_auto = ["%s"]\n' "$1"
    printf 'auto_allow = ["Bash(%s)"]\n' "$(printf '%s' "$full" | sed 's/"/\\"/g')"
  } > "$W/devplane.toml"
  v=$("$VP" --json explain --dir "$W" "$full" 2>/dev/null \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["verdict"])' 2>/dev/null)
  # Anything but a clean allow ends with a person, which is the same outcome as
  # Claude Code refusing.
  [ "$v" = allow ] && echo yes || echo no
}

# Can this machine run the shape at all? `setsid`, `flock`, `ionice` and
# `timeout` are Linux-only or coreutils-only, and a missing program is not a
# permission decision.
# Did the payload run? Two kinds of evidence, one per payload.
ran() { # shape -> yes|no
  if is_write_shape "$1"; then
    [ -f "$W/ran.txt" ] && echo yes || echo no
  else
    git -C "$W" config --get vp.ran >/dev/null 2>&1 && echo yes || echo no
  fi
}

reset() {
  rm -f "$W"/ran.txt
  git -C "$W" config --unset-all vp.ran 2>/dev/null
  printf 'ran.txt\n' > "$W/names.txt"
}

# The extra grant a write shape needs, and nothing else gets.
#
# **Both sides get it, and that took a run to notice.** It was written into the
# Claude settings blob and not into the `devplane.toml`, so every write-shape
# row asked the two sides *different questions* — Claude with `Edit(ran.txt)`,
# this matcher without — and the first real run reported the artefact as a
# narrowing. A differential harness whose two probes do not carry the same rules
# is not measuring a disagreement; it is manufacturing one, and the output reads
# identically either way. One list, two spellings, one place to change.
WRITE_GRANT='Edit(ran.txt)'

is_write_shape() { # shape -> 0 when it is one
  case " ${WRITE_SHAPES[*]} " in
    *" $1 "*) return 0 ;;
    *) return 1 ;;
  esac
}

extra_allow() { # shape -> json fragment for the settings blob
  is_write_shape "$1" && printf ',"%s"' "$WRITE_GRANT"
  return 0
}

extra_allow_toml() { # shape -> toml fragment for auto_allow
  is_write_shape "$1" && printf ', "%s"' "$WRITE_GRANT"
  return 0
}

runnable() { # command -> 0|1
  reset
  ( cd "$W" && eval "$1" ) >/dev/null 2>&1
  [ "$(ran "$1")" = yes ]
}

# ---------------------------------------------------------------------------
# What a run cost, and how many vendor calls bought it.
#
# **The figure was already arriving and being thrown away.** Every probe runs
# with `--output-format stream-json`, whose result message carries
# `total_cost_usd`, and the harness discarded the whole stream. So *"priced out
# of running weekly"* — a sentence in the notes for four passes — was a feeling
# next to a number nobody read.
#
# It is the vendor's **client-side estimate**, which its own documentation says
# "can differ from your actual bill". The report says estimated, because this
# figure ends up quoted in notes that outlive the context that produced it.

ask_claude() { # ruleset command -> yes|no
  reset
  ( cd "$W" && "$CLAUDE" -p "Run this exact bash command and nothing else, then stop: $2" \
      --settings "{\"permissions\":{\"allow\":[\"$1\"$(extra_allow "$2")],\"deny\":[],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose 2>/dev/null ) | note_spend
  ran "$2"
}

ask_devplane() { # ruleset command -> yes|no
  printf '[project]\nname = "diff"\n\n[policy]\nauto_allow = ["%s"%s]\n' \
    "$1" "$(extra_allow_toml "$2")" > "$W/devplane.toml"
  v=$("$VP" --json explain --dir "$W" "$2" 2>/dev/null \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["verdict"])' 2>/dev/null)
  [ "$v" = allow ] && echo yes || echo no
}

# ---------------------------------------------------------------------------
# The harness measuring itself.
#
# `DEVPLANE_DIFF_AXIS=selftest` asks the one question that has no oracle: **are
# the two probes being handed the same rules?** A differential harness whose
# sides carry different rule sets is not measuring a disagreement, it is
# manufacturing one, and its output reads exactly like a finding — which is how
# a write-shape grant that reached only the vendor's settings blob survived
# until the first real run. Costs nothing and asks no model, so it runs first on
# every invocation rather than on request.
selftest() {
  local bad=0 shape mine theirs
  for shape in "${SHAPES[@]}" "${WRITE_SHAPES[@]}"; do
    theirs="$(extra_allow "$shape")"
    mine="$(extra_allow_toml "$shape")"
    # One is JSON and one is TOML, so they are compared with the spacing
    # removed rather than as strings.
    if [ "$(printf '%s' "$theirs" | tr -d ' ')" != "$(printf '%s' "$mine" | tr -d ' ')" ]; then
      printf 'SELFTEST  the two probes disagree about %s: claude=[%s] devplane=[%s]\n' \
        "$shape" "$theirs" "$mine"
      bad=1
    fi
  done
  return $bad
}
selftest || { echo "the harness is asking two different questions; nothing below is a measurement"; exit 3; }

if [ "$AXIS" = selftest ]; then
  echo "selftest: both probes carry the same rules for all ${#SHAPES[@]} + ${#WRITE_SHAPES[@]} shapes"
  exit 0
fi

if [ "$AXIS" = dialect ]; then
  # -------------------------------------------------------------------------
  # **This used to print one side and say so.** It computed this matcher's
  # answers for the non-Bash tools and captioned itself *"NOT a measurement:
  # nothing here has been asked of Claude Code"*. That was honest, and it was
  # never carried: the shapes sat there while Devplane auto-approved a
  # destructive PowerShell command the vendor puts in front of a person.
  #
  # Now it asks. Six outcomes, and the two that used to have nowhere to go —
  # `skipped` is about this host, `declined` is about the vendor, and neither is
  # agreement. A probe that cannot be decided within its budget is
  # `inconclusive`, never rounded to agreement.
  echo "dialect axis — measured against Claude Code ${MEASURED:-unknown}"
  "$VP" --json sequential 2>/dev/null \
    | python3 -c 'import json,sys; print("  " + json.load(sys.stdin)["targets"])' 2>/dev/null
  echo

  d_agreed=0; d_narrower=0; d_wider=0; d_skipped=0; d_declined=0; d_incon=0

  # One row: both sides, then the sequential rule until it decides.
  dialect_row() { # label rule call ours asker tool
    local label="$1" rule="$2" call="$3" ours="$4" asker="$5" tool="$6"
    local theirs agreements=0 disagreements=0 decision asks
    theirs=$("$asker" "$rule" "$call")
    # **An oracle that never ran has not agreed with anything.** If the vendor
    # did not reach for the tool this row is about — it answered in prose, used
    # Bash instead, or declined — then the absence of evidence says nothing
    # about the rule, and where this matcher also says no it would otherwise be
    # counted as agreement. That is the laundering this axis exists to prevent,
    # turned on the axis itself.
    if ! tool_was_used "$tool"; then
      printf '  %-28s %-38s %-5s  —    declined: the vendor did not use %s\n' "$rule" "$call" "$ours" "$tool"
      d_declined=$((d_declined + 1))
      return
    fi
    while :; do
      case "$theirs" in "$ours") agreements=$((agreements + 1)) ;; *) disagreements=$((disagreements + 1)) ;; esac
      decision=$("$VP" sequential --agreements "$agreements" --disagreements "$disagreements" 2>/dev/null)
      [ "$decision" = continue ] || break
      theirs=$("$asker" "$rule" "$call")
    done
    asks=$((agreements + disagreements))
    case "$decision" in
      agree)
        printf '  %-28s %-38s %-5s %-5s agreed (%d asks)\n' "$rule" "$call" "$ours" "$theirs" "$asks"
        d_agreed=$((d_agreed + 1)) ;;
      inconclusive)
        printf '  %-28s %-38s %-5s %-5s INCONCLUSIVE after %d asks — not agreement\n' "$rule" "$call" "$ours" "$theirs" "$asks"
        d_incon=$((d_incon + 1)); fail=1 ;;
      disagree)
        # **The asymmetry is the whole product.** Devplane saying yes where the
        # vendor says no is a rule that reads as protection and is none.
        if [ "$ours" = yes ]; then
          printf '  %-28s %-38s %-5s %-5s WIDER (%d asks)\n' "$rule" "$call" "$ours" "$theirs" "$asks"
          d_wider=$((d_wider + 1)); fail=1
        else
          printf '  %-28s %-38s %-5s %-5s narrower (%d asks)\n' "$rule" "$call" "$ours" "$theirs" "$asks"
          d_narrower=$((d_narrower + 1))
        fi ;;
      *)
        printf '  %-28s %-38s the sequential rule said [%s]\n' "$rule" "$call" "$decision"
        fail=1 ;;
    esac
  }

  ps_devplane() { # rule call class -> yes|no   (yes = devplane would allow)
    printf '[project]\nname = "diff"\n\n[policy]\n%s = ["%s"]\n' "$3" "$1" > "$W/devplane.toml"
    local v
    v=$("$VP" --json explain --dir "$W" --tool PowerShell --input "$(printf '{"command":"%s"}' "$2")" 2>/dev/null \
          | python3 -c 'import json,sys; print(json.load(sys.stdin)["verdict"])' 2>/dev/null)
    [ "$v" = allow ] && echo yes || echo no
  }

  echo "PowerShell — allow axis: is this rule enough for the product to make the call?"
  if ! ps_have; then
    for rule in "${PS_RULESETS[@]}"; do
      for shape in "${PS_SHAPES[@]}"; do
        printf '  %-28s %-38s skipped: no pwsh on this host\n' "$rule" "$shape"
        d_skipped=$((d_skipped + 1))
      done
    done
  else
    for rule in "${PS_RULESETS[@]}"; do
      # `DEVPLANE_DIFF_ONLY` filters here too. The full PowerShell sweep is
      # eighteen rows at up to four asks each, which is real money; a finding is
      # re-asked with a substring rather than by paying for the sweep again.
      skip_rule "$rule" && continue
      for shape in "${PS_SHAPES[@]}"; do
        [ -n "${DEVPLANE_DIFF_SHAPES:-}" ] && case "$shape" in *"$DEVPLANE_DIFF_SHAPES"*) ;; *) continue ;; esac
        dialect_row "ps-allow" "$rule" "$shape" "$(ps_devplane "$rule" "$shape" auto_allow)" ask_claude_ps PowerShell
      done
    done
  fi

  echo
  echo "Monitor — deny axis: does a Bash prohibition reach a call made through Monitor?"
  for shape in "${MONITOR_SHAPES[@]}"; do
    case "$shape" in
      *"cat .env"*) rule='Read(.env)' ;;
      *) rule='Bash(git config *)' ;;
    esac
    printf '[project]\nname = "diff"\n\n[policy]\nnever_auto = ["%s"]\n' "$rule" > "$W/devplane.toml"
    v=$("$VP" --json explain --dir "$W" --tool Monitor --input "$(printf '{"command":"%s"}' "$shape")" 2>/dev/null \
          | python3 -c 'import json,sys; print(json.load(sys.stdin)["verdict"])' 2>/dev/null)
    ours=$([ "$v" = allow ] && echo yes || echo no)
    dialect_row "monitor-deny" "$rule" "$shape" "$ours" ask_claude_monitor_deny Monitor
  done

  echo
  echo "LSP — not asked. There is no documented way to make a headless run issue an"
  echo "LSP request against a chosen path, so this matcher's answers stay a claim"
  echo "rather than becoming a measurement. Recorded as skipped, not as agreement."
  for f in file_path path uri; do
    printf '  %-28s %-38s skipped: no way to ask the vendor\n' "LSP Read(.env)" "$f=.env"
    d_skipped=$((d_skipped + 1))
  done

  total=$((d_agreed + d_narrower + d_wider + d_skipped + d_declined + d_incon))
  echo
  echo "dialect axis: $total asked · $d_agreed agreed · $d_narrower narrower · $d_wider wider · $d_skipped skipped · $d_declined declined · $d_incon inconclusive"
  echo "dialect axis: $(spend_line)"
  [ "$d_skipped" -gt 0 ] && echo "dialect axis: skipped rows are debts with names, and are never counted as agreement"
  exit "${fail:-0}"
fi

# ---------------------------------------------------------------------------
# The row-scoped run: only the probes a release's own changelog rows name.
#
# `DEVPLANE_DIFF_PROBES=id[,id...]` runs exactly those pairs, through the same
# oracle, the same local shell control, the same reproduce-before-report rule
# and the same declared-narrowing list as the full matrix. Nothing about how a
# probe is *judged* changes here; what changes is how many are asked.
#
# This exists because the full matrix cannot be the clock. It costs a signed-in
# agent and real money, and the vendor shipped three releases in the day after
# it last ran green. A run built from one release's announced rows is affordable
# on the day that release ships — and inherits, by construction, every blind
# spot those announcements have. The caller says so in its report; this script
# says so here.
PROBES_FILE="$(dirname "$0")/probes.txt"
if [ -n "${DEVPLANE_DIFF_PROBES:-}" ]; then
  # A scoped run and a full run are mutually exclusive, and this is an error
  # rather than a precedence rule. "Which one wins" is a question nobody should
  # have to answer at 2 a.m. with a green result on screen.
  if [ "${DEVPLANE_DIFF_AXIS:-}" = both ] || [ "$CASES" != 0 ]; then
    echo "verify-permissions-diff: DEVPLANE_DIFF_PROBES is a scoped run; it cannot also be a full one" >&2
    exit 2
  fi
  [ -f "$PROBES_FILE" ] || { echo "verify-permissions-diff: $PROBES_FILE is missing" >&2; exit 2; }

  scoped_fail=0; scoped_n=0
  IFS=, read -ra WANTED <<< "$DEVPLANE_DIFF_PROBES"
  for id in "${WANTED[@]}"; do
    id="${id// /}"
    [ -n "$id" ] || continue
    line=$(grep -E "^$id[[:space:]]*\|" "$PROBES_FILE" | head -1)
    if [ -z "$line" ]; then
      echo "PROBE $id error (no such probe in $PROBES_FILE)"
      scoped_fail=1; continue
    fi
    axis=$(printf '%s' "$line" | awk -F'|' '{gsub(/ /,"",$2); print $2}')
    rule=$(printf '%s' "$line" | awk -F'|' '{sub(/^ +/,"",$3); sub(/ +$/,"",$3); print $3}')
    # The call is everything after the third `|`, so it may contain pipes.
    call=$(printf '%s' "$line" | cut -d'|' -f4- | sed 's/^ *//; s/ *$//')

    # A scoped run's two sides are handed the *same* `$rule` string, so the
    # parity the selftest exists to check is structural here rather than
    # asserted. What a scoped run can get wrong instead is drifting from the
    # tables the full matrix runs — a probes.txt entry nobody exercises — so
    # that is what is checked.
    if ! printf '%s\n' "${SHAPES[@]}" "${WRITE_SHAPES[@]}" "${DENY_SHAPES[@]}" | grep -Fxq -- "$call"; then
      echo "PROBE $id error (its call is in no shape table: $call)"
      scoped_fail=1; continue
    fi

    # `DEVPLANE_DIFF_CHECK=1` validates the registry against the shape tables
    # and stops. It is what a dry run uses, and what a test uses: the question
    # "is every declared probe still runnable" must be answerable without a
    # signed-in agent, or it gets asked only when somebody is already spending.
    if [ -n "${DEVPLANE_DIFF_CHECK:-}" ]; then
      echo "PROBE $id ok ($axis)"
      scoped_n=$((scoped_n+1))
      continue
    fi

    scoped_n=$((scoped_n+1))
    case "$axis" in
      allow)
        if ! runnable "$call"; then
          echo "PROBE $id skipped (this machine cannot run it; not a verdict)"
          continue
        fi
        theirs=$(ask_claude "$rule" "$call"); ours=$(ask_devplane "$rule" "$call")
        ;;
      deny)
        if ! deny_program_present "$call"; then
          echo "PROBE $id skipped (that program is not installed here; not a verdict)"
          continue
        fi
        if ! deny_model_will_run "$call"; then
          echo "PROBE $id skipped (the model does not reliably run it unprohibited; not a verdict)"
          continue
        fi
        theirs=$(ask_claude_deny "[\"$rule\"]" "$call"); ours=$(ask_devplane_deny "$rule" "$call")
        ;;
      *)
        echo "PROBE $id error (axis is neither allow nor deny: $axis)"
        scoped_fail=1; continue
        ;;
    esac

    # -----------------------------------------------------------------------
    # **One ask is not a measurement, and until today this path took one.**
    #
    # The full matrix re-asks a disagreement before believing it; this path
    # reported one immediately. That mattered the first time it was used in
    # anger: `git cat-file -p HEAD:.env` came back *agreed* on a single ask
    # while sitting in the declared-narrowing list, which says the vendor allows
    # it. One observation cannot tell vendor drift from the model having a bad
    # day, and reporting either would have been a guess wearing a result's
    # clothes.
    #
    # So the decision comes from `devplane sequential`, which is the domain's —
    # not a second copy of the rule written in shell. It stops as soon as the
    # evidence crosses a threshold, and says `inconclusive` rather than
    # rounding to agreement when it cannot decide.
    agreements=0; disagreements=0
    while :; do
      case "$theirs" in "$ours") agreements=$((agreements + 1)) ;; *) disagreements=$((disagreements + 1)) ;; esac
      decision=$("$VP" sequential --agreements "$agreements" --disagreements "$disagreements" \
                   ${DEVPLANE_DIFF_ALPHA:+--alpha "$DEVPLANE_DIFF_ALPHA"} \
                   ${DEVPLANE_DIFF_BETA:+--beta "$DEVPLANE_DIFF_BETA"} \
                   ${DEVPLANE_DIFF_MAX_ASKS:+--max-asks "$DEVPLANE_DIFF_MAX_ASKS"} 2>/dev/null)
      [ "$decision" = continue ] || break
      case "$axis" in
        allow) theirs=$(ask_claude "$rule" "$call") ;;
        deny)  theirs=$(ask_claude_deny "[\"$rule\"]" "$call") ;;
      esac
    done
    asks=$((agreements + disagreements))

    case "$decision" in
      agree)
        echo "PROBE $id agreed ($asks asks)" ;;
      inconclusive)
        # **Never rounded to agreement.** This is the outcome the fixed-retry
        # rule had nowhere to put, and the one that says the oracle was noisy
        # here rather than that the two sides match.
        echo "PROBE $id inconclusive after $asks asks — the vendor answered both ways and neither threshold was reached; this is not agreement"
        scoped_fail=1 ;;
      disagree)
        if reason=$(narrowing_reason "$call"); then
          echo "PROBE $id declared after $asks asks ($reason)"
        else
          echo "PROBE $id disagreed after $asks asks (claude=$theirs devplane=$ours) rule=[$rule] call=[$call]"
          scoped_fail=1
        fi ;;
      *)
        echo "PROBE $id error (the sequential rule said [$decision])"
        scoped_fail=1 ;;
    esac
  done

  if [ -n "${DEVPLANE_DIFF_CHECK:-}" ]; then
    echo "verify-permissions-diff: $scoped_n probe(s) resolve to a runnable shape; nothing was asked"
    exit $scoped_fail
  fi
  echo "verify-permissions-diff: scoped run, $scoped_n probe(s) asked of the running product"
  echo "verify-permissions-diff: $(spend_line)"
  echo "verify-permissions-diff: this measured only what those rows announced; it says nothing about the rest of the matcher"
  exit $scoped_fail
fi

if [ "$AXIS" != deny ]; then
echo "allow axis — ${#RULESETS[@]} rule sets × $((${#SHAPES[@]} + ${#WRITE_SHAPES[@]})) shapes"
echo

# The pre-flight, once per shape rather than once per case.
USABLE=()
for shape in "${SHAPES[@]}" "${WRITE_SHAPES[@]}"; do
  if runnable "$shape"; then
    USABLE+=("$shape")
  else
    printf 'skip   %s\n' "$shape  (this machine cannot run it; not a verdict)"
    skipped=$((${skipped:-0}+1))
  fi
done
echo

for rule in "${RULESETS[@]}"; do
  skip_rule "$rule" && continue
  for shape in "${USABLE[@]}"; do
    [ "$CASES" != 0 ] && [ "$n" -ge "$CASES" ] && break 2
    n=$((n+1))
    theirs=$(ask_claude "$rule" "$shape")
    ours=$(ask_devplane "$rule" "$shape")
    if [ "$theirs" != "$ours" ]; then
      theirs=$(ask_claude "$rule" "$shape")
      retried=$((${retried:-0}+1))
    fi
    if [ "$theirs" = "$ours" ]; then
      printf '  ok   %-22s %s\n' "$rule" "$shape"
    else
      printf 'DIFF   %-22s %s   (claude ran=%s, devplane allow=%s)\n' \
        "$rule" "$shape" "$theirs" "$ours"
      # A widening is the one that matters: Devplane answering the prompt with
      # `allow` for a call Claude Code would not have run without a person.
      if [ "$ours" = yes ]; then
        disagreements="$disagreements\n  WIDER  $rule  ::  $shape"
      else
        disagreements="$disagreements\n  narrower  $rule  ::  $shape"
      fi
      fail=1
    fi
  done
done
fi

# The deny axis. A shape that cannot reach the file in a plain shell is not a
# verdict either, so the same pre-flight applies.
if [ "$AXIS" = allow ]; then
  echo
  echo "verify-permissions-diff: $n cases on the allow axis, deny axis skipped"
  echo "verify-permissions-diff: $(spend_line)"
  exit $fail
fi

echo
echo "deny axis — ${#DENY_RULESETS[@]} rules × ${#DENY_SHAPES[@]} shapes"
echo
USABLE_DENY=()
for shape in "${DENY_SHAPES[@]}"; do
  if ! deny_program_present "$shape"; then
    printf 'skip   %s\n' "$shape  (that program is not installed here; not a verdict)"
    skipped=$((${skipped:-0}+1))
  elif ! deny_model_will_run "$shape"; then
    printf 'skip   %s\n' "$shape  (the model does not reliably run it unprohibited; not a verdict)"
    skipped=$((${skipped:-0}+1))
  else
    USABLE_DENY+=("$shape")
  fi
done

for rule in "${DENY_RULESETS[@]}"; do
  skip_rule "$rule" && continue
  for shape in "${USABLE_DENY[@]}"; do
    [ "$CASES" != 0 ] && [ "$n" -ge "$CASES" ] && break 2
    n=$((n+1))
    theirs=$(ask_claude_deny "[\"$rule\"]" "$shape")
    ours=$(ask_devplane_deny "$rule" "$shape")
    # A model answered the first one. Ask again before believing it.
    if [ "$theirs" != "$ours" ]; then
      # Four more chances for the evidence to appear, so a reported absence has
      # survived **five** asks.
      #
      # Three was not enough, and the arithmetic says why. Some shapes the model
      # runs about two times in three whatever the rules say —
      # `cat a b … z .env` is one — so three consecutive absences come up once
      # in twenty-seven, and a matrix of a few hundred rows finds that twice.
      # Both were reported as WIDER, the loudest thing here, and both were the
      # model. Five asks puts it at one in two hundred and forty-three, and only
      # a disagreement pays for them.
      theirs=$(claude_deny_ran_within 4 "[\"$rule\"]" "$shape")
      retried=$((${retried:-0}+1))
    fi
    if [ "$theirs" = "$ours" ]; then
      printf '  ok   %-18s %s\n' "$rule" "$shape"
    else
      # Same asymmetry as the allow axis, arrived at from the other side:
      # Devplane saying `allow` where Claude Code refused is a deny rule that
      # reads as protection and is none.
      if [ "$ours" = yes ]; then
        printf 'DIFF   %-18s %s   (claude ran=%s, devplane allow=%s)\n' \
          "$rule" "$shape" "$theirs" "$ours"
        disagreements="$disagreements\n  WIDER  $rule  ::  $shape"
        fail=1
      elif declared_narrowing "$shape"; then
        printf ' decl  %-18s %-34s %s\n' "$rule" "$shape" "($(narrowing_reason "$shape"))"
        declared=$((${declared:-0}+1))
      else
        printf 'DIFF   %-18s %s   (claude ran=%s, devplane allow=%s)\n' \
          "$rule" "$shape" "$theirs" "$ours"
        disagreements="$disagreements\n  narrower  $rule  ::  $shape"
        fail=1
      fi
    fi
  done
done

# ── The artifact ────────────────────────────────────────────────────────────
#
# **A run that leaves nothing behind is a claim somebody has to remember.**
# The measurement is the product's only unoccupied claim, and until now the only
# trace a full run left was scrollback: whoever ran it knew what it said, and
# nobody else could check. This writes the dated record — the release asked,
# the cases per axis, the shapes skipped rather than measured, and every
# declared narrowing with the reason it was declared.
#
# Skipped is the field that matters most and is the easiest to leave out. A
# skipped shape is **unmeasured, not clean**, and the skip set is not stable
# between runs because the oracle is a model — so the honest form of the claim
# is *this run measured these shapes*, never *the matrix is clean*.
record_dir="$(dirname "$0")/measurements"
mkdir -p "$record_dir"
floor=$(grep -oE 'VERIFIED_AGAINST: &str = "[0-9.]+"' "$(dirname "$0")/../src/core/policy.rs" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
asked=$(claude --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
narrowings=$(mktemp); trap 'rm -f "$narrowings"' EXIT
for e in "${EXPECTED_NARROWER[@]}"; do printf '%s\n' "$e" >> "$narrowings"; done
python3 "$(dirname "$0")/write-matrix-record.py" \
  "$record_dir/full-${floor:-unknown}.json" "${asked:-}" "$AXIS" "$n" \
  "${skipped:-0}" "${declared:-0}" "${retried:-0}" \
  "$([ "$fail" = 0 ] && echo green || echo red)" "$narrowings"

echo
if [ "$fail" = 0 ]; then
  echo "verify-permissions-diff: $n cases, no undeclared disagreements (${skipped:-0} shapes unrunnable here, ${declared:-0} declared narrowings, ${retried:-0} reproduced)"
  echo "verify-permissions-diff: $(spend_line)"
else
  # shellcheck disable=SC2059
  printf "verify-permissions-diff: $n cases (${skipped:-0} shapes unrunnable here), disagreements:$disagreements\n"
  echo
  echo "A WIDER row is a call Devplane would auto-approve and Claude Code puts"
  echo "in front of a person. That is the failure this layer exists to prevent."
fi
exit $fail
