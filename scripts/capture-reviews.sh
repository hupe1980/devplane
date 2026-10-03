#!/usr/bin/env bash
# Records GitHub's own answers for one pull request's reviews and its base
# branch's review requirement, as the test fixture of who approved a merge:
#
#   tests/fixtures/github/merge-decider/reviews.json     every page, joined
#   tests/fixtures/github/merge-decider/rules.json       GET rules/branches/<base>
#   tests/fixtures/github/merge-decider/protection.json  GET branches/<base>/protection
#                                                        (or {"status", "message"})
#   tests/fixtures/github/merge-decider/SOURCE           what was recorded, and when
#
# Usage: scripts/capture-reviews.sh <owner/repo> <pr-number> [--force]
#
# The pull request should carry a Copilot code review approval and a person's
# approval dismissed by a later push. Read-only: every request is a GET.
# The token is read from $GITHUB_TOKEN, or asked for without echo; it is
# passed to curl through a file descriptor, never as an argument, and is
# written nowhere. Never edit the recorded files by hand: record again.
set -euo pipefail

repo="${1:-}"
pr="${2:-}"
force="${3:-}"
case "$repo" in */*) ;; *) echo "usage: $0 <owner/repo> <pr-number> [--force]" >&2; exit 2 ;; esac
case "$pr" in ''|*[!0-9]*) echo "usage: $0 <owner/repo> <pr-number> [--force]" >&2; exit 2 ;; esac
case "$repo" in *[!A-Za-z0-9._/-]*) echo "not a repository name: $repo" >&2; exit 2 ;; esac

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/tests/fixtures/github/merge-decider"
if [ -e "$out/SOURCE" ] && [ "$force" != "--force" ]; then
  echo "$out already holds a recording; pass --force to replace it" >&2
  exit 2
fi
api="${GITHUB_API_URL:-https://api.github.com}"
version="2022-11-28"

token="${GITHUB_TOKEN:-}"
if [ -z "$token" ]; then
  printf 'GitHub token (repo read; not echoed): ' >&2
  IFS= read -rs token
  echo >&2
fi
[ -n "$token" ] || { echo "no token" >&2; exit 2; }

# One GET; the body to stdout, the status to fd 3.
get() {
  curl --proto '=https' -sS -o - -w '%{http_code}' \
    -H @<(printf 'Authorization: Bearer %s\n' "$token") \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: $version" \
    "$api$1"
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# The pull request: its head commit and base branch.
get "/repos/$repo/pulls/$pr" > "$tmp/pr.raw"
body="$(cat "$tmp/pr.raw")"
status="${body: -3}"
printf '%s' "${body%???}" > "$tmp/pr.json"
[ "$status" = "200" ] || { echo "the pull request could not be read ($status)" >&2; exit 1; }
head="$(jq -r '.head.sha' "$tmp/pr.json")"
base="$(jq -r '.base.ref' "$tmp/pr.json")"

# Every page of reviews, to GitHub's maximum page size, joined in order.
echo '[]' > "$tmp/reviews.json"
page=1
while :; do
  get "/repos/$repo/pulls/$pr/reviews?per_page=100&page=$page" > "$tmp/page.raw"
  raw="$(cat "$tmp/page.raw")"
  status="${raw: -3}"
  [ "$status" = "200" ] || { echo "the reviews could not be read ($status)" >&2; exit 1; }
  printf '%s' "${raw%???}" > "$tmp/page.json"
  jq -s '.[0] + .[1]' "$tmp/reviews.json" "$tmp/page.json" > "$tmp/joined.json"
  mv "$tmp/joined.json" "$tmp/reviews.json"
  n="$(jq 'length' "$tmp/page.json")"
  [ "$n" -lt 100 ] && break
  page=$((page + 1))
done

# A refusal is kept as GitHub said it, with its status.
keep() {
  local path="$1" file="$2" raw status
  get "$path" > "$tmp/x.raw"
  raw="$(cat "$tmp/x.raw")"
  status="${raw: -3}"
  printf '%s' "${raw%???}" > "$tmp/x.json"
  if [ "$status" = "200" ]; then
    jq . "$tmp/x.json" > "$tmp/$file"
  else
    jq --argjson s "$status" '{status: $s, message: (.message // "")}' "$tmp/x.json" > "$tmp/$file"
  fi
}
keep "/repos/$repo/rules/branches/$base?per_page=100" rules.json
keep "/repos/$repo/branches/$base/protection" protection.json

mkdir -p "$out"
jq . "$tmp/reviews.json" > "$out/reviews.json"
cp "$tmp/rules.json" "$out/rules.json"
cp "$tmp/protection.json" "$out/protection.json"
cat > "$out/SOURCE" <<EOF
repository = $repo
pull_request = $pr
head = $head
base = $base
api = $api
api_version = $version
captured = $(date -u +%Y-%m-%dT%H:%M:%SZ)
by = scripts/capture-reviews.sh
EOF
echo "recorded $repo#$pr ($head on $base) to $out"
