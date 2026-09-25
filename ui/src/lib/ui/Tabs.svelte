<script lang="ts" module>
  /// Where a key moves the selection in a strip of `ids`, or `null` for a key
  /// the strip does not answer. Arrows wrap; Home and End go to the ends.
  export function step(ids: string[], active: string, key: string): string | null {
    if (ids.length === 0) return null;
    const i = Math.max(0, ids.indexOf(active));
    if (key === "ArrowRight") return ids[(i + 1) % ids.length];
    if (key === "ArrowLeft") return ids[(i - 1 + ids.length) % ids.length];
    if (key === "Home") return ids[0];
    if (key === "End") return ids[ids.length - 1];
    return null;
  }
</script>

<script lang="ts">
  // A row of views over one thing. Every tab states its count when it has one;
  // an absent count is not a zero. The strip is one tab stop and arrows move
  // between tabs (the ARIA tabs pattern).
  import Icon from "./Icon.svelte";

  type Tab = { id: string; label: string; icon?: string; count?: number | null; tone?: string };
  let {
    tabs,
    active = $bindable(),
    label = "views",
  }: { tabs: Tab[]; active: string; label?: string } = $props();

  function key(e: KeyboardEvent) {
    const to = step(tabs.map((t) => t.id), active, e.key);
    if (to === null) return;
    e.preventDefault();
    active = to;
    const next = tabs.findIndex((t) => t.id === to);
    const el = (e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("[role=tab]")[next];
    el?.focus();
  }
</script>

<div class="tabs" role="tablist" aria-label={label} tabindex="-1" onkeydown={key}>
  {#each tabs as t (t.id)}
    <button
      role="tab"
      class="tab"
      aria-selected={t.id === active}
      tabindex={t.id === active ? 0 : -1}
      onclick={() => (active = t.id)}
    >
      {#if t.icon}<Icon name={t.icon} size={14} />{/if}
      <span>{t.label}</span>
      {#if t.count != null}<span class="count {t.tone ?? ''}">{t.count}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    gap: 2px;
    border-bottom: 1px solid var(--line);
    padding: 0 var(--s-3);
    overflow-x: auto;
    scrollbar-width: none;
    flex: none;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    height: 2.1rem;
    padding: 0 0.7rem;
    border: 0;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
    white-space: nowrap;
  }
  .tab:hover {
    color: var(--ink);
  }
  .tab[aria-selected="true"] {
    color: var(--ink);
    border-bottom-color: var(--accent);
  }
  .tab:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -3px;
  }
  .count {
    font-size: var(--t-xs);
    font-variant-numeric: tabular-nums;
    padding: 0 0.4em;
    border-radius: 999px;
    background: var(--raise);
    color: var(--dim);
  }
  .count.wait { color: var(--wait); }
  .count.fail { color: var(--fail); }
  .count.done { color: var(--done); }
</style>
