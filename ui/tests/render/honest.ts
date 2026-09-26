// Deciding honestly: *verified* never stands alone above a weakened check.
// Over the host's recorded responses: the qualifier on every surface that
// renders a change's state, Review first, an inbox that never offers, and host
// text with no literal markup anywhere.
import Doc from "../../src/surfaces/change/Doc.svelte";
import Gates from "../../src/surfaces/change/Gates.svelte";
import ChangeList from "../../src/surfaces/change/List.svelte";
import Palette from "../../src/surfaces/palette/Palette.svelte";
import PlanList from "../../src/surfaces/plan/List.svelte";
import PlanDoc from "../../src/surfaces/plan/Doc.svelte";
import Inbox from "../../src/surfaces/inbox/Inbox.svelte";
import Pane from "../../src/surfaces/review/Pane.svelte";
import { specs, key } from "../../src/surfaces/plan/store.svelte";
import { inline, blocks } from "../../src/lib/md";
import Inline from "../../src/lib/ui/Inline.svelte";
import changes from "../../fixtures/changes.json";
import inbox from "../../fixtures/inbox.json";
import specsRecorded from "../../fixtures/specs.json";
import { recorded } from "../fixtures";
import { bare, fail, html } from "../harness";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;
const all = import.meta.glob("../../fixtures/*.json", { eager: true, import: "default" }) as Record<string, Json>;

const TITLE = "Rate-limit the login route";
const rate: Json = recorded("change", TITLE);
const review: Json = recorded("review", TITLE);
const quiet: Json = recorded("change", "Drop the v1 API");
const WEAK = "1 check weakened";

/// The words beside the green: *verified*, then the qualifier, on one line.
function qualified(markup: string, where: string) {
  const text = bare(markup).replace(/<[^>]*>/g, " ").replace(/\s+/g, " ");
  if (!/verified · 1 check weakened/.test(text)) fail(`${where} shows verified without "· ${WEAK}" beside it`);
}

// ── The recording is the defect the audit found ─────────────────────────────
{
  if (rate.state !== "verified") fail(`the rate-limit recording is ${rate.state}, not verified — recapture`);
  if (rate.qualifier?.weakened !== 1 || rate.qualifier?.says !== WEAK)
    fail(`the rate-limit recording carries no qualifier: ${JSON.stringify(rate.qualifier)}`);
  if (quiet.qualifier?.says !== "") fail("a change that weakened nothing carries a qualifier");
}

// ── SC-001: five surfaces of five carry the qualifier beside verified ──────
{
  // 1. The change's own header.
  const doc = html(Doc, { id: rate.id, detail: rate, loaded: true });
  qualified(doc, "the change header");
  // 2. The Changes sidebar.
  const side = html(ChangeList, { all: [], open: () => {}, rows: changes });
  qualified(side, "the Changes sidebar");
  // 3. The palette.
  const pal = html(Palette, { from: "#change", changes });
  qualified(pal, "the palette");
  // 4. The Specifications page: the list and the specification itself.
  specs.projects = specsRecorded.projects as Json;
  qualified(html(PlanList, { open: () => {} }), "the Specifications list");
  const p: Json = (specsRecorded.projects as Json).find((x: Json) => x.plans.some((r: Json) => r.title === TITLE));
  const r = p?.plans.find((x: Json) => x.title === TITLE);
  if (!p || !r) fail("no recorded specification is worked by the rate-limit change");
  else qualified(html(PlanDoc, { focus: key(p, r) }), "the specification page");
  // 5. The inbox's ready item.
  const ready = inbox.items.find((i: Json) => i.kind === "ready_to_decide" && i.change_id === rate.id);
  if (!ready) fail("the inbox recording has no ready item for the rate-limit change");
  else {
    const out = html(Inbox, { items: [ready], loaded: true });
    if (!out.includes(WEAK)) fail(`the inbox's ready item does not say "${WEAK}"`);
    if (!/0 of \d+ hunks? marked/.test(bare(out))) fail("the ready item does not say how many hunks were marked");
    for (const s of ready.facts?.says ?? []) if (!out.includes(s)) fail(`the ready item drops "${s}"`);
  }
  // Nothing extra where nothing was weakened.
  const plain = html(Doc, { id: quiet.id, detail: quiet, loaded: true });
  if (/class="qualifier/.test(plain)) fail("a change that weakened nothing renders a qualifier");
}

// ── Review is the primary action while a weakened row is unseen ────────────
{
  const doc = html(Doc, { id: rate.id, detail: rate, loaded: true });
  // Svelte adds its scoping class, so the class list is read by word.
  const primary = doc.match(/<button class="[^"]*\bprimary\b[^"]*"[^>]*>[\s\S]*?<\/button>/g) ?? [];
  if (primary.length !== 1 || !/Review/.test(primary[0])) fail(`the primary action is not Review alone: ${primary.join(" | ")}`);
  if (!doc.includes("Offer as pull request")) fail("a verified change hides its offer rather than making it secondary");
  const read = { ...rate, qualifier: { ...rate.qualifier, unseen: 0 } };
  const seen = html(Doc, { id: rate.id, detail: read, loaded: true });
  const primarySeen = seen.match(/<button class="[^"]*\bprimary\b[^"]*"[^>]*>[\s\S]*?<\/button>/g) ?? [];
  if (primarySeen.length !== 1 || !/Offer as pull request/.test(primarySeen[0])) fail("with every row seen, offering is not the primary action");
  // The review shows each row's durable seen state and a way to mark it.
  const pane = html(Pane, { id: "", review });
  if (!pane.includes("not yet read") || !pane.includes("I have read this")) fail("the review does not show an unseen weakened row with a way to mark it read");
}

// ── SC-003: the inbox never offers in fewer steps than the review ──────────
{
  for (const i of inbox.items as Json[]) {
    if ((i.actions ?? []).includes("offer")) fail(`the inbox offers "${i.title}"`);
    if (i.kind === "ready_to_decide" && (i.actions ?? [])[0] !== "review") fail(`the ready item "${i.title}" does not lead with review`);
  }
  const ready = inbox.items.find((i: Json) => i.kind === "ready_to_decide");
  if (ready) {
    const out = html(Inbox, { items: [ready], loaded: true });
    if (/offer this change/i.test(out)) fail("the ready item has a one-step offer");
    const rev = out.indexOf(`href="#review/${encodeURIComponent(ready.change_id ?? "")}"`);
    if (rev === -1) fail("the ready item's review does not open the review");
    if (!/class="[^"]*\bact primary\b[^"]*" href="#review\//.test(out)) fail("review is not the ready item's primary action");
  }
}

// ── SC-004: no literal backtick or ** in anything a surface renders ────────
{
  /// Visible text outside <code>, <strong> and <pre> (verbatim output, which is
  /// somebody else's bytes and is shown as it was).
  const leaks = (markup: string): string[] => {
    const text = bare(markup)
      .replace(/<code[\s\S]*?<\/code>/g, " ")
      .replace(/<pre[\s\S]*?<\/pre>/g, " ")
      .replace(/<textarea[\s\S]*?<\/textarea>/g, " ")
      .replace(/<[^>]*>/g, " ");
    return (text.match(/.{0,30}(`|\*\*).{0,30}/g) ?? []).map((m) => m.trim());
  };
  const check = (markup: string, where: string) => {
    for (const l of leaks(markup)) fail(`${where} renders literal markup: "${l}"`);
  };
  for (const [path, body] of Object.entries(all)) {
    const name = path.split("/").pop() ?? "";
    if (name.startsWith("change-")) {
      check(html(Doc, { id: body.id, detail: body, loaded: true }), name);
      const cert = Object.entries(all).find(([p]) => p.endsWith(`certificate-${body.id}.json`))?.[1];
      check(html(Gates, { d: body, id: body.id, certificate: cert ?? null }), `${name} gates and certificate`);
    }
    if (name.startsWith("review-")) check(html(Pane, { id: "", review: body }), name);
    if (name === "inbox.json") check(html(Inbox, { items: body.items, loaded: true }), name);
    if (name === "changes.json") check(html(ChangeList, { all: [], open: () => {}, rows: body }), name);
  }
  check(html(PlanList, { open: () => {} }), "the Specifications list");

  // The renderer itself: markup rendered, everything else escaped, no links.
  const segs = (t: string) => inline(t).map((s) => `${s.k}:${s.s}`).join("|");
  if (segs("`check` exited **zero**") !== "code:check|t: exited |strong:zero") fail("inline markup is not rendered");
  // Segments render as text nodes, so a tag in host text stays text.
  const tagged = html(Inline, { text: "<script>`x`</script>" });
  if (tagged.includes("<script>")) fail("host text is not escaped");
  if (html(Inline, { text: "[a](javascript:x)" }).includes("<a")) fail("a link in host text became a link");
  if (blocks("# T\n\n- a `b`\n").map((b) => b.kind).join() !== "h,li") fail("the certificate's blocks are not read");
}
