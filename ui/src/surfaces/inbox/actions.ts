// What an inbox row's controls do — one copy, shared by the inbox and the
// answer window. Each action returns what it did as a sentence, so a refusal
// is never silent.

import { api } from "../../lib/api";
import type { Answer, Item } from "./Item.svelte";

/// What an action reports, and — for the one action that can be taken back —
/// how to take it back.
export type Outcome = { said: string; undo: { says: string; where: string } | null };

/// The routes behind the actions that are not answers (`attach` is absent: a
/// browser cannot attach). Each route is written out whole so the guard that
/// checks routes against the host's can read it.
export const ROUTES: Record<
  string,
  { of: "run" | "change"; route: string; says: string; body?: (item: Item) => unknown }
> = {
  focus: { of: "run", route: "/api/runs/{id}/focus", says: "raised the window that owns it" },
  retry: { of: "change", route: "/api/changes/{id}/retry", says: "retrying" },
  resume: { of: "change", route: "/api/changes/{id}/resume", says: "resumed" },
  // The only way a pull request opens. Where Devplane may not push, the host
  // answers with the commands instead.
  offer: {
    of: "change",
    route: "/api/changes/{id}/offer",
    says: "offer made — open the change to see what came of it",
  },
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

/// The routes behind a report row's controls, addressed by the report.
/// `open_draft` is the one control that writes to a forge, under the
/// person's own `gh`.
export const REPORT_ROUTES: Record<string, { route: string; says: string; body?: (reason: string) => unknown }> = {
  start_from_report: {
    route: "/api/reports/{id}/start",
    says: "started a change from it — the report is attached, and is answered as fixed when that change is offered or finished",
  },
  reject_report: {
    route: "/api/reports/{id}/resolve",
    says: "rejected — the project that filed it is told why",
    body: (reason) => ({ as: "rejected", reason }),
  },
  defer_report: {
    route: "/api/reports/{id}/resolve",
    says: "deferred — the project that filed it is told why",
    body: (reason) => ({ as: "deferred", reason }),
  },
  open_draft: {
    route: "/api/reports/{id}/open",
    says: "opened on GitHub with your gh, under your name",
  },
  discard_draft: {
    route: "/api/reports/{id}/resolve",
    says: "discarded — nothing was sent",
    body: () => ({ as: "discarded" }),
  },
};

const failed = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// A report row's control. A rejection or a deferral carries the reason the
/// filer is owed; the host refuses one without.
async function actOnReport(item: Item, action: string, reason: string): Promise<Outcome> {
  const r = REPORT_ROUTES[action];
  if (!r || !item.report) return { said: `${action} cannot be done from here`, undo: null };
  try {
    await api(r.route.replace("{id}", encodeURIComponent(item.report)), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(r.body ? r.body(reason) : {}),
    });
    return { said: r.says, undo: null };
  } catch (e) {
    return { said: `${action.replace(/_/g, " ")} did not land: ${failed(e)}`, undo: null };
  }
}

export async function act(item: Item, action: string, reason = ""): Promise<Outcome> {
  if (REPORT_ROUTES[action]) return actOnReport(item, action, reason);
  const r = ROUTES[action];
  const id = r?.of === "change" ? item.change_id : item.run_id;
  if (!r || !id) return { said: `${action} cannot be done from here`, undo: null };
  try {
    await api(r.route.replace("{id}", encodeURIComponent(id)), {
      method: "POST",
      ...(r.body ? { body: JSON.stringify(r.body(item)) } : {}),
    });
    return { said: r.says, undo: null };
  } catch (e) {
    return { said: `${action} did not land: ${failed(e)}`, undo: null };
  }
}

// Answering is a POST to the ask; the page composes no verdict and no rule.
// A form answer names its `field`.
export async function answer(item: Item, what: Answer): Promise<Outcome> {
  const ask = item.ask ?? item.request_id;
  if (!ask) return { said: "this one cannot be answered from here", undo: null };
  try {
    await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ ...what, from: "board" }),
    });
    // No undo: the answer is on the record and with the agent.
    return {
      said: "answered — on the record and on its way to the agent. That cannot be taken back.",
      undo: null,
    };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

export async function snooze(item: Item): Promise<Outcome> {
  // The route is what the item is about: a change, a run, or the project.
  // Always a whole route, so a guard can read it.
  const where = item.change_id
    ? `/api/changes/${encodeURIComponent(item.change_id)}/snooze`
    : item.run_id
      ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze`
      : item.project_id
        ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze`
        : null;
  if (!where) return { said: "there is nothing to snooze this against", undo: null };
  try {
    await api(`${where}?minutes=60`, { method: "POST" });
    // `minutes=0` is the way back.
    return { said: "hidden for an hour", undo: { says: "put it back", where: `${where}?minutes=0` } };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// Takes back the last snooze.
export async function takeBack(undo: { where: string }): Promise<Outcome> {
  try {
    await api(undo.where, { method: "POST" });
    return { said: "back in the list", undo: null };
  } catch (e) {
    return { said: `that did not land: ${failed(e)}`, undo: null };
  }
}

/// A convenience: the rule is also on screen as selectable text, for browsers
/// that withhold the clipboard.
export async function copyRule(rule: string): Promise<Outcome> {
  try {
    await navigator.clipboard?.writeText(rule);
    return { said: `copied: ${rule}`, undo: null };
  } catch {
    return { said: "no clipboard here — select the rule above", undo: null };
  }
}
