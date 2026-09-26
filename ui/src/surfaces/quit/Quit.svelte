<script lang="ts">
  // What quitting stops, said before it stops, then two buttons. The sentence
  // is the host's (`/api/quitting`, as `devplane quit` prints it), verbatim.
  // *Keep running* hides the window and changes nothing.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import { hideWindow } from "../../lib/frame";
  import { onAction } from "../../lib/keys";

  let {
    /// The sentence, planted by a harness; otherwise read from the host.
    says = null,
  }: { says?: string | null } = $props();

  const read = resource<{ says?: string }>(() => "/api/quitting", { tell: () => "devplane quit" });
  let refused = $state("");
  let stopping = $state(false);
  const shown = $derived(read.data ? (read.data.says ?? "") : says);
  // Unreadable is not empty: never print *this stops nothing*.
  const unreadable = $derived(
    refused ||
      (read.failure
        ? `Could not read what this would stop (${read.failure.says}). Quitting anyway ends anything it started.`
        : ""),
  );

  $effect(() =>
    onAction("leave", () => {
      keep();
      return true;
    }),
  );

  async function quit() {
    stopping = true;
    try {
      await api("/api/quit", { method: "POST" });
    } catch (e) {
      stopping = false;
      refused = `that did not land: ${failure(e).says}`;
    }
  }
  function keep() {
    hideWindow();
  }
</script>

<section class="quit" aria-labelledby="quit-head">
  <h2 id="quit-head">Quit Devplane?</h2>
  {#if shown !== null}
    <pre class="says">{shown}</pre>
  {:else if unreadable}
    <p class="says warn">{unreadable}</p>
  {:else}
    <p class="dim">reading what this would stop…</p>
  {/if}
  <div class="acts">
    <button class="primary" onclick={quit} disabled={stopping}>{stopping ? "stopping…" : "Quit"}</button>
    <button onclick={keep} disabled={stopping}>Keep running</button>
  </div>
</section>

<style>
  .quit { padding: var(--s-3) var(--s-4); }
  h2 { font-size: var(--t-md); margin: 0 0 var(--s-2); }
  /* The sentence as the CLI prints it, line for line. */
  .says { white-space: pre-wrap; font: inherit; margin: 0 0 var(--s-3); max-width: 60ch; }
  .warn { color: var(--fail); }
  .dim { color: var(--dim); }
  .acts { display: flex; gap: var(--s-2); }
</style>
