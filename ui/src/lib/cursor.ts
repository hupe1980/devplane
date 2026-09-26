// The list keys `bindList` binds, answered — one implementation for every
// list. The position is the caller's (usually the address), not a second
// copy kept here, so the cursor and the selection cannot disagree. Nothing
// animates.

import { onAction } from "./keys";

export type Cursor = {
  /// The surface whose list this is: on any other the keys are not taken.
  scope: string;
  size: () => number;
  /// Where the cursor is now, `-1` for nowhere.
  at: () => number;
  /// Moves the cursor to `index` (always within the list).
  go: (index: number) => void;
  /// What Enter does on the row under the cursor.
  open: (index: number) => void;
};

/// Subscribes to the list actions for one list. Call inside `$effect`; it
/// returns the unsubscribe, so a list that leaves the screen stops listening.
export function cursor(c: Cursor): () => void {
  const clamp = (i: number) => Math.min(Math.max(i, 0), c.size() - 1);
  /// A listener for one action: declines off its surface or with no rows.
  const on = (action: string, move: (at: number) => number | null) =>
    onAction(action, (surface) => {
      if (surface !== c.scope || c.size() === 0) return false;
      const to = move(c.at());
      if (to !== null) c.go(clamp(to));
      return true;
    });
  const offs = [
    on("next", (at) => at + 1),
    on("prev", (at) => (at < 0 ? 0 : at - 1)),
    on("first", () => 0),
    on("last", () => c.size() - 1),
    on("open", (at) => {
      c.open(clamp(at < 0 ? 0 : at));
      return null;
    }),
  ];
  return () => offs.forEach((off) => off());
}
