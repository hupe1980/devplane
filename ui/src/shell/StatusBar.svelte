<script lang="ts">
  // The status bar: the machine's state in one line. Each item is a surface's
  // own fact with its number, and opens it; the bar names no surface. Nothing
  // measures the person. The connection is a word and a dot, never the
  // verified green.
  import Icon from "../lib/ui/Icon.svelte";
  import type { StatusItem } from "../lib/surfaces";

  type Item = StatusItem & { surface: string; title: string };
  let {
    items,
    projects = null,
    pulse,
    bad,
    theme,
    cycleTheme,
    help,
    go,
  }: {
    items: Item[];
    /// Registered projects, or `null` before the feed has said.
    projects?: number | null;
    pulse: string;
    bad: boolean;
    theme: string;
    cycleTheme: () => void;
    help: () => void;
    go: (hash: string) => void;
  } = $props();
</script>

{#snippet fact(i: Item)}
  <button class="item {i.tone ?? ''}" class:hot={i.tone === "wait" && i.n > 0} onclick={() => go(`#${i.surface}`)} title="open {i.title}">
    {#if i.icon}<Icon name={i.icon} size={13} />{/if}{i.n} {i.word}
  </button>
{/snippet}

<footer class="status" aria-label="status">
  <span class="item pulse" class:bad role="status"><span class="dot"></span>{pulse}</span>
  {#each items.filter((i) => !i.end) as i (i.surface + i.word)}{@render fact(i)}{/each}
  {#if projects != null}<span class="item">{projects} projects</span>{/if}
  <span class="gap"></span>
  {#each items.filter((i) => i.end) as i (i.surface + i.word)}{@render fact(i)}{/each}
  <button class="item" onclick={cycleTheme} title="theme"><Icon name={theme === "light" ? "sun" : "moon"} size={13} />{theme}</button>
  <button class="item" onclick={help} aria-label="keys bound here"><Icon name="keyboard" size={13} /></button>
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    height: 1.6rem;
    padding: 0 var(--s-2);
    gap: 1px;
    background: var(--chrome);
    border-top: 1px solid var(--line);
    font-size: var(--t-xs);
    color: var(--dim);
    flex: none;
  }
  .gap {
    flex: 1;
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 100%;
    padding: 0 0.5rem;
    border: 0;
    border-radius: 0;
    background: none;
    color: inherit;
    font: inherit;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  button.item {
    cursor: pointer;
  }
  button.item:hover {
    background: var(--raise);
    color: var(--ink);
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--accent);
  }
  .pulse.bad {
    color: var(--fail);
  }
  .pulse.bad .dot {
    background: var(--fail);
  }
  .hot {
    color: var(--wait);
    font-weight: 600;
  }
  .fail {
    color: var(--fail);
  }
</style>
