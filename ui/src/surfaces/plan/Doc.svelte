<script lang="ts">
  // One specification: outline, boxes, cited and uncited requirements, open
  // questions, and the change working to it. Devplane reads the spec, never
  // grades it: counts and names only, no percentage or bar.
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import { run } from "../../lib/keys";
  import { specs, watch, key } from "./store.svelte";

  let { focus = "" }: { focus?: string } = $props();
  $effect(watch);

  const found = $derived.by(() => {
    for (const p of specs.projects ?? []) for (const r of p.plans) if (key(p, r) === focus) return { p, r };
    return null;
  });
  const total = $derived((specs.projects ?? []).reduce((n, p) => n + p.plans.length, 0));
</script>

{#if !focus || !found}
  {#if specs.projects === null}
    <div class="pad quiet">Reading the specifications…</div>
  {:else}
    <Empty
      icon="spec"
      title={total ? "Pick a specification" : "No specification on this machine"}
      body={total
        ? `${total} specifications across ${specs.projects.length} projects. A specification is read as it is on disk: its outline, its boxes, and the requirements its tasks cite.`
        : "Devplane reads Spec Kit, OpenSpec and Kiro folders where they already are, and a plain folder a project names in devplane.toml."}
    />
  {/if}
{:else}
  {@const r = found.r}
  <article class="doc">
    <header>
      <p class="where"><Icon name="folder" size={12} /> {found.p.project} <span>/</span> <code>{r.plan.path}</code></p>
      <h1>{r.plan.outline[0]?.text ?? r.plan.path}</h1>
      <div class="facts">
        {#if r.plan.progress}<span><b>{r.plan.progress.total}</b> boxes · <b>{r.plan.progress.done}</b> ticked</span>{/if}
        <span>{r.plan.files} files</span>
        {#if r.plan.open_questions > 0}<span class="wait"><Icon name="question" size={13} /> {r.plan.open_questions} open questions</span>{/if}
        {#if r.drifted}<span class="wait"><Icon name="alert" size={13} /> changed under a run</span>{/if}
      </div>
      <div class="acts">
        {#if r.change_id}
          <a class="btn" href={`#change/${encodeURIComponent(r.change_id)}`}><Icon name="change" size={14} /> {r.title}</a>
          {#if r.state}<Pill word={r.state} />{/if}
          {#if r.counts_says}<span class="quiet">{r.counts_says}</span>{/if}
        {:else}
          <button class="btn primary" onclick={() => run("new-change", "plan")}><Icon name="play" size={14} /> Start a change…</button>
          <span class="quiet">No change is working to this specification; pick it in the form.</span>
        {/if}
      </div>
      {#if r.contradicts_done}<p class="warn"><Icon name="alert" size={13} /> Its change is finished while boxes are still open.</p>{/if}
    </header>

    <div class="cols">
      <nav class="outline" aria-label="outline">
        <h2>Outline</h2>
        <ol>
          {#each r.plan.outline as h, i (i)}
            <li class="h{Math.min(h.level, 4)}" style:padding-left="{Math.max(0, h.level - 1) * 0.8}rem">{h.text}</li>
          {/each}
        </ol>
      </nav>
      <div class="main">
        {#if r.plan.questions?.length}
          <section class="card wait">
            <h2><Icon name="question" size={13} /> Waiting on a person</h2>
            <ul>{#each r.plan.questions as q, i (i)}<li>{typeof q === "string" ? q : JSON.stringify(q)}</li>{/each}</ul>
          </section>
        {/if}
        <section class="card">
          <h2>Requirements and the tasks that cite them</h2>
          {#if !r.trace}
            <p class="quiet">Not traced.</p>
          {:else if r.trace.unrecognised}
            <p class="quiet">No requirement notation was recognised in this folder. A project declares its own with <code>[spec] tokens</code> in devplane.toml; nothing is guessed.</p>
          {:else}
            <table>
              <thead><tr><th>requirement</th><th>tasks</th><th>ticked</th></tr></thead>
              <tbody>
                {#each r.trace.edges as [token, tasks] (token)}
                  <tr><td><code>{token}</code></td><td>{tasks.length}</td><td>{tasks.filter((t) => t.done).length}</td></tr>
                {/each}
              </tbody>
            </table>
            {#if r.trace.orphan_requirements.length}
              <p class="warn">No task cites: {r.trace.orphan_requirements.join(", ")}</p>
            {/if}
            {#if r.trace.tasks_citing_nothing.length}
              <p class="quiet">{r.trace.tasks_citing_nothing.length} tasks cite no requirement.</p>
            {/if}
          {/if}
        </section>
      </div>
    </div>
  </article>
{/if}

<style>
  .doc {
    padding: var(--s-5) var(--s-6);
    display: grid;
    gap: var(--s-4);
  }
  header {
    display: grid;
    gap: var(--s-2);
  }
  .where {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  h1 {
    margin: 0;
    font-size: 1.35rem;
    letter-spacing: -0.01em;
  }
  .facts {
    display: flex;
    gap: var(--s-4);
    font-size: var(--t-sm);
    color: var(--dim);
    flex-wrap: wrap;
  }
  .facts span {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.9rem;
    padding: 0 0.75rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    text-decoration: none;
    cursor: pointer;
  }
  .btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(14rem, 22rem) minmax(0, 1fr);
    gap: var(--s-4);
    align-items: start;
  }
  .outline,
  .card {
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    padding: var(--s-3) var(--s-4);
  }
  .outline ol {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: var(--t-sm);
    display: grid;
    gap: 0.2rem;
  }
  .h1 {
    font-weight: 650;
    color: var(--ink);
    margin-top: var(--s-2);
  }
  .h2 {
    color: var(--ink);
  }
  .h3,
  .h4 {
    color: var(--dim);
  }
  .main {
    display: grid;
    gap: var(--s-3);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
    margin: 0 0 var(--s-2);
  }
  .card.wait {
    border-color: var(--wait);
  }
  .card.wait h2 {
    color: var(--wait);
  }
  ul {
    margin: 0;
    padding-left: var(--s-4);
    font-size: var(--t-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--t-sm);
  }
  th {
    text-align: start;
    font-size: var(--t-xs);
    color: var(--faint);
    font-weight: 600;
    border-bottom: 1px solid var(--line);
    padding: var(--s-1) var(--s-2);
  }
  td {
    padding: var(--s-1) var(--s-2);
    border-bottom: 1px solid var(--line);
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .warn,
  .wait {
    color: var(--wait);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: var(--s-2) 0 0;
    font-size: var(--t-sm);
  }
  .pad {
    padding: var(--s-5);
  }
  @container (max-width: 70rem) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
