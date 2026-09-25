<script lang="ts">
  // A change's files in reading order: by declared role, or by the run that
  // wrote them, with *not asked for* apart. Each row: how much changed, whether
  // a declared test covers it, and how many hunks you marked (a count, never a
  // percentage; marks stay in this browser).
  import Icon from "../../lib/ui/Icon.svelte";
  import type { File } from "./types";

  type Section = { says: string; files: File[]; tone?: "wait" | "fail" };
  let {
    sections,
    selected,
    marked,
    pick,
  }: {
    sections: Section[];
    selected: string;
    marked: (f: File) => number;
    pick: (path: string) => void;
  } = $props();

  let folded = $state<Record<string, boolean>>({});
  const split = (p: string) => {
    const i = p.lastIndexOf("/");
    return i === -1 ? ["", p] : [p.slice(0, i + 1), p.slice(i + 1)];
  };
  const statusIcon = (s: string) => (s.startsWith("added") ? "plus" : s.startsWith("deleted") ? "x" : "file");
</script>

<nav class="files" aria-label="files in the change">
  {#each sections as s (s.says)}
    <button class="sec {s.tone ?? ''}" onclick={() => (folded = { ...folded, [s.says]: !folded[s.says] })} aria-expanded={!folded[s.says]}>
      <Icon name={folded[s.says] ? "right" : "down"} size={12} />
      <span>{s.says}</span>
      <span class="n">{s.files.length}</span>
    </button>
    {#if !folded[s.says]}
      {#each s.files as f (s.says + f.path)}
        {@const [dir, base] = split(f.path)}
        {@const done = marked(f)}
        <button class="file" aria-current={f.path === selected ? "true" : undefined} onclick={() => pick(f.path)} title={f.path}>
          <Icon name={statusIcon(f.status_says)} size={13} />
          <span class="name"><span class="dir">{dir}</span><b>{base}</b></span>
          <span class="delta"><span class="add">+{f.added}</span> <span class="del">−{f.removed}</span></span>
          <span class="meta">
            {#if f.coverage?.coverage === "covered"}<span class="cov" title={f.coverage_says ?? ""}><Icon name="shield" size={11} /></span>{/if}
            {#if f.hunks.length}<span class="seen" class:all={done === f.hunks.length}>{done}/{f.hunks.length}</span>{/if}
          </span>
        </button>
      {/each}
    {/if}
  {/each}
</nav>

<style>
  .files {
    display: flex;
    flex-direction: column;
    padding: var(--s-1) 0 var(--s-4);
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem var(--s-3);
    border: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 700;
    text-align: start;
    cursor: pointer;
    position: sticky;
    top: 0;
    background: var(--side);
    z-index: 1;
  }
  .sec.wait {
    color: var(--wait);
  }
  .sec.fail {
    color: var(--fail);
  }
  .sec .n {
    margin-left: auto;
    color: var(--faint);
  }
  .file {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-rows: auto auto;
    column-gap: var(--s-2);
    align-items: center;
    padding: 0.3rem var(--s-3) 0.3rem 1.4rem;
    border: 0;
    border-left: 2px solid transparent;
    background: none;
    color: var(--dim);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .file:hover {
    background: var(--raise);
  }
  .file[aria-current="true"] {
    background: var(--select);
    border-left-color: var(--accent);
    color: var(--ink);
  }
  .name {
    display: flex;
    min-width: 0;
    font-size: var(--t-sm);
    white-space: nowrap;
  }
  .dir {
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
  }
  .name b {
    color: var(--ink);
    font-weight: 600;
    flex: none;
  }
  .delta {
    font-family: var(--mono);
    font-size: 0.6875rem;
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .meta {
    grid-column: 2 / span 2;
    display: flex;
    gap: var(--s-2);
    align-items: center;
    font-size: 0.6875rem;
    color: var(--faint);
  }
  .meta:empty {
    display: none;
  }
  /* A declared test covers it — a mapping, not a pass. */
  .cov {
    color: var(--accent);
    display: inline-flex;
  }
  .seen.all {
    color: var(--ink);
  }
</style>
