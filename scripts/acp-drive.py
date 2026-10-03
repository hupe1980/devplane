#!/usr/bin/env python3
"""The scripted ACP client scripts/record-acp.sh drives an adapter with, and
the two passes it runs over the transcript afterwards.

  acp-drive.py --cwd DIR [--prompt TEXT] --out RAW -- <command…>
      initialize, session/new, then (with --prompt) one prompt; every
      session/request_permission is answered with its reject option, every
      other request from the agent with *method not found*.

  acp-drive.py --rewrite SCRATCH --home HOME --in RAW --to OUT
      absolute paths -> /work, session ids -> s-1, sign-in details and
      token-shaped values dropped.

  acp-drive.py --expect SESSION --to EXPECT
      merges every report of each tool call (a present field replaces, an
      absent or null one keeps) and writes the call whose merged input names
      the marker command; prints whether its input arrived in a later update.

A maintainer's tool; never run by the tests, by CI or by the host.
"""

import argparse, json, re, subprocess, sys, threading, time

MARKER = "devplane-replay-marker"
# What Devplane's client declares at `initialize`.
CLIENT_CAPABILITIES = {
    "fs": {"readTextFile": False, "writeTextFile": False},
    "terminal": False,
    "elicitation": {"form": {}},
}


def drive(args, command):
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    lock = threading.Lock()
    waiting = {}
    next_id = [0]

    def send(obj):
        with lock:
            child.stdin.write((json.dumps(obj) + "\n").encode())
            child.stdin.flush()

    def call(method, params, timeout):
        next_id[0] += 1
        rid = next_id[0]
        done = threading.Event()
        waiting[rid] = [done, None]
        send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        if not done.wait(timeout):
            raise SystemExit(f"{method} was not answered in {timeout}s")
        reply = waiting.pop(rid)[1]
        if "error" in reply:
            raise SystemExit(f"{method} failed: {json.dumps(reply['error'])}")
        return reply["result"]

    def reader():
        for raw in iter(child.stdout.readline, b""):
            try:
                msg = json.loads(raw)
            except ValueError:
                continue
            if "method" in msg and "id" in msg:
                if msg["method"] == "session/request_permission":
                    options = msg["params"].get("options", [])
                    reject = next((o for o in options if o.get("kind") == "reject_once"), None) or next(
                        (o for o in options if str(o.get("kind", "")).startswith("reject")), None
                    )
                    outcome = (
                        {"outcome": "selected", "optionId": reject["optionId"]}
                        if reject
                        else {"outcome": "cancelled"}
                    )
                    send({"jsonrpc": "2.0", "id": msg["id"], "result": {"outcome": outcome}})
                else:
                    send(
                        {
                            "jsonrpc": "2.0",
                            "id": msg["id"],
                            "error": {"code": -32601, "message": "method not found"},
                        }
                    )
            elif "id" in msg and msg["id"] in waiting:
                waiting[msg["id"]][1] = msg
                waiting[msg["id"]][0].set()

    threading.Thread(target=reader, daemon=True).start()
    try:
        call(
            "initialize",
            {"protocolVersion": 1, "clientCapabilities": CLIENT_CAPABILITIES},
            300,
        )
        session = call("session/new", {"cwd": args.cwd, "mcpServers": []}, 300)
        if args.prompt:
            call(
                "session/prompt",
                {
                    "sessionId": session["sessionId"],
                    "prompt": [{"type": "text", "text": args.prompt}],
                },
                600,
            )
    finally:
        try:
            child.stdin.close()
        except OSError:
            pass
        deadline = time.time() + 10
        while child.poll() is None and time.time() < deadline:
            time.sleep(0.1)
        if child.poll() is None:
            child.kill()


TOKEN = re.compile(
    r"(sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"
    r"|[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})"
)


def rewrite(args):
    lines = [json.loads(l) for l in open(args.__dict__["in"], encoding="utf-8") if l.strip()]
    sessions = set()
    for entry in lines:
        msg = entry["msg"]
        if isinstance(msg, dict):
            for holder in (msg.get("params") or {}, msg.get("result") or {}):
                if isinstance(holder, dict) and isinstance(holder.get("sessionId"), str):
                    sessions.add(holder["sessionId"])
    roots = sorted(
        {args.rewrite, args.rewrite.replace("/private", "", 1), "/private" + args.rewrite, args.home},
        key=len,
        reverse=True,
    )

    def scrub(value, key=None):
        if isinstance(value, dict):
            out = {}
            for k, v in value.items():
                if k == "authMethods" and isinstance(v, list):
                    out[k] = [{"id": m.get("id"), "name": m.get("name")} for m in v if isinstance(m, dict)]
                else:
                    out[k] = scrub(v, k)
            return out
        if isinstance(value, list):
            return [scrub(v) for v in value]
        if isinstance(value, str):
            for s in sessions:
                value = value.replace(s, "s-1")
            for r in roots:
                value = value.replace(r + "/work", "/work").replace(r, "/work")
            return TOKEN.sub("<dropped>", value)
        return value

    with open(args.to, "w", encoding="utf-8") as out:
        for entry in lines:
            out.write(json.dumps(scrub(entry), ensure_ascii=False) + "\n")


FIELDS = ("title", "kind", "status", "rawInput", "locations")


def expect(args):
    calls, first_input, later_input, asked = {}, {}, set(), {}
    for line in open(args.expect, encoding="utf-8"):
        entry = json.loads(line)
        msg = entry["msg"]
        if entry["dir"] != "out" or not isinstance(msg, dict):
            continue
        report = None
        if msg.get("method") == "session/update":
            update = msg["params"]["update"]
            if update.get("sessionUpdate") in ("tool_call", "tool_call_update"):
                report = update
        elif msg.get("method") == "session/request_permission":
            report = msg["params"]["toolCall"]
        if report is None:
            continue
        cid = report["toolCallId"]
        merged = calls.setdefault(cid, {})
        if cid not in first_input:
            first_input[cid] = report.get("rawInput")
        elif report.get("rawInput") is not None and report.get("rawInput") != first_input[cid]:
            later_input.add(cid)
        for f in FIELDS:
            if report.get(f) is not None:
                merged[f] = report[f]
        if msg.get("method") == "session/request_permission":
            asked[cid] = dict(merged)
    marker = args.marker or MARKER
    for cid, merged in asked.items():
        raw = merged.get("rawInput")
        if raw is not None and marker in json.dumps(raw):
            command = raw.get("command") if isinstance(raw, dict) else None
            json.dump(
                {"call_id": cid, "title": merged.get("title"), "command": command},
                open(args.to, "w", encoding="utf-8"),
                indent=2,
            )
            print("yes" if cid in later_input else "no")
            return
    # An adapter that sends no input at all: the call is found by its title,
    # and `command` is null — no rule can match what never arrived.
    for cid, merged in asked.items():
        if merged.get("rawInput") is None and marker in str(merged.get("title")):
            json.dump(
                {"call_id": cid, "title": merged.get("title"), "command": None},
                open(args.to, "w", encoding="utf-8"),
                indent=2,
            )
            print("no (the adapter sent no input for the call)")
            return
    raise SystemExit("no permission request in the transcript names the marker command")


def main():
    argv = sys.argv[1:]
    command = []
    if "--" in argv:
        i = argv.index("--")
        argv, command = argv[:i], argv[i + 1 :]
    p = argparse.ArgumentParser()
    p.add_argument("--cwd")
    p.add_argument("--prompt")
    p.add_argument("--out")
    p.add_argument("--rewrite")
    p.add_argument("--home")
    p.add_argument("--in")
    p.add_argument("--to")
    p.add_argument("--expect")
    p.add_argument("--marker")
    args = p.parse_args(argv)
    if args.rewrite:
        rewrite(args)
    elif args.expect:
        expect(args)
    else:
        drive(args, command)


if __name__ == "__main__":
    main()
