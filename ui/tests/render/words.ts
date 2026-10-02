// Plain words, no riddles, no markup: every surface, rendered over the host's
// recorded responses, says nothing in this corpus's own register (*alone*,
// *paired*) and shows no literal backtick or `**` — host text goes through the
// one markdown renderer. And one *unread*: the change's header, its Review tab
// and the review pane say the same number in the same words.
import Doc from "../../src/surfaces/change/Doc.svelte";
import Overview from "../../src/surfaces/change/Overview.svelte";
import Tasks from "../../src/surfaces/change/Tasks.svelte";
import Gates from "../../src/surfaces/change/Gates.svelte";
import Agent from "../../src/surfaces/change/Agent.svelte";
import Plan from "../../src/surfaces/change/Plan.svelte";
import ChangeList from "../../src/surfaces/change/List.svelte";
import Pane from "../../src/surfaces/change/review/Pane.svelte";
import Inbox from "../../src/surfaces/inbox/Inbox.svelte";
import InboxList from "../../src/surfaces/inbox/List.svelte";
import PlanList from "../../src/surfaces/plan/List.svelte";
import PlanDoc from "../../src/surfaces/plan/Doc.svelte";
import Palette from "../../src/surfaces/palette/Palette.svelte";
import Board from "../../src/surfaces/board/Board.svelte";
import Why from "../../src/surfaces/why/Why.svelte";
import GithubList from "../../src/surfaces/github/GithubList.svelte";
import Setup from "../../src/surfaces/setup/Setup.svelte";
import Answer from "../../src/surfaces/answer/Answer.svelte";
import Failed from "../../src/lib/Failed.svelte";
import Empty from "../../src/lib/ui/Empty.svelte";
import Md from "../../src/lib/ui/Md.svelte";
import { unread, unreadSays } from "../../src/surfaces/change/review/unread.svelte";
import { specs, key } from "../../src/surfaces/plan/store.svelte";
import changes from "../../fixtures/changes.json";
import inbox from "../../fixtures/inbox.json";
import board from "../../fixtures/board.json";
import forge from "../../fixtures/forge.json";
import specsRecorded from "../../fixtures/specs.json";
import { recorded } from "../fixtures";
import { bare, fail, html } from "../harness";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;
const all = import.meta.glob("../../fixtures/*.json", { eager: true, import: "default" }) as Record<string, Json>;

/// What a person reads, outside verbatim bytes (`<code>`, `<pre>`, a field
/// being typed in), with whitespace folded.
const read = (markup: string): string =>
  bare(markup)
    .replace(/<code[\s\S]*?<\/code>/g, " ")
    .replace(/<pre[\s\S]*?<\/pre>/g, " ")
    .replace(/<textarea[\s\S]*?<\/textarea>/g, " ")
    .replace(/<[^>]*>/g, " ")
    .replace(/\s+/g, " ");

/// The words this corpus uses to argue and a person must never decode.
const RIDDLES = /\b(alone|paired|never asked again)\b/i;

function plain(markup: string, where: string) {
  const text = read(markup);
  const riddle = RIDDLES.exec(text);
  if (riddle) fail(`${where} says "${riddle[0]}": ${text.slice(Math.max(0, riddle.index - 40), riddle.index + 30).trim()}`);
  for (const m of text.match(/.{0,30}(`|\*\*).{0,30}/g) ?? []) fail(`${where} shows literal markup: "${m.trim()}"`);
}

// ── Every surface, over every recording ───────────────────────────────────
{
  for (const [path, body] of Object.entries(all)) {
    const name = path.split("/").pop() ?? "";
    if (name.startsWith("change-")) {
      const review = Object.entries(all).find(([p]) => p.endsWith(`review-${body.id}.json`))?.[1] ?? null;
      plain(html(Doc, { id: body.id, detail: body, review, loaded: true }), `the change ${body.title}`);
      for (const view of ["overview", "tasks", "review", "gates", "ledger", "agent"])
        plain(html(Doc, { id: body.id, detail: body, review, loaded: true, view }), `the change ${body.title}, ${view}`);
      plain(html(Overview, { d: body, go: () => {} }), `${name} overview`);
      plain(html(Tasks, { d: body, lastRun: body.runs?.[0] ?? "" }), `${name} tasks`);
      plain(html(Gates, { d: body, id: body.id }), `${name} gates`);
      plain(html(Agent, { d: body, run: body.runs?.[0] ?? "" }), `${name} agent`);
      const sent = (body.run_rows ?? []).flatMap((r: Json) => r.sent ?? []);
      plain(html(Plan, { tasks: sent, steps: [] }), `${name} plan`);
      plain(html(Plan, { tasks: sent, steps: [{ content: "something else", status: "pending" }] }), `${name} plan with a step`);
    }
    if (name.startsWith("review-")) plain(html(Pane, { id: "", review: body }), name);
  }
  plain(html(ChangeList, { all: [], open: () => {}, rows: changes }), "the Changes sidebar");
  plain(html(Inbox, { items: inbox.items, loaded: true }), "the inbox");
  for (const item of inbox.items as Json[]) plain(html(Inbox, { items: [item], loaded: true }), `the inbox item ${item.title}`);
  plain(html(InboxList, { items: inbox.items, loaded: true, open: () => {} }), "the inbox list");
  plain(html(Answer, { items: inbox.items }), "the answer window");
  specs.projects = specsRecorded.projects as Json;
  plain(html(PlanList, { open: () => {} }), "the Specifications list");
  for (const p of specsRecorded.projects as Json[])
    for (const r of p.plans) plain(html(PlanDoc, { focus: key(p, r) }), `the specification ${r.plan.path}`);
  plain(html(Palette, { from: "#inbox", changes }), "the palette");
  plain(html(Board, { runs: board.runs ?? [], summary: board.summary, loaded: true }), "the sessions");
  plain(html(Why, {}), "the ledger");
  plain(html(GithubList, { loaded: true, failed: forge.error }), "the forge, failed");
  plain(html(Setup, {}), "the setup");
  plain(html(Failed, { what: "the forge", failure: { says: "gh is not logged in — run `gh auth login`", tell: "devplane doctor" } }), "a failure");
  plain(html(Empty, { title: "Nothing", body: "run `devplane doctor` for **more**" }), "an empty region");
  plain(html(Md, { text: "- T001 `src/a.ts` **done**\n- T002" }), "an agent's list");
  // The checks themselves: a riddle and a backtick are found.
  if (!RIDDLES.test("3 tasks alone") || !/`/.test(read("<p>`x`</p>"))) fail("the plain-words check finds nothing it should");
}

// ── One unread: header, Review tab and pane say the same words ────────────
{
  const rate: Json = recorded("change", "Rate-limit the login route");
  const review: Json = recorded("review", "Rate-limit the login route");
  const says = unreadSays(unread(review, rate.qualifier?.unseen ?? 0));
  if (!/\d+ hunks? unread · 1 weakened/.test(says)) fail(`the unread words are not "n hunks unread · 1 weakened": ${says}`);
  const doc = read(html(Doc, { id: rate.id, detail: rate, review, loaded: true }));
  const tab = read(html(Doc, { id: rate.id, detail: rate, review, loaded: true, view: "review" }));
  const pane = read(html(Pane, { id: "", review }));
  // The Review button, the Review tab's count and the pane's line.
  if ((doc.split(says).length - 1) < 2) fail(`the header and the Review tab do not both say "${says}"`);
  if (!pane.includes(says)) fail(`the review pane does not say "${says}"`);
  if (!tab.includes(says)) fail(`the Review tab does not say "${says}"`);
  // Evidence first: with a weakened row unread, no green sentence about the check.
  if (/exited zero against the working tree as it stands/.test(doc)) fail("the green check sentence is shown over an unread weakened check");
  const seen = { ...rate, qualifier: { ...rate.qualifier, unseen: 0 } };
  if (!/exited zero/.test(read(html(Doc, { id: rate.id, detail: seen, loaded: true })))) fail("the check sentence is gone even with every row read");
}

// ── Tasks: one row of counts, and no gates only when the host says so ────
{
  const rate: Json = recorded("change", "Rate-limit the login route");
  const counts = { tasks: 5, ticked: 2, seen_by_pass: 2, ticked_unsent: [], sent_unticked: [] };
  const tasks = read(html(Tasks, { d: { ...rate, counts }, lastRun: "" }));
  if (!tasks.includes("5 tasks · 3 sent · 2 ticked · 2 seen by a passing check")) fail(`the tasks row is not one line of counts: ${tasks.slice(0, 160)}`);
  if ((tasks.match(/2 ticked/g) ?? []).length !== 1) fail("the tasks view says its ticked count twice");
  if (/no gates declared/.test(tasks)) fail("no gates declared is said where gates are declared");
  const none = read(html(Tasks, { d: { ...rate, counts: { ...counts, seen_by_pass: null } }, lastRun: "" }));
  if (!none.includes("no gates declared")) fail("the host's no-gates count is not said");
  // A count the host did not send (an older recording) is not read as *no gates*.
  const { seen_by_pass: _gone, ...older } = counts;
  void _gone;
  if (/no gates declared/.test(read(html(Tasks, { d: { ...rate, counts: older }, lastRun: "" })))) fail("an absent count is said as no gates declared");
  if (!read(html(Tasks, { d: rate, lastRun: "" })).includes("Sent to the latest run")) fail("the sent section is not headed in plain words");
}
