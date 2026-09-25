<script lang="ts">
  // The editor's tab strip. A preview tab is italic until something pins it;
  // the close control is a real button; the middle button closes, as in every
  // editor; a double-click pins.
  import Icon from "../lib/ui/Icon.svelte";
  import type { Tab } from "./tabs.svelte";

  let {
    list,
    active,
    title,
    icon,
    pick,
    close,
    pin,
  }: {
    list: Tab[];
    active: number;
    title: (t: Tab) => string;
    icon: (t: Tab) => string | undefined;
    pick: (i: number) => void;
    close: (i: number) => void;
    pin: (i: number) => void;
  } = $props();
</script>

<div class="strip" role="tablist" aria-label="open tabs">
  {#each list as t, i (t.surface + "/" + t.focus)}
    <div
      class="tab"
      class:on={i === active}
      class:preview={!t.pinned}
      role="tab"
      tabindex={i === active ? 0 : -1}
      aria-selected={i === active}
      onclick={() => pick(i)}
      ondblclick={() => pin(i)}
      onauxclick={(e) => e.button === 1 && close(i)}
      onkeydown={(e) => e.key === "Enter" && pick(i)}
      title={title(t)}
    >
      {#if icon(t)}<Icon name={icon(t)!} size={14} />{/if}
      <span class="name">{title(t)}</span>
      <button
        class="x"
        aria-label="close {title(t)}"
        onclick={(e) => {
          e.stopPropagation();
          close(i);
        }}><Icon name="x" size={12} /></button
      >
    </div>
  {/each}
</div>

<style>
  .strip {
    display: flex;
    height: 2.2rem;
    flex: none;
    background: var(--side);
    border-bottom: 1px solid var(--line);
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    max-width: 16rem;
    padding: 0 0.35rem 0 0.75rem;
    border-right: 1px solid var(--line);
    color: var(--faint);
    font-size: var(--t-sm);
    cursor: pointer;
    white-space: nowrap;
    flex: none;
  }
  .tab:hover {
    color: var(--dim);
  }
  .tab.on {
    background: var(--bg);
    color: var(--ink);
    box-shadow: inset 0 2px 0 var(--accent);
  }
  .tab:focus-visible {
    outline: 1px solid var(--accent);
    outline-offset: -2px;
  }
  .preview .name {
    font-style: italic;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .x {
    display: grid;
    place-items: center;
    width: 1.25rem;
    height: 1.25rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: none;
    color: inherit;
    opacity: 0;
    cursor: pointer;
  }
  .tab:hover .x,
  .tab.on .x {
    opacity: 1;
  }
  .x:hover {
    background: var(--raise);
    color: var(--ink);
  }
</style>
