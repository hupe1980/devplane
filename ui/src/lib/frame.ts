// What the app's window injects into the page: a way to hide itself. In a
// browser tab it is absent and the call does nothing.

declare global {
  interface Window {
    __devplane_hide?: () => void;
  }
}

/// Asks the window to hide. `true` when a window was there to ask.
export function hideWindow(): boolean {
  if (typeof window === "undefined" || typeof window.__devplane_hide !== "function") return false;
  window.__devplane_hide();
  return true;
}

/// Whether this page is inside the app's window rather than a browser tab.
export function framed(): boolean {
  return typeof window !== "undefined" && typeof window.__devplane_hide === "function";
}
