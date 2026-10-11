#!/usr/bin/env bash
# Runs an ACP agent with its standard input and output passed straight
# through, and appends every complete line it sees to a transcript:
#
#   {"dir":"in","msg":<line>}    what the client sent the agent
#   {"dir":"out","msg":<line>}   what the agent sent the client
#
# Standard error passes through untouched. The exit status is the agent's.
#
#   scripts/acp-tee.sh <out.jsonl> <command…>
#
# A maintainer's tool for recording an adapter's own output; never run by the
# tests, by CI or by the host.
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <out.jsonl> <command…>" >&2
  exit 2
fi
out=$1
shift

# The program is passed with -c: a heredoc would take the agent's input.
program=$(cat <<'PY'
import json, subprocess, sys, threading

out_path, command = sys.argv[1], sys.argv[2:]
lock = threading.Lock()
out = open(out_path, "a", encoding="utf-8")

def record(direction, raw):
    line = raw.rstrip(b"\r\n")
    if not line:
        return
    try:
        msg = json.loads(line)
    except ValueError:
        # Not JSON-RPC: kept as a string so nothing is lost or invented.
        msg = line.decode("utf-8", "replace")
    with lock:
        out.write(json.dumps({"dir": direction, "msg": msg}, ensure_ascii=False) + "\n")
        out.flush()

child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)

def pump_in():
    try:
        for raw in iter(sys.stdin.buffer.readline, b""):
            record("in", raw)
            child.stdin.write(raw)
            child.stdin.flush()
    except (BrokenPipeError, ValueError):
        pass
    finally:
        try:
            child.stdin.close()
        except OSError:
            pass

def pump_out():
    for raw in iter(child.stdout.readline, b""):
        record("out", raw)
        sys.stdout.buffer.write(raw)
        sys.stdout.buffer.flush()

threading.Thread(target=pump_in, daemon=True).start()
reader = threading.Thread(target=pump_out)
reader.start()
status = child.wait()
reader.join()
sys.exit(status)
PY
)
exec python3 -c "$program" "$out" "$@"
