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
import { plain } from "../../src/lib/md";

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
    // Rendered as the host's sentence: read as the words a person sees, once.
    const norm = (s: string) => s.replace(/\s+/g, " ").trim();
    const says = norm(plain(d.standing_says ?? ""));
    if ((norm(visible(out)).split(says).length - 1) !== 1) fail(`"${d.standing_says}" is not said exactly once`);
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
  // Every weakened row read: the state and its green sentence are shown.
  const verified = doc({ ...stale, state: "verified", qualifier: { ...rate.qualifier, unseen: 0 }, standing_says: "verified · every declared gate exited zero" });
  if (!verified.includes("Offer as pull request")) fail("a verified change is not offered as a pull request");
  if (!/class="standing done/.test(verified)) fail("a verified standing is not the verified green");
  // A weakened row unread: Offer is held, and says why beside it. The
  // recording has one such row; the host refuses that offer. The button is
  // found by its label (held) or its words (live).
  const offer = (markup: string) =>
    markup.match(/<button[^>]*aria-label="Offer as pull request"[^>]*>|<button[^>]*>(?:(?!<\/button>)[\s\S])*?Offer as pull request/)?.[0] ?? "";
  if (rate.qualifier?.unseen !== 1) fail("the recorded change no longer has one unread weakened row, so this check guards nothing");
  const held = doc(rate);
  if (!offer(held)) fail("a verified change with an unread weakened row has no Offer at all");
  if (!/\bdisabled\b/.test(offer(held).split(">")[0])) fail("Offer is live while a weakened row is unread");
  if (!visible(held).includes("Read the weakened check first")) fail("a held Offer does not say why beside it");
  if (!/aria-describedby="offer-held"/.test(offer(held))) fail("a held Offer's reason is not tied to the button");
  if (!offer(verified) || /\bdisabled\b/.test(offer(verified).split(">")[0])) fail("Offer is held with every weakened row read");
  if (visible(verified).includes("weakened check first")) fail("Offer gives a reason to wait with nothing unread");
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

// ── The stepper is one line: where the change is, and the step after it ──
{
  const line = (props: Record<string, unknown>) => visible(html(Stepper, props)).replace(/\s+/g, " ").trim();
  if (line({ state: "verified" }) !== "verified → next: offer") fail(`the stepper does not read "verified → next: offer": ${line({ state: "verified" })}`);
  if (line({ state: "in flight" }) !== "in flight → next: run the gates") fail(`the stepper misplaces in flight: ${line({ state: "in flight" })}`);
  if (!/aria-current="step"[^>]*>in flight</.test(html(Stepper, { state: "in flight" }))) fail("the stepper does not mark where the change is");
  if (!line({ state: "gates failing" }).startsWith("gates failing")) fail("a state outside the six is not said in its own word");
  if (!/next: read 1 weakened check/.test(line({ state: "verified", weakUnread: 1 }))) fail("an unread weakened check is not the next step");
  if (line({ state: "offered", archived: true }) !== STATES.archived.word) fail("an archived change has a next step");
  for (const w of LIFE) if (!line({ state: w }).startsWith(STATES[w].word)) fail(`the stepper does not say ${w}`);
  if (/var\(--done\)/.test(source("../src/surfaces/change/Stepper.svelte"))) fail("the stepper spends the verified green");
}

// ── Counts are counts: no percentage, no fraction, no n of m ─────────────
//
// Ticked is what an agent wrote about its own work; seen by a passing check
// is a gate. They are two numbers side by side, never one figure over the
// other — and a task is never called verified, only a change is.
{
  const counts = { tasks: 15, ticked: 11, seen_by_pass: 9, ticked_unsent: ["T010 Write the docs (FR-004)"], sent_unticked: [] };
  const d = { ...stale, counts, counts_says: "11 ticked · 9 seen by a passing check", token_rows: [
    { token: "FR-003", tasks: 3, ticked: 2, seen_by_pass: 0, says: "3 tasks · 2 ticked · 0 seen by a passing check" },
    { token: "FR-004", tasks: 2, ticked: 2, seen_by_pass: null, says: "2 tasks · 2 ticked · no gates declared" }],
    drifts: [{ run: "r-9f2", changed_at: "2026-09-25T10:18:00Z", started_at: "2026-09-25T10:00:00Z",
      says: "the specification changed 18m 0s into run r-9f2 and the run never saw it" }] };
  const over = html(Overview, { d, go: () => {} });
  const tasks = html(Tasks, { d, lastRun: "r-9f2" });
  for (const [name, markup] of [["overview", over], ["tasks", tasks]] as const) {
    const text = visible(markup);
    if (/\d\s*%/.test(text)) fail(`the ${name} view renders a percentage`);
    if (/\d+\s*\/\s*\d+/.test(text)) fail(`the ${name} view renders a fraction: ${text.match(/.{0,40}\d+\s*\/\s*\d+.{0,20}/)?.[0]}`);
    if (/\d+ of \d+(?! commands)/.test(text)) fail(`the ${name} view renders an "n of m"`);
    if (/<progress|<meter/.test(bare(markup))) fail(`the ${name} view renders a bar`);
  }
  if (!tasks.includes("11 ticked · 9 seen by a passing check")) fail("the tasks view does not print the host's two counts");
  for (const [name, markup] of [["overview", over], ["tasks", tasks]] as const) {
    if (/\bverified\b/i.test(visible(markup))) fail(`the ${name} view calls a task verified`);
    if (/class="[^"]*\bdone\b[^"]*"[^>]*>\s*9\b/.test(markup)) fail(`the ${name} view paints the seen count green`);
  }
  if (!over.includes("seen by a passing check")) fail("the overview does not say what the third count is");
  if (!tasks.includes("T010 Write the docs (FR-004)")) fail("a box ticked that nobody was sent is not listed by text");
  if (!tasks.includes("no gates declared")) fail("a requirement with no gates does not say so");
  if (!tasks.includes("the specification changed 18m 0s into run r-9f2")) fail("the drift sentence is not printed");
  if (!/>Tell the run</.test(tasks) || !/>Accept what it saw</.test(tasks)) fail("the drift offers fewer than two decisions");
  const t = source("../src/surfaces/change/Tasks.svelte");
  if (!t.includes("/drift/tell`") || !t.includes("/drift/accept`") || !/JSON\.stringify\(\{ run \}\)/.test(t))
    fail("a drift decision does not post its run to a route spelled whole");
  // Absent is not zero: no gates declared is a sentence, not a 0.
  const none = html(Overview, { d: { ...d, counts: { ...counts, seen_by_pass: null } }, go: () => {} });
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

// ── The agent tab: a wait is words, and only what it can see is listed ───
{
  const agent = source("../src/surfaces/change/Agent.svelte");
  if (!/waiting on you — question/.test(agent) || !/"waiting" in s/.test(agent))
    fail("a waiting run's state object is not mapped to words, so it prints as [object Object]");
  if (/Commands run/.test(agent)) fail("the agent tab claims a list of finished commands it cannot see");
  // The review: `s` marks the one hunk under the cursor, by its digest, in
  // the host; a weakened row is read when its hunks are, never by a mark of
  // its own.
  const pane = source("../src/surfaces/change/review/Pane.svelte");
  if (!/markSeen\(change, x\.file\.path, x\.h\.digest\)/.test(pane)) fail("marking a hunk does not send that hunk's digest to the host");
  if (/matched:/.test(pane)) fail("the review still marks a weakened row by its matched text");
  // No count before the list is read.
  const unread = html(ChangeList, { open: () => {} });
  if (/class="n[^"]*">0</.test(unread)) fail("the changes sidebar prints 0 before it has read anything");
}

// ── The agent's stop claim: its words, after the weakened checks, before the gates ─
//
// Quoted under the agent's name, never markup and never a link; read after
// the state and its weakened checks, never first; and it moves neither the
// state nor the host's sentence about the check.
{
  const claim = {
    id: "sc-1",
    says: "the agent says the spec and the tests conflict",
    reason: "The spec says 30 days; <script>alert(1)</script> [see](https://x.example) **now**",
    paths: ["specs/043-sessions/spec.md", "tests/session_test.rs"],
    agent: "echo",
    run: "acp-claimed",
    at: "2026-09-27T10:02:00Z",
    origin_says: "saas — echo, run acp-claimed, change c-1, 2026-09-27 10:02 UTC",
    quoted: "",
    ending: "the agent stopped: it says the spec and the tests conflict",
    state: "open",
    can_tell: true,
  };
  const d = { ...stale, stop_claim: claim, claim_history: [claim] };
  const out = doc(d);
  // What a person reads: tags gone, entities read back, spaces collapsed.
  const words = (m: string) =>
    visible(m)
      .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&")
      .replace(/\s+/g, " ");
  const text = words(out);
  const at = (s: string) => text.indexOf(s);
  if (at(claim.says) < 0) fail("the change document does not carry the agent's claim");
  if (at(claim.says) < at(d.title)) fail("the claim is read before the change's title");
  const qualifier = rate.qualifier?.says ?? "";
  if (qualifier && at(qualifier) > at(claim.says)) fail("the claim is read before the weakened checks");
  const standing = "passed against tree 327517c";
  if (at(standing) < 0 || at(standing) < at(claim.says)) fail("the gates' sentence is read before the claim");
  if (/<script>alert/.test(out) || /href="https:\/\/x\.example"/.test(out) || /<strong>now/.test(out))
    fail("the agent's reason is rendered as markup");
  if (!text.includes("<script>alert(1)</script> [see](https://x.example) **now**")) fail("the agent's reason is not shown as its own text");
  if (!text.includes("specs/043-sessions/spec.md")) fail("the claim's paths are not shown");
  if (!/echo, run acp-claimed/.test(text)) fail("the claim is not attributed to its agent and run");
  if (!out.includes("Mark read") || !out.includes("Tell the agent")) fail("the claim offers no way out");
  if (doc({ ...d, stop_claim: { ...claim, can_tell: false } }).includes("Tell the agent")) fail("Tell the agent is offered where the run cannot be resumed");
  // The verdict is the gate's: the state and its sentence are the same with and without it.
  const without = words(doc(stale));
  if (!without.includes(standing) || !text.includes(standing)) fail("the claim moved the host's sentence about the check");
  const code = source("../src/surfaces/change/Claim.svelte");
  if (/\{@html|<Inline|<Md\b/.test(code)) fail("the claim is rendered through markup");
  // The list row: after the state, as the agent's.
  const list = words(html(ChangeList, { open: () => {}, rows: [{ id: "c-1", title: "Keep sessions for 30 days", state: "in flight", project_id: "/p/saas", stop_claim: { says: claim.says } }] }));
  if (!list.includes(claim.says)) fail("the change list does not carry the agent's claim");
  if (list.indexOf(claim.says) < list.indexOf("in flight")) fail("the change list reads the claim before the state");
  // The overview keeps every claim in its history, under its run.
  const over = html(Overview, { d: { ...d, claim_history: [{ ...claim, state: "closed", closed_says: "a person marked it read" }] }, go: () => {} });
  if (!/aria-label="the agent says the spec and the tests conflict — run acp-claimed · closed: a person marked it read/.test(over))
    fail("a closed claim is not kept in the change's history");
}
