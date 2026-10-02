// The inbox from the keyboard: every item key bound in both places an item is
// shown, printed on the control it presses, explained where it does not
// apply, and listed by `?` item first. The held answer is said with its
// seconds and its undo key. The browser keeps a few chords for itself, and a
// binding on one is refused.
import Inbox from "../../src/surfaces/inbox/Inbox.svelte";
import Answer from "../../src/surfaces/answer/Answer.svelte";
import recorded from "../../fixtures/inbox.json";
import { all, bind, combo, sheet } from "../../src/lib/keys";
import { ITEM_KEYS, whyNot } from "../../src/surfaces/inbox/keys";
import "../../src/shell/keys";
import { fail, html, source } from "../harness";

type Row = Parameters<typeof whyNot>[0];

// ── Reserved chords and collisions are refused, naming what they hit ─────
{
  const threw = (f: () => void) => {
    try {
      f();
      return "";
    } catch (e) {
      return e instanceof Error ? e.message : String(e);
    }
  };
  const reserved = threw(() => bind({ surface: "inbox", combo: "Mod+w", action: "planted", label: "a planted close" }));
  if (!reserved.includes("Mod+w")) fail(`a binding on a browser-reserved chord was not refused by name: ${reserved}`);
  const twice = threw(() => bind({ surface: "inbox", combo: "a", action: "planted", label: "a planted allow" }));
  if (!twice.includes("bound twice") || !twice.includes("allow the permission")) fail(`a planted collision does not name both bindings: ${twice}`);
  // The physical key, not the composed character: ⌥N on a Mac is `˜`.
  if (combo({ altKey: true, key: "˜", code: "KeyN", metaKey: false, ctrlKey: false, shiftKey: false }) !== "Alt+n")
    fail("a macOS-shaped Alt+n does not spell Alt+n");
  if (!all().some((b) => b.combo === "Alt+n")) fail("Alt+n is not a real binding, so the check above proves nothing");
}

// ── One item-key list, bound in the inbox and the answer window ──────────
{
  for (const scope of ["inbox", "answer"]) {
    for (const k of ITEM_KEYS) {
      const b = all().find((x) => x.surface === scope && x.combo === k.combo);
      if (!b || b.action !== k.action || !b.item) fail(`${scope} does not bind the item key ${k.combo} (${k.action})`);
    }
    for (const list of ["j", "k", "Enter"])
      if (!all().some((b) => b.surface === scope && b.combo === list)) fail(`${scope} has no list key ${list}`);
  }
  for (const scope of ["search", "setup", "why"])
    if (!all().some((b) => b.surface === scope && b.combo === "j")) fail(`${scope} has no cursor keys`);
  for (const [file, scope] of [["search/Search.svelte", "search"], ["setup/Setup.svelte", "setup"], ["answer/Answer.svelte", "answer"]])
    if (!new RegExp(`cursor\\(\\{\\s*scope: "${scope}"`).test(source(`../src/surfaces/${file}`))) fail(`${file} does not answer its list keys`);
  // No item key offers a change.
  if (ITEM_KEYS.some((k) => /offer/i.test(k.action + k.label))) fail("an inbox key offers a change");
}

// ── `?` lists this item, this surface, everywhere — in that order ────────
{
  const groups = sheet("inbox").map((g) => g.title);
  if (groups.join() !== "This item,This surface,Everywhere") fail(`the help sheet's groups are ${groups.join(", ")}`);
  const item = sheet("inbox")[0];
  const options = item.keys.find((r) => r.action === "item-option");
  if (options?.combos.join("") !== "123456789") fail("the nine option keys are not one row of the help sheet");
  if (sheet("inbox")[1].keys.some((r) => r.item)) fail("an item key is listed as the surface's");
  if (!/sheet\(current\)/.test(source("../src/App.svelte"))) fail("the help sheet is not read from the grouped registry");
}

// ── Each control prints the key that presses it ──────────────────────────
{
  const question: Row = {
    id: "q", kind: "question", level: "high", title: "Keep the route?", ask: "ask-q", actions: ["choose", "reply"],
    options: [{ id: "o1", label: "keep it" }, { id: "o2", label: "drop it" }],
  };
  const q = html(Inbox, { items: [question], loaded: true });
  for (const [n, label] of [["1", "keep it"], ["2", "drop it"]])
    if (!new RegExp(`data-key="${n}"[^>]*>${label}</button>(<!--[^>]*-->)*<kbd[^>]*>${n}</kbd>`).test(q)) fail(`option ${n} does not print its key`);
  if (!/data-key="t"/.test(q)) fail("the answer field is not reached by t");
  if (!/data-send/.test(q)) fail("the typed answer has no send for Mod+Enter to press");

  const perm: Row = { id: "p", kind: "permission", level: "high", title: "Bash: rm", ask: "ask-p", actions: ["allow", "deny"] };
  const p = html(Inbox, { items: [perm], loaded: true });
  if (!/data-key="a"[^>]*>allow<\/button><kbd[^>]*>a<\/kbd>/.test(p) || !/data-key="d"[^>]*>deny<\/button><kbd[^>]*>d<\/kbd>/.test(p))
    fail("allow and deny do not print a and d");

  const vendor: Row = {
    id: "v", kind: "permission", level: "high", title: "Permission: Bash", actions: ["open"], run_id: "r1",
    answer_in: "Claude Code owns this dialog — answer it in its own window", resume_command: "claude --resume 0b9e",
  };
  const v = html(Inbox, { items: [vendor], loaded: true });
  if (/>allow</.test(v) || /data-key="a"/.test(v)) fail("a permission the vendor owns offers allow");
  if (!/copy the resume command<\/button><kbd[^>]*>w<\/kbd>/.test(v)) fail("a vendor-owned ask does not offer w, saying it copies the resume command");
  if (!v.includes("claude --resume 0b9e")) fail("the resume command is not shown");
  const link = html(Inbox, { items: [{ ...vendor, session_link: "vscode://anthropic.claude-code/open?session=0b9e" }], loaded: true });
  if (!/open the session in VS Code<\/a><kbd[^>]*>w<\/kbd>/.test(link)) fail("a documented session link is not w, saying where it opens");
}

// ── A key that does not apply says why, in place ────────────────────────
{
  const vendor = recorded.items.find((i: Row) => i.answer_in) as Row;
  const question = recorded.items.find((i: Row) => i.kind === "question") as Row;
  const ready = recorded.items.find((i: Row) => i.kind === "ready_to_decide") as Row;
  if (!vendor || !question || !ready) fail("the inbox fixture lost a kind this check reads");
  else {
    if (!/owns this permission/.test(whyNot(vendor, "item-allow"))) fail(`a on a vendor-owned permission does not name its owner: ${whyNot(vendor, "item-allow")}`);
    if (!/never snoozed/.test(whyNot(question, "item-snooze"))) fail("s on a question does not say questions are never snoozed");
    if (!/are 2 options/.test(whyNot(question, "item-option", 3))) fail(`3 on a two-option question does not say how many there are: ${whyNot(question, "item-option", 3)}`);
    if (whyNot(ready, "item-review")) fail("r on a ready item is said not to apply");
    if (!whyNot(question, "item-review")) fail("r on a question says nothing");
    // Every key the help sheet calls out has its sentence wired.
    const keys = source("../src/surfaces/inbox/keys.ts");
    if (!/say\(whyNot\(/.test(keys)) fail("a key whose control is absent does not say why");
    if (!/answerItemKeys\(/.test(source("../src/surfaces/inbox/Item.svelte"))) fail("the item does not answer its keys");
  }
}

// ── The inbox fixture clears with at most two keys per item, no mouse ────
{
  // The keys that take each item out of the list, cheapest first.
  const exits: Array<[string, string, number]> = [
    ["item-option", "1", 1],
    ["item-allow", "a", 1],
    ["item-deny", "d", 1],
    ["item-snooze", "s", 1],
    ["item-review", "r", 1],
    ["item-session", "w", 1],
    ["item-open", "o", 1],
  ];
  for (const i of recorded.items as Row[]) {
    const way = exits.find(([action, k]) => !whyNot(i, action, Number(k) || 0));
    if (!way) fail(`"${i.title}" has no keyed way out`);
    else if (way[2] > 2) fail(`"${i.title}" takes ${way[2]} keys`);
    // And the control that key presses is on the item.
    const out = html(Inbox, { items: [i], loaded: true });
    if (way && !out.includes(`data-key="${way[1]}"`)) fail(`"${i.title}": ${way[1]} applies and nothing on the item carries it`);
  }
}

// ── The held answer: what, seconds left, z — and gone from the list ──────
{
  const until = new Date(Date.now() + 4_000).toISOString();
  const held = { ask: "ask-q", run: "r", title: "Keep the route?", says: "drop it", until, undo: "/api/asks/ask-q/undo" };
  const out = html(Inbox, { items: [{ id: "x", kind: "context_high", level: "high", title: "Context 89% full", actions: ["snooze"] }], loaded: true, held });
  if (!/Keep the route\?/.test(out) || !/drop it/.test(out)) fail("the held line does not say what is held");
  if (!/sending in \d s/.test(out)) fail("the held line does not count the seconds down");
  if (!/<kbd[^>]*>z<\/kbd> to undo/.test(out)) fail("the held line does not name z in words");
  const sent = html(Inbox, { items: [], loaded: true, landed: { ask: "a", title: "Bash: rm", ok: false, says: "deny did not land: the session ended" } });
  if (!sent.includes("did not land")) fail("an answer that did not land is silent");
  const acts = source("../src/surfaces/inbox/actions.ts");
  if (!/hold: true/.test(acts)) fail("the window's answer is not held");
  if (!/\(\?:answer\|undo\)/.test(source("../src/lib/api.ts"))) fail("taking a held answer back does not carry the window's proof");
  const win = html(Answer, { items: [{ id: "q", kind: "question", level: "high", title: "Keep?", ask: "a", actions: ["choose"], options: [{ id: "o", label: "keep" }] }] });
  if (!/<kbd[^>]*>1<\/kbd>/.test(win)) fail("the answer window does not print the option keys");
}
