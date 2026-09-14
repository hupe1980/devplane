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
set -u
cd "$(dirname "$0")/.." || exit 1

CASES="${1:-0}"   # 0 = the full built-in matrix

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

echo "differential permission check — ${#RULESETS[@]} rule sets × $((${#SHAPES[@]} + ${#WRITE_SHAPES[@]})) shapes"
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

echo
if [ "$fail" = 0 ]; then
  echo "verify-permissions-diff: $n cases, no disagreements (${skipped:-0} shapes unrunnable here)"
else
  # shellcheck disable=SC2059
  printf "verify-permissions-diff: $n cases (${skipped:-0} shapes unrunnable here), disagreements:$disagreements\n"
  echo
  echo "A WIDER row is a call Vibeplane would auto-approve and Claude Code puts"
  echo "in front of a person. That is the failure this layer exists to prevent."
fi
exit $fail
