#!/usr/bin/env bash
# Does Claude Code decide the way Devplane decides?
#
# `scripts/verify-claims.sh` pins every rule of the permission syntax to a line
# in the vendored specification. This script asks the *running product* instead,
# because a specification is a description and the thing that matters is the
# behaviour. It exists because reading the spec against the implementation found
# three disagreements — all silent, two of them widening — and being
# right about the spec is not the same as being right.
#
# It costs a few cents and needs a signed-in Claude Code. It is NOT part of CI:
# CI has no credentials and should not spend money. Run it when Claude Code
# changes in a way that touches permissions.
#
#   scripts/verify-permissions-live.sh
#
# What it does to your machine: nothing. It works in a scratch directory, passes
# its own permission rules with `--settings` so your `settings.json` is neither
# read for rules nor written, and runs `touch` on files inside that directory.
#
# ---------------------------------------------------------------------------
# Two traps, both of which produced a confident wrong answer before they were
# noticed, and both of which are why the controls below are not optional:
#
#   * `echo` is in Claude Code's built-in **read-only command set** and runs
#     with no permission check at all. A probe built on `echo` passes whatever
#     the rules say, and proves nothing.
#   * `--allowedTools Bash` pre-approves the whole tool, so every rule is
#     irrelevant. The probes must let `permissions.allow` do the work alone.
#
# The oracle is `--permission-mode dontAsk`: a call runs if and only if a rule
# (or the read-only set) covers it, with nobody to prompt. `touch` is used
# because it needs permission, is harmless, and leaves evidence on disk that
# does not depend on reading the model's prose.
set -u
cd "$(dirname "$0")/.." || exit 1

CLAUDE="${DEVPLANE_CLAUDE_BIN:-$(command -v claude || true)}"
if [ -z "$CLAUDE" ]; then
  for d in "$HOME/.claude/local/claude" \
           "$HOME"/.vscode/extensions/anthropic.claude-code-*/resources/native-binary/claude; do
    [ -x "$d" ] && CLAUDE="$d"
  done
fi
[ -x "${CLAUDE:-}" ] || { echo "no claude binary found; set DEVPLANE_CLAUDE_BIN"; exit 2; }
"$CLAUDE" auth status >/dev/null 2>&1 || { echo "claude is not signed in; nothing to ask"; exit 2; }

MODEL="${DEVPLANE_PROBE_MODEL:-claude-haiku-4-5-20251001}"
W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
fail=0; n=0

# probe <expect ran: yes|no> <label> <permissions json> <command>
probe() {
  want=$1; label=$2; perms=$3; cmd=$4
  n=$((n+1))
  rm -f "$W"/*.txt 2>/dev/null
  ( cd "$W" && "$CLAUDE" -p "Run this exact bash command and nothing else, then stop: $cmd" \
      --settings "{\"permissions\":$perms}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose >/dev/null 2>&1 )
  got=no; [ -n "$(ls "$W" 2>/dev/null | grep '\.txt$')" ] && got=yes
  if [ "$got" = "$want" ]; then
    printf 'OK   %s\n' "$label"
  else
    printf 'MISS %s  (expected ran=%s, got ran=%s)\n' "$label" "$want" "$got"
    fail=1
  fi
}

echo "Asking Claude Code ($("$CLAUDE" --version 2>/dev/null | head -1)) with model $MODEL"
echo

# Controls first. A harness that cannot tell allowed from refused makes every
# line under it meaningless, and both of these have been wrong.
probe yes "control: an allow rule lets it run" \
      '{"allow":["Bash(touch alpha*)"],"deny":[],"ask":[]}' 'touch alpha-pos.txt'
probe no  "control: nothing covers it, so it does not run" \
      '{"allow":["Bash(touch alpha*)"],"deny":[],"ask":[]}' 'touch beta-neg.txt'

# D79 — a rule is matched per subcommand, and the sides are asymmetric.
probe no  "D79: an allow does not approve a compound command it half-covers" \
      '{"allow":["Bash(touch alpha*)"],"deny":[],"ask":[]}' 'touch alpha-1.txt && touch beta-1.txt'
probe no  "D79: a deny matching one subcommand blocks the whole line" \
      '{"allow":["Bash(touch *)"],"deny":["Bash(touch beta*)"],"ask":[]}' 'touch alpha-2.txt && touch beta-2.txt'
probe no  "D79: an allow does not look past a non-safe assignment" \
      '{"allow":["Bash(touch alpha*)"],"deny":[],"ask":[]}' 'FOO=bar touch alpha-4.txt'

# D80 — the third list, and that it outranks a broader allow.
probe no  "D80: an ask rule outranks the allow beside it" \
      '{"allow":["Bash(touch *)"],"deny":[],"ask":["Bash(touch beta*)"]}' 'touch beta-3.txt'

# ---------------------------------------------------------------------------
# Path rules. These are the shapes most likely to be subtly wrong, because the
# anchor is invisible in the rule text and the allow/deny reading of the *same*
# pattern differs.
#
# `Read` cannot be probed this way: a read inside the working directory needs no
# approval, so it runs whatever the rules say — the same trap `echo` sets for
# commands. Writing does need approval, which is also what proves the claim that
# one `Edit(…)` rule covers every built-in tool that writes files.
writeprobe() { # <expect wrote: yes|no> <label> <permissions json> <relative path>
  want=$1; label=$2; perms=$3; rel=$4
  n=$((n+1))
  rm -rf "$W/tree"; mkdir -p "$W/tree/src" "$W/tree/nested/src" "$W/tree/nested/secrets"
  ( cd "$W/tree" && "$CLAUDE" -p "Use the Write tool to create the file $rel containing the single word ok. Do nothing else, then stop." \
      --settings "{\"permissions\":$perms}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose >/dev/null 2>&1 )
  got=no; [ -f "$W/tree/$rel" ] && got=yes
  if [ "$got" = "$want" ]; then
    printf 'OK   %s\n' "$label"
  else
    printf 'MISS %s  (expected wrote=%s, got wrote=%s)\n' "$label" "$want" "$got"
    fail=1
  fi
}

writeprobe yes "control: one Edit(…) rule covers the Write tool" \
      '{"allow":["Edit(src/**)"],"deny":[],"ask":[]}' 'src/a.txt'
writeprobe no  "control: a path the rule does not cover is refused" \
      '{"allow":["Edit(src/**)"],"deny":[],"ask":[]}' 'other/b.txt'
writeprobe no  "paths: a single-segment directory ANCHORS as an allow" \
      '{"allow":["Edit(src/**)"],"deny":[],"ask":[]}' 'nested/src/c.txt'
writeprobe no  "paths: the same pattern FLOATS as a deny, to any depth" \
      '{"allow":["Edit(nested/**)"],"deny":["Edit(secrets/**)"],"ask":[]}' 'nested/secrets/d.txt'

# ---------------------------------------------------------------------------
# The files a shell command touches.
#
# Claude Code checks a redirection's target against the `Edit` rules "as if
# Claude wrote or read that file directly", and applies `Read`/`Edit` deny
# rules to the operands of the file commands it recognises. Devplane matched a
# `Bash` rule against the command *text* only, so `Read(.env)` did not stop
# `cat .env` and `Edit(.env)` did not stop `echo x > .env` — two prohibitions
# that read as protection and were none.
#
# `echo` is deliberately the command here, and it is the trap from the header
# turned into the probe: `echo` alone needs no permission, so if a redirect
# added no check the write would land whatever the rules said. That it does
# *not* land is the whole evidence.
# ---------------------------------------------------------------------------

# shellprobe <expect file exists: yes|no> <label> <permissions json> <command> <path>
shellprobe() {
  local want="$1" label="$2" perms="$3" cmd="$4" rel="$5"
  n=$((n + 1))
  rm -f "$W/tree/$rel"
  mkdir -p "$(dirname "$W/tree/$rel")"
  ( cd "$W/tree" && "$CLAUDE" -p "Run exactly this shell command with the Bash tool and nothing else, then stop: $cmd" \
      --settings "{\"permissions\":$perms}" \
      --permission-mode dontAsk --model "$MODEL" \
      --output-format stream-json --verbose >/dev/null 2>&1 )
  got=no; [ -s "$W/tree/$rel" ] && got=yes
  if [ "$got" = "$want" ]; then
    printf 'OK   %s\n' "$label"
  else
    printf 'MISS %s  (expected wrote=%s, got wrote=%s)\n' "$label" "$want" "$got"
    fail=1
  fi
}

shellprobe yes "control: a redirect with no rule against it writes the file" \
      '{"allow":["Bash(echo *)"],"deny":[],"ask":[]}' 'echo ok > out.txt' 'out.txt'
shellprobe no  "shell: an Edit deny covers a redirection target" \
      '{"allow":["Bash(echo *)"],"deny":["Edit(secret.txt)"],"ask":[]}' 'echo ok > secret.txt' 'secret.txt'
shellprobe no  "shell: a Read deny also covers writing that file" \
      '{"allow":["Bash(echo *)"],"deny":["Read(secret.txt)"],"ask":[]}' 'echo ok > secret.txt' 'secret.txt'
shellprobe no  "shell: an allow for the command does not cover a target outside the tree" \
      '{"allow":["Bash(echo *)"],"deny":[],"ask":[]}' 'echo ok > ../escaped.txt' '../escaped.txt'

echo
echo "verify-permissions-live: $n probes, exit $fail"
exit $fail
