// What an inbox row's controls do — one copy, shared by the inbox and the
// answer window. Each action returns what it did as a sentence, so a refusal
// is never silent.

import { api } from "../../lib/api";
import { copyText } from "../../lib/resource.svelte";
import { safeHref } from "../../lib/href";
import { spell } from "../../lib/keys";
import type { Command } from "../../lib/offered.svelte";
import type { Answer, Item } from "./Item.svelte";

/// What an action reports, and — for the one action that can be taken back —
/// how to take it back. `commands` are what the host handed back for the
/// person to run instead (a push it was not allowed to make); they are shown
/// to copy, never summarised as done.
export type Outcome = { said: string; undo: { says: string; where: string } | null; commands?: string[] };

/// What the host answered a write with. Its own sentence wins over ours, and
/// commands it hands back are the outcome, not a detail.
type Answered = { says?: string; push?: string; create?: string; offer?: string } | null;
function outcome(r: Answered, ours: string): Outcome {
  const commands = [r?.push, r?.create].filter((c): c is string => !!c);
  if (commands.length) {
    return { said: r?.says ?? "Nothing was pushed — run these yourself:", undo: null, commands };
  }
  return { said: r?.says ?? ours, undo: null };
}

/// The routes behind the actions that are not answers (`attach` is absent: a
/// browser cannot attach). Each route is written out whole so the guard that
/// checks routes against the host's can read it.
export const ROUTES: Record<
  string,
  { of: "run" | "change"; route: string; says: string; body?: (item: Item) => unknown }
> = {
  retry: { of: "change", route: "/api/changes/{id}/retry", says: "retrying" },
  resume: { of: "change", route: "/api/changes/{id}/resume", says: "resumed" },
  // No `offer`: the inbox never offers a change. Offering happens from the
  // change itself, after its review, where a weakened check is read first.
  // Both name the run the drift is about; the route is the change's.
  tell_run: {
    of: "change",
    route: "/api/changes/{id}/drift/tell",
    says: "told — the run was handed the files that changed, and the decision is recorded as yours",
    body: (item) => ({ run: item.run_id }),
  },
  accept_drift: {
    of: "change",
    route: "/api/changes/{id}/drift/accept",
    says: "accepted — the change now works to what the run saw, and the decision is recorded as yours",
    body: (item) => ({ run: item.run_id }),
  },
};

const failed = (e: unknown) => (e instanceof Error ? e.message : String(e));

export async function act(item: Item, action: string): Promise<Outcome> {
  const r = ROUTES[action];
  const id = r?.of === "change" ? item.change_id : item.run_id;
  if (!r || !id) return { said: `${action} cannot be done from here`, undo: null };
  try {
    const res = await api<Answered>(r.route.replace("{id}", encodeURIComponent(id)), {
      method: "POST",
      ...(r.body ? { body: JSON.stringify(r.body(item)) } : {}),
    });
    return outcome(res, r.says);
  } catch (e) {
    return { said: `${action} did not land: ${failed(e)}`, undo: null };
  }
}

/// What the host holds after an answer from here, before it is sent.
export type Held = { ask: string; run: string; title: string; says: string; until: string; undo: string };
/// What the last held answer came to once sent: delivered, or why not.
export type Landed = { ask: string; title: string; ok: boolean; says: string };

// Answering is a POST to the ask; the page composes no verdict and no rule.
// A form answer names its `field`. The host holds the answer for a few
// seconds before it reaches the agent — `z` takes it back in that time — and
// sends it when the hold ends even if this page has closed. The undo is the
// host's (`held` on the inbox), not this outcome's, so a reload keeps it.
export async function answer(item: Item, what: Answer): Promise<Outcome> {
  const ask = item.ask ?? item.request_id;
  if (!ask) return { said: "this one cannot be answered from here", undo: null };
  try {
    const r = await api<{ held?: Held | null }>(`/api/asks/${encodeURIComponent(ask)}/answer`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ ...what, hold: true }),
    });
    // Once sent it is on the record and with the agent: it cannot be taken
    // back after that, and the held line says so when it happens.
    return {
      said: r?.held ? `${r.held.says} — held; z takes it back before it is sent` : "answered — on its way to the agent. That cannot be taken back.",
      undo: null,
    };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// The route a snooze of this item goes to — what it is about: a change, a
/// run, or the project (a pull request or an issue). Always a whole route,
/// so a guard can read it.
function snoozeRoute(item: Item): string | null {
  return item.change_id
    ? `/api/changes/${encodeURIComponent(item.change_id)}/snooze`
    : item.run_id
      ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze`
      : item.project_id
        ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze`
        : null;
}

/// Whether a snooze of this item can wait for activity: a pull request or
/// an issue has no session to report any, so it is snoozed for an hour.
export function untilActivity(item: Item): boolean {
  return !!(item.change_id || item.run_id);
}

/// `s`: hides this one kind until its session (or change) next does
/// something; `z` or the snoozed list brings it back. A question or a
/// permission is never snoozed — the host refuses, and so does the key.
export async function snooze(item: Item): Promise<Outcome> {
  if (!untilActivity(item)) return snoozeHour(item);
  const where = snoozeRoute(item);
  if (!where) return { said: "there is nothing to snooze this against", undo: null };
  const kind = encodeURIComponent(item.kind);
  try {
    await api(`${where}?until=activity&kind=${kind}`, { method: "POST" });
    return {
      said: `snoozed until the ${item.change_id ? "change" : "session"} next reports activity — counted under snoozed`,
      undo: { says: "put it back", where: `${where}?minutes=0&kind=${kind}` },
    };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// The keyless snooze beside `s`: for an hour, whatever happens meanwhile.
export async function snoozeHour(item: Item): Promise<Outcome> {
  const where = snoozeRoute(item);
  if (!where) return { said: "there is nothing to snooze this against", undo: null };
  const kind = encodeURIComponent(item.kind);
  try {
    await api(`${where}?minutes=60&kind=${kind}`, { method: "POST" });
    return { said: "hidden for an hour — counted under snoozed", undo: { says: "put it back", where: `${where}?minutes=0&kind=${kind}` } };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// One thing a snooze keeps off the list, as the host lists it: what it is,
/// when it comes back, and the route that brings it back now.
export type { SnoozedRow } from "../../wire/SnoozedRow";
import type { SnoozedRow } from "../../wire/SnoozedRow";

/// Brings one snoozed item back into the list.
export async function bringBack(row: SnoozedRow): Promise<Outcome> {
  try {
    await api(row.back, { method: "POST" });
    return { said: `${row.title} — back in the list`, undo: null };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// One action a row offers: the same set its controls, its keys and the
/// palette show. `id` is what the control carries as `data-action`; `key`
/// is the key printed on it, if any.
export type RowAction = { id: string; label: string; key: string | null };

/// Every action on this item's controls, in the order they are shown. A
/// function of the item alone; it never yields an offer — the inbox offers
/// nothing, whatever the review says.
export function rowActions(item: Item): RowAction[] {
  const acts = item.actions ?? [];
  const has = (a: string) => acts.includes(a);
  const out: RowAction[] = [];
  const form = (item.form ?? []).length > 0 ? (item.form ?? []) : null;
  if (form && (has("choose") || has("reply"))) {
    form.forEach((q, qi) => {
      q.options.forEach((o, oi) =>
        out.push({ id: `option:${q.field}:${oi}`, label: form.length > 1 ? `${q.title}: ${o.label}` : o.label, key: qi === 0 && oi < 9 ? String(oi + 1) : null }),
      );
      if (q.custom_field) out.push({ id: `reply:${q.field}`, label: `answer in your own words: ${q.title}`, key: qi === 0 ? "t" : null });
    });
    if (form.length > 1) out.push({ id: "send_form", label: `send the ${form.length} answers`, key: null });
  } else {
    if (has("choose")) {
      let n = 0;
      for (const o of item.options ?? []) {
        if (!o.id) continue;
        n += 1;
        out.push({ id: `option:${n}`, label: o.label, key: n <= 9 ? String(n) : null });
      }
    }
    if (has("reply")) out.push({ id: "reply", label: "answer in your own words", key: "t" });
  }
  if (item.offer) out.push({ id: "copy_rule", label: "copy the rule", key: null });
  const sessionLink = safeHref(item.session_link);
  if (item.answer_in && (sessionLink || item.resume_command))
    out.push({ id: "session", label: sessionLink ? "open the session in VS Code" : "copy the resume command", key: "w" });
  if (has("allow")) out.push({ id: "allow", label: "allow, once", key: "a" });
  if (has("deny")) out.push({ id: "deny", label: "deny", key: "d" });
  if (has("retry")) out.push({ id: "retry", label: "retry", key: null });
  if (has("resume")) out.push({ id: "resume", label: "resume", key: null });
  if (has("review") && item.change_id) out.push({ id: "review", label: "open the review", key: "r" });
  if (has("tell_run")) out.push({ id: "tell_run", label: "tell the run", key: null });
  if (has("accept_drift")) out.push({ id: "accept_drift", label: "accept the drift", key: null });
  if (has("answer")) out.push({ id: "answer", label: "answer in the specification", key: null });
  if (has("tell_agent")) out.push({ id: "tell_agent", label: "send your answer to the agent", key: null });
  if (has("open") && (item.run_id || item.change_id)) out.push({ id: "open", label: item.change_id ? "open the change" : "open the session", key: "o" });
  if ((has("open_pr") || has("open_issue")) && safeHref(item.url))
    out.push({ id: has("open_pr") ? "open_pr" : "open_issue", label: has("open_pr") ? "open pull request" : "open issue", key: null });
  if (safeHref(item.launch)) out.push({ id: "launch", label: "open an agent with this typed", key: null });
  if (has("snooze")) {
    const activity = untilActivity(item);
    out.push({ id: "snooze", label: activity ? "snooze until there is new activity" : "snooze for an hour", key: "s" });
    if (activity) out.push({ id: "snooze_hour", label: "snooze for an hour", key: null });
  }
  if (has("attach") && item.run_id) out.push({ id: "attach", label: "copy the attach command", key: null });
  return out;
}

/// The row's actions as the palette lists them — first, each with its key
/// spelled as the help sheet spells it. `run` takes one action.
export function rowCommands(item: Item, run: (a: RowAction) => void): Command[] {
  return rowActions(item).map((a) => ({
    label: a.label,
    icon: a.key ? "keyboard" : "right",
    hint: a.key ? spell(a.key) : undefined,
    run: () => run(a),
  }));
}

/// Runs a row action by pressing the control that carries it, so the
/// palette, the key and the button can never do two different things. A
/// field is focused rather than pressed. Returns whether the control was
/// there.
export function pressAction(root: ParentNode | null | undefined, id: string): boolean {
  const el = root?.querySelector<HTMLElement>(`[data-action="${CSS.escape(id)}"]`);
  if (!el) return false;
  if (el instanceof HTMLInputElement) el.focus();
  else el.click();
  return true;
}

/// Takes back the last snooze, or the answer the host is holding.
export async function takeBack(undo: { where: string }): Promise<Outcome> {
  try {
    const r = await api<{ says?: string } | null>(undo.where, { method: "POST" });
    return { said: r?.says ?? "back in the list", undo: null };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// Copies a command for the person to run, and says whether it was copied.
export async function copyCommand(cmd: string, then: string): Promise<string> {
  return (await copyText(cmd)) ? `copied: ${cmd}${then}` : `nothing was copied: this page has no clipboard — run ${cmd}`;
}

/// A convenience: the rule is also on screen as selectable text, for browsers
/// that withhold the clipboard.
export async function copyRule(rule: string): Promise<Outcome> {
  return (await copyText(rule))
    ? { said: `copied: ${rule}`, undo: null }
    : { said: "nothing was copied: this page has no clipboard — select the rule above", undo: null };
}
