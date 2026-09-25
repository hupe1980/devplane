// Deciding whether to merge: the host's order, the diff as text, and no verdict.
import Pane from "../../src/surfaces/review/Pane.svelte";
import Review from "../../src/surfaces/review/Review.svelte";
import Diff from "../../src/surfaces/review/Diff.svelte";
import { recorded as find } from "../fixtures";

// Found by title, never by an id a recapture changes.
const recorded = find("review", "Rate-limit the login route");
const nothing = find("review", "Drop the v1 API");
import { help } from "../../src/lib/keys";
import { bare, fail, html, source, visible } from "../harness";

const pane = (review: unknown) => html(Pane, { id: "", review });

// ── The review surface renders the review, never HTML ────────────────────
{
  const out = pane(recorded);
  const file = recorded.groups[0].files[0];
  for (const want of [recorded.shape_says, recorded.base, file.path, file.role_says, file.marker_commands[0]])
    if (!out.includes(want.replace(/&/g, "&amp;"))) fail(`the review does not render "${want}"`);
  if (!/class="hunk[ "]/.test(out)) fail("the review renders no hunk at all");
  // *Nothing to show* is the host's sentence, and the surface prints it.
  const empty = pane(nothing);
  if (!empty.includes(nothing.empty_says ?? "")) fail("a branch with no changes renders a blank rather than the finding");
  for (const rel of ["Pane", "Files", "Diff", "Review"]) {
    const src = source(`../src/surfaces/review/${rel}.svelte`);
    if (/body\.html/.test(src)) fail(`review/${rel} reads a server-rendered html field`);
  }
  const p = source("../src/surfaces/review/Pane.svelte");
  for (const needed of ["empty_says", "truncated_says", "coverage_absent", "unordered", "body_says"])
    if (!p.includes(needed)) fail(`the review never renders \`${needed}\``);
  // Not asked for sits under its own heading after every group.
  const groupsAt = p.indexOf("r.intent.groups.map");
  const unaskedAt = p.indexOf("r.intent.not_asked_for_heading");
  if (groupsAt < 0 || unaskedAt < groupsAt) fail("not asked for is not placed after every group");
  // Nothing chosen is an instruction.
  if (!html(Review, { chosen: "" }).includes("Pick a change to review")) fail("the review page with nothing chosen is a blank");
}

// ── No aggregate, no verdict: no score, nothing that says fine or safe ───
{
  const sources = ["Pane", "Files", "Diff"].map((f) => source(`../src/surfaces/review/${f}.svelte`)).join("\n");
  for (const [name, markup] of [["review", visible(pane(recorded))], ["review source", bare(sources)]] as const) {
    const text = markup.toLowerCase();
    if (/\d\s*%/.test(text)) fail(`${name} renders a percentage`);
    if (/<progress|<meter/.test(text)) fail(`${name} renders a bar`);
    for (const word of ["score", "confidence", "grade", "looks right"]) if (text.includes(word)) fail(`${name} says "${word}"`);
    for (const word of ["fine", "safe", "good"]) if (new RegExp(`\\b${word}\\b`).test(text)) fail(`${name} says "${word}"`);
  }
  const marks = source("../src/surfaces/review/marks.ts");
  if (!/localStorage/.test(marks) || !/try \{/.test(marks)) fail("the review marks are not kept in the browser behind a try");
  if (/api\(|fetch\(/.test(marks)) fail("a review mark is sent somewhere");
  for (const combo of ["j", "k", "n", "p", "s", "a", "f", "x", "v", "1", "2"]) {
    const b = help("review").find((x) => x.combo === combo && x.surface === "review");
    if (!b || !b.label.trim()) fail(`the review does not bind ${combo} with a label`);
  }
}

// ── The diff: numbers from the header, and split pairs removals with additions ─
{
  const hunk = {
    header: "@@ -10,5 +20,4 @@ fn login()",
    formatter_only: false,
    lines: [["context", "a"], ["removed", "b"], ["removed", "c"], ["removed", "c2"], ["added", "d"], ["added", "d2"], ["context", "e"]],
  };
  const mark = () => null;
  const rowsOf = (m: string) => [...m.matchAll(/<tr[^>]*>([\s\S]*?)<\/tr>/g)].map((r) =>
    [...r[1].matchAll(/<td[^>]*>([\s\S]*?)<\/td>/g)].map((c) => c[1].replace(/<!--[\s\S]*?-->/g, "").trim()));

  // Unified: old number, new number, sign, text — the numbers the header gave.
  const unified = rowsOf(html(Diff, { hunks: [hunk], mode: "unified", mark }));
  const want = [["10", "20", "", "a"], ["11", "", "−", "b"], ["12", "", "−", "c"], ["13", "", "−", "c2"],
    ["", "21", "+", "d"], ["", "22", "+", "d2"], ["14", "23", "", "e"]];
  if (JSON.stringify(unified.map((r) => [r[0], r[1], r[2].trim(), r[3]])) !== JSON.stringify(want))
    fail(`the unified diff's numbers are not the header's: ${JSON.stringify(unified)}`);

  // Split: the run of removals faces the run of additions line by line; the
  // one left over faces an empty cell, never a guessed partner.
  const split = rowsOf(html(Diff, { hunks: [hunk], mode: "split", mark }));
  const pairs = split.map((r) => [r[0], r[2], r[3], r[5]]);
  const expect = [["10", "a", "20", "a"], ["11", "b", "21", "d"], ["12", "c", "22", "d2"], ["13", "c2", "", ""], ["14", "e", "23", "e"]];
  if (JSON.stringify(pairs) !== JSON.stringify(expect)) fail(`the split diff pairs lines wrongly: ${JSON.stringify(pairs)}`);
  // The sign is on every changed line in both layouts, not colour alone.
  if (split[1][1] !== "−" || split[1][4] !== "+") fail("the split diff marks its lines by colour alone");

  // A formatter-only hunk is folded, with the reason and a way to open it.
  const folded = html(Diff, { hunks: [{ ...hunk, formatter_only: true }], mark });
  if (!/Formatting only[\s\S]*Show them/.test(folded)) fail("a formatter-only hunk is not folded with a way to open it");
  if (/<table/.test(folded)) fail("a formatter-only hunk is folded and drawn anyway");
  // Somebody else's text, as text.
  const hostile = html(Diff, { hunks: [{ ...hunk, lines: [["added", "<img src=x onerror=1>"]] }], mark });
  if (hostile.includes("<img src=x")) fail("a diff line reached the document as markup");
}
