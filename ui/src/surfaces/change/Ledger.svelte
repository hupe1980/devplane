<script lang="ts">
  // What was decided about this change, and on whose authority (person, rule,
  // timer, nobody, devplane). The default hides nothing; each chip narrows by
  // authority and shows what it would leave.
  import { api } from "../../lib/api";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import { ago } from "./types";

  type Decision = {
    id: string;
    at: string;
    authority: string;
    action: string;
    subject: string;
    outcome: string;
    reason?: string | null;
  };
  let { id }: { id: string } = $props();

  let rows = $state<Decision[] | null>(null);
  let error = $state("");
  let only = $state<string | null>(null);
  let selected = $state<string | null>(null);
  $effect(() => {
    const want = id;
    let live = true;
    api<Decision[]>(`/api/decisions?about=${encodeURIComponent(want)}&limit=500`)
      .then((r) => {
        if (live) rows = Array.isArray(r) ? r : [];
      })
      .catch((e) => {
        if (live) error = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });

  const AUTHORITIES = ["person", "rule", "timer", "nobody", "devplane"];
  const counts = $derived(
    AUTHORITIES.map((a) => [a, (rows ?? []).filter((r) => r.authority === a).length] as const),
  );
  const shown = $derived((rows ?? []).filter((r) => !only || r.authority === only));
  const columns: Column<Decision>[] = [
    { key: "at", label: "When", width: 110, sort: (r) => r.at },
    { key: "authority", label: "Authority", width: 120, sort: (r) => r.authority },
    { key: "action", label: "Action", width: 150, sort: (r) => r.action, mono: true },
    { key: "outcome", label: "Outcome", width: 110, sort: (r) => r.outcome },
    { key: "subject", label: "About", width: 320, mono: true },
    { key: "reason", label: "Why" },
  ];
  const pick = $derived(shown.find((r) => r.id === selected) ?? null);
</script>

<div class="ledger">
  <div class="chips" role="group" aria-label="by authority">
    <button class:on={only === null} onclick={() => (only = null)}>everything <span>{rows?.length ?? 0}</span></button>
    {#each counts as [a, n] (a)}
      <button class:on={only === a} disabled={n === 0} onclick={() => (only = only === a ? null : a)}>{a} <span>{n}</span></button>
    {/each}
  </div>
  {#if error}
    <p class="fail">The decisions could not be read: {error}</p>
  {:else}
    <div class="frame">
      <Grid id="change-ledger" {columns} rows={shown} key={(r) => r.id} bind:selected label="decisions about this change">
        {#snippet cell(r, c)}
          {#if c.key === "at"}<span title={r.at}>{ago(r.at)}</span>
          {:else if c.key === "authority"}<Pill word={r.authority} as={r.authority === "person" ? "work" : r.authority === "rule" ? "wait" : r.authority === "devplane" ? "none" : "fail"} />
          {:else if c.key === "outcome"}<Pill word={r.outcome} dot={false} />
          {:else if c.key === "action"}{r.action}
          {:else if c.key === "subject"}{r.subject}
          {:else}<span class="why">{r.reason ?? ""}</span>{/if}
        {/snippet}
        {#snippet empty()}<p class="quiet">{rows === null ? "Reading…" : only ? `Nothing about this change was decided by ${only}.` : "Nothing has been decided about this change."}</p>{/snippet}
      </Grid>
    </div>
    {#if pick}
      <aside class="detail">
        <b>{pick.action}</b> · {pick.outcome} · by {pick.authority} · {new Date(pick.at).toLocaleString()}
        <pre>{pick.subject}</pre>
        {#if pick.reason}<p>{pick.reason}</p>{/if}
      </aside>
    {/if}
  {/if}
</div>

<style>
  .ledger {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  .chips {
    display: flex;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .chips button {
    height: 1.7rem;
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
    font-variant-numeric: tabular-nums;
    margin-left: 0.2rem;
  }
  .chips button.on {
    border-color: var(--accent);
    color: var(--ink);
    background: var(--select);
  }
  .chips button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .frame {
    display: flex;
    height: min(60vh, 34rem);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .why {
    color: var(--dim);
  }
  .detail {
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    font-size: var(--t-sm);
  }
  .detail pre {
    margin: var(--s-2) 0;
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    color: var(--dim);
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
    padding: var(--s-4);
    margin: 0;
  }
  .fail {
    color: var(--fail);
  }
</style>
