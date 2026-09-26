<script lang="ts">
  // A host sentence with its `code` and **strong** rendered — as text nodes,
  // never as markup, so nothing in it can become a tag.
  import { inline, type Seg } from "../md";
  let { text = null, segs = null }: { text?: string | null; segs?: Seg[] | null } = $props();
  const parts = $derived(segs ?? inline(text));
</script>

{#each parts as p, i (i)}{#if p.k === "code"}<code>{p.s}</code>{:else if p.k === "strong"}<strong>{p.s}</strong>{:else}{p.s}{/if}{/each}
