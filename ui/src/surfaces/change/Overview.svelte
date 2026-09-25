<script lang="ts">
  // The overview of a change: evidence in four cards, facts in a grid, history
  // on one axis. Each card states a fact and links the tab that holds the rest.
  import { api } from "../../lib/api";
  import Icon from "../../lib/ui/Icon.svelte";
  import Props from "../../lib/ui/Props.svelte";
  import Timeline, { type Mark } from "../../lib/ui/Timeline.svelte";
  import { STATES } from "../../lib/State.svelte";
  import { exit, ago, type Detail } from "./types";

  type Decision = { id: string; at: string; authority: string; action: string; outcome: string; subject: string };
  let { d, go }: { d: Detail; go: (view: string) => void } = $props();

  let decisions = $state<Decision[]>([]);
  $effect(() => {
    const want = d.id;
    let live = true;
    api<Decision[]>(`/api/decisions?about=${encodeURIComponent(want)}&limit=200`)
      .then((r) => {
        if (live) decisions = Array.isArray(r) ? r : [];
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  const failing = $derived(d.gate && !d.gate.passed ? (d.gate.commands ?? []).filter((c) => !(c.outcome.outcome === "exited" && c.outcome.code === 0)) : []);
  const short = (s?: string | null) => (s ? s.slice(0, 10) : null);
  const tone = (a: string): Mark["tone"] => (a === "person" ? "work" : a === "rule" ? "wait" : a === "timer" || a === "nobody" ? "fail" : "none");

  const marks = $derived<Mark[]>([
    ...(d.created_at ? [{ at: d.created_at, lane: "change", label: "started", tone: "work" as const }] : []),
    ...(d.gates ?? []).map((g) => {
      const ok = g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);
      return { at: g.at, lane: "gates", label: `${g.gate} attempt ${g.attempt}: ${ok ? "passed" : "failed"}`, tone: ok ? ("done" as const) : ("fail" as const) };
    }),
    ...decisions.map((x) => ({ at: x.at, lane: "decisions", label: `${x.authority} · ${x.action} · ${x.outcome}`, tone: tone(x.authority) })),
    ...(d.archived_at ? [{ at: d.archived_at, lane: "change", label: STATES.archived.word, tone: "none" as const }] : []),
  ]);
  const byAuthority = $derived(
    Object.entries(
      decisions.reduce<Record<string, number>>((m, x) => ((m[x.authority] = (m[x.authority] ?? 0) + 1), m), {}),
    ),
  );
</script>

<div class="grid">
  <section class="cards">
    <button class="card" onclick={() => go("gates")}>
      <span class="k"><Icon name="gate" size={14} /> Gates</span>
      {#if !d.gate}
        <span class="v quiet">No gate has run against this change.</span>
      {:else if d.gate.passed}
        <span class="v" class:done={d.state === STATES.verified.word}>{d.gate.name} passed · attempt {d.gate.attempt}</span>
      {:else}
        <span class="v fail">{d.gate.name} failed · {failing.length} of {d.gate.commands?.length ?? 0} commands</span>
        {#each failing.slice(0, 2) as c (c.command)}<code class="cmd">{c.command} — {exit(c.outcome)}</code>{/each}
      {/if}
    </button>
    <button class="card" onclick={() => go("tasks")}>
      <span class="k"><Icon name="spec" size={14} /> Tasks</span>
      {#if d.counts}
        <span class="v"><b>{d.counts.tasks}</b> tasks · <b>{d.counts.ticked}</b> ticked · {#if d.counts.verified == null}no gates declared{:else}<b class="done">{d.counts.verified}</b> verified{/if}</span>
        {#if d.counts_says}<span class="s">{d.counts_says}</span>{/if}
        {#if d.counts.ticked_unsent?.length}<span class="s wait">{d.counts.ticked_unsent.length} ticked that no run was sent</span>{/if}
      {:else}
        <span class="v quiet">{d.spec ? "The specification has no tasks." : "No specification is attached."}</span>
      {/if}
    </button>
    <button class="card" onclick={() => go("ledger")}>
      <span class="k"><Icon name="ledger" size={14} /> Decisions</span>
      {#if decisions.length === 0}
        <span class="v quiet">Nothing has been decided about it.</span>
      {:else}
        <span class="v"><b>{decisions.length}</b> recorded</span>
        <span class="s">{byAuthority.map(([a, n]) => `${n} by ${a}`).join(" · ")}</span>
      {/if}
    </button>
    <button class="card" onclick={() => go("agent")}>
      <span class="k"><Icon name="agent" size={14} /> Agent</span>
      <span class="v"><b>{d.runs?.length ?? 0}</b> {d.runs?.length === 1 ? "run" : "runs"}{#if d.feedback_rounds} · {d.feedback_rounds} handed back{/if}</span>
      {#if d.stopped_summary}<span class="s fail">{d.stopped_summary}</span>{/if}
    </button>
  </section>

  {#if (d.drifts ?? []).length > 0}
    <section class="alert">
      <Icon name="alert" size={16} />
      <div>
        <b>The specification moved under a run.</b>
        {#each d.drifts ?? [] as x, i (i)}<p>{x.says}</p>{/each}
        <button onclick={() => go("tasks")}>Decide in Tasks</button>
      </div>
    </section>
  {/if}

  <section class="facts">
    <h2>Facts</h2>
    <Props
      rows={[
        { label: "Branch", value: d.branch, mono: true, missing: "none — works in place" },
        { label: "Worktree", value: d.worktree, mono: true, missing: "none" },
        { label: "Specification", value: d.spec, mono: true, missing: "none attached" },
        { label: "Tree now", value: d.tree_now ? `${short(d.tree_now.tree) ?? "?"} · ${d.tree_now.clean ? "clean" : `${d.tree_now.changed_files ?? 0} uncommitted`}` : null, mono: true, missing: "unknown" },
        { label: "Commit", value: short(d.tree_now?.commit), mono: true, missing: "no commits yet" },
        { label: "Pushed", value: d.tree_now?.reach?.replace(/_/g, " ") ?? null },
        { label: "Pull request", value: d.pull_request?.url ?? null, missing: "not offered" },
        { label: "Cost", value: d.cost_usd ? `$${d.cost_usd.toFixed(2)}` : null, missing: "not reported" },
        { label: "Started", value: ago(d.created_at) },
        { label: "Last moved", value: ago(d.updated_at) },
      ]}
    />
  </section>

  <section class="history">
    <h2>History</h2>
    <Timeline {marks} lanes={["change", "gates", "decisions"]} />
  </section>

  {#if (d.reports ?? []).length > 0}
    <section class="reports">
      <h2>Reports filed <span class="n">{d.reports?.length}</span></h2>
      {#each d.reports ?? [] as r (r.id)}
        <div class="report">
          <span>to <b>{r.target_says}</b> — {r.state_says}</span> <span class="quiet">{r.age_says}</span>
          <pre>{r.quoted}</pre>
        </div>
      {/each}
    </section>
  {/if}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(18rem, 26rem);
    gap: var(--s-5);
    align-items: start;
  }
  .cards,
  .alert,
  .history,
  .reports {
    grid-column: 1;
  }
  .facts {
    grid-column: 2;
    grid-row: 1 / span 3;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
    gap: var(--s-3);
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.35rem;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    text-align: start;
    cursor: pointer;
    min-width: 0;
  }
  .card:hover {
    border-color: var(--edge);
  }
  .k {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .v {
    font-size: var(--t-sm);
  }
  .s {
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .cmd {
    font-family: var(--mono);
    font-size: 0.6875rem;
    color: var(--fail);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .done { color: var(--done); }
  .fail { color: var(--fail); }
  .wait { color: var(--wait); }
  .quiet { color: var(--faint); }
  h2 {
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
    margin: 0 0 var(--s-3);
  }
  .facts {
    padding: var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  .alert {
    display: flex;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--wait);
    border-radius: var(--radius-lg);
    color: var(--wait);
    background: var(--panel);
  }
  .alert p {
    margin: var(--s-1) 0;
    color: var(--ink);
    font-size: var(--t-sm);
  }
  .alert button {
    margin-top: var(--s-2);
    font-size: var(--t-sm);
  }
  .report {
    padding: var(--s-2) 0;
    border-bottom: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  .report pre {
    margin: var(--s-1) 0 0;
    padding-left: var(--s-3);
    border-left: 2px solid var(--line);
    color: var(--dim);
    font-size: var(--t-xs);
    white-space: pre-wrap;
  }
  .n {
    color: var(--faint);
  }
  @container (max-width: 70rem) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .facts {
      grid-column: 1;
      grid-row: auto;
    }
  }
</style>
