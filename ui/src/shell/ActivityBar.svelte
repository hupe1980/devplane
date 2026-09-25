<script lang="ts">
  // The activity bar: one icon per listed surface, its count beside it. The
  // badge prints its number (never colour alone); the attention band's badge
  // is the wait colour; zero prints nothing. The label is tooltip and
  // accessible name.
  import Icon from "../lib/ui/Icon.svelte";
  import { BANDS, BAND_LABELS, type Band, type Surface, type Feed } from "../lib/surfaces";

  let {
    items,
    feed,
    current,
    pick,
  }: { items: Surface[]; feed: Feed; current: string; pick: (id: string) => void } = $props();

  // Each band is a named group, heard by a screen reader and seen as a gap.
  // The last band sits at the foot.
  const bands = $derived(
    BANDS.map((b) => ({ band: b, items: items.filter((s) => s.band === b) })).filter((g) => g.items.length > 0),
  );
  const top = $derived(bands.slice(0, -1));
  const bottom = $derived(bands.slice(-1));
</script>

{#snippet item(s: Surface)}
  {@const n = s.count?.(feed) ?? null}
  <button
    class="act"
    class:on={s.id === current}
    title={n ? `${s.title} — ${n}` : s.title}
    aria-label={n ? `${s.title}, ${n}` : s.title}
    aria-current={s.id === current ? "page" : undefined}
    onclick={() => pick(s.id)}
  >
    {#if s.icon}<Icon name={s.icon} size={20} stroke={1.6} />{:else}<b>{s.title.slice(0, 1)}</b>{/if}
    {#if n}<span class="badge" class:hot={s.band === "attention"}>{n > 99 ? "99+" : n}</span>{/if}
  </button>
{/snippet}

{#snippet group(g: { band: Band; items: Surface[] })}
  <div class="band" role="group" aria-label={BAND_LABELS[g.band]}>
    {#each g.items as s (s.id)}{@render item(s)}{/each}
  </div>
{/snippet}

<nav class="bar" aria-label="surfaces">
  {#each top as g (g.band)}{@render group(g)}{/each}
  <span class="gap"></span>
  {#each bottom as g (g.band)}{@render group(g)}{/each}
</nav>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 3rem;
    flex: none;
    padding: var(--s-2) 0;
    gap: 2px;
    background: var(--chrome);
    border-right: 1px solid var(--line);
  }
  .gap {
    flex: 1;
  }
  .band {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .band + .band {
    border-top: 1px solid var(--line);
    padding-top: var(--s-1);
  }
  .act {
    position: relative;
    display: grid;
    place-items: center;
    width: 3rem;
    height: 2.75rem;
    border: 0;
    border-left: 2px solid transparent;
    border-radius: 0;
    background: none;
    color: var(--faint);
    cursor: pointer;
  }
  .act:hover {
    color: var(--ink);
  }
  .act.on {
    color: var(--ink);
    border-left-color: var(--accent);
  }
  .act:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -4px;
  }
  .badge {
    position: absolute;
    right: 0.35rem;
    bottom: 0.3rem;
    min-width: 1.05rem;
    height: 1.05rem;
    padding: 0 0.25rem;
    border-radius: 999px;
    background: var(--raise);
    color: var(--ink);
    font-size: 0.625rem;
    font-weight: 600;
    line-height: 1.05rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .badge.hot {
    background: var(--wait);
    color: var(--chrome);
  }
</style>
