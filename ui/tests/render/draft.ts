// A specification drafted by the project's own methodology: what the change
// shows of it, the person's two decisions, the marker offer, and an inbox
// marker's answer controls.
import ChangeDraft from "../../src/surfaces/change/Draft.svelte";
import NewDraft from "../../src/surfaces/new/Draft.svelte";
import Marker from "../../src/surfaces/inbox/Marker.svelte";
import { fail, html } from "../harness";

const sent = "/speckit-specify Limit login attempts per account. Lock after five tries.";
const draft = (outcome: string) => ({
  methodology: "speckit" as const,
  agent: "claude",
  sent,
  folder: "specs/007-rate-limit",
  outcome: outcome as "pending" | "produced" | "none_produced" | "accepted" | "rejected",
});
const noop = () => {};

// ── What was sent is shown whole, and the decisions only once the turn ended ──
{
  const produced = html(ChangeDraft, {
    d: { id: "c-1", project_id: "/r", draft: draft("produced"), draft_says: ["the command wrote specs/007-rate-limit; review it, then accept or reject it"] },
    review: noop,
    reload: noop,
  });
  if (!produced.includes(sent)) fail("the change does not show exactly what the agent was sent");
  if (!/Review the draft/.test(produced) || !/Accept/.test(produced) || !/Reject/.test(produced))
    fail("a produced draft offers no review, accept and reject");
  if (!produced.includes("Spec Kit")) fail("the draft does not name whose command wrote it");

  const pending = html(ChangeDraft, { d: { id: "c-1", draft: draft("pending") }, review: noop, reload: noop });
  if (/Accept|Reject/.test(pending)) fail("a draft still being written can be accepted or rejected");

  const nothing = html(ChangeDraft, { d: { id: "c-1", draft: draft("none_produced") }, review: noop, reload: noop });
  if (/>Accept</.test(nothing)) fail("a draft that produced nothing can be accepted");
  if (!/Reject/.test(nothing)) fail("a draft that produced nothing cannot be rejected");

  const accepted = html(ChangeDraft, { d: { id: "c-1", draft: draft("accepted") }, review: noop, reload: noop });
  if (/Accept|Reject/.test(accepted)) fail("an accepted draft is offered the decision again");
}

// ── The marker setting: a diff where the tool defines a word, a sentence where it does not ──
{
  const speckit = html(ChangeDraft, {
    d: { id: "c-1", project_id: "/r", draft: draft("produced"), open_questions_offer: { says: "no words", diff: "+open_questions = [\"NEEDS CLARIFICATION\"]\n", base: "abc" } },
    review: noop,
    reload: noop,
  });
  if (!speckit.includes("open_questions = [&quot;NEEDS CLARIFICATION&quot;]") && !speckit.includes('open_questions = ["NEEDS CLARIFICATION"]'))
    fail("the offered diff is not shown before it is written");
  if (!/Add it to devplane.toml/.test(speckit)) fail("the offered diff cannot be accepted");
  const openspec = html(ChangeDraft, {
    d: { id: "c-1", draft: { ...draft("produced"), methodology: "openspec" as const }, open_questions_offer: { says: "OpenSpec defines no unresolved-question word" } },
    review: noop,
    reload: noop,
  });
  if (/Add it to devplane.toml/.test(openspec)) fail("a word is offered that OpenSpec does not define");
  if (!/OpenSpec defines no unresolved-question word/.test(openspec)) fail("the OpenSpec offer says nothing");
}

// ── A marker in the inbox: answered in place when bracketed, sent to the agent when not ──
{
  const m = { path: "specs/007-rate-limit/spec.md", line: 42, text: "- [NEEDS CLARIFICATION: which?]", bracketed: true };
  const bracketed = html(Marker, { item: { change_id: "c-1", actions: ["answer", "open", "snooze"], marker: m }, say: noop });
  if (!bracketed.includes("specs/007-rate-limit/spec.md:42")) fail("a marker item does not name its file and line");
  if (!/data-action="answer"/.test(bracketed) || !/show the edit/.test(bracketed))
    fail("a bracketed marker cannot be answered, or is written without the edit shown first");
  const loose = html(Marker, {
    item: { change_id: "c-1", run_id: "acp-1", actions: ["tell_agent", "open", "snooze"], marker: { ...m, bracketed: false } },
    say: noop,
  });
  if (!/data-action="tell_agent"/.test(loose) || !/send to the agent/.test(loose)) fail("an unbracketed marker's answer cannot reach the agent");
  if (/show the edit/.test(loose)) fail("Devplane offers to edit a line it does not edit");
}

// ── The draft form says what is sent, and that Devplane writes none of it ──
{
  const form = html(NewDraft, { projects: [{ id: "/r", name: "r", root: "/r", trusted: true }], agents: [], implement: noop, close: noop });
  if (!/Draft a specification/.test(form)) fail("the draft form has no heading");
  if (!/Devplane writes no word of the/.test(form)) fail("the draft form does not say Devplane writes none of the specification");
  if (!/What the agent will be sent/.test(form)) fail("the draft form does not show what will be sent");
}
