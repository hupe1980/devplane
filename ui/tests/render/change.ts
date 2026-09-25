// A change as a document: its standing, what can be done to it, and six views.
// Rendered over the host's own recorded responses.
import Doc from "../../src/surfaces/change/Doc.svelte";
import Overview from "../../src/surfaces/change/Overview.svelte";
import Tasks from "../../src/surfaces/change/Tasks.svelte";
import Stepper from "../../src/surfaces/change/Stepper.svelte";
import Plan from "../../src/surfaces/change/Plan.svelte";
import ChangeList from "../../src/surfaces/change/List.svelte";
import { pair } from "../../src/surfaces/change/pair";
import { STATES, LIFE } from "../../src/lib/State.svelte";
import { recorded } from "../fixtures";

// Recordings are found by their title, never by an id a recapture changes.
const rate = recorded("change", "Rate-limit the login route");
const untraced = recorded("change", "Drop the v1 API");
const ungated = recorded("change", "Retry only on 5xx");
// The recording is verified; a pass the tree has since moved under is the
// same change with the host's stale standing — derived, and said so here.
const stale = {
  ...rate,
  state: "in flight",
  standing: "stale",
  standing_says: "`check` passed against tree 327517c; the tree is 9a01be2 now",
};
import { bare, fail, html, source, visible } from "../harness";

const doc = (d: Record<string, unknown>) => html(Doc, { id: d.id, detail: d, loaded: true });

// ── The standing is the host's sentence, once, and green only when verified ─
{
  for (const d of [stale, untraced, ungated]) {
    const out = doc(d);
    if (!out.includes(d.title)) fail(`the change document does not carry its title "${d.title}"`);
    const says = (d.standing_says ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;");
    if ((out.split(says).length - 1) !== 1) fail(`"${d.standing_says}" is not said exactly once`);
  }
  // A gate that passed against a stale tree is not verified, and nothing on
  // the page spends the verified green on it.
  const out = doc(stale);
  if (/class="count done/.test(out)) fail("the Gates tab is verified-green over a stale pass");
  if (/class="standing done/.test(out)) fail("a stale standing is rendered as verified");
  if (/class="v[^"]*\bdone\b/.test(out)) fail("the overview's gate card is verified-green over a stale pass");
}

// ── Only the actions that can do something are offered ──────────────────
{
  const flight = doc(stale);
  if (flight.includes("Offer as pull request")) fail("a change that is not verified offers itself as a pull request");
  if (flight.includes("Pick it back up")) fail("a change that is not stopped offers to be picked back up");
  if (!flight.includes("Run gates")) fail("a change with a worktree cannot have its gates run");
  const verified = doc({ ...stale, state: "verified", standing_says: "verified · every declared gate exited zero" });
  if (!verified.includes("Offer as pull request")) fail("a verified change is not offered as a pull request");
  if (!/class="standing done/.test(verified)) fail("a verified standing is not the verified green");
  const stopped = doc({ ...stale, stopped: { at: "x" }, can_retry: true });
  if (!stopped.includes("Pick it back up") || !stopped.includes("Try again")) fail("a stopped change cannot be resumed or retried");
  if (doc({ ...stale, worktree: null }).includes("Run gates")) fail("a change with no worktree offers to run gates");
  // Each verb reaches a route spelled whole, which the route guard can check.
  const code = source("../src/surfaces/change/Doc.svelte");
  for (const verb of ["verify", "finish", "offer", "resume", "retry", "archive"])
    if (!code.includes(`/api/changes/\${id}/${verb}\``)) fail(`the ${verb} button's route is not spelled whole`);
  // Nothing chosen is an instruction, not a blank.
  if (!html(Doc, { id: "", loaded: true }).includes("Pick a change")) fail("the change editor with nothing chosen renders a blank");
  // A tab strip with a panel.
  if (!/role="tablist"/.test(flight) || !/role="tabpanel"/.test(flight)) fail("the six views are not a tab strip over a panel");
}

// ── The stepper places the host's state; it spells nothing ───────────────
{
  const out = html(Stepper, { state: "in flight" });
  for (const w of LIFE) if (!out.includes(`>${STATES[w].word}<`)) fail(`the stepper lacks the state word ${w}`);
  if (!/aria-current="step"[^>]*>[\s\S]{0,120}in flight/.test(out)) fail("the stepper does not mark in flight as where the change is");
  const odd = html(Stepper, { state: "gates failing" });
  if (!odd.includes("gates failing")) fail("a state outside the six is not said in its own word");
  if (/class="[^"]*verified/.test(odd) || /class="[^"]*verified/.test(out)) fail("the stepper is verified-green on an unverified change");
  if (!/class="[^"]*verified/.test(html(Stepper, { state: "verified" }))) fail("a verified change is not marked verified on the stepper");
}

// ── Counts are counts: no percentage, no fraction, no n of m ─────────────
//
// Ticked is what an agent wrote about its own work; verified is a gate. They
// are two numbers side by side, never one figure over the other.
{
  const counts = { tasks: 15, ticked: 11, verified: 9, ticked_unsent: ["T010 Write the docs (FR-004)"], sent_unticked: [] };
  const d = { ...stale, counts, counts_says: "11 ticked · 9 verified", token_rows: [
    { token: "FR-003", tasks: 3, ticked: 2, verified: 0, says: "3 tasks · 2 ticked · 0 verified" },
    { token: "FR-004", tasks: 2, ticked: 2, verified: null, says: "2 tasks · 2 ticked · no gates declared" }],
    drifts: [{ run: "r-9f2", changed_at: "2026-09-25T10:18:00Z", started_at: "2026-09-25T10:00:00Z",
      says: "the specification changed 18m 0s into run r-9f2 and the run never saw it" }] };
  const over = html(Overview, { d, go: () => {} });
  const tasks = html(Tasks, { d, lastRun: "r-9f2" });
  for (const [name, markup] of [["overview", over], ["tasks", tasks]] as const) {
    const text = visible(markup);
    if (/\d\s*%/.test(text)) fail(`the ${name} view renders a percentage`);
    if (/\d+\s*\/\s*\d+/.test(text)) fail(`the ${name} view renders a fraction`);
    if (/\d+ of \d+(?! commands)/.test(text)) fail(`the ${name} view renders an "n of m"`);
    if (/<progress|<meter/.test(bare(markup))) fail(`the ${name} view renders a bar`);
  }
  if (!tasks.includes("11 ticked · 9 verified")) fail("the tasks view does not print the host's two counts");
  if (!tasks.includes("T010 Write the docs (FR-004)")) fail("a box ticked that nobody was sent is not listed by text");
  if (!tasks.includes("no gates declared")) fail("a requirement with no gates does not say so");
  if (!tasks.includes("the specification changed 18m 0s into run r-9f2")) fail("the drift sentence is not printed");
  if (!/>Tell the run</.test(tasks) || !/>Accept what it saw</.test(tasks)) fail("the drift offers fewer than two decisions");
  const t = source("../src/surfaces/change/Tasks.svelte");
  if (!t.includes("/drift/tell`") || !t.includes("/drift/accept`") || !/JSON\.stringify\(\{ run \}\)/.test(t))
    fail("a drift decision does not post its run to a route spelled whole");
  // Absent is not zero: no gates declared is a sentence, not a 0.
  const none = html(Overview, { d: { ...d, counts: { ...counts, verified: null } }, go: () => {} });
  if (!none.includes("no gates declared")) fail("an unverifiable count is rendered as a number");
}

// ── The plan beside the tasks, paired by key, never guessed ──────────────
{
  const rows = pair(
    [{ text: "T001 wire the route" }, { text: "T002 write the docs" }],
    [{ content: "First, T001 wire the route", status: "completed" }, { content: "then tidy", status: "pending" }],
  );
  if (rows.length !== 3) fail(`pairing produced ${rows.length} rows, not 3`);
  if (!(rows[0].task && rows[0].step)) fail("a step that names a task does not sit beside it");
  if (rows[1].step !== null) fail("a task nothing names got a step");
  if (rows[2].task !== null || !rows[2].step) fail("an unmatched step does not sit alone");
  const planned = html(Plan, { tasks: [{ text: "T001 wire the route" }, { text: "T002 write the docs" }],
    steps: [{ content: "First, T001 wire the route", status: "completed" }, { content: "then tidy", status: "pending" }] });
  const paired = planned.match(/<li class="row[^"]*\bpair\b[^"]*"[^>]*>[\s\S]*?<\/li>/);
  if (!paired || !paired[0].includes("T001 wire the route") || !paired[0].includes("First, T001 wire the route"))
    fail("a matched pair does not sit on one row");
  if (/\d+ of \d+/.test(planned)) fail("the plan column renders an n of m");
}

// ── The agent: the record beside the composer, and what survives first ───
{
  const a = source("../src/surfaces/change/Agent.svelte");
  if (!/\/stop`\)/.test(a)) fail("stopping does not read what survives before posting");
  if (!/r\?\.says/.test(a)) fail("the composer does not show the host's sentence about delivery");
  if (!/aria-label="another turn for the agent"/.test(a)) fail("the composer is unlabelled");
}

// ── The sidebar claims nothing before it has read ────────────────────────
{
  const out = html(ChangeList, { all: [], open: () => {} });
  if (/No change has been started/.test(out)) fail("the change list says nothing was started before it has read");
  if (!/class="skel/.test(out)) fail("the change list renders no skeleton before it has read");
}
