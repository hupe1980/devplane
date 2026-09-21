<script lang="ts">
  // Why a row is here: the decision log for one thing.
  //
  // **It reads data, and the page it replaces read markup.** The old why pane
  // fetched `/api/decisions/pane`, which the daemon rendered as HTML — the one
  // place a surface was handed a document rather than facts. Carrying that
  // forward would mean `{@html}`, and every value in a decision row came from
  // an agent, a command line or a repository.
  //
  // So it reads `/api/decisions` and renders it. The daemon still composes the
  // *sentence* — `reason` is its words, not this page's — which is the rule
  // that matters; what changes is that the markup is the surface's.
  import { api } from "../../lib/api";
  import type { Decision } from "../../wire/Decision";
  import { clip } from "../../lib/text";

  let { about = "", title = "" }: { about?: string; title?: string } = $props();

  let rows = $state<Decision[]>([]);
  let said = $state("");

  $effect(() => {
    if (!about) return;
    void (async () => {
      try {
        rows = await api<Decision[]>(`/api/decisions?about=${encodeURIComponent(about)}`);
        said = "";
      } catch (e) {
        said = `the decision log could not be read: ${e instanceof Error ? e.message : String(e)}`;
      }
    })();
  });
</script>

<section aria-labelledby="why-head">
  <h2 id="why-head">Why this is here</h2>
  {#if title}<p class="dim">{title}</p>{/if}
  <p class="said" role="status" aria-live="polite">{said}</p>

  {#if !about}
    <p class="empty">Open a row and this shows what was decided about it, and by whom.</p>
  {:else if rows.length === 0 && !said}
    <!-- **A result, not a blank.** Nothing decided is a fact about the row,
         and it is the reassuring one: it got here without anything being
         decided in this person's name. -->
    <p class="empty">
      Nothing was decided about this. It is here because of what it is, not because of anything
      Devplane did.
    </p>
  {:else}
    <ul role="list">
      {#each rows as d (d.id)}
        <li>
          <span class="at">{clip(d.at, 19)}</span>
          <!-- **The authority, which is what this log is for.** Five values,
               and an unrecognised one is printed as received rather than
               mapped to a guess. -->
          <span class="who">{d.authority}</span>
          <span class="did">{d.action}</span>
          <span class="outcome">{d.outcome}</span>
          {#if d.reason}
            <!-- The daemon's sentence. "allowed" is not an answer; "allowed by
                 `Bash(pnpm test *)`" is. -->
            <span class="reason">{d.reason}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .3rem; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; gap: .6rem; align-items: baseline; padding: .1rem 0; flex-wrap: wrap; }
  .at { color: var(--faint); }
  .who { width: 4rem; flex: none; color: var(--dim); }
  .outcome, .dim, .said, .empty { color: var(--dim); }
  .reason { flex-basis: 100%; color: var(--dim); padding-left: 1rem; }
  .empty { max-width: 60ch; }
</style>
