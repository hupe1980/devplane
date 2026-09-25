<script lang="ts">
  // The review of one change, on its own page — `#review/<id>` and the
  // `?review=<id>` link land here. The same pane is a change's Review tab.
  import Empty from "../../lib/ui/Empty.svelte";
  import Pane from "./Pane.svelte";
  import type { ReviewBody } from "./types";

  let {
    chosen = "",
    title = "",
    review = null,
  }: { chosen?: string; title?: string; review?: ReviewBody | null } = $props();
</script>

<div class="page">
  {#if !chosen}
    <Empty icon="split" title="Pick a change to review" body="A review reads one change against its base, in the order its project declared." />
  {:else}
    <h1>Review{#if title}: {title}{/if}</h1>
    <Pane id={chosen} {review} />
  {/if}
</div>

<style>
  .page {
    padding: var(--s-5) var(--s-6);
  }
  h1 {
    margin: 0 0 var(--s-3);
    font-size: 1.25rem;
  }
</style>
