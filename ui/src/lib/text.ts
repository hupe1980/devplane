// Outside values reach the document as text: Svelte escapes `{value}`, and a
// guard forbids the raw-HTML construct in surfaces. These helpers only shape
// text before it is rendered.

/// Shortens for a row, ending in a one-character ellipsis so the width in
/// characters is the width that renders.
export function clip(value: string, max: number): string {
  const chars = [...value];
  return chars.length <= max ? value : chars.slice(0, Math.max(0, max - 1)).join("") + "…";
}

/// A duration a person can read: `4s`, `12m`, `3h`, `2d`, rounded down.
export function ago(seconds: number): string {
  // An unparsable or absent timestamp says nothing rather than `NaNs`.
  if (!Number.isFinite(seconds)) return "";
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

/// `1 item` / `2 items`.
export function plural(n: number, one: string, many: string): string {
  return n === 1 ? one : many;
}

/// The host's size sentence in plain words: *2 files · 2+ 0− lines · 0 shared
/// · 1 security* reads *2 files · +2 −0 lines · 1 security file*. A role with
/// nothing in it is left out rather than said as a zero.
export function shape(says: string | null | undefined): string {
  if (!says) return "";
  return says
    .split(" · ")
    .map((part) => {
      const lines = /^(\d+)\+ (\d+)− lines$/.exec(part);
      if (lines) return `+${lines[1]} −${lines[2]} lines`;
      const role = /^(\d+) (shared|security)$/.exec(part);
      if (role) return role[1] === "0" ? "" : `${role[1]} ${role[2]} ${role[1] === "1" ? "file" : "files"}`;
      return part;
    })
    .filter(Boolean)
    .join(" · ");
}

/// English for a list: `Codex, OpenCode and Gemini CLI`.
export function listed(xs: string[]): string {
  if (xs.length <= 1) return xs[0] ?? "";
  return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
}
