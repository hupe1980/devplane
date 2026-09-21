<script lang="ts">
  // Start work, on one project or several.
  //
  // **The panel before the button is the feature.** Every other launcher in
  // this category sends and then tells you what went wrong; this one names
  // every refusal *before* anything is written, because a fan-out that half
  // fires is the one failure a control plane cannot take back.
  import type { PreflightFinding } from "../../wire/PreflightFinding";
  import type { PreflightReason } from "../../wire/PreflightReason";
  import { clip } from "../../lib/text";

  type Target = { id: string; name: string };

  let {
    projects = [],
    templates = [],
    preflight = [],
    prompt = $bindable(""),
    chosen = $bindable<string[]>([]),
  }: {
    projects?: Target[];
    templates?: Array<{ id: string; title: string }>;
    preflight?: PreflightFinding[];
    prompt?: string;
    chosen?: string[];
  } = $props();

  // **The daemon decides the position, not this page.** Draft above three
  // targets is a rule with a reason — a fan-out that starts six agents on a
  // typo is six worktrees to clean up — and a surface re-deriving it is the
  // second place that rule would live.
  const refusals = $derived(preflight.filter((p) => p.refusal !== null));
  const ready = $derived(preflight.filter((p) => p.refusal === null));
  const losing = $derived(preflight.filter((p) => (p.would_lose_fields ?? []).length > 0));

  // One sentence per refusal, and no two read alike: a reader has to know
  // which of five things to go and fix.
  const why: Record<PreflightReason, string> = {
    untrusted: "not trusted yet — `devplane trust` it first",
    dirty_worktree: "has uncommitted changes, so an agent would build on them",
    no_such_agent: "names an agent this machine cannot start",
    config_will_not_load: "has a devplane.toml that will not parse",
    over_ceiling: "is already at the number of agents it allows at once",
  };
</script>

<section aria-labelledby="dispatch-head">
  <h2 id="dispatch-head">Start work</h2>

  <textarea
    bind:value={prompt}
    rows="3"
    placeholder="what should the agent do?"
    aria-label="what should the agent do?"
  ></textarea>

  {#if templates.length > 0}
    <fieldset>
      <legend>start from something you already wrote</legend>
      {#each templates as t (t.id)}
        <button type="button">{t.title}</button>
      {/each}
    </fieldset>
  {/if}

  <fieldset>
    <legend>where</legend>
    {#each projects as p (p.id)}
      <label>
        <input type="checkbox" value={p.id} bind:group={chosen} />
        {p.name}
      </label>
    {/each}
  </fieldset>

  <!-- **What will happen, before it happens.** Named per target, so a person
       can fix one rather than being told the batch failed. -->
  <div class="will" aria-live="polite">
    {#if preflight.length === 0}
      <p class="dim">Choose a project and this says what will happen.</p>
    {:else}
      <p>
        <b>{ready.length}</b>
        {ready.length === 1 ? "project is" : "projects are"} ready.
        {#if refusals.length > 0}
          <b class="refused">{refusals.length}</b> cannot take this.
        {/if}
      </p>
      {#if refusals.length > 0}
        <ul>
          {#each refusals as r (r.project)}
            <li class="refused">
              <b>{clip(String(r.project), 32)}</b>
              {r.refusal ? why[r.refusal] : ""}
            </li>
          {/each}
        </ul>
      {/if}
      <!-- A warning and never a refusal: the artefact still works in the tool
           that wrote it, and the documented error is about leaving it. -->
      {#each losing as l (l.project)}
        <p class="warn">
          <b>{clip(String(l.project), 32)}</b> would drop
          {(l.would_lose_fields ?? []).join(", ")} — it still runs there.
        </p>
      {/each}
    {/if}
  </div>
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .5rem; }
  textarea { width: 100%; max-width: 44rem; font: inherit; padding: .4rem;
             background: var(--panel); color: var(--ink); border: 1px solid var(--line); }
  fieldset { border: 1px solid var(--line); margin: .5rem 0; padding: .4rem .6rem; }
  legend { color: var(--dim); font-size: .8rem; }
  label { margin-right: .8rem; }
  .will { margin-top: .6rem; }
  .will ul { list-style: none; margin: .2rem 0; padding: 0; }
  .refused { color: var(--fail); }
  .warn { color: var(--wait); }
  .dim { color: var(--dim); }
</style>
