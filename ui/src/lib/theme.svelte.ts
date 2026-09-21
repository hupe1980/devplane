// Light or dark, and who decided.
//
// **Three states, not two.** The system preference is the default and not the
// only say: a person who works in a light editor and wants a dark board is not
// overridden by their operating system, and a person who has never expressed a
// preference still gets the one their system implies. `system` is therefore a
// real value rather than the absence of one.
//
// The choice is kept in `localStorage` because it is a per-viewer convenience:
// it never needs to reach another device, another person or the daemon, and a
// board that failed to load because storage was blocked would be worse than one
// that forgets which theme you picked.

const KEY = "devplane_theme";

export type Theme = "system" | "light" | "dark";

function read(): Theme {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

export function theme() {
  const state = $state({ choice: read() });

  function apply(next: Theme) {
    state.choice = next;
    // `system` removes the attribute rather than setting a third value: the
    // stylesheet's media query is what answers then, and an attribute saying
    // "system" would be a value every selector has to know to ignore.
    if (next === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", next);
    try {
      if (next === "system") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, next);
    } catch {
      // A private window with site data blocked. The theme still applies for
      // this tab; only remembering it fails, and that is not worth an error.
    }
  }

  return {
    state,
    /// Called once at start-up to put the stored choice back on the document.
    restore: () => apply(state.choice),
    /// Cycles system → light → dark → system, which keeps the control to one
    /// button. A three-way segmented control is more discoverable and costs
    /// three times the width in a header that has a job to do.
    cycle: () => apply(state.choice === "system" ? "light" : state.choice === "light" ? "dark" : "system"),
  };
}
