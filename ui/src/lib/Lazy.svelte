<script lang="ts" module>
  import type { Component } from "svelte";
  import type { Loader } from "./surfaces";

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const loaded = new Map<Loader, Component<any>>();
</script>

<script lang="ts">
  // A surface's page, loaded on first use and kept. Until it is here the
  // region shows the shape of what is coming, never words.
  import { untrack } from "svelte";
  import Skeleton from "./Skeleton.svelte";

  let {
    load = undefined,
    component = undefined,
    ...rest
  }: {
    load?: Loader;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    component?: Component<any>;
    [prop: string]: unknown;
  } = $props();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let fetched = $state<Component<any> | null>(untrack(() => (load ? (loaded.get(load) ?? null) : null)));
  let failed = $state("");
  $effect(() => {
    const want = load;
    failed = "";
    if (!want) return;
    const have = loaded.get(want);
    if (have) {
      fetched = have;
      return;
    }
    fetched = null;
    want()
      .then((m) => {
        loaded.set(want, m.default);
        if (load === want) fetched = m.default;
      })
      .catch((e) => {
        if (load === want) failed = e instanceof Error ? e.message : String(e);
      });
  });
  const Page = $derived(component ?? fetched);
</script>

{#if Page}
  <Page {...rest} />
{:else if failed}
  <p class="failed" role="alert">This page did not load: {failed}. Reload the window.</p>
{:else}
  <div class="wait"><Skeleton /></div>
{/if}

<style>
  .wait {
    padding: 0 var(--s-5);
  }
  .failed {
    margin: var(--s-4) var(--s-5);
    color: var(--fail);
    font-size: var(--t-sm);
  }
</style>
