<script module lang="ts">
  // The state language, in one table: every state has a colour, a glyph and a
  // word, so colour is never the only channel. The only green is *verified* —
  // every declared gate exited zero — and this is the one place it is paired.
  //
  // Runs: working, needs you, failed, idle. Changes: the host's six words and
  // glyphs (`ChangeState::as_str`, `glyph`), held to that source by a guard;
  // gates may add *no gates declared* or *stale*.
  export type Kind =
    | "working"
    | "needs you"
    | "failed"
    | "verified"
    | "idle"
    | "drafted"
    | "isolated"
    | "in flight"
    | "offered"
    | "archived"
    | "no gates declared"
    | "stale";

  // The tones are prefixed because a shared file may not quote a surface id.
  export const STATES: Record<Kind, { glyph: string; word: string; tone: string }> = {
    working: { glyph: "●", word: "working", tone: "t-work" },
    "needs you": { glyph: "◆", word: "needs you", tone: "t-wait" },
    failed: { glyph: "✕", word: "failed", tone: "t-fail" },
    verified: { glyph: "✓", word: "verified", tone: "t-done" },
    idle: { glyph: "○", word: "idle", tone: "t-dim" },
    drafted: { glyph: "·", word: "drafted", tone: "t-dim" },
    isolated: { glyph: "⎇", word: "isolated", tone: "t-ink" },
    "in flight": { glyph: "▶", word: "in flight", tone: "t-work" },
    offered: { glyph: "↗", word: "offered", tone: "t-ink" },
    archived: { glyph: "▣", word: "archived", tone: "t-dim" },
    "no gates declared": { glyph: "∅", word: "no gates declared", tone: "t-wait" },
    stale: { glyph: "≠", word: "stale", tone: "t-wait" },
  };

  /// A change's life, in order. The stepper reads it here: one spelling.
  export const LIFE = ["drafted", "isolated", "in flight", "verified", "offered", "archived"] as const satisfies readonly Kind[];
  export type Life = (typeof LIFE)[number];

  /// The table's row for a word the host sent, or `null` for an unknown one,
  /// which is then rendered as the bare word, never a guessed glyph.
  export function kind(word: string | null | undefined): Kind | null {
    return word != null && word in STATES ? (word as Kind) : null;
  }
</script>

<script lang="ts">
  let {
    state,
    /// A more exact word than the table's (*waiting*, *met*); glyph and colour
    /// stay the table's. Empty where neighbouring words already carry it.
    word,
    /// Keep the word for assistive technology only, where the line already
    /// carries it. Never the default: a bare glyph needs a legend.
    quiet = false,
  }: { state: Kind; word?: string; quiet?: boolean } = $props();

  const row = $derived(STATES[state]);
  const said = $derived(word ?? row.word);
</script>

<span class="state {row.tone}">
  <span class="glyph" aria-hidden="true">{row.glyph}</span>{#if !said}{:else if quiet}<span class="sr-only">{said}</span>{:else}{said}{/if}
</span>

<style>
  .state {
    display: inline-flex;
    align-items: baseline;
    gap: var(--s-1);
    white-space: nowrap;
  }
  .glyph { font-family: var(--mono); }
  .t-work { color: var(--work); }
  .t-wait { color: var(--wait); font-weight: 600; }
  .t-fail { color: var(--fail); font-weight: 600; }
  .t-done { color: var(--done); }
  .t-dim { color: var(--dim); }
  .t-ink { color: var(--ink); }
</style>
