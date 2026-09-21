<script lang="ts">
  // What is configured, and why there is nothing to edit.
  //
  // **A person who came looking for a settings form deserves an answer rather
  // than a missing button.** Every value here is a file: Devplane reads them
  // and never writes them, because the rules are committed and reviewed like
  // code — and an agent on this machine runs as the same user, so a route that
  // edited them would be reachable by the party they exist to bound.
  let { where = "", rows = [] }: { where?: string; rows?: Array<{ k: string; v: string }> } =
    $props();
</script>

<section aria-labelledby="setup-head">
  <h2 id="setup-head">What is configured</h2>
  {#if where}<p class="dim">{where}</p>{/if}

  {#if rows.length === 0}
    <p class="empty">
      Nothing is configured for this project yet. <code>devplane check</code> reads its
      <code>devplane.toml</code> and says what it will do.
    </p>
  {:else}
    <dl>
      {#each rows as r (r.k)}
        <dt>{r.k}</dt>
        <dd>{r.v}</dd>
      {/each}
    </dl>
  {/if}

  <p class="foot">
    Every value here is a file. Devplane reads them and never writes them: the rules are
    committed and reviewed like code, and an agent on this machine runs as you.
  </p>
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .3rem; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: .15rem .8rem; margin: .3rem 0; }
  dt { color: var(--dim); font-size: .82rem; }
  dd { margin: 0; }
  .dim, .empty, .foot { color: var(--dim); }
  .foot { font-size: .82rem; margin-top: .6rem; max-width: 70ch; }
  .empty { max-width: 60ch; }
</style>
