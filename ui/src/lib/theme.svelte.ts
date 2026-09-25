// Light, dark or system (the default). The choice is a per-viewer
// convenience, kept in `localStorage` and forgotten if storage is blocked.

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
    // `system` removes the attribute, so the stylesheet's media query answers.
    if (next === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", next);
    try {
      if (next === "system") localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, next);
    } catch {
      // Storage blocked: the theme still applies, it is just not remembered.
    }
  }

  return {
    state,
    /// Called once at start-up to put the stored choice back on the document.
    restore: () => apply(state.choice),
    /// Cycles system → light → dark → system, so the control is one button.
    cycle: () => apply(state.choice === "system" ? "light" : state.choice === "light" ? "dark" : "system"),
  };
}
