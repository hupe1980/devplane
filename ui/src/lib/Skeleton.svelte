<script lang="ts">
  // The shape of what is coming, and no words: before the first poll a surface
  // claims nothing. The rows do not animate.
  //
  // A skeleton promises data. When the host's feed failed before anything was
  // read — or the caller's own read failed (`failed`) — that promise is false,
  // so it says *not read* once instead, and the failure is named where it is
  // owned (the shell's banner, or the caller's failed state).
  import { unread } from "./live.svelte";

  let { rows = 3, failed = false }: { rows?: number; failed?: boolean } = $props();
</script>

{#if failed || unread.failed}
  <p class="unread" role="status">Not read — nothing here is current.</p>
{:else}
  <div class="skeleton" aria-busy="true" aria-label="reading">
    {#each { length: rows } as _, i (i)}
      <div class="row"><span class="bar" style:width="{40 + ((i * 23) % 40)}%"></span></div>
    {/each}
  </div>
{/if}

<style>
  .skeleton { display: grid; margin-top: var(--s-4); }
  .row {
    height: 28px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--line);
  }
  .bar {
    display: block;
    height: 0.6em;
    border-radius: 2px;
    background: var(--line);
  }
  .unread {
    margin: var(--s-4) 0 0;
    color: var(--dim);
    font-size: var(--t-sm);
  }
</style>
