<script lang="ts">
  // Search every session's tool calls, questions and errors. It says that
  // prompts and replies are never recorded, so cannot be found.
  import { api } from "../../lib/api";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";

  type Hit = { run_id: string; at: string; text: string };
  let { q = "" }: { q?: string } = $props();

  let query = $state("");
  let hits = $state<Hit[]>([]);
  let ran = $state(false);
  let busy = $state(false);
  let said = $state("");
  let selected = $state<string | null>(null);

  $effect(() => {
    if (q && q !== query) {
      query = q;
      void run();
    }
  });

  async function run() {
    const want = query.trim();
    if (!want) return;
    busy = true;
    hits = [];
    try {
      const r = await api<{ hits?: Hit[] }>(`/api/search?q=${encodeURIComponent(want)}`);
      hits = r.hits ?? [];
      ran = true;
      said = "";
    } catch (e) {
      said = `The search did not run: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
    }
  }
  const columns: Column<Hit>[] = [
    { key: "at", label: "When", width: 150, sort: (h) => h.at, mono: true },
    { key: "run", label: "Session", width: 150, mono: true },
    { key: "text", label: "What matched" },
  ];
</script>

<div class="page">
  <h1 class="sr-only">Search every session</h1>
  <form class="bar" onsubmit={(e) => { e.preventDefault(); void run(); }}>
    <label class="field">
      <Icon name="search" size={15} />
      <input type="search" bind:value={query} aria-label="search every session" placeholder="A command, a question, an error" />
    </label>
    <button type="submit" disabled={busy}>{busy ? "Searching…" : "Search"}</button>
  </form>
  <p class="note">Tool calls, questions and errors from every session. Prompts and replies are never recorded, so they are never found.</p>
  {#if said}<p class="fail">{said}</p>{/if}

  {#if ran && hits.length === 0 && !busy}
    <Empty icon="search" title="Nothing matched" body={`Nothing any session recorded contains “${query.trim()}”.`} />
  {:else if hits.length > 0}
    <div class="frame">
      <Grid id="search" {columns} rows={hits} key={(h) => h.run_id + h.at + h.text} bind:selected label="matches">
        {#snippet cell(h, c)}
          {#if c.key === "at"}{new Date(h.at).toLocaleString()}
          {:else if c.key === "run"}<a href={`#why/${encodeURIComponent(h.run_id)}`}>{h.run_id.slice(0, 14)}</a>
          {:else}<span class="t">{h.text}</span>{/if}
        {/snippet}
      </Grid>
    </div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--s-4) var(--s-5);
    gap: var(--s-3);
    box-sizing: border-box;
  }
  .bar {
    display: flex;
    gap: var(--s-2);
    max-width: 48rem;
  }
  .field {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: 0 var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--faint);
  }
  .field:focus-within {
    border-color: var(--accent);
  }
  .field input {
    flex: 1;
    border: 0;
    box-shadow: none;
    background: none;
    color: var(--ink);
    font: inherit;
    padding: 0.45rem 0;
    outline: none;
  }
  .note {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .fail {
    margin: 0;
    color: var(--fail);
    font-size: var(--t-sm);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  a {
    color: var(--accent);
    text-decoration: none;
  }
  .t {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
