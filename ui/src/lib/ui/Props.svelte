<script lang="ts">
  // Facts about one thing, as label and value. A `null` value is printed as
  // unknown, never left blank: absence is not an empty cell.
  import type { Snippet } from "svelte";

  type Row = { label: string; value?: string | null; mono?: boolean; tone?: string; missing?: string };
  let { rows, extra }: { rows: Row[]; extra?: Snippet } = $props();
</script>

<dl class="props">
  {#each rows as r (r.label)}
    <dt>{r.label}</dt>
    <dd class:mono={r.mono} class={r.tone ?? ""} class:missing={r.value == null || r.value === ""}>
      {r.value == null || r.value === "" ? (r.missing ?? "—") : r.value}
    </dd>
  {/each}
  {#if extra}{@render extra()}{/if}
</dl>

<style>
  .props {
    display: grid;
    grid-template-columns: minmax(7rem, max-content) 1fr;
    gap: 0.3rem var(--s-4);
    margin: 0;
    font-size: var(--t-sm);
  }
  dt {
    color: var(--faint);
    white-space: nowrap;
  }
  dd {
    margin: 0;
    color: var(--ink);
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .mono {
    font-family: var(--mono);
    font-size: var(--t-xs);
    line-height: 1.7;
  }
  .missing {
    color: var(--faint);
    font-style: italic;
  }
  .work { color: var(--work); }
  .wait { color: var(--wait); }
  .fail { color: var(--fail); }
  .done { color: var(--done); }
</style>
