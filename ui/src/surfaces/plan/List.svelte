<script lang="ts">
  // Every specification on this machine, by project: folder, ticked boxes,
  // the change working to it, and anything waiting on a person. A project with
  // no layout says which three it looked for.
  import Icon from "../../lib/ui/Icon.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Qualifier from "../../lib/Qualifier.svelte";
  import { specs, watch, key } from "./store.svelte";

  let { focus = "", open }: { focus?: string; open: (f: string, pin?: boolean) => void } = $props();
  $effect(watch);

  let filter = $state("");
  let folded = $state<Record<string, boolean>>({});
  const name = (path: string) => path.split("/").filter(Boolean).pop() ?? path;
  const projects = $derived(
    (specs.projects ?? [])
      .map((p) => ({
        ...p,
        plans: p.plans.filter((r) => !filter.trim() || `${r.plan.path} ${r.title ?? ""}`.toLowerCase().includes(filter.trim().toLowerCase())),
      }))
      .filter((p) => p.plans.length > 0),
  );
  /// Projects with no specification layout: one counted line, not a paragraph each.
  const bare = $derived((specs.projects ?? []).filter((p) => p.plans.length === 0));
  const sentence = $derived(bare[0]?.no_layout ?? "");
  const total = $derived((specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
</script>

<div class="list">
  <header><span class="t">Specifications</span><span class="n">{total}</span></header>
  <label class="filter">
    <Icon name="filter" size={13} />
    <input bind:value={filter} placeholder="Filter specifications" aria-label="filter the specifications" />
  </label>
  <div class="rows">
    {#if specs.error}
      <p class="quiet fail">The specifications could not be read: {specs.error}</p>
    {:else if specs.projects === null}
      {#each [0, 1, 2] as i (i)}<div class="skel"></div>{/each}
    {/if}
    {#each projects as p (p.project_id)}
      <button class="group" onclick={() => (folded = { ...folded, [p.project_id]: !folded[p.project_id] })} aria-expanded={!folded[p.project_id]}>
        <Icon name={folded[p.project_id] ? "right" : "down"} size={12} />
        <Icon name="folder" size={13} />
        <span>{p.project}</span>
        <span class="n">{p.plans.length}</span>
      </button>
      {#if !folded[p.project_id]}
        {#each p.plans as r (r.plan.path)}
          {@const k = key(p, r)}
          <button class="row" aria-current={k === focus ? "true" : undefined} onclick={() => open(k)} ondblclick={() => open(k, true)}>
            <span class="title"><Icon name="spec" size={13} /> {name(r.plan.path)}</span>
            <span class="meta">
              {#if r.plan.progress}<span class="boxes" title="boxes ticked in the task file · {r.plan.progress.total} tasks">{r.plan.progress.done} ticked</span>{/if}
              {#if r.state}<Pill word={r.state} /><Qualifier q={r.qualifier} />{/if}
              {#if r.plan.open_questions > 0}<span class="wait"><Icon name="question" size={11} /> {r.plan.open_questions}</span>{/if}
              {#if r.drifted}<span class="wait"><Icon name="alert" size={11} /> drifted</span>{/if}
            </span>
          </button>
        {/each}
      {/if}
    {/each}
    {#if bare.length > 0 && !filter.trim()}
      <details class="bare">
        <summary><Icon name="folder" size={13} /> {bare.length} {bare.length === 1 ? "project has" : "projects have"} no specification layout</summary>
        <p>{bare.map((p) => p.project).join(", ")}</p>
        <p class="why"><Inline text={sentence} /></p>
      </details>
    {/if}
    {#if specs.omitted > 0}<p class="none">{specs.omitted} more not shown.</p>{/if}
  </div>
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    height: 2.2rem;
    padding: 0 var(--s-4);
    flex: none;
  }
  .t {
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
  }
  .n {
    font-size: var(--t-xs);
    color: var(--faint);
    margin-left: auto;
  }
  header .n {
    margin-left: 0;
  }
  .filter {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin: 0 var(--s-3) var(--s-2);
    padding: 0 var(--s-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--faint);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
    box-shadow: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    padding: 0.3rem 0;
    outline: none;
  }
  .rows {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    width: 100%;
    padding: 0.3rem var(--s-3);
    border: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 700;
    cursor: pointer;
    text-align: start;
  }
  .row {
    display: grid;
    gap: 0.2rem;
    width: 100%;
    padding: 0.4rem var(--s-3) 0.4rem 2.2rem;
    border: 0;
    border-left: 2px solid transparent;
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }
  .row:hover {
    background: var(--raise);
  }
  .row[aria-current="true"] {
    background: var(--select);
    border-left-color: var(--accent);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--t-sm);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .boxes {
    font-family: var(--mono);
  }
  .wait {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    color: var(--wait);
  }
  .none,
  .quiet {
    margin: var(--s-1) var(--s-4) var(--s-2) 2.2rem;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .fail {
    color: var(--fail);
  }
  .bare {
    margin: var(--s-3) var(--s-3) 0;
    padding: var(--s-2) var(--s-3);
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .bare summary {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
  }
  .bare p {
    margin: var(--s-2) 0 0;
    color: var(--dim);
  }
  .bare .why {
    color: var(--faint);
  }
  .skel {
    height: 2.6rem;
    margin: var(--s-1) var(--s-3);
    border-radius: var(--radius);
    background: var(--raise);
    opacity: 0.6;
  }
</style>
