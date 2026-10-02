<script lang="ts" module>
  import type { Qualifier } from "../wire/Qualifier";

  /// Whether what a change's own diff weakened leads its state: while any
  /// weakened row is unread, the weakening is the first thing said and the
  /// state follows it, subordinate and without its colour.
  export const weakLeads = (q: Qualifier | null | undefined): boolean => !!q && q.says !== "" && q.unseen > 0;
</script>

<script lang="ts">
  // A change's state, in the one order every surface uses. Evidence first:
  // *1 check weakened · verified* while a weakened row is unread, in the
  // danger weight, with *verified* after it as a plain qualifier — never the
  // green first. Once every row is read: the state, then what was weakened.
  import Pill from "./ui/Pill.svelte";
  import Icon from "./ui/Icon.svelte";
  import QualifierLine from "./Qualifier.svelte";

  let { state, q = null }: { state: string; q?: Qualifier | null } = $props();
  const leads = $derived(weakLeads(q));
</script>

{#if leads && q}
  <span class="lead" title="{q.says} — {q.unseen} not yet read; offering waits for every one">
    <span class="weak"><Icon name="alert" size={12} />{q.says}</span><span class="sep" aria-hidden="true">·</span><span class="sub">{state}</span>
  </span>
{:else}
  <Pill word={state} /><QualifierLine {q} />
{/if}

<style>
  .lead {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    white-space: nowrap;
    font-size: var(--t-xs);
    min-width: 0;
  }
  .weak {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    padding: 0 0.5em;
    height: 1.35rem;
    border-radius: 999px;
    border: 1px solid var(--fail);
    color: var(--fail);
    font-weight: 700;
  }
  .sep {
    color: var(--faint);
  }
  /* Subordinate: the word, not the verified green. */
  .sub {
    color: var(--dim);
    font-weight: 400;
  }
</style>
