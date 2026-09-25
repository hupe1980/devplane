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

/// English for a list: `Codex, OpenCode and Gemini CLI`.
export function listed(xs: string[]): string {
  if (xs.length <= 1) return xs[0] ?? "";
  return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
}
