<script lang="ts">
  // Where a change is in its six-state life. The host computes the state; this
  // only places it. Nothing is green but *verified*.
  import { LIFE as STEPS, STATES, type Life } from "../../lib/State.svelte";
  const VERIFIED = STEPS.indexOf(STATES.verified.word as Life);

  let { state, archived = false, offered = false }: { state: string; archived?: boolean; offered?: boolean } = $props();

  const at = $derived.by(() => {
    if (archived) return STEPS.length - 1;
    if (offered) return STEPS.length - 2;
    const i = STEPS.indexOf(state as Life);
    // A state outside the six (stopped, gates failing) sits at *in flight*.
    return i === -1 ? 2 : i;
  });
  const off = $derived(!STEPS.includes(state as Life) && !archived && !offered);
</script>

<ol class="stepper" aria-label="where this change is">
  {#each STEPS as s, i (s)}
    <li class:past={i < at} class:here={i === at} class:verified={i === at && i === VERIFIED} class:off={i === at && off} aria-current={i === at ? "step" : undefined}>
      <span class="node" aria-hidden="true"></span>
      <span class="label">{i === at && off ? state : STATES[s].word}</span>
    </li>
  {/each}
</ol>

<style>
  .stepper {
    display: flex;
    list-style: none;
    margin: var(--s-2) 0 0;
    padding: 0;
    max-width: 52rem;
  }
  li {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    position: relative;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  li::before {
    content: "";
    position: absolute;
    top: 0.3rem;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--line);
  }
  li.past::before {
    background: var(--accent);
  }
  li:first-child::before {
    left: 0.3rem;
  }
  .node {
    position: relative;
    width: 0.65rem;
    height: 0.65rem;
    border-radius: 50%;
    border: 2px solid var(--edge);
    background: var(--bg);
  }
  .past .node {
    border-color: var(--accent);
    background: var(--accent);
  }
  .here .node {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .here {
    color: var(--ink);
    font-weight: 600;
  }
  .verified .node {
    border-color: var(--done);
    background: var(--done);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--done) 25%, transparent);
  }
  .verified {
    color: var(--done);
  }
  .off .node {
    border-color: var(--wait);
  }
  .off {
    color: var(--wait);
  }
  .label {
    white-space: nowrap;
  }
</style>
