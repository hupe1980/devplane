<script lang="ts">
  // One unresolved line of a change's specification, answered from the inbox.
  // A bracketed marker is replaced by the person's words, exactly: the edit is
  // shown first and written only when they confirm. An unbracketed one is not
  // Devplane's to edit, so the answer goes to the change's agent as the
  // person's own message, and its edit comes back as a diff.
  import { api } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";

  type MarkerAt = { path: string; line: number; text: string; bracketed: boolean };

  let {
    item,
    say,
  }: {
    item: { change_id?: string | null; run_id?: string | null; actions?: string[]; marker?: MarkerAt | null };
    say: (sentence: string) => void;
  } = $props();

  const m = $derived(item.marker ?? null);
  const has = (a: string) => (item.actions ?? []).includes(a);
  let words = $state("");
  /// The edit the host would write, shown before it is.
  let diff = $state("");
  let busy = $state(false);

  async function clarify(confirm: boolean) {
    if (!m || !item.change_id || busy) return;
    if (!words.trim()) return say("type your answer first — an empty answer is not one");
    busy = true;
    try {
      const r = await api<{ diff?: string; says?: string }>(`/api/changes/${encodeURIComponent(item.change_id)}/clarify`, {
        method: "POST",
        body: JSON.stringify({ path: m.path, line: m.line, text: m.text, answer: words, confirm }),
      });
      diff = confirm ? "" : (r.diff ?? "");
      if (confirm) words = "";
      say(r.says ?? (confirm ? "written" : "this is the whole edit"));
    } catch (e) {
      say(`that did not land: ${failure(e).says}`);
    } finally {
      busy = false;
    }
  }
  async function tell() {
    if (!item.run_id || busy) return;
    if (!words.trim()) return say("type your answer first — an empty answer is not one");
    busy = true;
    try {
      const r = await api<{ says?: string }>(`/api/runs/${encodeURIComponent(item.run_id)}/prompt`, {
        method: "POST",
        body: JSON.stringify({ text: words }),
      });
      words = "";
      say(`sent to the agent as your message${r.says ? ` — ${r.says}` : ""}; its edit comes back as a diff`);
    } catch (e) {
      say(`that did not land: ${failure(e).says}`);
    } finally {
      busy = false;
    }
  }
</script>

{#if m}
  <div class="marker">
    <code class="at">{m.path}:{m.line}</code>
    {#if has("answer") || has("tell_agent")}
      <div class="reply">
        <input
          type="text"
          bind:value={words}
          placeholder={has("answer") ? "your answer replaces the bracketed marker" : "your answer, for the agent"}
          aria-label="your answer to {m.path} line {m.line}"
          data-action={has("answer") ? "answer" : "tell_agent"}
        />
        {#if has("answer")}
          <button disabled={busy} onclick={() => clarify(false)}>show the edit</button>
        {:else}
          <button disabled={busy} onclick={tell}>send to the agent</button>
        {/if}
      </div>
    {/if}
    {#if diff}
      <pre class="diff">{diff}</pre>
      <div class="reply">
        <button class="primary" disabled={busy} onclick={() => clarify(true)}>write it</button>
        <button disabled={busy} onclick={() => (diff = "")}>not now</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .marker {
    display: grid;
    gap: var(--s-2);
  }
  .at {
    font-family: var(--mono);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .reply {
    display: flex;
    gap: var(--s-2);
    align-items: center;
    flex-wrap: wrap;
  }
  .reply input {
    flex: 1;
    min-width: 12rem;
    font: inherit;
    font-size: var(--t-sm);
  }
  .diff {
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    font-family: var(--mono);
    font-size: var(--t-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
</style>
