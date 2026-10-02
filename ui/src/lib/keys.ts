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

/// A surface's current mode, where one region of it takes keys the rest of
/// it binds differently: a change's Review tab moves hunks with `j`/`k` while
/// its list moves changes with them. A mode is a scope `<surface>:<mode>`
/// whose bindings win over the surface's own while it is set.
const modes = new Map<string, string>();

/// Sets a surface's mode; returns the clear, for an `$effect`.
export function setMode(surface: string, mode: string): () => void {
  modes.set(surface, mode);
  return () => {
    if (modes.get(surface) === mode) modes.delete(surface);
  };
}

/// The scopes a key on `surface` is looked up in, most specific first.
function scopes(surface: string): string[] {
  const m = modes.get(surface);
  return [...(m ? [`${surface}:${m}`] : []), surface, "global"];
}

/// What `?` prints for a surface: the globals, then the surface's own, then
/// its current mode's — a key the mode rebinds is listed once, as the mode's.
export function help(surface: string): Binding[] {
  const m = modes.get(surface);
  const own = m ? bindings.filter((b) => b.surface === `${surface}:${m}`) : [];
  const taken = new Set(own.map((b) => b.combo));
  return [
    ...bindings.filter((b) => b.surface === "global"),
    ...bindings.filter((b) => b.surface === surface && !taken.has(b.combo)),
    ...own,
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

/// Runs an action by name, as a key would; whether a listener took it.
export function run(action: string, surface: string): boolean {
  const set = listeners.get(action);
  if (!set || set.size === 0) return false;
  for (const fn of set) {
    if (fn(surface) === true) return true;
  }
  return false;
}

/// A chord's first key, waiting for its second.
let pending: string | null = null;
let pendingAt = 0;
const CHORD_MS = 800;

/// The combo a key event spells: modifiers first, `Mod` for ⌘ on a Mac and
/// Ctrl elsewhere, then the key as typed.
///
/// With Alt held the key is read from `e.code`, the physical key: on macOS
/// Option composes a character (⌥N is a dead key, ⌥W is `∑`, ⌥] is `‘`), so
/// `e.key` would never spell `Alt+n` there.
export function combo(e: Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">): string {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("Mod");
  if (e.altKey) parts.push("Alt");
  const physical = e.altKey ? fromCode(e.code) : null;
  const key = physical ?? keyName(e.key);
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

/// The unshifted character a physical key types on a US layout, for the keys
/// an Alt chord may use; `null` for any other key.
const CODES: Record<string, string> = {
  BracketLeft: "[",
  BracketRight: "]",
  Minus: "-",
  Equal: "=",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  Backquote: "`",
  Backslash: "\\",
};
export function fromCode(code: string | undefined): string | null {
  if (!code) return null;
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1].toLowerCase();
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit) return digit[1];
  return CODES[code] ?? null;
}

/// Whether this page runs on a Mac, where the help sheet spells ⌘ and ⌥.
function mac(): boolean {
  if (typeof navigator === "undefined") return false;
  const n = navigator as Navigator & { userAgentData?: { platform?: string } };
  return /mac/i.test(n.userAgentData?.platform ?? n.userAgent ?? "");
}

/// A combo as the person's keyboard labels it: `⌥N` and `⌘K` on a Mac,
/// `Alt+n` and `Ctrl+k` elsewhere.
export function spell(c: string): string {
  const onMac = mac();
  return c
    .split(" ")
    .map((chord) => {
      const parts = chord.split("+");
      const key = parts.pop() ?? "";
      const mods = parts.map((m) =>
        m === "Mod" ? (onMac ? "⌘" : "Ctrl") : m === "Alt" ? (onMac ? "⌥" : "Alt") : m === "Shift" ? (onMac ? "⇧" : "Shift") : m,
      );
      const k = parts.length && key.length === 1 ? key.toUpperCase() : key;
      return onMac ? mods.join("") + k : [...mods, k].join("+");
    })
    .join(" ");
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

/// Whether the event came from a control that answers these keys itself:
/// Enter and Space press a focused button or follow a focused link, and the
/// arrows move within a tab list or a list box. The registry never takes them
/// from it.
const NATIVE_KEYS = new Set(["Enter", "Space", "Up", "Down", "Left", "Right"]);
function native(e: KeyboardEvent, c: string): boolean {
  if (!NATIVE_KEYS.has(c)) return false;
  const t = e.target as HTMLElement | null;
  if (!t || typeof t.closest !== "function") return false;
  return !!t.closest("button, a[href], summary, [role=button], [role=link], [role=tab], [role=option], [role=menuitem], [role=separator], [role=checkbox], [role=radio], [role=switch]");
}

/// Whether the event came from somewhere text is typed.
export function typing(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null;
  if (!t) return false;
  const tag = t.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || t.isContentEditable;
}

/// Whether a combo still fires while text is being typed: `Esc`, and a bound
/// ⌘/Ctrl chord, which types nothing — ⌘C stays the field's because it is not
/// bound. A bare key never does, and neither does an ⌥/Alt chord: on a Mac ⌥
/// composes a character (⌥3 is `£`, ⌥W is `∑`), so the field owns it.
export const fromField = (c: string): boolean => c === "Esc" || c.startsWith("Mod+");

/// Dispatches one key event for the surface on screen; returns whether a
/// binding took it. While an input has focus only `Esc` and bound ⌘/Ctrl
/// chords fire.
export function dispatch(surface: string, e: KeyboardEvent): boolean {
  // An IME is composing: the key belongs to the composition.
  if (e.isComposing) return false;
  const c = combo(e);
  const within = scopes(surface);
  /// Runs the combo in the most specific scope that binds it and takes it; a
  /// mode's listener that declines falls through to the surface's key.
  const fire = (k: string): boolean => {
    for (const s of within) {
      const hit = bindings.find((b) => b.surface === s && b.combo === k);
      if (hit && run(hit.action, surface)) {
        e.preventDefault();
        return true;
      }
    }
    return false;
  };
  if (typing(e)) {
    if (!fromField(c)) return false;
    pending = null;
    return fire(c);
  }
  if (native(e, c)) return false;

  const now = Date.now();
  if (pending && now - pendingAt < CHORD_MS) {
    const chord = `${pending} ${c}`;
    pending = null;
    if (fire(chord)) return true;
  }
  pending = null;

  // Only a key a listener took is kept from the page; one nobody answered
  // (a list key with no list on screen) scrolls or types as it would.
  if (fire(c)) return true;
  // The first half of a chord this surface knows.
  if (bindings.some((b) => within.includes(b.surface) && b.combo.startsWith(`${c} `))) {
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
bind({ surface: "global", combo: "Esc", action: "leave", label: "leave: close the help, the palette or the new-change form" });

/// The list keys, for a surface that shows a list and answers them through
/// `lib/cursor`. Bound per surface, so each is in exactly one scope and `?`
/// lists them where they work. `open` says what Enter does there.
export function bindList(surface: string, open: string): void {
  bind({ surface, combo: "Down", action: "next", label: "next" });
  bind({ surface, combo: "j", action: "next", label: "next" });
  bind({ surface, combo: "Up", action: "prev", label: "previous" });
  bind({ surface, combo: "k", action: "prev", label: "previous" });
  bind({ surface, combo: "Enter", action: "open", label: open });
  bind({ surface, combo: "g g", action: "first", label: "first" });
  bind({ surface, combo: "G", action: "last", label: "last" });
}
