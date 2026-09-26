<script lang="ts">
  // One change, as a document: what it is, where it stands, what you can do
  // to it, and six views over it. The state decides the toolbar: only actions
  // that can do something are offered. Every sentence is the host's.
  import { untrack } from "svelte";
  import { api, Refused } from "../../lib/api";
  import { resource, copyText, failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Commands from "../../lib/Commands.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import Qualifier from "../../lib/Qualifier.svelte";
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

  /// The change this document is about, as a value. Everything below keys on
  /// this, never on the prop: a prop is re-read whenever the shell's props
  /// object is, and the view, the result sentence and the refusal must
  /// survive every poll that did not change which change this is.
  const key = $derived(id);

  const read = resource<Detail>(() => (key ? `/api/changes/${encodeURIComponent(key)}` : null), {
    tell: () => `devplane change show ${key}`,
  });
  const d = $derived(read.data ?? planted);

  let said = $state("");
  /// Commands the host handed back to run by hand (an offer it may not push).
  let commands = $state<string[]>([]);
  const pending = writer();

  let view = $state("overview");
  type Row = { path: string; why: string; matched: string };
  /// The host's refusal to offer, naming every unseen row.
  let refusal = $state<{ says: string; rows: Row[]; seen_with: string } | null>(null);
  /// The row a refusal's action opens in the review.
  let reviewAt = $state("");

  // Another change: its own view, and none of the last one's sentences.
  let lastMoved: string | null = null;
  $effect.pre(() => {
    void key;
    untrack(() => {
      view = "overview";
      said = "";
      commands = [];
      refusal = null;
      reviewAt = "";
      lastMoved = null;
    });
  });

  // Read again whenever the board says this change moved — the state's
  // value, not the board object it came in.
  const moved = $derived(brief?.state ?? "");
  $effect(() => {
    const m = moved;
    untrack(() => {
      if (lastMoved !== null && m !== lastMoved) void read.reload();
      lastMoved = m;
    });
  });

  const verified = $derived(d?.state === "verified");
  /// Weakened rows no person has marked seen: while any remain, Review is the
  /// primary action and an offer is refused by the host.
  const unseen = $derived(d?.qualifier?.unseen ?? 0);
  function readRow(path: string) {
    reviewAt = path;
    view = "review";
  }
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
  /// What each verb came to, when the host says nothing of its own. Never a
  /// present tense for work that has finished by the time this is shown.
  const PAST: Record<Verb, string> = {
    verify: "the gates ran",
    finish: "finished — recorded as yours",
    offer: STATES.offered.word,
    resume: "picked back up",
    retry: "trying again",
    archive: "archived — the worktree is removed and the record kept",
  };
  /// What each verb is doing while it runs, beside its elapsed time.
  const DOING: Record<Verb, string> = {
    verify: "Running the gates",
    finish: "Finishing",
    offer: "Offering",
    resume: "Picking it back up",
    retry: "Trying again",
    archive: "Archiving",
  };
  type Answered = {
    offer?: string;
    push?: string;
    create?: string;
    pull_request?: { url: string };
    says?: string;
    passed?: boolean;
    summary?: string;
  } | null;
  /// The sentence a write came to, from what the host answered.
  function outcome(verb: Verb, r: Answered): string {
    if (verb === "offer" && r?.offer !== "opened") {
      return "Nothing was pushed — [github] pull_request is not set, so the pull request is yours to open. Run:";
    }
    if (verb === "verify" && typeof r?.passed === "boolean") {
      return `${r.passed ? "The gates passed" : "The gates failed"}${r.summary ? `: ${r.summary}` : "."}`;
    }
    return (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "");
  }
  async function act(verb: Verb) {
    const at = key;
    if (!at || pending.busy) return;
    await pending.run(verb, async () => {
      try {
        const r = await api<Answered>(ROUTE[verb](encodeURIComponent(at)), { method: "POST" });
        if (at !== key) return;
        said = outcome(verb, r);
        commands = verb === "offer" && r?.offer !== "opened" ? [r?.push, r?.create].filter((c): c is string => !!c) : [];
        refusal = null;
        await read.reload();
      } catch (e) {
        if (at !== key) return;
        commands = [];
        const body = e instanceof Refused ? (e.body as { refused?: string; says?: string; rows?: Row[]; seen_with?: string } | null) : null;
        if (body?.refused === "weakened_unseen") {
          said = "";
          refusal = { says: body.says ?? "", rows: body.rows ?? [], seen_with: body.seen_with ?? "" };
        } else {
          const f = failure(e, `devplane change show ${at}`);
          said = `That did not land: ${f.says}. \`${f.tell}\` tells more.`;
        }
      }
    });
  }
  async function openIn(place: "editor" | "terminal") {
    try {
      const r = await api<{ opened: boolean; path?: string; says?: string }>(
        `/api/changes/${encodeURIComponent(key)}/open?in=${place}`,
        { method: "POST" },
      );
      const where = r.path ?? d?.worktree ?? "";
      said = r.opened
        ? `Opened ${where || "the worktree"} in the ${place}.`
        : (r.says ?? (where ? `Open it yourself: ${where}` : `It was not opened in the ${place}.`));
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    }
  }
  async function copy(text: string, what: string) {
    said = (await copyText(text)) ? `Copied ${what}.` : `Nothing was copied — this page has no clipboard. ${what}: ${text}`;
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
{:else if read.failure && !d}
  <div class="failed"><Failed what="this change" failure={read.failure} /></div>
{:else if !d}
  <div class="loading"><div class="bar"></div><div class="bar short"></div></div>
{:else}
  <article class="doc">
    <header class="head">
      <div class="line1">
        <h1>{d.title || d.id}</h1>
        <Pill word={d.state} /><Qualifier q={d.qualifier} />
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
      <p class="standing {verified ? 'done' : ''}"><Inline text={d.standing_says} /></p>

      <div class="toolbar" role="toolbar" aria-label="what you can do to this change">
        <button class:primary={unseen > 0 || !verified} onclick={() => (view = "review")}><Icon name="split" size={14} /> Review{#if unseen > 0}<span class="count">{unseen} unseen</span>{/if}</button>
        {#if d.worktree}
          <button onclick={() => act("verify")} disabled={!!pending.busy}><Icon name="gate" size={14} /> Run gates</button>
        {/if}
        {#if verified}
          <button class:primary={unseen === 0} onclick={() => act("offer")} disabled={!!pending.busy}><Icon name="forge" size={14} /> Offer as pull request</button>
        {/if}
        {#if d.stopped}
          <button onclick={() => act("resume")} disabled={!!pending.busy}><Icon name="play" size={14} /> Pick it back up</button>
          {#if d.can_retry}<button onclick={() => act("retry")} disabled={!!pending.busy}><Icon name="refresh" size={14} /> Try again</button>{/if}
        {/if}
        {#if !d.completion && !d.archived_at && d.runs?.length}
          <button onclick={() => act("finish")} disabled={!!pending.busy}><Icon name="check" size={14} /> Finish</button>
        {/if}
        <span class="sep"></span>
        {#if d.worktree}
          <button class="quiet" onclick={() => openIn("editor")} title="open the worktree in your editor"><Icon name="external" size={14} /> Editor</button>
          <button class="quiet" onclick={() => openIn("terminal")} title="open a terminal in the worktree"><Icon name="terminal" size={14} /> Terminal</button>
        {/if}
        {#if !d.archived_at && (d.completion || d.stopped)}
          <button class="quiet" disabled={!!pending.busy} onclick={() => act("archive")} title="remove the worktree; the branch and the record are kept"><Icon name="folder" size={14} /> Archive</button>
        {/if}
      </div>
      <!-- Mounted always, so the first sentence is announced too. -->
      <p class="said" class:empty={!said && !pending.busy} role="status" aria-live="polite">
        {#if pending.busy}{DOING[pending.busy as Verb]} · {pending.elapsed} — nothing else can be started on this change until it is back.{:else}{said}{/if}
      </p>
      {#if commands.length && !pending.busy}<Commands {commands} />{/if}
      {#if read.phase === "stale" && read.failure}<Failed what="this change" failure={read.failure} at={read.at} stale />{/if}
      {#if refusal}
        <div class="refusal" role="alert">
          <p><b>Not offered</b> — {refusal.says}. Nothing was pushed.</p>
          <ul>
            {#each refusal.rows as row (row.path + row.matched)}
              <li>
                <code>{row.path}</code> <span><Inline text={row.why} /></span>
                <button onclick={() => readRow(row.path)}><Icon name="split" size={13} /> Read it</button>
              </li>
            {/each}
          </ul>
          <p class="how">Mark each row seen in the review, or from a terminal of your own: <code>{refusal.seen_with}</code></p>
        </div>
      {/if}
      <Stepper state={d.state} archived={!!d.archived_at} offered={!!d.pull_request} />
    </header>

    <Tabs {tabs} bind:active={view} label="views of this change" />

    <div class="view" role="tabpanel" aria-label={tabs.find((t) => t.id === view)?.label}>
      {#if view === "overview"}
        <Overview {d} go={(v) => (view = v)} />
      {:else if view === "tasks"}
        <Tasks {d} {lastRun} reload={read.reload} />
      {:else if view === "review"}
        <Pane id={key} focus={reviewAt} />
      {:else if view === "gates"}
        <Gates {d} id={key} />
      {:else if view === "ledger"}
        <Ledger id={key} />
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
  .count {
    margin-left: 0.3rem;
    font-size: var(--t-xs);
    font-weight: 600;
  }
  .refusal {
    font-size: var(--t-sm);
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--wait);
    background: var(--panel);
    display: grid;
    gap: var(--s-1);
  }
  .refusal p {
    margin: 0;
  }
  .refusal ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: grid;
    gap: var(--s-1);
  }
  .refusal li {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .refusal li span {
    color: var(--wait);
  }
  .refusal button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.6rem;
    padding: 0 0.5rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .refusal .how {
    color: var(--dim);
  }
  .refusal code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .said.empty {
    padding: 0;
    border: 0;
    background: none;
    margin-top: calc(-1 * var(--s-2));
  }
  .failed {
    padding: var(--s-5) var(--s-6);
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
