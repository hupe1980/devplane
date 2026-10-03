<script lang="ts">
  import { declaresNoCheck } from "../../lib/NoCheck.svelte";
  // One change, as a document: what it is, where it stands, what you can do
  // to it, and six views over it. The state decides the toolbar: only actions
  // that can do something are offered. Every sentence is the host's.
  //
  // Evidence first: while a weakened check is unread the header leads with
  // it, *verified* follows as a plain word, and the green sentence about the
  // check is not shown — it would be the first reassurance on the page. The
  // header is one line (`Head`); the view tabs stick under it. The address
  // names the view and, in the review, the hunk under the cursor.
  import { untrack } from "svelte";
  import { api, Refused } from "../../lib/api";
  import { resource, copyText, failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Commands from "../../lib/Commands.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import { weakLeads } from "../../lib/ChangeState.svelte";
  import { onAction, setMode } from "../../lib/keys";
  import { offerCommands } from "../../lib/offered.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import Head from "./Head.svelte";
  import { STATES } from "../../lib/State.svelte";
  import Overview from "./Overview.svelte";
  import Gates from "./Gates.svelte";
  import Ledger from "./Ledger.svelte";
  import Agent from "./Agent.svelte";
  import Tasks from "./Tasks.svelte";
  import Pane from "./review/Pane.svelte";
  import Draft from "./Draft.svelte";
  import Refusal, { type OfferRefusal, type Row } from "./Refusal.svelte";
  import { unread, unreadSays } from "./review/unread.svelte";
  import type { ReviewBody } from "./review/types";
  import type { Detail } from "./types";

  let {
    id = "",
    brief = null,
    loaded = false,
    detail: planted = null,
    review: plantedReview = null,
    /// The view the address names: `#change/<id>/<view>`, and in the review
    /// `#change/<id>/review/<path>@<digest>`.
    view: asked = "",
    /// Moves the address to another view; absent in the render harness.
    showView = null,
  }: {
    id?: string;
    brief?: { state: string; updated?: string } | null;
    loaded?: boolean;
    detail?: Detail | null;
    review?: ReviewBody | null;
    view?: string;
    showView?: ((view: string) => void) | null;
  } = $props();

  /// The change this document is about, as a value. Everything below keys on
  /// this, never on the prop: a prop is re-read whenever the shell's props
  /// object is, and the result sentence and the refusal must survive every
  /// poll that did not change which change this is.
  const key = $derived(id);

  const read = resource<Detail>(() => (key ? `/api/changes/${encodeURIComponent(key)}` : null), {
    tell: () => `devplane change show ${key}`,
  });
  const d = $derived(read.data ?? planted);
  /// The review, read once for the header, the Review tab's count and the
  /// pane, so all three count the same rows.
  const reviewRead = resource<ReviewBody>(() => (key ? `/api/changes/${encodeURIComponent(key)}/review` : null), {
    tell: () => `devplane change review ${key}`,
  });
  const review = $derived(reviewRead.data ?? plantedReview);

  let said = $state("");
  /// Commands the host handed back to run by hand (an offer it may not push).
  let commands = $state<string[]>([]);
  const pending = writer();

  const VIEWS = ["overview", "tasks", "review", "gates", "ledger", "agent"] as const;
  /// The view picked here when there is no address to hold it (the harness).
  let local = $state("overview");
  const askedView = $derived(asked.split("/")[0]);
  /// The hunk the address names, in the review: `<path>@<digest>`.
  const askedHunk = $derived(askedView === "review" ? asked.slice("review/".length) : "");
  const view = $derived((VIEWS as readonly string[]).includes(askedView) ? askedView : local);
  /// An address naming a view this change does not have says so.
  const lost = $derived(askedView && !(VIEWS as readonly string[]).includes(askedView) ? `There is no view called "${askedView}"; this is the overview.` : "");
  function setView(v: string) {
    local = v;
    showView?.(v === "overview" ? "" : v);
  }
  /// The host's refusal to offer, naming every unseen row.
  let refusal = $state<OfferRefusal | null>(null);
  /// The row a refusal's action opens in the review.
  let reviewAt = $state("");

  // Another change: none of the last one's sentences.
  let lastMoved: string | null = null;
  $effect.pre(() => {
    void key;
    untrack(() => {
      local = "overview";
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
      if (lastMoved !== null && m !== lastMoved) {
        void read.reload();
        void reviewRead.reload();
      }
      lastMoved = m;
    });
  });

  const verified = $derived(d?.state === STATES.verified.word);
  /// Weakened rows no person has marked read: while any remain, Review is the
  /// primary action and an offer is refused by the host.
  const unseen = $derived(d?.qualifier?.unseen ?? 0);
  const leads = $derived(weakLeads(d?.qualifier));
  /// One *unread*, said once: on the Review tab. Hunks only — the weakened
  /// rows are counted by the state beside the title.
  const unreadNow = $derived(unreadSays({ hunks: unread(review, unseen).hunks, weakened: 0 }));
  /// The tree has uncommitted work the last check did not run over.
  const uncovered = $derived(
    !!d?.tree_now && d.tree_now.clean === false && !!d.gate?.ran_at?.tree && d.gate.ran_at.tree !== d.tree_now.tree,
  );
  /// The host's sentence about the check, unless it would reassure over
  /// something unread or a tree it did not see.
  const standing = $derived(d?.standing_says && !(verified && (leads || uncovered)) ? d.standing_says : "");
  function readRow(path: string) {
    reviewAt = path;
    setView("review");
  }
  const lastRun = $derived(d?.runs?.[d.runs.length - 1] ?? "");

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
    not_opened_says?: string;
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
      return `${r?.not_opened_says ? `${r.not_opened_says}. ` : ""}Nothing was pushed — [github] pull_request is not set, so the pull request is yours to open. Run:`;
    }
    if (verb === "verify" && typeof r?.passed === "boolean") {
      return `${r.passed ? "The gates exited zero" : "The gates failed"}${r.summary ? `: ${r.summary}` : "."}`;
    }
    return (r?.says ?? PAST[verb]) + (r?.pull_request?.url ? ` — ${r.pull_request.url}` : "") + (r?.not_opened_says ? `. ${r.not_opened_says}` : "");
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
        await Promise.all([read.reload(), reviewRead.reload()]);
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
    { id: "review", label: "Review", icon: "split", count: unreadNow || null, tone: unseen > 0 ? "fail" : undefined },
    { id: "gates", label: "Gates", icon: "gate", count: d?.gates?.length ?? null, tone: d?.gate ? (verified && !leads ? "done" : d.gate.passed ? undefined : "fail") : undefined },
    { id: "ledger", label: "Ledger", icon: "ledger" },
    { id: "agent", label: "Agent", icon: "agent", count: d?.runs?.length ?? null },
  ]);

  // The review's keys while its tab shows; the list's `j`/`k` otherwise.
  $effect(() => (view === "review" ? setMode("change", "review") : undefined));
  $effect(() => {
    const to = (v: string) => () => (setView(v), true);
    const offs = [
      onAction("review", to("review")),
      onAction("view-overview", to("overview")),
      onAction("view-tasks", to("tasks")),
      onAction("view-review", to("review")),
      onAction("view-gates", to("gates")),
      onAction("view-ledger", to("ledger")),
      onAction("view-agent", to("agent")),
    ];
    return () => offs.forEach((off) => off());
  });
  /// Can be offered: the host's own guard, said here as the same test the
  /// toolbar uses. The host still refuses an offer over an unread row.
  const canOffer = $derived(verified && unseen === 0);
  /// Why Offer is held, said beside it; empty when it is not.
  /// The count is the state's own (*1 check weakened*), said once; this says
  /// what to do about it.
  const offerHeld = $derived(verified && unseen > 0 ? `Read the weakened ${unseen === 1 ? "check" : "checks"} first` : "");
  // The palette's *this change* commands, each through this document's code.
  $effect(() => {
    if (!d) return;
    return offerCommands(d.title || d.id, [
      { label: `Review${unreadNow ? ` — ${unreadNow}` : ""}`, icon: "split", run: () => setView("review") },
      ...(d.worktree && !declaresNoCheck(d) ? [{ label: "Run gates", icon: "gate", run: () => void act("verify") }] : []),
      ...(canOffer ? [{ label: "Offer as pull request", icon: "forge", run: () => void act("offer") }] : []),
    ]);
  });
</script>

{#if !id}
  {#if loaded}
    <Empty icon="change" title="Pick a change" body="Choose one in the list, or start a new one — an isolated worktree, an agent in it, and the project's own gates." />
  {:else}
    <div class="loading"><Skeleton rows={4} /></div>
  {/if}
{:else if read.failure && !d}
  <div class="failed"><Failed what="this change" failure={read.failure} /></div>
{:else if !d}
  <div class="loading"><Skeleton rows={4} /></div>
{:else}
  <article class="doc" class:reviewing={view === "review"}>
    <Head
      {d}
      {view}
      {unseen}
      {verified}
      {leads}
      {standing}
      {offerHeld}
      busy={pending.busy ?? ""}
      notOpened={review?.not_opened ?? []}
      {act}
      {openIn}
      {copy}
      review={() => setView("review")}
      reload={() => void read.reload()}
    />
    <!-- Mounted always, so the first sentence is announced too. -->
    <p class="said" class:empty={!said && !pending.busy && !lost} role="status" aria-live="polite">
      {#if pending.busy}{DOING[pending.busy as Verb]} · {pending.elapsed} — nothing else can be started on this change until it is back.{:else}<Inline text={said || lost} />{/if}
    </p>
    {#if commands.length && !pending.busy}<div class="under"><Commands {commands} /></div>{/if}
    {#if read.phase === "stale" && read.failure}<div class="under"><Failed what="this change" failure={read.failure} at={read.at} stale /></div>{/if}
    {#if refusal}<div class="under"><Refusal {refusal} read={readRow} /></div>{/if}
    {#if d.draft}<Draft {d} review={() => setView("review")} reload={() => void read.reload()} />{/if}

    <div class="tabbar"><Tabs {tabs} bind:active={() => view, setView} label="views of this change" /></div>

    <div class="view" class:review={view === "review"} role="tabpanel" aria-label={tabs.find((t) => t.id === view)?.label}>
      {#if view === "overview"}
        <Overview {d} go={setView} />
      {:else if view === "tasks"}
        <Tasks {d} {lastRun} reload={read.reload} />
      {:else if view === "review"}
        <Pane id={key} focus={reviewAt || askedHunk} source={reviewRead} review={plantedReview} counted onaddress={(at) => showView?.(`review/${at}`)} />
      {:else if view === "gates"}
        <Gates {d} id={key} reload={() => read.reload()} weakened={(review?.groups ?? []).flatMap((g) => g.weakened ?? [])} toLine={readRow} />
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
    container: doc / inline-size;
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .said {
    margin: 0 var(--s-6) var(--s-1);
    font-size: var(--t-sm);
    color: var(--dim);
    padding: var(--s-1) var(--s-3);
    border-left: 2px solid var(--accent);
    background: var(--panel);
  }
  /* Kept in the page for the announcement, and out of the layout. */
  .said.empty {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: 0;
    border: 0;
    overflow: hidden;
    clip-path: inset(50%);
  }
  .under {
    padding: 0 var(--s-6) var(--s-1);
  }
  .tabbar {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
    min-width: 0;
  }
  .view {
    flex: 1;
    padding: var(--s-4) var(--s-6) var(--s-6);
    min-width: 0;
  }
  /* The review fills the editor and scrolls inside its column, so moving
     hunk to hunk never scrolls the header away. */
  .doc.reviewing {
    height: 100%;
    min-height: 0;
  }
  .view.review {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: var(--s-2) var(--s-4) var(--s-2);
  }
  .failed,
  .loading {
    padding: var(--s-5) var(--s-6);
  }
</style>
