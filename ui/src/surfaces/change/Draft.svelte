<script lang="ts">
  // A drafted specification, as the change carries it: exactly what was sent,
  // where the methodology wrote, and the person's two decisions. Review comes
  // first — the folder is this change's diff — then Accept or Reject once the
  // agent's turn has ended. Every sentence is the host's.
  import { api } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Icon from "../../lib/ui/Icon.svelte";

  type DraftFacts = {
    methodology: "speckit" | "openspec";
    agent: string;
    sent: string;
    folder: string;
    outcome: "pending" | "produced" | "none_produced" | "accepted" | "rejected";
  };
  type Offer = { says: string; diff?: string; base?: string };

  let {
    d,
    review,
    reload,
  }: {
    d: { id: string; project_id?: string; draft?: DraftFacts | null; draft_says?: string[]; open_questions_offer?: Offer | null };
    review: () => void;
    reload: () => void;
  } = $props();

  const draft = $derived(d.draft ?? null);
  const NAME = { speckit: "Spec Kit", openspec: "OpenSpec" } as const;
  const decidable = $derived(draft?.outcome === "produced" || draft?.outcome === "none_produced");

  const pending = writer();
  let said = $state("");
  async function decide(verb: "accept" | "reject") {
    if (pending.busy) return;
    await pending.run(verb, async () => {
      try {
        const id = encodeURIComponent(d.id);
        const route = verb === "accept" ? `/api/changes/${id}/draft/accept` : `/api/changes/${id}/draft/reject`;
        const r = await api<{ says?: string }>(route, { method: "POST" });
        said = r?.says ?? (verb === "accept" ? "accepted" : "rejected");
        reload();
      } catch (e) {
        said = `That did not land: ${failure(e).says}.`;
      }
    });
  }
  async function addMarker(base: string) {
    if (pending.busy || !d.project_id) return;
    await pending.run("marker", async () => {
      try {
        const r = await api<{ says?: string }>(`/api/projects/${encodeURIComponent(d.project_id ?? "")}/config/open-questions`, {
          method: "POST",
          body: JSON.stringify({ accept: true, base }),
        });
        said = r?.says ?? "written";
        reload();
      } catch (e) {
        said = `That did not land: ${failure(e).says}.`;
      }
    });
  }
</script>

{#if draft}
  <section class="draft" aria-label="the drafted specification">
    <p class="head">
      <Icon name="spec" size={14} />
      <b>Drafted by {NAME[draft.methodology]}'s own command</b>
      <span class="dim">· {draft.agent} · {draft.folder}</span>
    </p>
    <pre class="sent" title="exactly what the agent was sent">{draft.sent}</pre>
    {#each d.draft_says ?? [] as s (s)}<p class="says">{s}</p>{/each}
    {#if decidable}
      <div class="acts">
        <button class="primary" onclick={review}>Review the draft</button>
        {#if draft.outcome === "produced"}<button disabled={!!pending.busy} onclick={() => decide("accept")}>Accept</button>{/if}
        <button disabled={!!pending.busy} onclick={() => decide("reject")}>Reject</button>
      </div>
    {/if}
    {#if d.open_questions_offer}
      <div class="offer">
        <p class="says">{d.open_questions_offer.says}</p>
        {#if d.open_questions_offer.diff && d.open_questions_offer.base}
          <pre class="diff">{d.open_questions_offer.diff}</pre>
          <button disabled={!!pending.busy} onclick={() => addMarker(d.open_questions_offer?.base ?? "")}>Add it to devplane.toml</button>
        {/if}
      </div>
    {/if}
    {#if said}<p class="says" role="status">{said}</p>{/if}
  </section>
{/if}

<style>
  .draft {
    display: grid;
    gap: var(--s-2);
    margin: var(--s-2) var(--s-4);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin: 0;
    flex-wrap: wrap;
  }
  .dim {
    color: var(--dim);
    font-size: var(--t-sm);
  }
  .sent,
  .diff {
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    font-family: var(--mono);
    font-size: var(--t-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .says {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink);
  }
  .acts {
    display: flex;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .offer {
    display: grid;
    gap: var(--s-2);
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
</style>
