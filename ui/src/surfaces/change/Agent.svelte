<script lang="ts">
  // The agent working on this change: what it said, wrote and is running now,
  // beside a composer to steer it without stopping it. Stop is two presses: the first
  // shows the host's sentence about what survives, the second stops.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import type { Tone } from "../../lib/ui/Pill.svelte";
  import type { Detail } from "./types";

  /// The run as `/api/runs/{id}` sends it — the host's own record, which has
  /// no generated wire type. `state` is a word (`working`) or, for a wait, an
  /// object naming what it waits on (`{ "waiting": "question" }`,
  /// `{ "waiting": { "other": "sandbox request" } }`).
  type Run = {
    id: string;
    agent?: string;
    state?: unknown;
    model?: string | null;
    wrote?: string[];
    recent_tools?: Array<{ tool: string; input?: { command?: string; file_path?: string } | null; ok?: boolean | null }>;
  };

  /// A run's state as a word a person reads, and its colour family. A wait
  /// names what it is waiting on and whether that is you.
  function runState(s: unknown): { word: string; tone?: Tone } | null {
    if (typeof s === "string") return { word: s.replace(/_/g, " ") };
    if (s && typeof s === "object" && "waiting" in s) {
      const w = (s as { waiting: unknown }).waiting;
      if (w === "permission") return { word: "waiting on you — permission", tone: "wait" };
      if (w === "question") return { word: "waiting on you — question", tone: "wait" };
      if (w === "idle") return { word: "idle — waiting for the next turn", tone: "none" };
      if (w === "job") return { word: "waiting on a command it started", tone: "work" };
      if (w && typeof w === "object" && "other" in w) return { word: `waiting on you — ${String((w as { other: unknown }).other)}`, tone: "wait" };
      return { word: "waiting on you", tone: "wait" };
    }
    return s == null ? null : { word: "in a state this page does not know", tone: "none" };
  }
  type Message = { id: string; role: string; text: string; at?: string };

  let { d, run }: { d: Detail; run: string } = $props();

  /// How many turns one read asks for; more exist when a read comes back full.
  const LIMIT = 200;
  /// How often a watched run is read again: there is no stream route, so the
  /// conversation is polled while this view is open (and the page visible).
  const EVERY_MS = 2_000;

  const key = $derived(run);
  const runRead = resource<Run>(() => (key ? `/api/runs/${encodeURIComponent(key)}` : null), {
    every: EVERY_MS,
    tell: () => `devplane show ${key}`,
  });
  const said_ = resource<Message[]>(() => (key ? `/api/runs/${encodeURIComponent(key)}/messages?limit=${LIMIT}` : null), {
    every: EVERY_MS,
    tell: () => `devplane show ${key}`,
  });
  const detail = $derived(runRead.data);
  const messages = $derived(Array.isArray(said_.data) ? said_.data : []);
  let said = $state("");
  let draft = $state("");
  let sending = $state(false);
  let survives = $state("");
  $effect(() => {
    void key;
    survives = "";
    said = "";
  });

  const shownState = $derived(runState(detail?.state));
  /// Tool calls still in flight. The host keeps what a call was asked to do
  /// only until it finishes, so finished calls are not listed at all.
  const running = $derived(
    (detail?.recent_tools ?? [])
      .filter((t) => t.ok == null)
      .map((t) => ({ tool: t.tool, what: t.input?.command ?? t.input?.file_path ?? "" })),
  );

  async function send() {
    const text = draft.trim();
    if (!text || !run || sending) return;
    sending = true;
    try {
      const r = await api<{ says?: string }>(`/api/runs/${encodeURIComponent(run)}/prompt`, {
        method: "POST",
        body: JSON.stringify({ text }),
      });
      draft = "";
      said = r?.says ?? "Sent.";
      void said_.reload();
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    } finally {
      sending = false;
    }
  }
  async function askStop() {
    try {
      const r = await api<{ says?: string }>(`/api/runs/${encodeURIComponent(run)}/stop`);
      survives = r?.says ?? "Stopping ends this run.";
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    }
  }
  async function stop() {
    try {
      await api(`/api/runs/${encodeURIComponent(run)}/stop`, { method: "POST" });
      survives = "";
      said = "Stopped.";
      void runRead.reload();
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    }
  }
  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void send();
    }
  }
</script>

{#if !run}
  <p class="quiet">No agent has run on this change yet.</p>
{:else}
  <div class="agent">
    <section class="convo">
      <header class="bar">
        <Icon name="agent" size={15} />
        <b>{detail?.agent ?? "agent"}</b>
        {#if detail?.model}<span class="quiet">{detail.model}</span>{/if}
        {#if shownState}<Pill word={shownState.word} as={shownState.tone} />{/if}
        <code class="quiet">{run.slice(0, 18)}</code>
        {#if d.runs && d.runs.length > 1}<span class="quiet">· latest of {d.runs.length} runs</span>{/if}
      </header>
      {#if said_.failure}<div class="fail"><Failed what="the conversation" failure={said_.failure} at={said_.at} stale={said_.data !== null} /></div>{/if}
      {#if runRead.failure}<div class="fail"><Failed what="the run" failure={runRead.failure} at={runRead.at} stale={runRead.data !== null} /></div>{/if}
      <ol class="turns">
        {#if said_.phase === "loading"}
          <li class="quiet">reading…</li>
        {:else if said_.data !== null && messages.length === 0}
          <li class="quiet">Nothing has been said yet.</li>
        {:else if messages.length >= LIMIT}
          <li class="quiet">Only {LIMIT} turns are shown here; <code>devplane show {run}</code> has every one.</li>
        {/if}
        {#each messages as m (m.id)}
          <li class="turn {m.role}">
            <span class="who">{m.role === "user" ? "you" : m.role}</span>
            <div class="text">{m.text}</div>
          </li>
        {/each}
      </ol>
      <form
        class="composer"
        onsubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        <textarea bind:value={draft} rows="3" placeholder="Another turn for the agent — it is queued if one is under way" aria-label="another turn for the agent" onkeydown={keydown}></textarea>
        <div class="send">
          <span class="quiet">⌘↵ to send</span>
          {#if survives}
            <span class="warn">{survives}</span>
            <button type="button" class="danger" onclick={stop}>Stop the run</button>
            <button type="button" onclick={() => (survives = "")}>Keep it running</button>
          {:else}
            <button type="button" class="ghost" onclick={askStop}><Icon name="stop" size={12} /> Stop…</button>
          {/if}
          <button type="submit" class="primary" disabled={!draft.trim() || sending}><Icon name="play" size={12} /> Send</button>
        </div>
      </form>
      {#if said}<p class="said" role="status">{said}</p>{/if}
    </section>

    <aside class="record">
      <h2>Files written <span>{detail ? (detail.wrote?.length ?? 0) : ""}</span></h2>
      <ul>
        {#each detail?.wrote ?? [] as f (f)}<li><Icon name="file" size={12} /><code>{f}</code></li>{:else}<li class="quiet">{detail ? "none yet" : runRead.failure ? "not read — see above" : "reading…"}</li>{/each}
      </ul>
      <h2>Running now <span>{detail ? running.length : ""}</span></h2>
      <ul>
        {#each running as c, i (i)}
          <li><Icon name="terminal" size={12} /><span>{c.tool}</span>{#if c.what}<code>{c.what}</code>{/if}</li>
        {:else}<li class="quiet">{detail ? "no tool call in flight" : runRead.failure ? "not read — see above" : "reading…"}</li>{/each}
      </ul>
    </aside>
  </div>
{/if}

<style>
  .agent {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 22rem);
    gap: var(--s-4);
    align-items: start;
  }
  .convo {
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    display: flex;
    flex-direction: column;
    min-height: 24rem;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
    border-bottom: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  .fail {
    padding: 0 var(--s-3);
  }
  .turns {
    list-style: none;
    margin: 0;
    padding: var(--s-3);
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    flex: 1;
    overflow: auto;
    max-height: 32rem;
  }
  .turn {
    display: grid;
    gap: 0.2rem;
    max-width: 88%;
  }
  .turn.user {
    align-self: flex-end;
  }
  .who {
    font-size: 0.6875rem;
    color: var(--faint);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .turn.user .who {
    text-align: end;
  }
  .text {
    padding: var(--s-2) var(--s-3);
    border-radius: var(--radius-lg);
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: var(--t-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .turn.user .text {
    background: var(--select);
  }
  .composer {
    border-top: 1px solid var(--line);
    padding: var(--s-2) var(--s-3) var(--s-3);
    display: grid;
    gap: var(--s-2);
  }
  textarea {
    width: 100%;
    resize: vertical;
    min-height: 4rem;
    background: var(--bg);
    color: var(--ink);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: var(--s-2);
    font: inherit;
    font-size: var(--t-sm);
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .send {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .send .quiet {
    margin-right: auto;
  }
  .send button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: var(--t-sm);
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .ghost {
    background: none;
    border-color: transparent;
    color: var(--dim);
  }
  .danger {
    color: var(--fail);
    border-color: var(--fail);
  }
  .warn {
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .said {
    margin: 0 var(--s-3) var(--s-3);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .record {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  h2 {
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
    margin: var(--s-2) 0 0;
  }
  h2 span {
    font-weight: 400;
  }
  .record ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.25rem;
  }
  .record li {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--t-xs);
    color: var(--dim);
    min-width: 0;
  }
  code {
    font-family: var(--mono);
    font-size: 0.6875rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  @container (max-width: 70rem) {
    .agent {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
