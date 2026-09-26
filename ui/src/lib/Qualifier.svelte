<script lang="ts">
  // What a change's own diff weakened, beside its state and on the same line:
  // *verified · 1 check weakened*. Words as well as colour, in the warning
  // weight, with how many no person has marked seen; nothing at all when
  // nothing was weakened. The words are the
  // host's (`Qualifier::says`), counted by kind, never a score.
  import type { Qualifier } from "../wire/Qualifier";

  let { q, compact = false }: { q: Qualifier | null | undefined; compact?: boolean } = $props();
  const shown = $derived(!!q && q.says !== "");
</script>

{#if shown && q}<span class="qualifier" class:compact title={q.unseen > 0 ? `${q.says} — ${q.unseen} not yet marked seen` : `${q.says} — every one marked seen`}><span class="sep" aria-hidden="true">·</span> {q.says}{#if q.unseen > 0}<span class="unseen">({q.unseen} unseen)</span>{/if}</span>{/if}

<style>
  .qualifier {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3em;
    color: var(--wait);
    font-weight: 600;
    font-size: var(--t-xs);
    white-space: nowrap;
  }
  .sep {
    color: var(--dim);
    font-weight: 400;
  }
  .unseen {
    font-weight: 400;
  }
  .compact {
    font-size: var(--t-xs);
  }
</style>
