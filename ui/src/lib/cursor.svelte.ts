// A cursor over a list, moved by the list keys `bindList` binds — one
// implementation for every list. Nothing animates.

import { onAction } from "./keys";

export function cursor(size: () => number, open: (index: number) => void) {
  let index = $state(0);
  const clamp = (i: number) => Math.min(Math.max(i, 0), Math.max(size() - 1, 0));

  /// Subscribes to the list actions. Call inside `$effect`; it returns the
  /// unsubscribe, so a surface that leaves the screen stops listening.
  function attach(): () => void {
    const offs = [
      onAction("next", () => {
        index = clamp(index + 1);
        return true;
      }),
      onAction("prev", () => {
        index = clamp(index - 1);
        return true;
      }),
      onAction("first", () => {
        index = 0;
        return true;
      }),
      onAction("last", () => {
        index = clamp(size() - 1);
        return true;
      }),
      onAction("open", () => {
        if (size() > 0) open(clamp(index));
        return true;
      }),
    ];
    return () => offs.forEach((off) => off());
  }

  return {
    get index() {
      return clamp(index);
    },
    set index(v: number) {
      index = clamp(v);
    },
    attach,
  };
}
