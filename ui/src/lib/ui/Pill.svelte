<script lang="ts" module>
  // A state word with its colour family. The word is always printed; the
  // colour only names the family, and an unknown word is neutral.
  export type Tone = "work" | "wait" | "fail" | "done" | "none";

  // The only green is verified: `done` is the `check` gate exiting zero, so
  // it goes to *verified* and a gate that *passed* and nothing else — not
  // *completed*, *allow*, *merged* or *archived*, which stay neutral.
  const TONES: Array<[Tone, RegExp]> = [
    ["fail", /fail|stopped|broken|denied|deny|refused|error|lost|abandon/i],
    ["wait", /need|wait|ask|question|permission|held|stale|drift|blocked|review/i],
    ["done", /^(verified|passed)$/i],
    ["work", /work|flight|running|isolated|driving|live|active|in progress/i],
  ];

  /// The family a state word belongs to, from its own text.
  export function tone(word: string | null | undefined): Tone {
    if (!word) return "none";
    for (const [t, re] of TONES) if (re.test(word)) return t;
    return "none";
  }
</script>

<script lang="ts">
  let {
    word,
    as = undefined,
    dot = true,
    title = undefined,
  }: { word: string; as?: Tone; dot?: boolean; title?: string } = $props();
  const t = $derived(as ?? tone(word));
</script>

<span class="pill {t}" {title}>
  {#if dot}<span class="dot" aria-hidden="true"></span>{/if}{word}
</span>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.4em;
    padding: 0 0.5em;
    height: 1.35rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    font-size: var(--t-xs);
    font-weight: 600;
    white-space: nowrap;
    color: var(--dim);
    background: var(--panel);
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: currentColor;
    flex: none;
  }
  .work { color: var(--work); }
  .wait { color: var(--wait); }
  .fail { color: var(--fail); }
  .done { color: var(--done); }
</style>
