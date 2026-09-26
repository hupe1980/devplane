<script lang="ts">
  // The specification's tasks against the work: ticked, sent to which run, and
  // seen by a passing check. Ticked (the agent's word) and seen by a passing
  // check (its run closed before a check that passed) are two columns, never
  // one figure. A task is never called verified — only a change is.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { writer } from "../../lib/write.svelte";
  import Plan from "./Plan.svelte";
  import type { Detail } from "./types";
  import type { Step } from "./pair";

  let { d, lastRun, reload = async () => {} }: { d: Detail; lastRun: string; reload?: () => Promise<void> } = $props();

  const run = $derived(lastRun);
  const read = resource<{ plan?: Step[] }>(() => (run ? `/api/runs/${encodeURIComponent(run)}` : null), {
    tell: () => `devplane show ${run}`,
  });
  const steps = $derived(read.data?.plan ?? []);
  /// Loaded (or nothing to load): until then the plan's counts are not shown.
  const planKnown = $derived(!run || read.phase !== "loading");
  let said = $state("");
  const pending = writer();

  const sent = $derived(
    (d.run_rows ?? []).find((r) => r.id === lastRun)?.sent ?? d.run_rows?.[d.run_rows.length - 1]?.sent ?? [],
  );

  async function decide(verb: "tell" | "accept", run: string) {
    said = "";
    await pending.run(verb === "tell" ? "Telling the run" : "Accepting", async () => {
      try {
        const id = encodeURIComponent(d.id);
        const route = verb === "tell" ? `/api/changes/${id}/drift/tell` : `/api/changes/${id}/drift/accept`;
        await api(route, { method: "POST", body: JSON.stringify({ run }) });
        said = verb === "tell" ? "Told — the run was handed the files that changed." : "Accepted — the change now works to what the run saw.";
        await reload();
      } catch (e) {
        said = `That did not land: ${failure(e).says}`;
      }
    });
  }
</script>

<div class="tasks">
  {#if d.counts}
    <div class="counts">
      <div><span class="big">{d.counts.tasks}</span><span class="lbl">tasks</span></div>
      <div><span class="big">{d.counts.ticked}</span><span class="lbl">ticked by an agent</span></div>
      <div>
        {#if d.counts.seen_by_pass == null}<span class="big quiet">—</span><span class="lbl">no gates declared</span>
        {:else}<span class="big">{d.counts.seen_by_pass}</span><span class="lbl">seen by a passing check</span>{/if}
      </div>
      <p class="says">{d.counts_says}</p>
    </div>
    {#if d.counts.ticked_unsent?.length}
      <section class="note wait">
        <h3><Icon name="alert" size={14} /> Ticked, but no run was sent them <span class="n">{d.counts.ticked_unsent.length}</span></h3>
        <ul>{#each d.counts.ticked_unsent as t (t)}<li>{t}</li>{/each}</ul>
      </section>
    {/if}
    {#if d.counts.sent_unticked?.length}
      <section class="note">
        <h3>Sent, not ticked <span class="n">{d.counts.sent_unticked.length}</span></h3>
        <ul>{#each d.counts.sent_unticked as t (t)}<li>{t}</li>{/each}</ul>
      </section>
    {/if}
  {:else}
    <p class="quiet">{d.spec ? `${d.spec} has no task file this change can read.` : "No specification is attached to this change, so there are no tasks to trace."}</p>
  {/if}

  {#if (d.drifts ?? []).length > 0}
    <section class="note wait">
      <h3><Icon name="alert" size={14} /> The specification changed under a run</h3>
      {#each d.drifts ?? [] as x, i (i)}
        <div class="drift">
          <p>{x.says}</p>
          <button disabled={!!pending.busy} onclick={() => decide("tell", x.run)}>Tell the run</button>
          <button disabled={!!pending.busy} onclick={() => decide("accept", x.run)}>Accept what it saw</button>
        </div>
      {/each}
      {#if pending.busy}<p class="said" role="status">{pending.busy}… {pending.elapsed}</p>{:else if said}<p class="said" role="status">{said}</p>{/if}
    </section>
  {/if}

  {#if (d.token_rows ?? []).length > 0}
    <section>
      <h2>Requirements and the tasks that cite them</h2>
      <table>
        <thead><tr><th>requirement</th><th>tasks</th><th>ticked</th><th>seen by a passing check</th><th></th></tr></thead>
        <tbody>
          {#each d.token_rows ?? [] as r (r.token)}
            <tr>
              <td class="mono">{r.token}</td>
              <td>{r.tasks}</td>
              <td>{r.ticked}</td>
              <td class:quiet={r.seen_by_pass == null}>{r.seen_by_pass ?? "no gates"}</td>
              <td class="quiet">{r.says ?? ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}

  <section>
    <h2>What the latest run was sent, and its own plan</h2>
    {#if read.failure}<Failed what="the run's own plan" failure={read.failure} at={read.at} stale={read.data !== null} />{/if}
    {#if planKnown}
      <Plan tasks={sent.map((t) => ({ text: t.text, path: t.path, line: t.line }))} {steps} />
    {:else}
      <p class="quiet" aria-busy="true">Reading the run's own plan…</p>
    {/if}
  </section>

  {#if (d.run_rows ?? []).length > 1}
    <section>
      <h2>Every run</h2>
      <ul class="runs">
        {#each d.run_rows ?? [] as r (r.id)}<li><code>{r.id.slice(0, 16)}</code> {r.sent_says}</li>{/each}
      </ul>
    </section>
  {/if}

  {#if d.plan?.outline?.length}
    <section>
      <h2>Outline of {d.plan.path}</h2>
      <ol class="outline">
        {#each d.plan.outline as h, i (i)}<li style:padding-left="{Math.max(0, h.level - 1) * 1}rem" class="h{h.level}">{h.text}</li>{/each}
      </ol>
    </section>
  {/if}
</div>

<style>
  .tasks {
    display: grid;
    gap: var(--s-5);
    max-width: 72rem;
  }
  .counts {
    display: flex;
    align-items: flex-end;
    gap: var(--s-6);
    flex-wrap: wrap;
  }
  .counts div {
    display: flex;
    flex-direction: column;
  }
  .big {
    font-size: 1.75rem;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }
  .lbl {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .says {
    margin: 0;
    color: var(--dim);
    font-size: var(--t-sm);
  }
  h2 {
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
    margin: 0 0 var(--s-2);
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: var(--t-sm);
    margin: 0 0 var(--s-2);
  }
  .note {
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  .note.wait {
    border-color: var(--wait);
  }
  .note.wait h3 {
    color: var(--wait);
  }
  .note ul {
    margin: 0;
    padding-left: var(--s-4);
    font-size: var(--t-sm);
  }
  .drift {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }
  .drift p {
    flex-basis: 100%;
    margin: 0;
    font-size: var(--t-sm);
  }
  .said {
    color: var(--dim);
    font-size: var(--t-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--t-sm);
  }
  th {
    text-align: start;
    color: var(--faint);
    font-size: var(--t-xs);
    font-weight: 600;
    padding: var(--s-1) var(--s-2);
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: var(--s-1) var(--s-2);
    border-bottom: 1px solid var(--line);
  }
  .mono,
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .runs {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: var(--t-sm);
  }
  .outline {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .outline .h1 {
    color: var(--ink);
    font-weight: 600;
  }
  .outline .h2 {
    color: var(--ink);
  }
  .quiet { color: var(--faint); }
  .n { color: var(--faint); font-weight: 400; }
</style>
