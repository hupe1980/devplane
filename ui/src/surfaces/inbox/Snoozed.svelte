<script lang="ts">
  // What a snooze keeps off the list, counted where the list is counted and
  // listed on request, each with when it comes back and a way to bring it
  // back now. Nothing is hidden without a count.
  import Icon from "../../lib/ui/Icon.svelte";
  import { reread } from "../../lib/live.svelte";
  import { bringBack, type SnoozedRow } from "./actions";

  let {
    count = 0,
    rows = [],
    whole = false,
  }: {
    /// The host's snoozed number: what the rows add up to.
    count?: number;
    rows?: SnoozedRow[];
    /// Over a narrowed list: the snoozed are the whole inbox's.
    whole?: boolean;
  } = $props();

  let open = $state(false);
  let said = $state("");
  let busy = $state(false);
  async function back(row: SnoozedRow) {
    busy = true;
    said = (await bringBack(row)).said;
    busy = false;
    reread();
  }
</script>

{#if count > 0}
  <div class="snoozed">
    <button class="toggle" aria-expanded={open} aria-controls="snoozed-rows" onclick={() => (open = !open)}>
      <Icon name="clock" size={13} />
      {count} snoozed{whole ? " across the inbox" : ""}
      <span>{open ? "hide" : "list them"}</span>
    </button>
    {#if open}
      <ul id="snoozed-rows" aria-label="snoozed">
        {#each rows as r (r.id)}
          <li>
            <span class="title">{r.title}</span>
            <span class="says">{r.kind.replace(/_/g, " ")} · {r.says}</span>
            <button onclick={() => back(r)} disabled={busy}>bring back</button>
          </li>
        {/each}
      </ul>
    {/if}
    {#if said}<p class="said" role="status">{said}</p>{/if}
  </div>
{/if}

<style>
  .snoozed {
    padding: var(--s-1) var(--s-3);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0;
    border: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    cursor: pointer;
  }
  .toggle span {
    color: var(--accent);
  }
  ul {
    list-style: none;
    margin: var(--s-1) 0 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0 var(--s-2);
    align-items: center;
  }
  .title {
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .says {
    grid-column: 1;
  }
  li button {
    grid-column: 2;
    grid-row: 1 / span 2;
    font: inherit;
  }
  .said {
    margin: var(--s-1) 0 0;
  }
</style>
