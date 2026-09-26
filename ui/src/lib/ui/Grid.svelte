<script lang="ts" module>
  export type Column<R> = {
    key: string;
    label: string;
    /// Initial width in px; the last column takes what is left when omitted.
    width?: number;
    align?: "start" | "end";
    /// The value sorted on. No `sort` means the column does not sort.
    sort?: (row: R) => string | number | null | undefined;
    mono?: boolean;
  };

  /// Which way a column is sorted, or `null` for the host's order.
  export type Sorting = { key: string; dir: 1 | -1 } | null;

  /// A header click: ascending, descending, then back to the host's order.
  export function nextSort<R>(by: Sorting, c: Column<R>): Sorting {
    if (!c.sort) return by;
    if (by?.key !== c.key) return { key: c.key, dir: 1 };
    if (by.dir === 1) return { key: c.key, dir: -1 };
    return null;
  }

  /// The rows as shown; unsorted, the same array in the host's ranking.
  export function sorted<R>(rows: R[], columns: Column<R>[], by: Sorting): R[] {
    if (!by) return rows;
    const col = columns.find((c) => c.key === by.key);
    if (!col?.sort) return rows;
    const f = col.sort;
    return [...rows].sort((a, b) => {
      const x = f(a);
      const y = f(b);
      if (x == null && y == null) return 0;
      if (x == null) return 1;
      if (y == null) return -1;
      return (x < y ? -1 : x > y ? 1 : 0) * by.dir;
    });
  }
</script>

<script lang="ts" generics="T">
  // The data grid every workbench list uses. Columns sort and resize (widths
  // remembered per grid); rows group with a count in each header, and a fold
  // never hides without its number; ↑/↓, j/k and Enter walk rows when focused.
  //
  // It never re-sorts the host's ranking unless a person clicks a header.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";


  let {
    id,
    columns,
    rows,
    key,
    cell,
    group = undefined,
    groupLabel = undefined,
    selected = $bindable(null),
    open = undefined,
    empty = undefined,
    dense = false,
    label = "rows",
  }: {
    id: string;
    columns: Column<T>[];
    rows: T[];
    key: (row: T) => string;
    cell: Snippet<[T, Column<T>]>;
    group?: (row: T) => string;
    groupLabel?: Snippet<[string, number]>;
    selected?: string | null;
    open?: (row: T) => void;
    empty?: Snippet;
    dense?: boolean;
    label?: string;
  } = $props();

  // ── widths, remembered ───────────────────────────────────────────────────
  const wkey = () => `vp-grid:${id}`;
  function load(): Record<string, number> {
    try {
      return JSON.parse(localStorage.getItem(wkey()) ?? "{}");
    } catch {
      return {};
    }
  }
  let widths = $state<Record<string, number>>(load());
  const template = $derived(
    columns
      .map((c, i) => {
        const w = widths[c.key] ?? c.width;
        return w ? `${w}px` : i === columns.length - 1 ? "minmax(8rem, 1fr)" : "auto";
      })
      .join(" "),
  );
  let resizing: { key: string; x: number; from: number } | null = null;
  function grab(e: PointerEvent, c: Column<T>) {
    e.stopPropagation();
    const th = (e.currentTarget as HTMLElement).parentElement!;
    resizing = { key: c.key, x: e.clientX, from: th.getBoundingClientRect().width };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function drag(e: PointerEvent) {
    if (!resizing) return;
    widths = { ...widths, [resizing.key]: Math.max(48, resizing.from + e.clientX - resizing.x) };
  }
  function drop() {
    if (!resizing) return;
    resizing = null;
    try {
      localStorage.setItem(wkey(), JSON.stringify(widths));
    } catch {
      /* the widths last the tab */
    }
  }

  // ── sorting: none until a person asks ────────────────────────────────────
  let by = $state<Sorting>(null);
  function toggle(c: Column<T>) {
    by = nextSort(by, c);
  }
  const ordered = $derived(sorted(rows, columns, by));

  // ── groups, each with its count ──────────────────────────────────────────
  let folded = $state<Record<string, boolean>>({});
  const groups = $derived.by(() => {
    if (!group) return [{ name: "", rows: ordered }];
    const out: Array<{ name: string; rows: T[] }> = [];
    const at = new Map<string, number>();
    for (const r of ordered) {
      const g = group(r);
      if (!at.has(g)) {
        at.set(g, out.length);
        out.push({ name: g, rows: [] });
      }
      out[at.get(g)!].rows.push(r);
    }
    return out;
  });
  const visible = $derived(groups.flatMap((g) => (folded[g.name] ? [] : g.rows)));

  // ── the keyboard ─────────────────────────────────────────────────────────
  let body: HTMLElement | undefined = $state();
  function keydown(e: KeyboardEvent) {
    if (visible.length === 0) return;
    const i = visible.findIndex((r) => key(r) === selected);
    let next = i;
    if (e.key === "ArrowDown" || e.key === "j") next = Math.min(visible.length - 1, i + 1);
    else if (e.key === "ArrowUp" || e.key === "k") next = Math.max(0, i === -1 ? 0 : i - 1);
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = visible.length - 1;
    else if (e.key === "Enter" && i !== -1) {
      e.preventDefault();
      open?.(visible[i]);
      return;
    } else return;
    e.preventDefault();
    selected = key(visible[next]);
    queueMicrotask(() =>
      body?.querySelector<HTMLElement>(`[data-key="${CSS.escape(selected ?? "")}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  }
</script>

<div
  class="grid"
  class:dense
  role="grid"
  aria-label={label}
  aria-rowcount={rows.length}
  tabindex="0"
  onkeydown={keydown}
  style="--cols: {template}"
>
  <div class="head" role="row">
    {#each columns as c (c.key)}
      <div
        class="th"
        class:end={c.align === "end"}
        class:sortable={!!c.sort}
        role="columnheader"
        tabindex={c.sort ? 0 : -1}
        aria-sort={by?.key === c.key ? (by.dir === 1 ? "ascending" : "descending") : "none"}
        title={c.sort ? `sort by ${c.label.toLowerCase()} (Enter or Space)` : undefined}
        onclick={() => toggle(c)}
        onkeydown={(e) => {
          // A header's own keys: they sort, and never reach the rows' Enter.
          if (c.sort && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            e.stopPropagation();
            toggle(c);
          }
        }}
      >
        <span>{c.label}</span>
        {#if by?.key === c.key}<Icon name={by.dir === 1 ? "down" : "right"} size={12} />{/if}
        <span
          class="grip"
          role="presentation"
          onpointerdown={(e) => grab(e, c)}
          onpointermove={drag}
          onpointerup={drop}
          onpointercancel={drop}
        ></span>
      </div>
    {/each}
  </div>
  <div class="body" bind:this={body}>
    {#if rows.length === 0}
      {#if empty}{@render empty()}{/if}
    {:else}
      {#each groups as g (g.name)}
        {#if group}
          <button class="group" onclick={() => (folded = { ...folded, [g.name]: !folded[g.name] })}>
            <Icon name={folded[g.name] ? "right" : "down"} size={12} />
            {#if groupLabel}{@render groupLabel(g.name, g.rows.length)}{:else}<span>{g.name}</span>{/if}
            <span class="n">{g.rows.length}</span>
          </button>
        {/if}
        {#if !folded[g.name]}
          {#each g.rows as r (key(r))}
            <div
              class="row"
              role="row"
              data-key={key(r)}
              aria-selected={selected === key(r)}
              tabindex="-1"
              onclick={() => (selected = key(r))}
              ondblclick={() => open?.(r)}
              onkeydown={() => {}}
            >
              {#each columns as c (c.key)}
                <div class="td" class:end={c.align === "end"} class:mono={c.mono} role="gridcell">
                  {@render cell(r, c)}
                </div>
              {/each}
            </div>
          {/each}
        {/if}
      {/each}
    {/if}
  </div>
</div>

<style>
  .grid {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    font-size: var(--t-sm);
    outline: none;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: var(--cols);
    align-items: center;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    color: var(--faint);
    font-size: var(--t-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex: none;
  }
  .th {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.4rem var(--s-3);
    white-space: nowrap;
    overflow: hidden;
    user-select: none;
  }
  .th.sortable {
    cursor: pointer;
  }
  .th.sortable:hover {
    color: var(--ink);
  }
  .grip {
    position: absolute;
    right: 0;
    top: 20%;
    bottom: 20%;
    width: 5px;
    cursor: col-resize;
    border-right: 1px solid var(--line);
  }
  .grip:hover {
    border-right-color: var(--accent);
  }
  .body {
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  .row {
    border-bottom: 1px solid var(--line);
    cursor: default;
    min-height: 2.1rem;
  }
  .dense .row {
    min-height: 1.75rem;
  }
  .row:hover {
    background: var(--raise);
  }
  .row[aria-selected="true"] {
    background: var(--select);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .td {
    padding: 0.3rem var(--s-3);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .end {
    justify-content: flex-end;
    text-align: end;
  }
  .mono {
    font-family: var(--mono);
    font-size: var(--t-xs);
    font-variant-numeric: tabular-nums;
  }
  .group {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    padding: 0.35rem var(--s-3);
    border: 0;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 600;
    text-align: start;
    cursor: pointer;
    position: sticky;
    top: 0;
  }
  .group .n {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    color: var(--faint);
  }
  .grid:focus-visible .row[aria-selected="true"] {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
</style>
