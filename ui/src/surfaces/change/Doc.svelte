<script lang="ts">
  // One change, as a document: what it is, where it stands, what you can do
  // to it, and six views over it. The state decides the toolbar: only actions
  // that can do something are offered. Every sentence is the host's.
  import { api } from "../../lib/api";
  import { onAction } from "../../lib/keys";
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import Stepper from "./Stepper.svelte";
  import { STATES } from "../../lib/State.svelte";
  import Overview from "./Overview.svelte";
  import Gates from "./Gates.svelte";
  import Ledger from "./Ledger.svelte";
  import Agent from "./Agent.svelte";
  import Tasks from "./Tasks.svelte";
  import Pane from "../review/Pane.svelte";
  import type { Detail } from "./types";

  let {
    id = "",
    brief = null,
    loaded = false,
    detail: planted = null,
  }: {
    id?: string;
    brief?: { state: string; updated?: string } | null;
    loaded?: boolean;
    detail?: Detail | null;
  } = $props();

  let fetched = $state<Detail | null>(null);
  let error = $state("");
  let said = $state("");
  let busy = $state("");
  const d = $derived(fetched ?? planted);

  let view = $state("overview");
  $effect(() => {
    void id;
    view = "overview";
  });

  // Read on open, and again whenever the board says this change moved.
  const moved = $derived(brief?.state ?? "");
  $effect(() => {
    const want = id;
    void moved;
    if (!want) return;
    let live = true;
    api<Detail>(`/api/changes/${encodeURIComponent(want)}`)
      .then((r) => {
        if (live) {
          fetched = r;
          error = "";
        }
      })
      .catch((e) => {
        if (live) error = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });
  $effect(() => {
    void id;
    fetched = null;
    said = "";
  });

  const verified = $derived(d?.state === "verified");
  const lastRun = $derived(d?.runs?.[d.runs.length - 1] ?? "");
  const project = $derived((d?.project_id ?? "").split("/").filter(Boolean).pop() ?? "");

  type Verb = "verify" | "finish" | "offer" | "resume" | "retry" | "archive";
  /// Each verb's route, spelled whole so a guard can hold every one to a
  /// route the host serves.
  const ROUTE: Record<Verb, (id: string) => string> = {
    verify: (id) => `/api/changes/${id}/verify`,
    finish: (id) => `/api/changes/${id}/finish`,
    offer: (id) => `/api/changes/${id}/offer`,
    resume: (id) => `/api/changes/${id}/resume`,
    retry: (id) => `/api/changes/${id}/retry`,
    archive: (id) => `/api/changes/${id}/archive`,
  };
  const PAST: Record<Verb, string> = {
    verify: "the gates are running",
    finish: "finished — recorded as yours",
    offer: STATES.offered.word,
    resume: "picked back up",
    retry: "trying again",
    archive: "archived — the worktree is removed and the record kept",
  };
  async function act(verb: Verb) {
    if (!id || busy) return;
    busy = verb;
    try {
      const r = await api<{ offer?: string; push?: string; create?: string; pull_request?: { url: string }; says?: string }>(
        ROUTE[verb](encodeURIComponent(id)),
        { method: "POST" },
      );
      said =
        verb === "offer" && r?.offer !== "opened"
          ? `Nothing was pushed — [github] pull_request is not set. Run: ${r?.push ?? ""} && ${r?.create ?? ""}`
          : (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "");
      fetched = await api<Detail>(`/api/changes/${encodeURIComponent(id)}`);
    } catch (e) {
      said = `That did not land: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = "";
    }
  }
  async function openIn(place: "editor" | "terminal") {
    try {
      const r = await api<{ opened: boolean; path: string; says?: string }>(
        `/api/changes/${encodeURIComponent(id)}/open?in=${place}`,
        { method: "POST" },
      );
      said = r.opened ? `Opened ${r.path} in the ${place}.` : (r.says ?? `Open it yourself: ${r.path}`);
    } catch (e) {
      said = `That did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard?.writeText(text);
      said = `Copied ${what}.`;
    } catch {
      said = `No clipboard here: ${text}`;
    }
  }

  const tabs = $derived([
    { id: "overview", label: "Overview", icon: "eye" },
    { id: "tasks", label: "Tasks", icon: "spec", count: d?.counts?.tasks ?? null },
    { id: "review", label: "Review", icon: "split", count: d?.review_files ?? null },
    { id: "gates", label: "Gates", icon: "gate", count: d?.gates?.length ?? null, tone: d?.gate ? (verified ? "done" : d.gate.passed ? undefined : "fail") : undefined },
    { id: "ledger", label: "Ledger", icon: "ledger" },
    { id: "agent", label: "Agent", icon: "agent", count: d?.runs?.length ?? null },
  ]);

  $effect(() =>
    onAction("review", () => {
      view = "review";
      return true;
    }),
  );
</script>

{#if !id}
  <Empty icon="change" title={loaded ? "Pick a change" : "Reading the changes…"} body="Choose one in the list, or start a new one — an isolated worktree, an agent in it, and the project's own gates." />
{:else if error && !d}
  <Empty icon="alert" title="This change could not be read" body={error} />
{:else if !d}
  <div class="loading"><div class="bar"></div><div class="bar short"></div></div>
{:else}
  <article class="doc">
    <header class="head">
      <div class="line1">
        <h1>{d.title || d.id}</h1>
        <Pill word={d.state} />
        {#if d.waiting_says}<Pill word={d.waiting_says} as="wait" />{/if}
      </div>
      <div class="chips">
        <span class="chip"><Icon name="folder" size={12} /> {project}</span>
        {#if d.branch}
          <button class="chip mono" title="copy the branch name" onclick={() => copy(d!.branch!, "the branch name")}><Icon name="change" size={12} /> {d.branch}</button>
        {/if}
        {#if d.spec}<span class="chip mono"><Icon name="spec" size={12} /> {d.spec}</span>{/if}
        {#if d.in_place}<span class="chip warn"><Icon name="alert" size={12} /> in place — no parallel safety</span>{/if}
        {#if d.shape_says}<span class="chip">{d.shape_says}</span>{/if}
      </div>
      <p class="standing {verified ? 'done' : ''}">{d.standing_says}</p>

      <div class="toolbar" role="toolbar" aria-label="what you can do to this change">
        <button onclick={() => (view = "review")}><Icon name="split" size={14} /> Review</button>
        {#if d.worktree}
          <button onclick={() => act("verify")} disabled={!!busy}><Icon name="gate" size={14} /> {busy === "verify" ? "Running…" : "Run gates"}</button>
        {/if}
        {#if verified}
          <button class="primary" onclick={() => act("offer")} disabled={!!busy}><Icon name="forge" size={14} /> Offer as pull request</button>
        {/if}
        {#if d.stopped}
          <button onclick={() => act("resume")}><Icon name="play" size={14} /> Pick it back up</button>
          {#if d.can_retry}<button onclick={() => act("retry")}><Icon name="refresh" size={14} /> Try again</button>{/if}
        {/if}
        {#if !d.completion && !d.archived_at && d.runs?.length}
          <button onclick={() => act("finish")} disabled={!!busy}><Icon name="check" size={14} /> Finish</button>
        {/if}
        <span class="sep"></span>
        {#if d.worktree}
          <button class="quiet" onclick={() => openIn("editor")} title="open the worktree in your editor"><Icon name="external" size={14} /> Editor</button>
          <button class="quiet" onclick={() => openIn("terminal")} title="open a terminal in the worktree"><Icon name="terminal" size={14} /> Terminal</button>
        {/if}
        {#if !d.archived_at && (d.completion || d.stopped)}
          <button class="quiet" onclick={() => act("archive")} title="remove the worktree; the branch and the record are kept"><Icon name="folder" size={14} /> Archive</button>
        {/if}
      </div>
      {#if said}<p class="said" role="status" aria-live="polite">{said}</p>{/if}
      <Stepper state={d.state} archived={!!d.archived_at} offered={!!d.pull_request} />
    </header>

    <Tabs {tabs} bind:active={view} label="views of this change" />

    <div class="view" role="tabpanel" aria-label={tabs.find((t) => t.id === view)?.label}>
      {#if view === "overview"}
        <Overview {d} go={(v) => (view = v)} />
      {:else if view === "tasks"}
        <Tasks {d} {lastRun} />
      {:else if view === "review"}
        <Pane {id} />
      {:else if view === "gates"}
        <Gates {d} {id} />
      {:else if view === "ledger"}
        <Ledger {id} />
      {:else if view === "agent"}
        <Agent {d} run={lastRun} />
      {/if}
    </div>
  </article>
{/if}

<style>
  .doc {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .head {
    padding: var(--s-5) var(--s-6) var(--s-3);
    display: grid;
    gap: var(--s-2);
    background: var(--bg);
  }
  .line1 {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 1.35rem;
    font-weight: 650;
    letter-spacing: -0.015em;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.4rem;
    padding: 0 0.5rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    white-space: nowrap;
  }
  button.chip {
    cursor: pointer;
  }
  button.chip:hover {
    color: var(--ink);
    border-color: var(--edge);
  }
  .chip.mono {
    font-family: var(--mono);
  }
  .chip.warn {
    color: var(--wait);
  }
  .standing {
    margin: 0;
    color: var(--dim);
    font-size: var(--t-sm);
    max-width: 90ch;
  }
  .standing.done {
    color: var(--done);
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
    align-items: center;
    margin-top: var(--s-1);
  }
  .toolbar button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.9rem;
    padding: 0 0.7rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
  }
  .toolbar button:hover:not(:disabled) {
    border-color: var(--edge);
    background: var(--raise);
  }
  .toolbar button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .toolbar .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .toolbar .quiet {
    background: none;
    border-color: transparent;
    color: var(--dim);
  }
  .sep {
    flex: 1;
  }
  .said {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--dim);
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--accent);
    background: var(--panel);
  }
  .view {
    flex: 1;
    padding: var(--s-4) var(--s-6) var(--s-6);
    min-width: 0;
  }
  .loading {
    padding: var(--s-6);
    display: grid;
    gap: var(--s-3);
  }
  .bar {
    height: 1.4rem;
    width: 50%;
    border-radius: var(--radius);
    background: var(--raise);
  }
  .bar.short {
    width: 30%;
  }
</style>
