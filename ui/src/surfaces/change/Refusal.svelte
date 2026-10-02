<script lang="ts" module>
  import type { WeakKind } from "../../wire/WeakKind";
  export type Row = { path: string; why: string; matched: string; kind?: WeakKind };
  export type OfferRefusal = { says: string; rows: Row[]; seen_with: string };
</script>

<script lang="ts">
  // The host's refusal to offer, naming every unseen weakened row, each with
  // the way into the review that reads it.
  import Inline from "../../lib/ui/Inline.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { weakKindSays } from "../../lib/weakKind";

  let { refusal, read }: { refusal: OfferRefusal; read: (path: string) => void } = $props();
</script>

<div class="refusal" role="alert">
  <p><b>Not offered</b> — <Inline text={refusal.says} />. Nothing was pushed.</p>
  <ul>
    {#each refusal.rows as row (row.path + row.matched)}
      <li>
        <code>{row.path}</code> {#if row.kind}<b class="kind">{weakKindSays[row.kind]}</b> {/if}<span><Inline text={row.why} /></span>
        <button onclick={() => read(row.path)}><Icon name="split" size={13} /> Read it</button>
      </li>
    {/each}
  </ul>
  <p class="how">Mark each row read in the review, or from a terminal of your own: <code>{refusal.seen_with}</code></p>
</div>

<style>
  .refusal {
    font-size: var(--t-sm);
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--wait);
    background: var(--panel);
    display: grid;
    gap: var(--s-1);
  }
  .refusal p,
  .refusal ul {
    margin: 0;
  }
  .refusal ul {
    padding: 0;
    list-style: none;
    display: grid;
    gap: var(--s-1);
  }
  .refusal li {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .refusal li span {
    color: var(--wait);
  }
  .refusal button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.6rem;
    padding: 0 0.5rem;
    font-size: var(--t-xs);
  }
  .refusal .how {
    color: var(--dim);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
