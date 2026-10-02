// The keys on one inbox item, bound into every surface that shows one — the
// inbox and the answer window — from this one list, so both offer the same
// keys and `?` lists them under *this item*. Each key is printed on the
// control it triggers, and a key whose control is not on the item says why
// in place rather than doing nothing. No key here offers a change: offering
// is the change's own, after its review.

import { bind, onAction, spell } from "../../lib/keys";
import type { Item } from "./Item.svelte";

/// One row of the table: the key, the action it runs, what `?` says.
export const ITEM_KEYS: ReadonlyArray<{ combo: string; action: string; label: string }> = [
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => ({ combo: String(n), action: "item-option", label: "choose the option with that number" })),
  { combo: "t", action: "item-type", label: "type an answer in your own words" },
  { combo: "Mod+Enter", action: "item-send", label: "send the typed answer (from inside the field)" },
  { combo: "a", action: "item-allow", label: "allow the permission, once" },
  { combo: "d", action: "item-deny", label: "deny the permission" },
  { combo: "w", action: "item-session", label: "reach the session the vendor owns: open its link, or copy its resume command" },
  { combo: "s", action: "item-snooze", label: "snooze the item for an hour" },
  { combo: "o", action: "item-open", label: "open the change or session the item is about" },
  { combo: "r", action: "item-review", label: "open the change's review" },
  { combo: "z", action: "item-undo", label: "undo: take back the answer before it is sent, or put a snoozed item back" },
];

/// Binds the item keys into one surface's scope.
export function bindItemKeys(surface: string): void {
  for (const k of ITEM_KEYS) bind({ surface, ...k, item: true });
}

/// The vendor that owns a dialog Devplane cannot answer, out of the host's
/// sentence (`Claude Code owns this dialog — …`), for saying where it is.
function owner(item: Item): string {
  return /^(.+?) owns this dialog/.exec(item.answer_in ?? "")?.[1] ?? "the agent's own window";
}

/// How many options the keys `1`–`9` can reach on this item: the first
/// unanswered field's on a form, the flat list's otherwise.
export function optionCount(item: Item, picked: ReadonlySet<string> = new Set()): number {
  const has = (a: string) => (item.actions ?? []).includes(a);
  if (!has("choose") && !(item.form ?? []).length) return 0;
  const field = (item.form ?? []).find((q) => !picked.has(q.field));
  if (item.form?.length) return field?.options.length ?? 0;
  return (item.options ?? []).filter((o) => o.id).length;
}

/// Why a key does nothing on this item, in words — or `""` when it applies.
/// The item's own controls are the test: a key applies exactly where the
/// control it is printed on is shown.
export function whyNot(item: Item, action: string, n = 0): string {
  const acts = item.actions ?? [];
  const has = (a: string) => acts.includes(a);
  const vendor = !!item.answer_in;
  const reach = item.session_link || item.resume_command ? " — w reaches its session" : "";
  const kind = item.kind.replace(/_/g, " ");
  switch (action) {
    case "item-option": {
      const count = optionCount(item);
      if (vendor) return `${owner(item)} owns this question: answer it there${reach}`;
      if (count === 0) return `this ${kind} has no options to choose`;
      if (n > count) return `there ${count === 1 ? "is 1 option" : `are ${count} options`}; ${n} picks nothing`;
      return "";
    }
    case "item-type":
    case "item-send":
      if (has("reply") || (item.form ?? []).some((q) => q.custom_field)) return "";
      return vendor ? `${owner(item)} owns this question: answer it there${reach}` : `this ${kind} takes no answer in words`;
    case "item-allow":
    case "item-deny": {
      const want = action === "item-allow" ? "allow" : "deny";
      if (has(want)) return "";
      if (vendor) return `${owner(item)} owns this permission: Devplane cannot answer it${reach}`;
      return `${action === "item-allow" ? "a" : "d"} answers a permission; this is a ${kind}`;
    }
    case "item-session":
      if (vendor && (item.session_link || item.resume_command)) return "";
      if (vendor) return `${owner(item)} documents no way to reach this session from here`;
      return "w is for a dialog the vendor owns; this one is answered here";
    case "item-snooze":
      if (has("snooze")) return "";
      if (item.kind === "permission" || item.kind.startsWith("question"))
        return "a question or a permission is never snoozed: it waits for an answer";
      return `this ${kind} cannot be snoozed`;
    case "item-open":
      return (has("open") || has("review")) && (item.change_id || item.run_id) ? "" : `this ${kind} is about nothing that opens`;
    case "item-review":
      return has("review") && item.change_id ? "" : "r opens the review of a change ready to decide";
    default:
      return "";
  }
}

/// Answers the item keys for the item shown in `root`, on `scope`. Each key
/// presses the control printed with it (`data-key`), so a key and its button
/// can never do two different things; where that control is absent, the key
/// says why. Returns the unsubscribe, for an `$effect`.
export function answerItemKeys(
  scope: string,
  root: () => HTMLElement | undefined,
  item: () => Item,
  say: (sentence: string) => void,
): () => void {
  const control = (k: string) => root()?.querySelector<HTMLElement>(`[data-key="${k}"]`) ?? null;
  const press =
    (action: string, run?: (el: HTMLElement) => void) =>
    (surface: string, combo: string): boolean => {
      if (surface !== scope) return false;
      const el = control(combo);
      // An answer already out: the control is disabled, and so is its key.
      if (el && ((el as HTMLButtonElement).disabled || el.closest("fieldset:disabled"))) return true;
      if (el) (run ?? ((x: HTMLElement) => x.click()))(el);
      else say(whyNot(item(), action, Number(combo) || 0) || `${spell(combo)} does nothing on this item right now`);
      return true;
    };
  const offs = [
    onAction("item-option", press("item-option")),
    onAction("item-type", press("item-type", (el) => el.focus())),
    // From inside the field only: the send beside the field it is in.
    onAction("item-send", (surface) => {
      if (surface !== scope) return false;
      const field = document.activeElement as HTMLElement | null;
      const send = field && root()?.contains(field) ? field.closest(".reply")?.querySelector<HTMLElement>("[data-send]") : null;
      if (send) send.click();
      else say(whyNot(item(), "item-send") || "this sends from inside the answer field — t puts you there");
      return true;
    }),
    onAction("item-allow", press("item-allow")),
    onAction("item-deny", press("item-deny")),
    onAction("item-session", press("item-session")),
    onAction("item-snooze", press("item-snooze")),
    onAction("item-open", press("item-open")),
    onAction("item-review", press("item-review")),
  ];
  return () => offs.forEach((off) => off());
}
