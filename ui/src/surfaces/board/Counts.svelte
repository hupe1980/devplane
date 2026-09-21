<script lang="ts">
  // The header counts.
  //
  // **Every session is in exactly one of them.** Naming three of five above a
  // total reads as a breakdown and is not one — so a bucket with nothing in it
  // is absent rather than shown as a zero, and the ones that are shown add up.
  type Summary = {
    projects: number;
    runs: number;
    working: number;
    needs_you: number;
    idle: number;
    failed: number;
    dormant: number;
    cost_usd: number;
    open_issues: number;
    open_prs: number;
    forge_needs_you: number;
    asks_waiting: number;
  };

  let { summary }: { summary: Summary } = $props();
</script>

<div class="counts">
  <!-- **The one that is a claim on your attention leads, and the rest are
       context.** They were seven identically-weighted cards, so *1 need you*
       and *5 projects* looked equally worth reading — on a surface whose whole
       job is to say where to look. -->
  {#if summary.needs_you}
    <span class="card loud wait"><b>{summary.needs_you}</b> need you</span>
  {/if}
  {#if summary.asks_waiting}
    <span class="card loud wait"><b>{summary.asks_waiting}</b> waiting on you</span>
  {/if}
  {#if summary.failed}
    <span class="card loud fail"><b>{summary.failed}</b> failed</span>
  {/if}

  <span class="card"><b>{summary.working}</b> working</span>
  {#if summary.idle}<span class="card"><b>{summary.idle}</b> idle</span>{/if}
  <span class="card"><b>{summary.runs}</b> sessions</span>
  <span class="card"><b>{summary.projects}</b> projects</span>
  <!-- `spent`, not `spent today`: it is the sum over the sessions on this
       board, which is not a period. -->
  {#if summary.cost_usd > 0}<span class="card"><b>${summary.cost_usd.toFixed(2)}</b> spent</span>{/if}
  {#if summary.open_issues + summary.open_prs > 0}
    <span class="card">
      <b>{summary.open_issues + summary.open_prs}</b> issues &amp; PRs{#if summary.forge_needs_you}
        <span class="wait">· {summary.forge_needs_you} need you</span>{/if}
    </span>
  {/if}
  <!-- Counted, not listed: twenty rows that all look equally alive answer no
       question at all. -->
  {#if summary.dormant}<span class="card quiet">{summary.dormant} quiet</span>{/if}
</div>

<style>
  .counts { display: flex; flex-wrap: wrap; gap: var(--s-2); margin: var(--s-4) 0 var(--s-5); }

  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 5.5rem;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .card b {
    font-size: var(--t-lg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--ink);
    line-height: 1.1;
  }

  /* **Weight, not only colour.** The cards that are a claim on your attention
     carry a coloured edge and a heavier figure; the rest are context and read
     as context in greyscale too. */
  .loud { border-color: currentColor; }
  .loud b { font-size: 1.5rem; }
  .wait { color: var(--wait); }
  .wait b { color: var(--wait); }
  .fail { color: var(--fail); }
  .fail b { color: var(--fail); }
  .quiet { align-self: center; border: 0; background: none; min-width: 0; }
</style>
