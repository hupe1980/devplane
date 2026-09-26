// Which item the inbox shows, from its address and what is left in the list.
//
// The address says what it names: `#inbox/item=<id>` a row, `#inbox/ask=<id>`
// a link naming an ask, and a bare `#inbox/<name>` a project. A named row or
// ask that has left the list is said to have left — the page never quietly
// shows another item as if it were the one asked for.

import type { Item } from "./Item.svelte";

/// What the inbox address names.
export type Address = { item: string; wanted: string; project: string };

/// Reads the focus half of `#inbox/<focus>`. A bare id that matches a row is
/// still taken as that row, so an older address keeps working.
export function address(focus: string, items: ReadonlyArray<{ id: string }>): Address {
  if (focus.startsWith("item=")) return { item: focus.slice("item=".length), wanted: "", project: "" };
  if (focus.startsWith("ask=")) return { item: "", wanted: focus.slice("ask=".length), project: "" };
  if (focus && items.some((i) => i.id === focus)) return { item: focus, wanted: "", project: "" };
  return { item: "", wanted: "", project: focus };
}

/// The focus that names one row.
export const itemFocus = (id: string) => `item=${id}`;

/// Where the last named row sat. When it leaves the list (answered, or
/// resolved elsewhere) the row that slid into its place is shown next, not
/// the top of the list. Shared by the list and the item beside it.
const last = $state({ id: "", index: 0 });

/// Records where a named row sits; call from an effect, never a derivation.
export function remember(id: string, index: number): void {
  if (last.id !== id || last.index !== index) {
    last.id = id;
    last.index = index;
  }
}

/// `item` when the named row has left the list, `ask` when the followed ask
/// was already answered, `""` when what is shown is what was named.
export type Gone = "" | "item" | "ask";

export function place(shown: Item[], a: Pick<Address, "item" | "wanted">): { current: Item | null; gone: Gone } {
  if (a.item) {
    const i = shown.findIndex((x) => x.id === a.item);
    if (i !== -1) return { current: shown[i], gone: "" };
    const at = last.id === a.item ? Math.min(last.index, shown.length - 1) : 0;
    return { current: shown[Math.max(0, at)] ?? null, gone: "item" };
  }
  if (a.wanted) {
    const x = shown.find((y) => y.ask === a.wanted || y.request_id === a.wanted);
    return x ? { current: x, gone: "" } : { current: shown[0] ?? null, gone: "ask" };
  }
  return { current: shown[0] ?? null, gone: "" };
}
