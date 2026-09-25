<script lang="ts">
  // The changes sidebar: every change grouped by project, its state as a word;
  // a pick opens as an editor tab. Reads `/api/changes` (the board's brief has
  // no project), again only when the board's ids or states move.
  import { api } from "../../lib/api";
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import { run } from "../../lib/keys";

  type Brief = { id: string; state: string };
  type Row = {
    id: string;
    title: string;
    state: string;
    project_id: string;
    branch?: string | null;
    waiting_says?: string | null;
    in_place?: boolean;
    archived_at?: string | null;
    updated_at?: string;
    counts_says?: string | null;
  };

  let {
    all = [],
    focus = "",
    open,
  }: { all?: Brief[]; focus?: string; open: (id: string, pin?: boolean) => void } = $props();

  let rows = $state<Row[] | null>(null);
  let error = $state("");
  let filter = $state("");
  let showArchived = $state(false);

  // Re-read when the board says the set or a state changed.
  const signature = $derived(all.map((b) => `${b.id}:${b.state}`).join("|"));
  $effect(() => {
    void signature;
    let live = true;
    api<Row[]>("/api/changes")
      .then((r) => {
        if (live) {
          rows = Array.isArray(r) ? r : [];
          error = "";
        }
      })
      .catch((e) => {
        if (live) error = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });

  const name = (p: string) => p.split("/").filter(Boolean).pop() ?? p;
  const visible = $derived(
    (rows ?? []).filter(
      (r) =>
        (showArchived || !r.archived_at) &&
        (!filter.trim() ||
          `${r.title} ${r.branch ?? ""} ${name(r.project_id)} ${r.state}`.toLowerCase().includes(filter.trim().toLowerCase())),
    ),
  );
  const groups = $derived.by(() => {
    const m = new Map<string, Row[]>();
    for (const r of visible) {
      const k = name(r.project_id);
      m.set(k, [...(m.get(k) ?? []), r]);
    }
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });
  const archived = $derived((rows ?? []).filter((r) => r.archived_at).length);
  let folded = $state<Record<string, boolean>>({});

  function key(e: KeyboardEvent) {
    const flat = groups.flatMap(([g, rs]) => (folded[g] ? [] : rs));
    if (flat.length === 0) return;
    const i = flat.findIndex((r) => r.id === focus);
    let next = i;
    if (e.key === "ArrowDown" || e.key === "j") next = Math.min(flat.length - 1, i + 1);
    else if (e.key === "ArrowUp" || e.key === "k") next = Math.max(0, i - 1);
    else if (e.key === "Enter" && i !== -1) {
      open(flat[i].id, true);
      return;
    } else return;
    e.preventDefault();
    open(flat[next].id);
  }
</script>

<div class="list">
  <header>
    <span class="t">Changes</span>
    <span class="n">{visible.length}</span>
    <button class="icon" title="Start a new change (Alt+N)" aria-label="start a new change" onclick={() => run("new-change", "change")}>
      <Icon name="plus" size={15} />
    </button>
  </header>
  <label class="filter">
    <Icon name="filter" size={13} />
    <input bind:value={filter} placeholder="Filter by title, branch, project, state" aria-label="filter the changes" />
  </label>

  <div class="rows" role="listbox" aria-label="changes" tabindex="0" onkeydown={key}>
    {#if error}
      <p class="quiet fail">The changes could not be read: {error}</p>
    {:else if rows === null}
      {#each [0, 1, 2] as i (i)}<div class="skel"></div>{/each}
    {:else if rows.length === 0}
      <p class="quiet">No change has been started on this machine. <button class="link" onclick={() => run("new-change", "change")}>Start one</button> — an isolated worktree, an agent in it, and the project's own gates.</p>
    {:else if visible.length === 0}
      <p class="quiet">No change matches “{filter}”.</p>
    {:else}
      {#each groups as [g, rs] (g)}
        <button class="group" onclick={() => (folded = { ...folded, [g]: !folded[g] })} aria-expanded={!folded[g]}>
          <Icon name={folded[g] ? "right" : "down"} size={12} />
          <Icon name="folder" size={13} />
          <span>{g}</span>
          <span class="n">{rs.length}</span>
        </button>
        {#if !folded[g]}
          {#each rs as r (r.id)}
            <div
              class="row"
              role="option"
              tabindex="-1"
              aria-selected={r.id === focus}
              onclick={() => open(r.id)}
              ondblclick={() => open(r.id, true)}
              onkeydown={() => {}}
            >
              <span class="title">{r.title || r.id}</span>
              <span class="meta">
                <Pill word={r.state} />
                {#if r.waiting_says}<span class="wait">{r.waiting_says}</span>{/if}
                {#if r.in_place}<span class="dim">in place</span>{/if}
              </span>
              {#if r.branch}<span class="branch"><Icon name="change" size={11} /> {r.branch}</span>{/if}
            </div>
          {/each}
        {/if}
      {/each}
    {/if}
  </div>
  {#if archived > 0}
    <button class="foot" onclick={() => (showArchived = !showArchived)}>
      {showArchived ? "Hide" : "Show"} {archived} archived
    </button>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    height: 2.2rem;
    padding: 0 var(--s-2) 0 var(--s-4);
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
    font-variant-numeric: tabular-nums;
  }
  header .icon {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--dim);
    cursor: pointer;
  }
  header .icon:hover {
    background: var(--raise);
    color: var(--ink);
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
    flex: none;
  }
  .filter:focus-within {
    border-color: var(--accent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: none;
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
    outline: none;
    padding-bottom: var(--s-3);
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
  .group .n {
    margin-left: auto;
  }
  .row {
    display: grid;
    gap: 0.2rem;
    padding: 0.45rem var(--s-3) 0.45rem 2.2rem;
    cursor: pointer;
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: var(--raise);
  }
  .row[aria-selected="true"] {
    background: var(--select);
    border-left-color: var(--accent);
  }
  .rows:focus-visible .row[aria-selected="true"] {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  .title {
    color: var(--ink);
    font-size: var(--t-sm);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-xs);
    min-width: 0;
  }
  .wait {
    color: var(--wait);
  }
  .dim {
    color: var(--faint);
  }
  .branch {
    font-family: var(--mono);
    font-size: 0.6875rem;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .quiet {
    margin: var(--s-3) var(--s-4);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .quiet.fail {
    color: var(--fail);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .skel {
    height: 3.2rem;
    margin: var(--s-1) var(--s-3);
    border-radius: var(--radius);
    background: var(--raise);
    opacity: 0.6;
  }
  .foot {
    flex: none;
    border: 0;
    border-top: 1px solid var(--line);
    background: none;
    color: var(--faint);
    font: inherit;
    font-size: var(--t-xs);
    padding: var(--s-2);
    cursor: pointer;
  }
  .foot:hover {
    color: var(--ink);
  }
</style>
