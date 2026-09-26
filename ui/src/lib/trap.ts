// A modal's focus: kept inside while it is open, and handed back to where it
// was when it closes. Used as `use:trap` on an overlay marked `aria-modal`.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function trap(node: HTMLElement): { destroy: () => void } {
  const before = document.activeElement as HTMLElement | null;
  const inside = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);
  // The overlay's own autofocus wins; otherwise its first control, else itself.
  queueMicrotask(() => {
    if (node.contains(document.activeElement)) return;
    const first = inside()[0];
    if (first) first.focus();
    else {
      if (!node.hasAttribute("tabindex")) node.setAttribute("tabindex", "-1");
      node.focus();
    }
  });
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== "Tab") return;
    const all = inside();
    if (all.length === 0) {
      e.preventDefault();
      return;
    }
    const first = all[0];
    const last = all[all.length - 1];
    const at = document.activeElement;
    if (e.shiftKey && (at === first || !node.contains(at))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (at === last || !node.contains(at))) {
      e.preventDefault();
      first.focus();
    }
  };
  node.addEventListener("keydown", onKey);
  return {
    destroy() {
      node.removeEventListener("keydown", onKey);
      // Back where it was, if that is still on the page.
      if (before && before.isConnected && typeof before.focus === "function") queueMicrotask(() => before.focus());
    },
  };
}
