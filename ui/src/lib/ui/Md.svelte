<script lang="ts">
  // A host-composed or agent-written block of markdown, through the one
  // renderer (`lib/md.ts`): headings, paragraphs, list items, table rows and
  // fenced code, each as text nodes — never markup, never a link.
  import { blocks } from "../md";
  import Inline from "./Inline.svelte";

  let { text = "" }: { text?: string | null } = $props();
  const parts = $derived(blocks(text));
</script>

<div class="md">
  {#each parts as b, i (i)}
    {#if b.kind === "pre"}<pre>{b.text}</pre>
    {:else if b.kind === "h"}<p class="h"><Inline segs={b.segs} /></p>
    {:else if b.kind === "li"}<p class="li"><span aria-hidden="true">•</span> <Inline segs={b.segs} /></p>
    {:else if b.kind === "row"}<p class="row"><Inline segs={b.segs} /></p>
    {:else}<p><Inline segs={b.segs} /></p>{/if}
  {/each}
</div>

<style>
  .md {
    display: grid;
    gap: var(--s-1);
    min-width: 0;
  }
  p,
  pre {
    margin: 0;
  }
  .h {
    font-weight: 650;
  }
  .li {
    padding-left: var(--s-3);
    text-indent: calc(-1 * var(--s-3));
  }
  pre {
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    padding: var(--s-2);
    border-radius: var(--radius);
    background: var(--chrome);
  }
  .md :global(code) {
    font-family: var(--mono);
    font-size: 0.92em;
  }
</style>
