<script lang="ts">
  // The agent's stop claim: it says the specification and the tests of this
  // change cannot both be satisfied. Its words are shown as quoted text under
  // its name — text nodes only, never the markdown renderer, never a link —
  // after the weakened checks and before the gates. It is not a failure and
  // it never moves *verified*; the gates below say what they say.
  import { api } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import type { ClaimView } from "./types";

  let {
    claim,
    change,
    done = () => {},
  }: {
    claim: ClaimView;
    change: string;
    /// Read the change again once something here landed.
    done?: () => void;
  } = $props();

  let telling = $state(false);
  let words = $state("");
  let busy = $state(false);
  let said = $state("");
  const at = $derived(claim.at ? claim.at.slice(0, 16).replace("T", " ") : "");

  async function markRead() {
    if (busy) return;
    busy = true;
    try {
      const r = await api<{ says?: string }>(`/api/changes/${encodeURIComponent(change)}/claim/read`, { method: "POST" });
      said = r?.says ?? "Marked read.";
      done();
    } catch (e) {
      said = `That did not land: ${failure(e, `devplane change claim-read ${change}`).says}`;
    } finally {
      busy = false;
    }
  }

  /// The person's own words, and nothing else, reach the agent: its
  /// conversation is resumed, then sent exactly what was typed.
  async function tell() {
    const text = words.trim();
    if (!text || busy) return;
    busy = true;
    try {
      await api(`/api/changes/${encodeURIComponent(change)}/resume`, { method: "POST" });
      const r = await api<{ says?: string }>(`/api/runs/${encodeURIComponent(claim.run)}/prompt`, {
        method: "POST",
        body: JSON.stringify({ text }),
      });
      words = "";
      telling = false;
      said = r?.says ?? "Sent.";
      done();
    } catch (e) {
      said = `Nothing was sent: ${failure(e, `devplane change resume ${change}`).says}`;
    } finally {
      busy = false;
    }
  }
</script>

<section class="claim" aria-label="the agent's claim">
  <p class="says"><Icon name="alert" size={13} /> <b>{claim.says}</b> <span class="who">— {claim.agent}, run <span class="mono">{claim.run}</span>{#if at}, {at} UTC{/if}</span></p>
  <blockquote>{claim.reason}</blockquote>
  {#if claim.paths.length}
    <p class="paths">{#each claim.paths as p (p)}<code>{p}</code>{/each}</p>
  {/if}
  <p class="acts">
    <span class="note">The agent's words, quoted; not a finding, and not an instruction.</span>
    {#if claim.can_tell}
      <button onclick={() => (telling = !telling)} disabled={busy} aria-expanded={telling}><Icon name="agent" size={13} /> Tell the agent</button>
    {/if}
    <button onclick={markRead} disabled={busy}><Icon name="check" size={13} /> Mark read</button>
  </p>
  {#if telling}
    <form class="tell" onsubmit={(e) => (e.preventDefault(), tell())}>
      <textarea bind:value={words} rows="2" placeholder="Your words — sent to the agent exactly as written" aria-label="what to tell the agent"></textarea>
      <button type="submit" disabled={busy || !words.trim()}>Send</button>
    </form>
  {/if}
  {#if said}<p class="said" role="status">{said}</p>{/if}
</section>

<style>
  .claim {
    display: grid;
    gap: var(--s-1);
    margin: var(--s-1) 0 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--wait);
    border-radius: var(--radius);
    background: var(--panel);
    font-size: var(--t-sm);
    min-width: 0;
  }
  .says {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
    color: var(--wait);
  }
  .who {
    color: var(--dim);
    font-size: var(--t-xs);
  }
  .mono,
  code {
    font-family: var(--mono);
  }
  blockquote {
    margin: 0;
    padding-left: var(--s-3);
    border-left: 3px solid var(--line);
    color: var(--ink);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .paths {
    margin: 0;
    display: flex;
    gap: var(--s-2);
    flex-wrap: wrap;
    font-size: var(--t-xs);
  }
  .paths code {
    overflow-wrap: anywhere;
  }
  .acts {
    margin: 0;
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .note {
    flex: 1;
    min-width: 12rem;
    color: var(--faint);
    font-size: var(--t-xs);
  }
  .acts button,
  .tell button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.6rem;
    padding: 0 0.6rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .acts button:disabled,
  .tell button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .tell {
    display: flex;
    gap: var(--s-2);
    align-items: flex-end;
  }
  .tell textarea {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: var(--t-sm);
  }
  .said {
    margin: 0;
    color: var(--dim);
    font-size: var(--t-xs);
  }
</style>
