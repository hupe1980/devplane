<script lang="ts">
  // The title bar: where you are, the find box, starting a change, and the two
  // side-region toggles. The find box is a button that opens the palette, so
  // there is one way to find things.
  import Icon from "../lib/ui/Icon.svelte";

  let {
    crumbs,
    sideOpen,
    panelOpen,
    find,
    create,
    toggleSide,
    togglePanel,
  }: {
    crumbs: string[];
    sideOpen: boolean;
    panelOpen: boolean;
    find: () => void;
    create: () => void;
    toggleSide: () => void;
    togglePanel: () => void;
  } = $props();

  const mod = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl+";
</script>

<header class="title">
  <div class="brand">
    <svg class="mark" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
      <rect x="2" y="2" width="20" height="20" rx="5" fill="var(--accent)" />
      <path d="M8 7h4.5a5 5 0 0 1 0 10H8z" fill="none" stroke="var(--chrome)" stroke-width="2.2" />
    </svg>
    <b>Devplane</b>
    {#each crumbs as c, i (i)}
      <Icon name="right" size={12} />
      <span class="crumb" class:last={i === crumbs.length - 1}>{c}</span>
    {/each}
  </div>

  <button class="find" onclick={find} aria-label="find anything — changes, sessions, commands">
    <Icon name="search" size={14} />
    <span>Find a change, a session, a command…</span>
    <kbd>{mod}K</kbd>
  </button>

  <div class="acts">
    <button class="primary" onclick={create} title="Start a new change (Alt+N)">
      <Icon name="plus" size={14} /> New change
    </button>
    <button class="icon" class:on={sideOpen} onclick={toggleSide} title="Sidebar ({mod}B)" aria-label="toggle the sidebar" aria-pressed={sideOpen}>
      <Icon name="sidebar" size={16} />
    </button>
    <button class="icon" class:on={panelOpen} onclick={togglePanel} title="Activity panel ({mod}J)" aria-label="toggle the activity panel" aria-pressed={panelOpen}>
      <Icon name="panel" size={16} />
    </button>
  </div>
</header>

<style>
  .title {
    display: grid;
    grid-template-columns: 1fr minmax(16rem, 34rem) 1fr;
    align-items: center;
    gap: var(--s-3);
    height: 2.5rem;
    padding: 0 var(--s-3);
    background: var(--chrome);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    min-width: 0;
    color: var(--faint);
    font-size: var(--t-sm);
    white-space: nowrap;
    overflow: hidden;
  }
  .brand b {
    color: var(--ink);
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .mark {
    flex: none;
  }
  .crumb {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .crumb.last {
    color: var(--ink);
  }
  .find {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    height: 1.7rem;
    padding: 0 var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--faint);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
    min-width: 0;
  }
  .find:hover {
    border-color: var(--edge);
    color: var(--dim);
  }
  .find span {
    flex: 1;
    text-align: start;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.6875rem;
    color: var(--faint);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 0.3rem;
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: var(--s-1);
  }
  .acts button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.7rem;
    border-radius: var(--radius);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
  }
  .primary {
    padding: 0 0.7rem;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .primary:hover {
    filter: brightness(1.08);
  }
  .icon {
    width: 1.9rem;
    justify-content: center;
    border: 1px solid transparent;
    background: none;
    color: var(--faint);
  }
  .icon:hover,
  .icon.on {
    color: var(--ink);
    background: var(--raise);
  }
</style>
