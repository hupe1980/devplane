// What a server render cannot show but a function or the source can: how a
// key event is read (a macOS ⌥ chord included), that a focused control keeps
// its own Enter, which links are allowed, how a review mark is keyed, and the
// source shapes that keep a page's state through a poll. The browser check
// (`tests/browser.mjs`) drives the same bugs in a real page.
import { all, bind, combo, dispatch, fromField, onAction, run, spell } from "../../src/lib/keys";
import "../../src/shell/keys";
import { safeHref } from "../../src/lib/href";
import { markKey } from "../../src/surfaces/change/review/marks";
import Failed from "../../src/lib/Failed.svelte";
import { fail, html, source, visible, walk } from "../harness";

type Ev = Parameters<typeof combo>[0];
const key = (e: Partial<Ev>): Ev => ({ key: "", code: "", metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...e });

// ── ⌥ chords are read from the physical key ──────────────────────────────
{
  // What macOS Chrome and WKWebView send: Option composes a character.
  const mac: Array<[Ev, string]> = [
    [key({ altKey: true, key: "Dead", code: "KeyN" }), "Alt+n"],
    [key({ altKey: true, key: "∑", code: "KeyW" }), "Alt+w"],
    [key({ altKey: true, key: "‘", code: "BracketRight" }), "Alt+]"],
    [key({ altKey: true, key: "“", code: "BracketLeft" }), "Alt+["],
  ];
  for (const [ev, want] of mac) {
    const got = combo(ev);
    if (got !== want) fail(`⌥ with e.key "${ev.key}" (${ev.code}) spells ${got}, not ${want}`);
    if (!all().some((b) => b.combo === want)) fail(`${want} is no binding, so this check guards nothing`);
  }
  // Without Alt the character is the key: `G` and `?` spell themselves.
  if (combo(key({ key: "G", code: "KeyG", shiftKey: true })) !== "G") fail("Shift+g does not spell G");
  if (combo(key({ key: "?", code: "Slash", shiftKey: true })) !== "?") fail("? does not spell ?");
  if (combo(key({ key: "k", code: "KeyK", metaKey: true })) !== "Mod+k") fail("⌘K does not spell Mod+k");
  // The help sheet spells a chord the way the keyboard is labelled.
  const alt = spell("Alt+n");
  if (alt !== "Alt+N" && alt !== "⌥N") fail(`Alt+n is spelled ${alt} in the help`);
}

// ── A focused control keeps Enter; only a key a listener took is kept ────
{
  const off = onAction("open", (surface) => surface === "inbox");
  let prevented = 0;
  const on = (tag: string, matches: boolean) =>
    ({
      key: "Enter",
      code: "Enter",
      metaKey: false,
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
      isComposing: false,
      target: { tagName: tag, isContentEditable: false, closest: () => (matches ? {} : null) },
      preventDefault: () => (prevented += 1),
    }) as unknown as KeyboardEvent;
  if (dispatch("inbox", on("BUTTON", true))) fail("Enter on a focused button was taken by the list's open");
  if (prevented !== 0) fail("Enter on a focused button was prevented, so the button never fires");
  if (!dispatch("inbox", on("DIV", false))) fail("Enter on the page did not reach the list's open");
  if (prevented !== 1) fail("a key a listener took was not kept from the page");
  // A key nobody answered is not swallowed.
  prevented = 0;
  if (dispatch("board", on("DIV", false)) || prevented) fail("Enter where no list listens was swallowed");
  off();
  const declines = onAction("check-decline", () => false);
  if (run("check-decline", "inbox")) fail("run() says a key was taken when every listener declined");
  declines();
}

// ── A text field owns its keys: no bare key and no ⌥ chord leaves it ─────
//
// On a Mac ⌥3 types `£` and ⌥W types `∑`; taken by the registry they would
// switch to the review or close the tab under a half-typed draft.
{
  // The real ⌥3 and ⌥W, whose listeners live in components this render
  // never mounts, and a scope of this check's own for the rest.
  bind({ surface: "field-check", combo: "j", action: "field-check-bare", label: "a key a field must keep" });
  bind({ surface: "field-check", combo: "Mod+e", action: "field-check-mod", label: "a command chord" });
  const took: string[] = [];
  const offs = ["view-review", "close-tab", "field-check-bare", "field-check-mod"].map((a) =>
    onAction(a, () => (took.push(a), true)),
  );
  const ev = (e: Partial<Ev>, target: { tagName: string; isContentEditable?: boolean }) =>
    ({
      ...key(e),
      isComposing: false,
      target: { isContentEditable: false, closest: () => null, ...target },
      preventDefault: () => {},
    }) as unknown as KeyboardEvent;
  const fields = [{ tagName: "INPUT" }, { tagName: "TEXTAREA" }, { tagName: "DIV", isContentEditable: true }];
  const kept: Array<[string, string, Partial<Ev>]> = [
    ["⌥3", "change", { altKey: true, key: "£", code: "Digit3" }],
    ["⌥W", "change", { altKey: true, key: "∑", code: "KeyW" }],
    ["j", "field-check", { key: "j", code: "KeyJ" }],
  ];
  for (const t of fields) {
    const where = t.isContentEditable ? "a contenteditable" : t.tagName === "INPUT" ? "an input" : "a textarea";
    for (const [name, surface, e] of kept) {
      took.length = 0;
      if (dispatch(surface, ev(e, t)) || took.length) fail(`${name} in ${where} was taken by the registry (${took.join()})`);
    }
    took.length = 0;
    if (!dispatch("field-check", ev({ key: "e", code: "KeyE", metaKey: true }, t)) || !took.includes("field-check-mod"))
      fail(`a bound ⌘ chord in ${where} does not fire`);
  }
  if (!fromField("Esc")) fail("Esc in a field no longer leaves");
  if (!fromField("Mod+k")) fail("⌘K in a field no longer opens the palette");
  // The same keys on the page still fire: the field is the only thing that
  // keeps them.
  for (const [name, surface, e] of kept) {
    took.length = 0;
    if (!dispatch(surface, ev(e, { tagName: "DIV" })) || took.length !== 1) fail(`${name} on the page does nothing`);
  }
  offs.forEach((off) => off());
}

// ── Host links: an allowlist of schemes, in one helper ───────────────────
{
  for (const bad of ["javascript:alert(1)", " JavaScript:alert(1)", "data:text/html,x", "file:///etc/passwd", "vbscript:x", "//evil.example"])
    if (safeHref(bad) !== null) fail(`the link ${bad} is allowed`);
  for (const ok of ["https://github.com/o/r/pull/1", "http://127.0.0.1:1/x", "devplane://change/c-1", "#change/c-1", "vscode://anthropic.claude-code/open?session=1", "claude-cli://open?repo=a&q=b"])
    if (safeHref(ok) === null) fail(`the link ${ok} is refused`);
  const raw = walk("../src/").filter((f) => f.endsWith(".svelte")).map((f) => [f, source(f)] as const);
  for (const [f, text] of raw)
    if (/href=\{(item\.(url|launch)|r\.url)\}|window\.open\(r\.url/.test(text)) fail(`${f} puts a host link in an href without safeHref`);
}

// ── A review mark is the hunk's content, not its header ──────────────────
{
  const a = markKey("c-1", "src/a.ts", "@@ -1,2 +1,2 @@", [["added", "let x = 1;"]]);
  const b = markKey("c-1", "src/a.ts", "@@ -1,2 +1,2 @@", [["added", "let x = 2;"]]);
  if (a === b) fail("a rewritten hunk with the same header keeps its seen mark");
  if (a !== markKey("c-1", "src/a.ts", "@@ -1,2 +1,2 @@", [["added", "let x = 1;"]])) fail("one hunk keys two ways");
}

// ── A failure says why and what tells more ───────────────────────────────
{
  const out = visible(html(Failed, { what: "the projects", failure: { says: "database is locked", tell: "devplane doctor" } })).replace(/\s+/g, " ");
  if (!/Could not read the projects: database is locked/.test(out) || !out.includes("devplane doctor")) fail("a failed read does not say why and what tells more");
  const stale = visible(html(Failed, { what: "the projects", failure: { says: "x", tell: "y" }, at: Date.now() - 60_000, stale: true }));
  if (!/as read 1m ago/.test(stale)) fail("a stale region does not say when it was read");
}

// ── The shapes that keep a page's state through a poll ───────────────────
{
  const app = source("../src/App.svelte");
  if (!/\$effect\(\(\) => untrack\(start\)\)/.test(app)) fail("the poll is started tracked, so each navigation restarts it");
  if (/page\.select\(feed as Feed, under\.f\)\s*:\s*\{\}\)\s*;/.test(app) && !/settlePage\(/.test(app)) fail("the page is handed a fresh props object every poll");
  if (!/settlePage\(page \? page\.select/.test(app)) fail("the page's props are not settled by identity");
  if (/href="#surface"/.test(app)) fail("the skip link is a fragment link, which overwrites the route");
  for (const f of ["../src/surfaces/inbox/Inbox.svelte", "../src/surfaces/answer/Answer.svelte"]) {
    const text = source(f);
    if (!/\{#key (current|first)\.id\}\s*<Row/.test(text)) fail(`${f} renders its item without {#key}, so a typed reply carries to the next`);
  }
  const doc = source("../src/surfaces/change/Doc.svelte");
  if (/\$effect\(\(\) => \{\s*void id;/.test(doc)) fail("the change document resets on the id prop, not its value");
  if (!/const key = \$derived\(id\)/.test(doc)) fail("the change document does not key on the id's value");
  const search = source("../src/surfaces/search/Search.svelte");
  if (/if \(q && q !== query\)/.test(search)) fail("the search box is reset by its own typing");
  // No surface swallows a failed read, and none claims a copy it did not make.
  for (const f of walk("../src/").filter((x) => /\.(svelte|ts)$/.test(x))) {
    const text = source(f);
    if (/\.catch\(\(\) => \{\s*\}\)/.test(text)) fail(`${f} swallows a failed request`);
    if (/await navigator\.clipboard\?\.writeText/.test(text)) fail(`${f} says copied when there is no clipboard`);
    if (/signal: AbortSignal\.timeout\(TIMEOUT_MS\)/.test(text)) fail(`${f} cuts every write off at the read timeout`);
  }
}
