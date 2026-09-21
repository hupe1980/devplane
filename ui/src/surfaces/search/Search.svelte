<script lang="ts">
  // Search tool calls, questions and errors across every session.
  //
  // **Prompt and response text is deliberately absent.** Devplane never turns
  // on the telemetry flags that would carry it, so this searches what the
  // machine actually recorded — commands, questions, summaries — and says so,
  // rather than returning nothing for a phrase somebody remembers typing.
  import { api } from "../../lib/api";
  import { clip } from "../../lib/text";

  type Hit = { run_id: string; at: string; text: string };

  let query = $state("");
  let hits = $state<Hit[]>([]);
  let ran = $state(false);
  let said = $state("");

  async function run() {
    const q = query.trim();
    if (!q) return;
    try {
      const r = await api<{ hits?: Hit[] }>(`/api/search?q=${encodeURIComponent(q)}`);
      hits = r.hits ?? [];
      ran = true;
      said = "";
    } catch (e) {
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
</script>

<section aria-labelledby="search-head">
  <h2 id="search-head">Search</h2>

  <form onsubmit={(e) => { e.preventDefault(); void run(); }}>
    <input type="search" bind:value={query} aria-label="search" placeholder="a command, a question, an error" />
    <button type="submit">search</button>
  </form>

  <p class="said" role="status" aria-live="polite">{said}</p>

  {#if ran && hits.length === 0}
    <!-- A result, not a blank — and it says what is *not* searched, because a
         person looking for a prompt they typed would otherwise conclude the
         search is broken. -->
    <p class="empty">
      Nothing matched <b>{clip(query, 40)}</b>. Prompts and replies are not searched: Devplane
      never records them.
    </p>
  {:else if hits.length > 0}
    <ul role="list">
      {#each hits as h (h.run_id + h.at)}
        <li><code>{clip(h.run_id, 8)}</code> <span>{clip(h.text, 120)}</span></li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .3rem; }
  form { display: flex; gap: .3rem; }
  input { flex: 1; max-width: 30rem; font: inherit; padding: .25rem .35rem;
          background: var(--panel); color: var(--ink); border: 1px solid var(--line); }
  ul { list-style: none; margin: .4rem 0 0; padding: 0; }
  li { display: flex; gap: .6rem; padding: .1rem 0; }
  .said, .empty { color: var(--dim); }
  .empty { max-width: 60ch; }
</style>
