<script lang="ts">
  // **What each project is working to**, across every repository on the machine.
  //
  // The square "render a specification" is occupied four times over and every
  // entrant is a VS Code extension scoped to the repository the editor has
  // open. What none of them can reach is the join: a plan's box counts beside a
  // session's state, for six repositories at once. So this page is not a
  // progress view — it is *what is wrong*, and the two numbers worth a glance
  // are **boxes still open** and **questions nobody answered**.
  //
  // No bar above the fold. A bar answers *how far*; 84 % hides both numbers
  // behind a colour.
  import type { Plan } from "../../wire/Plan";

  export type PlanRow = {
    /// Present only where a piece of Work names this plan. A plan without one
    /// is a plan the repository has; with one, it is a plan somebody is working
    /// to — and the page must not blur them.
    work_id: string | null;
    title: string | null;
    phase: string | null;
    contradicts_done: boolean;
    drifted: boolean | null;
    plan: Plan;
  };
  export type ProjectRow = {
    project_id: string;
    project: string;
    root: string;
    declares_markers: boolean;
    declares_plans: boolean;
    plans: PlanRow[];
  };

  let { projects = [], omitted = 0, loaded = false, failed = "", retry }: {
    projects?: ProjectRow[];
    /// How many projects this page is **not** showing. A page that silently
    /// listed the first forty would be one whose completeness nobody can check.
    omitted?: number;
    loaded?: boolean;
    failed?: string;
    /// Reads it again. A failure whose only way out is a reload is worse than
    /// it looks: `devplane open` puts the token in the URL once and the page
    /// strips it, so reloading a tab is not free.
    retry?: () => void;
  } = $props();

  // A project with nothing in flight has no current plan. It is listed so the
  // page is an inventory rather than a filter — and never given a guessed one.
  const withPlans = $derived(projects.filter((p) => p.plans.length > 0));
  const without = $derived(
    projects.filter((p) => p.plans.length === 0 && p.declares_plans),
  );
  /// Projects that were never asked where their plans are. Silence here is *not
  /// configured*, not *none*.
  const unasked = $derived(
    projects.filter((p) => p.plans.length === 0 && !p.declares_plans),
  );

  function boxes(p: Plan): string {
    // An absent task list is absent. `0 of 0` is unreachable from the type, and
    // saying "complete" about a plan with no boxes is the most confident wrong
    // thing this page could do.
    if (!p.progress) return "no task list";
    const open = p.progress.total - p.progress.done;
    if (open === 0) return `all ${p.progress.total} done`;
    return `${open} of ${p.progress.total} open`;
  }
</script>

<section aria-labelledby="plan">
  <h2 id="plan">What each project is working to</h2>

  {#if failed}
    <p class="empty" role="status">
      The plans could not be read: {failed}
      {#if retry}<button class="undo" onclick={retry}>try again</button>{/if}
    </p>
  {:else if !loaded}
    <p class="empty">Reading the plans…</p>
  {:else if projects.length === 0}
    <p class="empty">No projects yet. <code>devplane trust .</code> in a repository.</p>
  {:else}
    {#each withPlans as p (p.project_id)}
      <article class="project">
        <h3>{p.project}</h3>
        <!-- **Keyed on the path, not the work id.** A plan a Work names carries
             one; a plan the repository merely has carries `null` — so keying on
             it gave thirteen rows the same key, Svelte threw
             `each_key_duplicate` mid-render, and the surface never left its
             loading state. The path is unique within a project and is what the
             row is *about*. -->
        {#each p.plans as row (row.plan.path)}
          <div class="plan" class:contradicts={row.contradicts_done}>
            <div class="head">
              {#if row.work_id}
                <a href={`#work/${row.work_id}`}>{row.title}</a>
                <span class="phase">{row.phase}</span>
              {:else}
                <span class="name">{row.plan.path.split("/").pop()}</span>
              {/if}
            </div>

            {#if !row.plan.present}
              <p class="line fail">
                Names <code>{row.plan.path}</code>, and there is no such specification.
              </p>
            {:else}
              <p class="line">
                <code>{row.plan.path}</code>
                <span class="sep">·</span>
                <span class={row.plan.progress && row.plan.progress.done < row.plan.progress.total
                  ? "open"
                  : "dim"}>{boxes(row.plan)}</span>
                {#if row.plan.open_questions > 0}
                  <span class="sep">·</span>
                  <span class="wait"
                    >{row.plan.open_questions} question{row.plan.open_questions === 1 ? "" : "s"}
                    nobody answered</span>
                {/if}
              </p>
            {/if}

            {#if row.drifted}
              <p class="line fail">
                The specification changed while this work was running.
              </p>
            {/if}

            {#if row.contradicts_done}
              <p class="line fail">
                <strong>Done</strong>, and the plan it answers is not.
              </p>
            {/if}

            {#if row.plan.truncated}
              <p class="line dim">{row.plan.truncated}</p>
            {/if}

            {#if row.plan.questions.length > 0}
              <ul class="questions">
                {#each row.plan.questions as q (q.path + q.text)}
                  <li><code>{q.path}</code> {q.text}</li>
                {/each}
              </ul>
            {/if}

            {#if row.plan.outline.length > 0}
              <details>
                <summary>{row.plan.files} document{row.plan.files === 1 ? "" : "s"}</summary>
                <ul class="outline">
                  {#each row.plan.outline as h, i (i)}
                    <li style={`padding-left:${Math.min(h.level, 4) * 0.6}rem`}>{h.text}</li>
                  {/each}
                </ul>
              </details>
            {/if}
          </div>
        {/each}
        {#if !p.declares_markers}
          <p class="note">
            This project has not said which words mark an unresolved question, so none are
            collected. Add <code>[spec] open_questions</code> to its <code>devplane.toml</code>.
          </p>
        {/if}
      </article>
    {/each}

    {#if omitted > 0}
      <p class="foot">
        {omitted} more project{omitted === 1 ? "" : "s"} are not shown. <code>devplane work list</code>
        has every one.
      </p>
    {/if}

    <!-- **Two silences, and they are different facts.** A project that has not
         said where its plans live has not reported having none — it has not
         been asked. Telling a reader "no plans" for a repository full of them
         is the page being confidently wrong about the thing it exists for. -->
    {#if unasked.length > 0}
      <p class="foot">
        Not looked for in: {unasked.map((p) => p.project).join(", ")}. Add
        <code>[spec] plans = "specs"</code> to a project's <code>devplane.toml</code> and its
        plans appear here.
      </p>
    {/if}
    {#if without.length > 0}
      <p class="foot">
        No plans in: {without.map((p) => p.project).join(", ")}.
      </p>
    {/if}
  {/if}
</section>

<style>
  .undo {
    margin-left: 0.4rem;
    background: none;
    border: 1px solid var(--line);
    color: var(--ink);
    padding: 0.1rem 0.4rem;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .project {
    margin-bottom: 1.5rem;
  }
  h3 {
    font-size: 0.95rem;
    margin: 0 0 0.4rem;
  }
  .plan {
    border-left: 2px solid var(--line);
    padding: 0.35rem 0 0.35rem 0.7rem;
    margin-bottom: 0.6rem;
  }
  .plan.contradicts {
    border-left-color: var(--fail);
  }
  .head {
    display: flex;
    gap: 0.6rem;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .phase {
    color: var(--dim);
    font-size: 0.8rem;
  }
  .line {
    margin: 0.2rem 0;
    font-size: 0.85rem;
  }
  .sep {
    color: var(--faint);
    margin: 0 0.3rem;
  }
  .open {
    color: var(--work);
  }
  .wait {
    color: var(--wait);
  }
  .fail {
    color: var(--fail);
  }
  .dim {
    color: var(--dim);
  }
  .questions {
    margin: 0.3rem 0 0;
    padding-left: 1rem;
    font-size: 0.82rem;
    color: var(--wait);
  }
  .outline {
    margin: 0.3rem 0 0;
    padding-left: 0;
    list-style: none;
    font-size: 0.82rem;
    color: var(--dim);
  }
  .note,
  .foot,
  .empty {
    color: var(--dim);
    font-size: 0.82rem;
  }
  details summary {
    cursor: pointer;
    color: var(--dim);
    font-size: 0.82rem;
  }
</style>
