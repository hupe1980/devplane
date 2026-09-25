<script lang="ts">
  // The bottom panel. Activity: every session, newest event first, one line
  // each. Sight: which vendors are watched end to end, read but unproved, or
  // seen only when Devplane drives them — said once here, not in every empty
  // list.
  import Icon from "../lib/ui/Icon.svelte";
  import Pill from "../lib/ui/Pill.svelte";

  type Run = {
    id: string;
    project_name?: string | null;
    agent?: string | null;
    mode?: string | null;
    state?: string | null;
    summary?: string | null;
    last_event_at?: string | null;
    context_percent?: number | null;
  };
  type Board = {
    runs?: Run[];
    watching?: { watched?: string[]; unproved?: string[]; driven_only?: string[] };
    coverage?: { projects?: number; unreadable?: string[] };
  };
  /// Shows a run on whichever surface holds runs; `null` when none does, and
  /// the rows are then plain text.
  let { board, open }: { board: Board | null; open: ((run: string) => void) | null } = $props();

  let view = $state<"activity" | "sight">("activity");
  const runs = $derived(
    [...(board?.runs ?? [])].sort((a, b) => (b.last_event_at ?? "").localeCompare(a.last_event_at ?? "")),
  );
  const w = $derived(board?.watching ?? {});

  function clock(at?: string | null): string {
    if (!at) return "--:--:--";
    const d = new Date(at);
    return Number.isNaN(d.getTime()) ? "--:--:--" : d.toTimeString().slice(0, 8);
  }
</script>

<section class="panel" aria-label="activity panel">
  <div class="head" role="tablist" aria-label="panel views">
    <button role="tab" aria-selected={view === "activity"} onclick={() => (view = "activity")}>
      Activity <span class="n">{runs.length}</span>
    </button>
    <button role="tab" aria-selected={view === "sight"} onclick={() => (view = "sight")}>
      Sight <span class="n">{(w.unproved?.length ?? 0) + (w.driven_only?.length ?? 0)}</span>
    </button>
  </div>
  <div class="body">
    {#if view === "activity"}
      {#if runs.length === 0}
        <p class="quiet">No session has reported anything yet.</p>
      {:else}
        <ol class="log">
          {#each runs as r (r.id)}
            <li>
              <button onclick={() => open?.(r.id)} disabled={!open}>
                <time>{clock(r.last_event_at)}</time>
                <span class="proj">{r.project_name ?? "—"}</span>
                <span class="agent">{r.agent ?? "agent"}{r.mode === "driven" ? " · driven" : ""}</span>
                <Pill word={r.state ?? "unknown"} />
                <span class="what">{r.summary ?? ""}</span>
                {#if r.context_percent != null}<span class="ctx">{Math.round(r.context_percent)}% context</span>{/if}
              </button>
            </li>
          {/each}
        </ol>
      {/if}
    {:else}
      <dl class="sight">
        <dt><Icon name="eye" size={13} /> Watched end to end</dt>
        <dd>{w.watched?.join(", ") || "none"}</dd>
        <dt><Icon name="alert" size={13} /> Read, not yet proved against a live session</dt>
        <dd>{w.unproved?.join(", ") || "none"}</dd>
        <dt><Icon name="agent" size={13} /> Seen only when Devplane starts them</dt>
        <dd>{w.driven_only?.join(", ") || "none"}</dd>
        {#if board?.coverage?.unreadable?.length}
          <dt><Icon name="x" size={13} /> Projects whose configuration cannot be read</dt>
          <dd>{board.coverage.unreadable.join(", ")}</dd>
        {/if}
      </dl>
      <p class="quiet">A session in a tool that is not watched here is not on the board — the board is the limit of this machine's sight, not a report that nothing is running.</p>
    {/if}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    background: var(--side);
  }
  .head {
    display: flex;
    gap: var(--s-1);
    padding: 0 var(--s-3);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .head button {
    height: 1.9rem;
    border: 0;
    border-bottom: 1px solid transparent;
    margin-bottom: -1px;
    border-radius: 0;
    background: none;
    color: var(--faint);
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    cursor: pointer;
    padding: 0 0.5rem;
  }
  .head button[aria-selected="true"] {
    color: var(--ink);
    border-bottom-color: var(--accent);
  }
  .n {
    font-variant-numeric: tabular-nums;
    color: var(--faint);
    margin-left: 0.2rem;
  }
  .body {
    overflow: auto;
    flex: 1;
    min-height: 0;
    padding: var(--s-1) 0;
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0;
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .log button {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: 100%;
    padding: 0.15rem var(--s-4);
    border: 0;
    border-radius: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    text-align: start;
    cursor: pointer;
    white-space: nowrap;
  }
  .log button:hover {
    background: var(--raise);
    color: var(--ink);
  }
  time {
    color: var(--faint);
  }
  .proj {
    color: var(--accent);
    min-width: 6rem;
  }
  .agent {
    min-width: 7rem;
  }
  .what {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--ink);
  }
  .ctx {
    color: var(--faint);
  }
  .sight {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--s-2) var(--s-4);
    margin: var(--s-3) var(--s-4);
    font-size: var(--t-sm);
  }
  .sight dt {
    color: var(--faint);
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .sight dd {
    margin: 0;
    color: var(--ink);
  }
  .quiet {
    margin: var(--s-2) var(--s-4);
    color: var(--faint);
    font-size: var(--t-xs);
  }
</style>
