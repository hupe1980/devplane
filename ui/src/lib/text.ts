// Outside values reach the document as text.
//
// **The old page needed an `esc()` at every interpolation** because it built
// `innerHTML` strings: a title carrying `<script>` was one forgotten call away
// from executing, and the only thing standing between the two was somebody
// remembering. Svelte escapes `{value}` by construction, so the rule changes
// shape rather than going away — what has to be guarded now is the one
// construct that opts out.
//
// So there is no `esc()` here. There is a guard that `{@html}` never appears in
// a surface, and these helpers for the cases that legitimately shape text
// before it is rendered.

/// Shortens for a row, without pretending the rest is not there.
///
/// **The ellipsis is a character, not three dots**, so a width measured in
/// characters is the width that renders — the same reason the terminal pads by
/// visible width rather than by `len()`.
export function clip(value: string, max: number): string {
  const chars = [...value];
  return chars.length <= max ? value : chars.slice(0, Math.max(0, max - 1)).join("") + "…";
}

/// A duration a person can read: `4s`, `12m`, `3h`, `2d`.
///
/// **Rounded down, with a floor of one unit.** `16h` for sixteen and a half is
/// how everybody reads "how long ago"; rounding up would say `17h` for a gap
/// that has not arrived.
export function ago(seconds: number): string {
  // **An unreadable duration is not a duration.** `Math.floor(NaN)` is `NaN`
  // and renders as `NaNs`, which is worse than saying nothing: a row claiming
  // to have waited `NaNm` is a row a person stops trusting. A timestamp the
  // daemon could not parse, a clock that moved backwards, a field that arrived
  // absent — all of them reach here as something that is not a number.
  if (!Number.isFinite(seconds)) return "";
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
}

/// `1 item` / `2 items`. A count in a sentence has to agree with it.
export function plural(n: number, one: string, many: string): string {
  return n === 1 ? one : many;
}
