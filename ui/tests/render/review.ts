// Deciding whether to merge: the host's order, the diff as text, and no verdict.
import Pane from "../../src/surfaces/change/review/Pane.svelte";
import Diff from "../../src/surfaces/change/review/Diff.svelte";
import { recorded as find } from "../fixtures";
import { shape } from "../../src/lib/text";

// Found by title, never by an id a recapture changes.
const recorded = find("review", "Rate-limit the login route");
const nothing = find("review", "Drop the v1 API");
import { help } from "../../src/lib/keys";
import { bare, fail, html, source, visible } from "../harness";

const pane = (review: unknown) => html(Pane, { id: "", review });

// ── The review pane renders the review, never HTML ───────────────────────
{
  const out = pane(recorded);
  const file = recorded.groups[0].files[0];
  for (const want of [shape(recorded.shape_says), recorded.base, file.path, file.role_says, file.marker_commands[0]])
    if (!out.includes(want.replace(/&/g, "&amp;"))) fail(`the review does not render "${want}"`);
  if (!/class="hunk[ "]/.test(out)) fail("the review renders no hunk at all");
  // *Nothing to show* is the host's sentence, and the surface prints it.
  const empty = pane(nothing);
  // The size in plain words: no role said as a zero, no `2+ 0−`.
  if (shape("2 files · 2+ 0− lines · 0 shared · 1 security") !== "2 files · +2 −0 lines · 1 security file") fail(`the size sentence is not plain: ${shape("2 files · 2+ 0− lines · 0 shared · 1 security")}`);
  if (!empty.includes(nothing.empty_says ?? "")) fail("a branch with no changes renders a blank rather than the finding");
  for (const rel of ["Pane", "Files", "Diff", "FileSection", "Comments"]) {
    const src = source(`../src/surfaces/change/review/${rel}.svelte`);
    if (/body\.html/.test(src)) fail(`review/${rel} reads a server-rendered html field`);
  }
  const p = source("../src/surfaces/change/review/Pane.svelte");
  const both = p + source("../src/surfaces/change/review/FileSection.svelte");
  for (const needed of ["empty_says", "truncated_says", "coverage_absent", "unordered", "body_says"])
    if (!both.includes(needed)) fail(`the review never renders \`${needed}\``);
  // Not asked for sits under its own heading after every group.
  const groupsAt = p.indexOf("r.intent.groups.map");
  const unaskedAt = p.indexOf("r.intent.not_asked_for_heading");
  if (groupsAt < 0 || unaskedAt < groupsAt) fail("not asked for is not placed after every group");
}

// ── No aggregate, no verdict: no score, nothing that says fine or safe ───
{
  const sources = ["Pane", "Files", "Diff", "FileSection", "Comments"].map((f) => source(`../src/surfaces/change/review/${f}.svelte`)).join("\n");
  for (const [name, markup] of [["review", visible(pane(recorded))], ["review source", bare(sources)]] as const) {
    const text = markup.toLowerCase();
    if (/\d\s*%/.test(text)) fail(`${name} renders a percentage`);
    if (/<progress|<meter/.test(text)) fail(`${name} renders a bar`);
    for (const word of ["score", "confidence", "grade", "looks right"]) if (text.includes(word)) fail(`${name} says "${word}"`);
    for (const word of ["fine", "safe", "good"]) if (new RegExp(`\\b${word}\\b`).test(text)) fail(`${name} says "${word}"`);
  }
  // One *seen*, the host's: no mark is kept in this browser.
  const unreadSrc = source("../src/surfaces/change/review/unread.svelte.ts");
  if (/localStorage/.test(unreadSrc) || /markKey|readMark|writeMark/.test(sources)) fail("a review mark is still kept in the browser");
  if (!/\/review\/seen/.test(unreadSrc)) fail("a review mark is not sent to the host");
  for (const combo of ["j", "k", "n", "p", "s", "f", "x", "v", "1", "2", "Mod+Enter"]) {
    const b = help("change:review").find((x) => x.combo === combo && x.surface === "change:review");
    if (!b || !b.label.trim()) fail(`the review does not bind ${combo} with a label`);
  }
  // No second verdict in the browser: *seen* is the only mark.
  if (help("change:review").some((x) => x.combo === "a" && x.surface === "change:review")) fail("the review still binds a to an accept mark");
  if (/accepted/.test(unreadSrc + sources)) fail("the review still knows an accepted mark");
}

// ── The diff: numbers from the header, and split pairs removals with additions ─
{
  const hunk = {
    header: "@@ -10,5 +20,4 @@ fn login()",
    formatter_only: false,
    lines: [["context", "a"], ["removed", "b"], ["removed", "c"], ["removed", "c2"], ["added", "d"], ["added", "d2"], ["context", "e"]],
  };
  const mark = () => false;
  const rowsOf = (m: string) => [...m.matchAll(/<tr[^>]*>([\s\S]*?)<\/tr>/g)].map((r) =>
    [...r[1].matchAll(/<td[^>]*>([\s\S]*?)<\/td>/g)].map((c) => c[1].replace(/<!--[\s\S]*?-->/g, "").trim()));

  // Unified: old number, new number, sign, text — the numbers the header gave.
  const unified = rowsOf(html(Diff, { hunks: [hunk], mode: "unified", seen: mark }));
  const want = [["10", "20", "", "a"], ["11", "", "−", "b"], ["12", "", "−", "c"], ["13", "", "−", "c2"],
    ["", "21", "+", "d"], ["", "22", "+", "d2"], ["14", "23", "", "e"]];
  if (JSON.stringify(unified.map((r) => [r[0], r[1], r[2].trim(), r[3]])) !== JSON.stringify(want))
    fail(`the unified diff's numbers are not the header's: ${JSON.stringify(unified)}`);

  // Split: the run of removals faces the run of additions line by line; the
  // one left over faces an empty cell, never a guessed partner.
  const split = rowsOf(html(Diff, { hunks: [hunk], mode: "split", seen: mark }));
  const pairs = split.map((r) => [r[0], r[2], r[3], r[5]]);
  const expect = [["10", "a", "20", "a"], ["11", "b", "21", "d"], ["12", "c", "22", "d2"], ["13", "c2", "", ""], ["14", "e", "23", "e"]];
  if (JSON.stringify(pairs) !== JSON.stringify(expect)) fail(`the split diff pairs lines wrongly: ${JSON.stringify(pairs)}`);
  // The sign is on every changed line in both layouts, not colour alone.
  if (split[1][1] !== "−" || split[1][4] !== "+") fail("the split diff marks its lines by colour alone");

  // A formatter-only hunk is folded, with the reason and a way to open it.
  const folded = html(Diff, { hunks: [{ ...hunk, formatter_only: true }], seen: mark });
  if (!/Formatting only[\s\S]*Show them/.test(folded)) fail("a formatter-only hunk is not folded with a way to open it");
  if (/<table/.test(folded)) fail("a formatter-only hunk is folded and drawn anyway");
  // Somebody else's text, as text.
  const hostile = html(Diff, { hunks: [{ ...hunk, lines: [["added", "<img src=x onerror=1>"]] }], seen: mark });
  if (hostile.includes("<img src=x")) fail("a diff line reached the document as markup");
}

// ── One column: every hunk of every file, in the tree's order ────────────
{
  const out = pane(recorded);
  const order = (recorded.groups as Array<{ files: Array<{ path: string; hunks: unknown[] }> }>).flatMap((g) => g.files);
  // Every file has a section, in reading order, and every hunk is drawn.
  const at = order.map((f) => out.indexOf(`data-path="${f.path}"`));
  if (at.some((i) => i < 0)) fail("a file of the change has no section in the column");
  if (at.some((i, k) => k > 0 && i < at[k - 1])) fail("the column is not in the tree's order");
  const hunks = order.reduce((n, f) => n + f.hunks.length, 0);
  if ((out.match(/id="hunk-\d+"/g) ?? []).length !== hunks) fail(`the column does not draw every hunk (${hunks})`);
  // The weakened file leads, and its strip says so.
  const weak = (recorded.groups[0].weakened ?? [])[0];
  if (weak && at[0] !== out.indexOf(`data-path="${weak.path}"`)) fail("the weakened check's file does not lead the column");
  if (weak && !/check weakened/.test(visible(out))) fail("the weakened file's strip does not say so");
  // The tree counts what is unread, in words: never n of m.
  const tree = visible(out);
  if (/\b\d+\s*\/\s*\d+\b/.test(tree.replace(/[\w.-]+\/[\w./-]+/g, ""))) fail("the tree renders a fraction");
  if (!/\d+ unread|all seen/.test(tree)) fail("the tree does not count unread hunks in words");
  if (/ of \d+ hunks/.test(tree)) fail("the tree renders an n of m");
}

// ── The weakened line is marked in the gutter, by glyph and word, with its gate ─
{
  const hunk = { header: "@@ -1,1 +1,2 @@", formatter_only: false, lines: [["context", "a"], ["added", "it.skip(\"x\")"]] };
  const weak = [{ path: "t.test.ts", why: "adds a skip marker", kind: "skip", matched: "it.skip(\"x\")", seen: false, gate_says: "the `check` gate ran over it: `npm test`" }];
  const out = html(Diff, { hunks: [hunk], weak });
  if (!/class="[^"]*\bweakline\b/.test(out)) fail("the weakened line is not marked");
  if (!out.includes("⚠")) fail("the weakened line has no glyph in its gutter");
  if (!/check weakened here/.test(visible(out))) fail("the weakened line is marked by colour alone, with no word");
  if (!visible(out).includes("npm test")) fail("the weakened line does not name the gate that ran it");
}

// ── A comment sits under its line; a moved one says so ───────────────────
{
  const hunk = { header: "@@ -1,1 +1,2 @@", formatter_only: false, lines: [["context", "a"], ["added", "b"]] };
  const c = (over: Record<string, unknown>) => ({
    id: "rc-1", change_id: "c", path: "a.ts", line: 2, side: "new", hunk_header: hunk.header, hunk_digest: "d", line_text: "b",
    body: "why b?", tree: "t", created_at: "x", sent_at: null, run_id: null, anchor: { is: "same" }, hunk_changed: null, ...over,
  });
  const out = visible(html(Diff, { hunks: [hunk], comments: [c({})] }));
  if (!out.includes("why b?") || !out.includes("pending")) fail("a pending comment is not drawn under its line");
  const sent = visible(html(Diff, { hunks: [hunk], comments: [c({ sent_at: "y", run_id: "r", hunk_changed: true })] }));
  if (!/the agent changed this hunk since/.test(sent)) fail("a sent comment does not say its hunk changed");
  const moved = visible(html(Diff, { hunks: [hunk], comments: [c({ line: 9, anchor: { is: "moved", line: 2 } })] }));
  if (!/moved from line 9/.test(moved)) fail("a moved comment does not say so");
  const hostile = html(Diff, { hunks: [hunk], comments: [c({ body: "<img src=x onerror=1>" })] });
  if (hostile.includes("<img src=x")) fail("a comment reached the document as markup");
}

// ── A file over a thousand changed lines folds with its count ────────────
{
  const big = { ...recorded.groups[0].files[0], added: 1200, removed: 0 };
  const out = html(Pane, { id: "", review: { ...recorded, groups: [{ ...recorded.groups[0], files: [big] }] } });
  if (!/1200 changed lines in \d+ hunks? — collapsed/.test(visible(out))) fail("an oversized file is not collapsed with its count");
  if (/class="hunk[ "]/.test(out)) fail("an oversized file is collapsed and drawn anyway");
}
