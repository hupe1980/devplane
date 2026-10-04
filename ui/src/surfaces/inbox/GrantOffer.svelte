<script lang="ts">
  // The person's own grant, offered on a permission they have answered the
  // same way several times: the exact rule, the answers it comes from, what
  // the project declares it may write, and the lines it adds to their file.
  // Nothing is written until they confirm, and then only through the route
  // that takes the window's proof for this root and this rule.
  import { api, Refused } from "../../lib/api";

  export type GrantOffer = {
    rule: string;
    root: string;
    file: string;
    allowed: number;
    derived_from: Array<{ ask: string; at: string; from: string }>;
    writes: string[];
    writes_says: string;
    widens: boolean;
    diff: Array<{ op: string; text: string }>;
  };

  let {
    offer,
    no = null,
    lapsed = null,
    say,
  }: {
    offer: GrantOffer | null | undefined;
    /// Why no grant is offered, where that is worth saying.
    no?: string | null;
    /// A grant of theirs that would have answered this, and lapsed.
    lapsed?: string | null;
    say: (sentence: string) => void;
  } = $props();

  /// `shown` → `confirming` → `writing` → `written`; a refusal goes back.
  let step = $state<"shown" | "confirming" | "writing" | "written">("shown");

  const fromWhere = $derived.by(() => {
    if (!offer) return "";
    const window = offer.derived_from.filter((a) => a.from === "window").length;
    const notification = offer.derived_from.length - window;
    const parts = [];
    if (window) parts.push(`${window} from the window`);
    if (notification) parts.push(`${notification} from a notification`);
    return parts.join(", ");
  });

  async function write() {
    if (!offer) return;
    step = "writing";
    try {
      await api("/api/grants", {
        method: "POST",
        body: JSON.stringify({ root: offer.root, rule: offer.rule }),
      });
      step = "written";
      say(`added ${offer.rule} to ${offer.file} — it answers this from the next call`);
    } catch (e) {
      step = "shown";
      say(e instanceof Refused ? e.message : "the grant was not written");
    }
  }
</script>

{#if !offer}
  {#if no}<p class="elsewhere">{no}</p>{/if}
{:else}<section class="grant" aria-label="always allow this in this project">
  {#if step === "written"}
    <p class="dim">Written to {offer.file}. It answers this from the next call; answer this one above.</p>
  {:else}
    <p>
      You allowed <code>{offer.rule}</code> {offer.allowed} times ({fromWhere}).
      {#if step === "shown"}
        <button onclick={() => (step = "confirming")}>Always allow in this project…</button>
      {/if}
    </p>
    {#if step !== "shown"}
      <p class="dim">
        It may write: {offer.writes.join(", ")}. {offer.writes_says}.
        {#if offer.widens}Your agent's own settings would ask about this — the grant widens the agent.{/if}
      </p>
      <pre class="diff" aria-label="the lines added to {offer.file}">{#each offer.diff as l}<span
            class:add={l.op === "+"}>{l.op} {l.text}
</span>{/each}</pre>
      <p class="confirm">
        <button class="primary" disabled={step === "writing"} onclick={write}
          >Write to {offer.file}</button
        >
        <button disabled={step === "writing"} onclick={() => (step = "shown")}>Cancel</button>
      </p>
    {/if}
  {/if}
</section>{/if}
{#if lapsed}<p class="elsewhere">{lapsed}</p>{/if}

<style>
  .grant {
    display: grid;
    gap: var(--s-2);
    font-size: var(--t-sm);
    padding: var(--s-3);
    border: 1px dashed var(--line);
    border-radius: var(--radius-lg);
  }
  .grant p {
    margin: 0;
  }
  code,
  .diff {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .diff {
    margin: 0;
    padding: var(--s-2);
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .add {
    color: var(--add);
    background: var(--add-bg);
  }
  .confirm {
    display: flex;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .dim,
  .elsewhere {
    color: var(--dim);
  }
  .elsewhere {
    margin: 0;
    font-size: var(--t-sm);
  }
</style>
