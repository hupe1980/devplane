<script lang="ts">
  // What a read that did not land looks like, on every surface: the host's
  // reason and the command that tells more. `stale` when the last good answer
  // is still on screen, with its age; otherwise nothing was read at all, and
  // the region says so instead of looking empty or calm.
  //
  // When the host itself is not answering, the shell's one banner already
  // names that and its command: a region then says only that it was not read,
  // once, rather than repeating the host's error in every pane.
  import type { Failure } from "./resource.svelte";
  import Inline from "./ui/Inline.svelte";
  import { ago } from "./text";

  let {
    what,
    failure,
    at = null,
    stale = false,
  }: {
    /// What could not be read, as a noun: `this change`, `the projects`.
    what: string;
    failure: Failure;
    /// When the data on screen was read, for a stale region.
    at?: number | null;
    stale?: boolean;
  } = $props();
  const What = $derived(what.charAt(0).toUpperCase() + what.slice(1));
</script>

{#if failure.down}
  <p class="failed down" class:stale role="status">
    {#if stale}{What} as read {at ? `${ago((Date.now() - at) / 1000)} ago` : "earlier"} — not read since.{:else}{What}: not read.{/if}
  </p>
{:else}
  <p class="failed" class:stale role="alert">
    {#if stale}
      Showing {what} as read {at ? `${ago((Date.now() - at) / 1000)} ago` : "earlier"} — the re-read failed: <Inline text={failure.says} />.
    {:else}
      Could not read {what}: <Inline text={failure.says} />.
    {/if}
    <span class="tell"><code>{failure.tell}</code> tells more.</span>
  </p>
{/if}

<style>
  .failed {
    margin: var(--s-2) 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--fail);
    border-radius: var(--radius);
    color: var(--fail);
    font-size: var(--t-sm);
  }
  .failed.stale {
    border-style: dashed;
  }
  .failed.down {
    border: 0;
    padding: var(--s-1) 0;
    color: var(--dim);
  }
  .tell {
    color: var(--dim);
  }
  code,
  .failed :global(code) {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
