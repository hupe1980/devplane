<script lang="ts">
  // The banner over data that is not the present: when the host stops
  // answering, the last good read stays on screen, marked with its age.
  import { ago } from "./text";

  let {
    error,
    stale_since = null,
  }: { error: string; stale_since?: string | null } = $props();

  /// Ticks, so the age keeps growing while the host is away.
  let now = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => (now = Date.now()), 1_000);
    return () => clearInterval(id);
  });

  const age = $derived(
    stale_since ? ago(Math.max(0, (now - Date.parse(stale_since)) / 1000)) : "",
  );
</script>

<p class="stale" role="status">
  {error}
  {#if age}
    <span class="age">· showing what was read {age} ago</span>
  {:else}
    <span class="age">· nothing has been read yet</span>
  {/if}
</p>

<style>
  .stale {
    color: var(--fail);
    font-size: var(--t-sm);
    border-top: 1px solid currentColor;
    border-bottom: 1px solid currentColor;
    padding: var(--s-2) 0;
    margin: var(--s-3) 0;
  }
  .age { color: var(--dim); }
</style>
