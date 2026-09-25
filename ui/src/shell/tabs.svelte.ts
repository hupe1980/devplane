// The editor's open tabs. The address is the source of truth
// (`#<surface>/<focus>` says what is showing); tabs are a memory laid over it.
//
// A glance opens into one *preview* tab that the next glance replaces;
// double-click, Enter or an edit pins it. Kept in `sessionStorage` only.

export type Tab = { surface: string; focus: string; pinned: boolean };

const KEY = "vp-tabs";

function load(): Tab[] {
  try {
    const v = JSON.parse(sessionStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((t) => typeof t?.surface === "string") : [];
  } catch {
    return [];
  }
}

export const tabs = $state<{ list: Tab[]; active: number }>({ list: load(), active: -1 });

function keep() {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(tabs.list));
  } catch {
    /* the tabs last the page */
  }
}

const same = (t: Tab, surface: string, focus: string) => t.surface === surface && t.focus === focus;

/// The address changed: show that as a tab, reusing the preview slot.
export function show(surface: string, focus: string, pin = false): void {
  const at = tabs.list.findIndex((t) => same(t, surface, focus));
  if (at !== -1) {
    if (pin) tabs.list[at].pinned = true;
    tabs.active = at;
    keep();
    return;
  }
  const preview = tabs.list.findIndex((t) => !t.pinned);
  const tab = { surface, focus, pinned: pin };
  if (preview !== -1) {
    tabs.list[preview] = tab;
    tabs.active = preview;
  } else {
    tabs.list.push(tab);
    tabs.active = tabs.list.length - 1;
  }
  keep();
}

/// Pins the active tab — an edit, a double-click, Enter.
export function pin(): void {
  const t = tabs.list[tabs.active];
  if (t && !t.pinned) {
    t.pinned = true;
    keep();
  }
}

/// Closes a tab and says which tab, if any, is showing afterwards.
export function close(i: number): Tab | null {
  if (i < 0 || i >= tabs.list.length) return null;
  tabs.list.splice(i, 1);
  if (tabs.list.length === 0) {
    tabs.active = -1;
    keep();
    return null;
  }
  if (tabs.active >= i) tabs.active = Math.max(0, tabs.active - 1);
  keep();
  return tabs.list[tabs.active];
}

/// Closes every tab but the active one.
export function closeOthers(): void {
  const t = tabs.list[tabs.active];
  tabs.list = t ? [t] : [];
  tabs.active = t ? 0 : -1;
  keep();
}

export function hashOf(t: Tab): string {
  return `#${t.surface}${t.focus ? `/${encodeURIComponent(t.focus)}` : ""}`;
}
