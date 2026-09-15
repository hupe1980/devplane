#!/usr/bin/env bash
# Differential testing: does Vibeplane's matcher agree with the running Claude
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
# `cases` caps how many are run, for a quick pass. `VIBEPLANE_DIFF_AXIS=allow`
# or `=deny` runs one half; the default runs both.
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
# `vibeplane says allow`, and all three of Vibeplane's non-allow verdicts end
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
AXIS="${VIBEPLANE_DIFF_AXIS:-both}"   # allow | deny | both

CLAUDE="${VIBEPLANE_CLAUDE_BIN:-$(command -v claude || true)}"
if [ -z "$CLAUDE" ]; then
  for d in "$HOME/.claude/local/claude" \
           "$HOME"/.vscode/extensions/anthropic.claude-code-*/resources/native-binary/claude; do
    [ -x "$d" ] && CLAUDE="$d"
  done
fi
[ -x "${CLAUDE:-}" ] || { echo "no claude binary found; set VIBEPLANE_CLAUDE_BIN"; exit 2; }
"$CLAUDE" auth status >/dev/null 2>&1 || { echo "claude is not signed in; nothing to ask"; exit 2; }

VP="${VIBEPLANE_BIN:-target/debug/vibeplane}"
[ -x "$VP" ] || cargo build -q || exit 2

MODEL="${VIBEPLANE_PROBE_MODEL:-claude-haiku-4-5-20251001}"
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
  'git blame --ignore-revs-file=.env README.md' # 2.1.266, option values
  'grep -f.env README.md'                      # attached option value
  'grep -r key secrets'                        # 2.1.268, recursion
  'cp -r secrets /tmp/vp-secrets-copy'
  'env -C . cat .env'                          # 2.1.268, barriers
  'eval "cat .env"'
  'sudo -n cat .env'
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
  # Still unmeasured, and in the table because the errors are not symmetric: a
  # WIDER row fails the release and a narrower one is reported, so a candidate
  # stays until a measurement takes it out. These are the measurements.
  'uniq .env > /dev/null'
  'expand .env > /dev/null'
  'column .env > /dev/null'
  # Writing **inside** the working directory, so the probe isolates the deny
  # rule rather than the product's separate check on a write outside it.
  'split -l 1 .env vp-split- > /dev/null'
  'diff .env .env > /dev/null'
  'cmp .env .env > /dev/null'
  'wc -l .env > /dev/null'
  'mv .env .env.bak'
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

# Narrowings this project chose, with the reason. A row here is reported as a
# declared divergence rather than as a finding — but only ever a *narrowing*: a
# WIDER row is a call Vibeplane would auto-approve and the product would not,
# and nothing may excuse one.
#
# There is one rule behind every row, rather than four case-by-case choices:
# **Vibeplane is exactly as strict as Claude Code, except for commands that
# exist to defeat text matching, where it is stricter by declaration.** `eval`,
# `sudo`, `doas` and `exec` take a command and run it under another name; the
# running product treats what they are handed as opaque, and a `never_auto`
# rule that quoting steps around is not a prohibition. The cost is a prompt,
# never a refusal.
# One row per barrier shape in DENY_SHAPES. The list was short of `env` for a
# run, which is the failure mode a declared-divergence list has: the rule above
# covered it all along and the enumeration did not, so a row that was a *choice*
# was reported as a *finding*. A shape belongs here when `ANALYSIS_BARRIERS`
# names its program — nothing else may be added.
EXPECTED_NARROWER=(
  'eval "cat .env"'
  'env -C . cat .env'
  'sudo -n cat .env'
  'exec cat .env'
  'doas cat .env'
)

declared_narrowing() { # shape -> 0 when declared
  for e in "${EXPECTED_NARROWER[@]}"; do
    [ "$e" = "$1" ] && return 0
  done
  return 1
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

ask_claude_deny() { # deny-rule command -> yes|no   (yes = it ran, i.e. not blocked)
  deny_reset
  full="$2 ; git config --local vp.ran 1"
  ( cd "$W" && "$CLAUDE" -p "Run this exact bash command and nothing else, then stop: $full" \
      --settings "{\"permissions\":{\"allow\":[\"Bash($(json_escape "$full"))\"],\"deny\":[\"$1\"],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose >/dev/null 2>&1 )
  deny_ran
}

ask_vibeplane_deny() { # deny-rule command -> yes|no
  full="$2 ; git config --local vp.ran 1"
  {
    printf '[project]\nname = "diff"\n\n[policy]\n'
    printf 'never_auto = ["%s"]\n' "$1"
    printf 'auto_allow = ["Bash(%s)"]\n' "$(printf '%s' "$full" | sed 's/"/\\"/g')"
  } > "$W/vibeplane.toml"
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
  case " ${WRITE_SHAPES[*]} " in
    *" $1 "*) [ -f "$W/ran.txt" ] && echo yes || echo no ;;
    *) git -C "$W" config --get vp.ran >/dev/null 2>&1 && echo yes || echo no ;;
  esac
}

reset() {
  rm -f "$W"/ran.txt
  git -C "$W" config --unset-all vp.ran 2>/dev/null
  printf 'ran.txt\n' > "$W/names.txt"
}

# The extra grant a write shape needs, and nothing else gets.
extra_allow() { # shape -> json fragment
  case " ${WRITE_SHAPES[*]} " in
    *" $1 "*) printf ',"Edit(ran.txt)"' ;;
    *) printf '' ;;
  esac
}

runnable() { # command -> 0|1
  reset
  ( cd "$W" && eval "$1" ) >/dev/null 2>&1
  [ "$(ran "$1")" = yes ]
}

ask_claude() { # ruleset command -> yes|no
  reset
  ( cd "$W" && "$CLAUDE" -p "Run this exact bash command and nothing else, then stop: $2" \
      --settings "{\"permissions\":{\"allow\":[\"$1\"$(extra_allow "$2")],\"deny\":[],\"ask\":[]}}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose >/dev/null 2>&1 )
  ran "$2"
}

ask_vibeplane() { # ruleset command -> yes|no
  printf '[project]\nname = "diff"\n\n[policy]\nauto_allow = ["%s"]\n' "$1" > "$W/vibeplane.toml"
  v=$("$VP" --json explain --dir "$W" "$2" 2>/dev/null \
        | python3 -c 'import json,sys; print(json.load(sys.stdin)["verdict"])' 2>/dev/null)
  [ "$v" = allow ] && echo yes || echo no
}

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
  for shape in "${USABLE[@]}"; do
    [ "$CASES" != 0 ] && [ "$n" -ge "$CASES" ] && break 2
    n=$((n+1))
    theirs=$(ask_claude "$rule" "$shape")
    ours=$(ask_vibeplane "$rule" "$shape")
    if [ "$theirs" != "$ours" ]; then
      theirs=$(ask_claude "$rule" "$shape")
      retried=$((${retried:-0}+1))
    fi
    if [ "$theirs" = "$ours" ]; then
      printf '  ok   %-22s %s\n' "$rule" "$shape"
    else
      printf 'DIFF   %-22s %s   (claude ran=%s, vibeplane allow=%s)\n' \
        "$rule" "$shape" "$theirs" "$ours"
      # A widening is the one that matters: Vibeplane answering the prompt with
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
  exit $fail
fi

echo
echo "deny axis — ${#DENY_RULESETS[@]} rules × ${#DENY_SHAPES[@]} shapes"
echo
USABLE_DENY=()
for shape in "${DENY_SHAPES[@]}"; do
  if deny_program_present "$shape"; then
    USABLE_DENY+=("$shape")
  else
    printf 'skip   %s\n' "$shape  (that program is not installed here; not a verdict)"
    skipped=$((${skipped:-0}+1))
  fi
done

for rule in "${DENY_RULESETS[@]}"; do
  for shape in "${USABLE_DENY[@]}"; do
    [ "$CASES" != 0 ] && [ "$n" -ge "$CASES" ] && break 2
    n=$((n+1))
    theirs=$(ask_claude_deny "$rule" "$shape")
    ours=$(ask_vibeplane_deny "$rule" "$shape")
    # A model answered the first one. Ask again before believing it.
    if [ "$theirs" != "$ours" ]; then
      theirs=$(ask_claude_deny "$rule" "$shape")
      retried=$((${retried:-0}+1))
    fi
    if [ "$theirs" = "$ours" ]; then
      printf '  ok   %-18s %s\n' "$rule" "$shape"
    else
      # Same asymmetry as the allow axis, arrived at from the other side:
      # Vibeplane saying `allow` where Claude Code refused is a deny rule that
      # reads as protection and is none.
      if [ "$ours" = yes ]; then
        printf 'DIFF   %-18s %s   (claude ran=%s, vibeplane allow=%s)\n' \
          "$rule" "$shape" "$theirs" "$ours"
        disagreements="$disagreements\n  WIDER  $rule  ::  $shape"
        fail=1
      elif declared_narrowing "$shape"; then
        printf ' decl  %-18s %s   (stricter on purpose)\n' "$rule" "$shape"
        declared=$((${declared:-0}+1))
      else
        printf 'DIFF   %-18s %s   (claude ran=%s, vibeplane allow=%s)\n' \
          "$rule" "$shape" "$theirs" "$ours"
        disagreements="$disagreements\n  narrower  $rule  ::  $shape"
        fail=1
      fi
    fi
  done
done

echo
if [ "$fail" = 0 ]; then
  echo "verify-permissions-diff: $n cases, no undeclared disagreements (${skipped:-0} shapes unrunnable here, ${declared:-0} declared narrowings, ${retried:-0} reproduced)"
else
  # shellcheck disable=SC2059
  printf "verify-permissions-diff: $n cases (${skipped:-0} shapes unrunnable here), disagreements:$disagreements\n"
  echo
  echo "A WIDER row is a call Vibeplane would auto-approve and Claude Code puts"
  echo "in front of a person. That is the failure this layer exists to prevent."
fi
exit $fail
