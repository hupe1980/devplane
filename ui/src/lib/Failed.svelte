<script lang="ts">
  // What a read that did not land looks like, on every surface: what could
  // not be read, what that means for the page, the command that tells more, a
  // retry, and the host's raw answer behind a disclosure. `stale` when the
  // last good answer is still on screen, with its age; otherwise nothing was
  // read at all, and the region says so instead of looking empty or calm.
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
    retry = undefined,
  }: {
    /// What could not be read, as a noun: `this change`, `the projects`.
    what: string;
    failure: Failure;
    /// When the data on screen was read, for a stale region.
    at?: number | null;
    stale?: boolean;
    /// Reads again; defaults to the failure's own (a resource sets it).
    retry?: () => Promise<void> | void;
  } = $props();
  const What = $derived(what.charAt(0).toUpperCase() + what.slice(1));
  const again = $derived(retry ?? failure.retry);
  let retrying = $state(false);
  async function doRetry() {
    if (!again || retrying) return;
    retrying = true;
    try {
      await again();
    } finally {
      retrying = false;
    }
  }
</script>

{#if failure.down}
  <p class="failed down" class:stale role="status">
    {#if stale}{What} as read {at ? `${ago((Date.now() - at) / 1000)} ago` : "earlier"} — not read since.{:else}{What}: not read.{/if}
  </p>
{:else}
  <div class="failed" class:stale role="alert">
    <p>
      {#if stale}
        Showing {what} as read {at ? `${ago((Date.now() - at) / 1000)} ago` : "earlier"} — the re-read failed: <Inline text={failure.says} />.
      {:else}
        Could not read {what}: <Inline text={failure.says} />. Nothing on this page is current.
      {/if}
      <span class="tell"><code>{failure.tell}</code> tells more.</span>
    </p>
    {#if again || failure.raw}
      <div class="more">
        {#if again}<button type="button" onclick={doRetry} disabled={retrying}>{retrying ? "Reading…" : "Retry"}</button>{/if}
        {#if failure.raw}
          <details>
            <summary>raw error</summary>
            <pre data-quoted>{failure.raw}</pre>
          </details>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .failed {
    margin: var(--s-2) 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--fail);
    border-radius: var(--radius);
    color: var(--fail);
    font-size: var(--t-sm);
    display: grid;
    gap: var(--s-2);
  }
  .failed p {
    margin: 0;
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
  .more {
    display: flex;
    flex-wrap: wrap;
    align-items: start;
    gap: var(--s-3);
    color: var(--dim);
  }
  summary {
    cursor: pointer;
    font-size: var(--t-xs);
  }
  pre {
    margin: var(--s-1) 0 0;
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 12rem;
    overflow: auto;
  }
  code,
  .failed :global(code) {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
