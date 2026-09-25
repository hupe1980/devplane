<script lang="ts">
  // Every session on this machine, watched or driven, as one grid. The counts
  // are both summary and filter. A selected row offers raising its window or
  // attaching a terminal. An empty grid says what Devplane cannot see, never
  // calm.
  import { api } from "../../lib/api";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import Props from "../../lib/ui/Props.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";

  type Run = {
    id: string;
    project?: string;
    project_name?: string | null;
    agent?: string | null;
    mode?: string | null;
    permission_mode?: string | null;
    state?: string | null;
    waiting_for?: unknown;
    cwd?: string | null;
    branch?: string | null;
    model?: string | null;
    name?: string | null;
    summary?: string | null;
    cost_usd?: number | null;
    cost_unknown?: boolean;
    context_percent?: number | null;
    rate_limit_percent?: number | null;
    tool_calls?: number;
    subagents?: number;
    idle_seconds?: number;
    last_event_at?: string | null;
    plan_done?: number | null;
    plan_total?: number | null;
    sent_says?: string | null;
  };
  type Summary = Record<string, number>;

  let {
    runs = [],
    summary = null,
    thresholds = null,
    watching = null,
    loaded = false,
    error = null,
    focus = "",
  }: {
    runs?: Run[];
    summary?: Summary | null;
    thresholds?: { context_high_percent?: number } | null;
    watching?: { driven_only?: string[]; unproved?: string[] } | null;
    loaded?: boolean;
    /// Why the feed is not current: an empty grid then is a gap, not calm.
    error?: string | null;
    focus?: string;
  } = $props();

  const bucket = (r: Run) => {
    const s = (r.state ?? "").toLowerCase();
    if (/wait|ask|need|block/.test(s)) return "needs you";
    if (/work|run|busy|think/.test(s)) return "working";
    if (/fail|error/.test(s)) return "failed";
    if (/idle/.test(s)) return "idle";
    if (/complete|done|finish|exit/.test(s)) return "done";
    // An unknown state is counted as *other*, never guessed into *done*.
    return "other";
  };
  const BUCKETS = ["working", "needs you", "idle", "failed", "done", "other"];
  let only = $state<string | null>(null);
  let groupBy = $state<"project" | "state" | "none">("project");
  let selected = $state<string | null>(null);
  // A link to one session (`#board/<id>`, the activity panel) selects it.
  $effect(() => {
    if (focus) selected = focus;
  });

  const counts = $derived(BUCKETS.map((b) => [b, runs.filter((r) => bucket(r) === b).length] as const));
  const shown = $derived(only ? runs.filter((r) => bucket(r) === only) : runs);
  const pick = $derived(runs.find((r) => r.id === selected) ?? null);
  /// The threshold is the host's; with none, nothing is marked crowded.
  const high = $derived(thresholds?.context_high_percent ?? null);
  const crowded = (r: Run) => high != null && r.context_percent != null && r.context_percent >= high;
  /// What this board cannot see, said beside an empty one.
  const blind = $derived(
    [
      watching?.driven_only?.length ? `${watching.driven_only.join(", ")} appear only when Devplane starts them.` : "",
      watching?.unproved?.length ? `${watching.unproved.join(", ")} ${watching.unproved.length === 1 ? "is" : "are"} read, but has not been proved against a live session.` : "",
    ]
      .filter(Boolean)
      .join(" "),
  );

  function when(at?: string | null): string {
    if (!at) return "—";
    const s = Math.max(0, Math.round((Date.now() - Date.parse(at)) / 1000));
    return s < 60 ? `${s}s` : s < 3600 ? `${Math.round(s / 60)}m` : s < 86400 ? `${Math.round(s / 3600)}h` : `${Math.round(s / 86400)}d`;
  }
  const columns: Column<Run>[] = [
    { key: "state", label: "State", width: 130, sort: (r) => bucket(r) },
    { key: "session", label: "Session", width: 220, sort: (r) => r.name ?? r.agent ?? r.id },
    { key: "project", label: "Project", width: 120, sort: (r) => r.project_name ?? "" },
    { key: "doing", label: "Doing", width: 320 },
    { key: "context", label: "Context", width: 90, align: "end", sort: (r) => r.context_percent ?? -1, mono: true },
    { key: "cost", label: "Cost", width: 80, align: "end", sort: (r) => r.cost_usd ?? -1, mono: true },
    { key: "tools", label: "Tools", width: 70, align: "end", sort: (r) => r.tool_calls ?? 0, mono: true },
    { key: "last", label: "Last", align: "end", sort: (r) => r.last_event_at ?? "", mono: true },
  ];

  let said = $state("");
  async function raise(r: Run) {
    try {
      await api(`/api/runs/${encodeURIComponent(r.id)}/focus`, { method: "POST" });
      said = "Raised its window.";
    } catch (e) {
      said = `That did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
  async function copy(t: string) {
    try {
      await navigator.clipboard?.writeText(t);
      said = `Copied: ${t}`;
    } catch {
      said = t;
    }
  }
</script>

<div class="board">
  <header class="head">
    <h1>Sessions</h1>
    <div class="chips" role="group" aria-label="by state">
      <button class:on={only === null} onclick={() => (only = null)}>all <span>{runs.length}</span></button>
      {#each counts as [b, n] (b)}
        <button class:on={only === b} class={b.replace(" ", "-")} disabled={n === 0} onclick={() => (only = only === b ? null : b)}>{b} <span>{n}</span></button>
      {/each}
    </div>
    <span class="gap"></span>
    <label class="group">Group
      <select bind:value={groupBy} aria-label="group by">
        <option value="project">by project</option>
        <option value="state">by state</option>
        <option value="none">none</option>
      </select>
    </label>
    {#if summary?.cost_usd}<span class="cost" title="reported by the vendors' own telemetry">${summary.cost_usd.toFixed(2)} reported today</span>{/if}
  </header>

  {#if !loaded && runs.length === 0}
    <Skeleton />
  {:else if runs.length === 0 && error}
    <p class="quiet">The last read had no session in it, and Devplane has not answered since.</p>
  {:else if runs.length === 0}
    <Empty
      icon="sessions"
      title="No session is reporting"
      body="Sessions you start yourself appear here as soon as they report — Claude Code with no configuration, others once connected."
      limit={blind}
    />
  {:else}
    <div class="frame">
      <Grid
        id="sessions"
        {columns}
        rows={shown}
        key={(r) => r.id}
        group={groupBy === "none" ? undefined : groupBy === "project" ? (r) => r.project_name ?? "no project" : bucket}
        bind:selected
        label="sessions"
      >
        {#snippet cell(r, c)}
          {#if c.key === "state"}<Pill word={r.state ?? "unknown"} />
          {:else if c.key === "session"}
            <span class="sess"><Icon name={r.mode === "driven" ? "agent" : "eye"} size={13} /> <b>{r.name ?? r.agent ?? "session"}</b> <span class="dim">{r.model ?? ""}</span></span>
          {:else if c.key === "project"}{r.project_name ?? "—"}
          {:else if c.key === "doing"}<span class="dim">{r.summary ?? ""}</span>
          {:else if c.key === "context"}
            {#if r.context_percent != null}<span class="ctx" class:hot={crowded(r)} title={crowded(r) ? "context nearly full" : undefined}>{#if crowded(r)}<Icon name="alert" size={12} /><span class="sr-only">context nearly full</span>{/if}{Math.round(r.context_percent)}%</span>{:else}<span class="dim" title="not reported">—</span>{/if}
          {:else if c.key === "cost"}{#if r.cost_unknown || r.cost_usd == null}<span class="dim">—</span>{:else}${r.cost_usd.toFixed(2)}{/if}
          {:else if c.key === "tools"}{r.tool_calls ?? 0}
          {:else}{when(r.last_event_at)}{/if}
        {/snippet}
        {#snippet empty()}<p class="quiet">{only ? `No session is ${only}.` : "No session."}</p>{/snippet}
      </Grid>

      {#if pick}
        <aside class="drawer" aria-label="the session">
          <header>
            <b>{pick.name ?? pick.agent ?? "session"}</b>
            <Pill word={pick.state ?? "unknown"} />
            <button class="x" aria-label="close" onclick={() => (selected = null)}><Icon name="x" size={13} /></button>
          </header>
          <Props
            rows={[
              { label: "Project", value: pick.project_name },
              { label: "Agent", value: pick.agent },
              { label: "Mode", value: pick.mode === "driven" ? "driven by Devplane" : "watched" },
              { label: "Permissions", value: pick.permission_mode, missing: "not reported" },
              { label: "Model", value: pick.model, missing: "not reported" },
              { label: "Branch", value: pick.branch, mono: true, missing: "none" },
              { label: "Directory", value: pick.cwd, mono: true },
              { label: "Plan", value: pick.plan_total ? `${pick.plan_done ?? 0} of ${pick.plan_total} steps done` : null, missing: "no plan reported" },
              { label: "Sent", value: pick.sent_says },
              { label: "Subagents", value: pick.subagents ? String(pick.subagents) : null, missing: "none" },
              { label: "Session", value: pick.id, mono: true },
            ]}
          />
          <div class="acts">
            <button onclick={() => raise(pick!)}><Icon name="external" size={13} /> Raise its window</button>
            <button onclick={() => copy(`devplane attach ${pick!.id}`)}><Icon name="terminal" size={13} /> Copy attach command</button>
          </div>
          {#if said}<p class="said">{said}</p>{/if}
        </aside>
      {/if}
    </div>
  {/if}
</div>

<style>
  .board {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--s-4) var(--s-5) var(--s-4);
    gap: var(--s-3);
    box-sizing: border-box;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .chips {
    display: flex;
    gap: var(--s-1);
    flex-wrap: wrap;
  }
  .chips button {
    height: 1.6rem;
    padding: 0 0.6rem;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--panel);
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .chips button span {
    color: var(--faint);
    margin-left: 0.15rem;
    font-variant-numeric: tabular-nums;
  }
  .chips button.on {
    border-color: var(--accent);
    background: var(--select);
    color: var(--ink);
  }
  .chips button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .chips .needs-you:not(:disabled) {
    color: var(--wait);
  }
  .chips .failed:not(:disabled) {
    color: var(--fail);
  }
  .gap {
    flex: 1;
  }
  .group {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .group select {
    font-size: var(--t-xs);
  }
  .cost {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .sess {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .sess b {
    font-weight: 550;
    color: var(--ink);
  }
  .dim {
    color: var(--faint);
  }
  .hot {
    color: var(--wait);
    font-weight: 700;
  }
  .drawer {
    width: 22rem;
    flex: none;
    border-left: 1px solid var(--line);
    background: var(--side);
    padding: var(--s-3) var(--s-4);
    overflow: auto;
    display: grid;
    align-content: start;
    gap: var(--s-3);
  }
  .drawer header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .x {
    margin-left: auto;
    display: grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    padding: 0;
    border: 0;
    background: none;
    color: var(--dim);
    cursor: pointer;
  }
  .acts {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .acts button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    justify-content: flex-start;
    font-size: var(--t-sm);
  }
  .said {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .quiet {
    padding: var(--s-4);
    color: var(--faint);
    margin: 0;
  }
</style>
