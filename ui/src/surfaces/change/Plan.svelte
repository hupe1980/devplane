<script lang="ts">
  // The tasks the run was sent beside the plan the agent reported, paired by
  // key. The status beside a step is the agent's own word.
  import { pair } from "./pair";
  import type { Step, Task } from "./pair";

  let {
    tasks = [],
    steps = [],
  }: {
    /// What the run was sent, from its record.
    tasks?: Task[];
    /// What the agent said it would do, as it reported it.
    steps?: Step[];
  } = $props();

  const rows = $derived(pair(tasks, steps));
  const matched = $derived(rows.filter((r) => r.task && r.step).length);
</script>

<section class="plan" aria-label="tasks and the agent's plan">
  <div class="heads">
    <h3>tasks sent <span class="n">{tasks.length}</span></h3>
    <h3>the agent's plan <span class="n">{steps.length}</span></h3>
  </div>
  {#if tasks.length === 0 && steps.length === 0}
    <p class="dim">No tasks were sent and no plan was reported.</p>
  {:else}
    <ol class="rows">
      {#each rows as r, i (i)}
        <li class="row" class:pair={!!(r.task && r.step)}>
          <span class="task">
            {#if r.task}<span class="text">{r.task.text}</span>{#if r.task.path}<span class="where">{r.task.path}{r.task.line ? `:${r.task.line}` : ""}</span>{/if}{/if}
          </span>
          <span class="step">
            {#if r.step}<span class="status">{r.step.status.replace(/_/g, " ")}</span><span class="text">{r.step.content}</span>{/if}
          </span>
        </li>
      {/each}
    </ol>
    <!-- Three counts, never a fraction. -->
    <p class="dim tally">{matched} paired · {tasks.length - matched} tasks alone · {steps.length - matched} steps alone</p>
  {/if}
</section>

<style>
  .heads, .row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: var(--s-4); }
  h3 { font-size: var(--t-xs); font-weight: 600; color: var(--dim); margin: 0 0 var(--s-1); }
  .n { font-weight: 400; margin-left: var(--s-1); }
  .rows { list-style: none; margin: 0; padding: 0; border-top: 1px solid var(--line); }
  .row { padding: var(--s-2) 0; border-bottom: 1px solid var(--line); font-size: var(--t-sm); }
  .row.pair { background: var(--panel); }
  .task, .step { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .text { overflow-wrap: anywhere; }
  .where, .status { color: var(--dim); font-size: var(--t-xs); }
  .dim { color: var(--dim); }
  .tally { font-size: var(--t-xs); margin: var(--s-1) 0 0; }
</style>
