// The key registry: every binding in one list, and a collision is a failure.
// A surface binds through `bind()` from its own `index.ts`; `?` renders
// `help()` from the same list; `scripts/check-keys.mjs` fails the build on a
// collision. Actions are names, so the list can be read without running it.

export type Scope = string | "global";

export type Binding = {
  /// The surface id, or `global` for a key that works on every surface.
  surface: Scope;
  /// `Mod+k`, `?`, `Esc`, `Enter`, `Up`, `Down`, `j`, `G`, or a chord `g g`.
  combo: string;
  /// The name a subscriber listens for.
  action: string;
  /// What `?` prints. Never empty.
  label: string;
};

const bindings: Binding[] = [];
/// A listener says whether it took the action: the first that does ends the
/// dispatch, so a surface's own answer to `leave` beats the shell's.
type Listener = (surface: string) => boolean | void;
const listeners = new Map<string, Set<Listener>>();

/// Registers one binding, or throws naming the pair it collides with.
export function bind(b: Binding): void {
  if (!b.label.trim()) throw new Error(`${b.surface}: ${b.combo} (${b.action}) has no label`);
  const combo = normalise(b.combo);
  const same = bindings.find((x) => x.combo === combo && x.surface === b.surface);
  if (same) {
    throw new Error(
      `${b.combo} is bound twice in ${b.surface}: "${same.label}" (${same.action}) and "${b.label}" (${b.action})`,
    );
  }
  const global = bindings.find((x) => x.combo === combo && x.surface === "global");
  if (global && b.surface !== "global") {
    throw new Error(
      `${b.surface} rebinds ${b.combo}, which is global: "${global.label}" (${global.action}) against "${b.label}" (${b.action})`,
    );
  }
  const shadowed = b.surface === "global" ? bindings.find((x) => x.combo === combo) : undefined;
  if (shadowed) {
    throw new Error(
      `global ${b.combo} ("${b.label}", ${b.action}) is already bound in ${shadowed.surface}: "${shadowed.label}" (${shadowed.action})`,
    );
  }
  bindings.push({ ...b, combo });
}

/// Every binding, in the order bound.
export function all(): readonly Binding[] {
  return bindings;
}

/// What `?` prints for a surface: the globals, then the surface's own.
export function help(surface: string): Binding[] {
  return [
    ...bindings.filter((b) => b.surface === "global"),
    ...bindings.filter((b) => b.surface === surface),
  ];
}

/// Subscribes to an action by name. Returns the unsubscribe.
export function onAction(action: string, fn: Listener): () => void {
  let set = listeners.get(action);
  if (!set) {
    set = new Set();
    listeners.set(action, set);
  }
  set.add(fn);
  return () => {
    set?.delete(fn);
  };
}

/// Runs an action by name, as a key would.
export function run(action: string, surface: string): boolean {
  const set = listeners.get(action);
  if (!set || set.size === 0) return false;
  for (const fn of set) {
    if (fn(surface) === true) return true;
  }
  return true;
}

/// A chord's first key, waiting for its second.
let pending: string | null = null;
let pendingAt = 0;
const CHORD_MS = 800;

/// The combo a key event spells: modifiers first, `Mod` for ⌘ on a Mac and
/// Ctrl elsewhere, then the key as typed.
export function combo(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("Mod");
  if (e.altKey) parts.push("Alt");
  const key = keyName(e.key);
  // Shift is spelled by the character itself — `?`, `G` — so it is only
  // named where the key has no shifted form.
  if (e.shiftKey && key.length > 1) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

function keyName(key: string): string {
  switch (key) {
    case "Escape":
      return "Esc";
    case "ArrowUp":
      return "Up";
    case "ArrowDown":
      return "Down";
    case "ArrowLeft":
      return "Left";
    case "ArrowRight":
      return "Right";
    case " ":
      return "Space";
    default:
      return key;
  }
}

function normalise(c: string): string {
  return c
    .split(" ")
    .map((part) =>
      part
        .split("+")
        .map((k) => (k === "Escape" ? "Esc" : k === "Cmd" || k === "Ctrl" || k === "Meta" ? "Mod" : k))
        .join("+"),
    )
    .join(" ");
}

/// Whether the event came from somewhere text is typed.
export function typing(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  if (!t) return false;
  const tag = t.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || t.isContentEditable;
}

/// Dispatches one key event for the surface on screen; returns whether a
/// binding took it. Nothing but `Esc` fires while an input has focus.
export function dispatch(surface: string, e: KeyboardEvent): boolean {
  const c = combo(e);
  if (typing(e) && c !== "Esc") return false;
  const scoped = (x: Binding) => x.surface === surface || x.surface === "global";

  const now = Date.now();
  if (pending && now - pendingAt < CHORD_MS) {
    const chord = `${pending} ${c}`;
    pending = null;
    const hit = bindings.find((b) => scoped(b) && b.combo === chord);
    if (hit) {
      e.preventDefault();
      run(hit.action, surface);
      return true;
    }
  }
  pending = null;

  const hit = bindings.find((b) => scoped(b) && b.combo === c);
  if (hit) {
    e.preventDefault();
    run(hit.action, surface);
    return true;
  }
  // The first half of a chord this surface knows.
  if (bindings.some((b) => scoped(b) && b.combo.startsWith(`${c} `))) {
    pending = c;
    pendingAt = now;
    e.preventDefault();
    return true;
  }
  return false;
}

// The keys every surface has. `?` and `Esc` are the shell's; the palette's
// opener is bound here and answered by the palette, which subscribes to it.
bind({ surface: "global", combo: "Mod+k", action: "open-palette", label: "command palette" });
bind({ surface: "global", combo: "?", action: "help", label: "keys bound here" });
bind({ surface: "global", combo: "Esc", action: "leave", label: "leave: close the palette or the help, or back to the list" });

/// The list keys, for every surface that shows one. Bound per surface, so
/// each is in exactly one scope and `?` lists them where they work.
export function bindList(surface: string): void {
  bind({ surface, combo: "Down", action: "next", label: "next" });
  bind({ surface, combo: "j", action: "next", label: "next" });
  bind({ surface, combo: "Up", action: "prev", label: "previous" });
  bind({ surface, combo: "k", action: "prev", label: "previous" });
  bind({ surface, combo: "Enter", action: "open", label: "open" });
  bind({ surface, combo: "g g", action: "first", label: "first" });
  bind({ surface, combo: "G", action: "last", label: "last" });
}
