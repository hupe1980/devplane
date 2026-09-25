// Moving the address from a surface: `replaceState`, so Back does not walk
// every row glanced at. It fires no event, so the shell's is raised by hand.

export function go(hash: string): void {
  history.replaceState({}, "", hash || "#");
  dispatchEvent(new HashChangeEvent("hashchange"));
}
