<script lang="ts">
  // What a read that did not land looks like, on every surface: the host's
  // reason and the command that tells more. `stale` when the last good answer
  // is still on screen, with its age; otherwise nothing was read at all, and
  // the region says so instead of looking empty or calm.
  import type { Failure } from "./resource.svelte";
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
</script>

<p class="failed" class:stale role="alert">
  {#if stale}
    Showing {what} as read {at ? `${ago((Date.now() - at) / 1000)} ago` : "earlier"} — the re-read failed: {failure.says}.
  {:else}
    Could not read {what}: {failure.says}.
  {/if}
  <span class="tell"><code>{failure.tell}</code> tells more.</span>
</p>

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
  .tell {
    color: var(--dim);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
