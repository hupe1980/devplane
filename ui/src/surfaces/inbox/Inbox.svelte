<script lang="ts">
  // What needs a person, and answering it: the item the address names.
  import { api } from "../../lib/api";
  import { ago } from "../../lib/text";
  import Skeleton from "../../lib/Skeleton.svelte";
  import Row from "./Item.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import type { Answer, Item } from "./Item.svelte";
  import * as actions from "./actions";

  type Summary = { kind: string; project: string | null; count: number; level: string };
  type Inhibited = { cause: string; count: number; because: string };
  type Close = {
    since_last_look?: string | null;
    clear?: boolean;
    quiet?: boolean;
    sentences?: string[];
    next?: string | null;
    keeps_running?: string | null;
  };
  /// Which vendors the host can see at all, so *Clear.* can name the sessions
  /// it cannot.
  type Watching = { watched?: string[]; unproved?: string[]; driven_only?: string[] };

  let {
    items = [],
    folded = [],
    inhibited = [],
    close = null,
    project = "",
    wanted = "",
    focus = "",
    loaded = false,
    stale_since = null,
    error = null,
    watching = null,
  }: {
    items?: Item[];
    folded?: Summary[];
    inhibited?: Inhibited[];
    close?: Close | null;
    /// The project this list is narrowed to, out of `#inbox/<project>`.
    project?: string;
    /// The ask a link named, out of `#inbox/ask=<id>`: the cursor lands on it.
    wanted?: string;
    focus?: string;
    /// Whether the feed has ever arrived. Until it has, an empty list means
    /// *nothing read*, not *nothing needs you*.
    loaded?: boolean;
    /// When the data on screen was read, while the host is not answering.
    stale_since?: string | null;
    /// Why the feed is not the present, when it is not.
    error?: string | null;
    watching?: Watching | null;
  } = $props();

  /// The vendors whose sessions never reach this inbox unless Devplane
  /// started them, joined as a person would say them.
  const unseen = $derived.by(() => {
    const v = [...(watching?.driven_only ?? []), ...(watching?.unproved ?? [])];
    return v.length <= 1 ? v.join("") : `${v.slice(0, -1).join(", ")} and ${v[v.length - 1]}`;
  });

  /// The narrowed list comes from the host, not from filtering the feed, so
  /// the window and the terminal narrow by one computation.
  let narrowedFeed = $state<{
    items: Item[];
    folded: Summary[];
    inhibited: Inhibited[];
    narrowed: { count: number; projects: string[]; no_such_project: boolean } | null;
  } | null>(null);

  $effect(() => {
    const want = project.trim();
    if (!want) {
      narrowedFeed = null;
      return;
    }
    let live = true;
    // Not a look: a narrowed view must not advance the close's boundary.
    api<typeof narrowedFeed & object>(`/api/inbox?project=${encodeURIComponent(want)}`)
      .then((r) => {
        if (live) narrowedFeed = r;
      })
      .catch(() => {
        if (live) narrowedFeed = null;
      });
    return () => {
      live = false;
    };
  });

  const shown = $derived(narrowedFeed?.items ?? items);
  const shownFolded = $derived(narrowedFeed?.folded ?? folded);
  const shownInhibited = $derived(narrowedFeed?.inhibited ?? inhibited);
  /// No close over a narrowed list: a day is not one project. The host
  /// withholds it too.
  const shownClose = $derived(project.trim() ? null : close);

  /// What the last answer did, so a refusal is never silent.
  let said = $state("");

  /// The one action that can be taken back, and the route that does it. Set by
  /// the reversible action, cleared by every other, so *undo* is only offered
  /// when it can be delivered.
  let undo = $state<{ says: string; where: string } | null>(null);

  /// The controls' work lives in `actions.ts`, shared with the answer window;
  /// this surface reports what each did and keeps the one undo.
  async function act(item: Item, action: string, reason?: string) {
    const r = await actions.act(item, action, reason);
    said = r.said;
    undo = r.undo;
  }
  async function answer(item: Item, what: Answer) {
    const r = await actions.answer(item, what);
    said = r.said;
    undo = r.undo;
  }
  async function snooze(item: Item) {
    const r = await actions.snooze(item);
    said = r.said;
    undo = r.undo;
  }
  /// Takes back the last snooze; cleared whether or not it succeeds.
  async function takeBack() {
    if (!undo) return;
    said = (await actions.takeBack(undo)).said;
    undo = null;
  }
  async function copyRule(rule: string) {
    said = (await actions.copyRule(rule)).said;
  }

  const nothingRaised = $derived(
    shown.length === 0 && shownFolded.length === 0 && shownInhibited.length === 0,
  );

  /// The item on screen: the one the address names, else the top of the list.
  const current = $derived(
    shown.find((x) => x.id === focus) ??
      (wanted ? shown.find((x) => x.ask === wanted || x.request_id === wanted) : undefined) ??
      shown[0] ??
      null,
  );
  const position = $derived(Math.max(0, shown.findIndex((x) => x.id === current?.id)));

  let now = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });

</script>

<section class="detail" aria-label="the item">
  {#if said || undo}
    <p class="said" role="status" aria-live="polite">
      {said}
      {#if undo}<button class="undo" onclick={takeBack}>{undo.says}</button>{/if}
    </p>
  {/if}

  {#if !loaded && nothingRaised}
    <Skeleton />
  {:else if nothingRaised && error}
    <!-- Not *Clear.*: the host has not answered since the last empty read. -->
    <p class="dim stale">The last read had nothing for you, and Devplane has not answered since.</p>
  {:else if nothingRaised}
    <!-- The close: what the day came to, only over a feed that is loaded and current. -->
    <div class="close">
      <span class="ok"><Icon name="check" size={22} /></span>
      <h1>Clear.</h1>
      {#if shownClose?.quiet}
        <p>Nothing needed you, and nothing was decided for you.</p>
      {:else}
        {#each shownClose?.sentences ?? [] as s, si (si)}<p>{s}</p>{/each}
      {/if}
      {#if shownClose?.next}<p class="dim">Next · {shownClose.next}</p>{/if}
      {#if shownClose?.keeps_running}<p class="dim">{shownClose.keeps_running}</p>{/if}
      {#if unseen}<p class="dim">Sessions in {unseen} are not on this board unless Devplane started them.</p>{/if}
    </div>
  {:else if current}
    {#if error}
      <p class="hairline stale">
        stale · as read {stale_since ? ago(Math.max(0, (Date.now() - Date.parse(stale_since)) / 1000)) + " ago" : "before Devplane stopped answering"}
      </p>
    {/if}
    <header class="top">
      <span class="lvl {current.level}">{current.level}</span>
      <span class="kind">{current.kind.replace(/_/g, " ")}</span>
      {#if current.project_name}<span class="proj"><Icon name="folder" size={12} /> {current.project_name}</span>{/if}
      {#if current.since}<span class="age" title={current.since}>{ago(Math.max(0, (now - Date.parse(current.since)) / 1000))} ago</span>{/if}
      <span class="pos">{position + 1} of {shown.length}</span>
    </header>
    <ul role="list" class="one" class:stale={!!error}>
      <Row
        item={current}
        current={true}
        age=""
        {answer}
        {act}
        {snooze}
        {copyRule}
        say={(s) => (said = s)}
      />
    </ul>
    <footer class="nav">
      {#if current.change_id}<a href={`#change/${encodeURIComponent(current.change_id)}`}><Icon name="change" size={13} /> Open the change</a>{/if}
      <span class="keys"><kbd>j</kbd> <kbd>k</kbd> in the list move between items</span>
    </footer>
  {/if}
</section>

<style>
  .detail {
    padding: var(--s-5) var(--s-6);
    max-width: 62rem;
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    margin-bottom: var(--s-3);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .lvl {
    padding: 0.1rem 0.5rem;
    border-radius: 999px;
    border: 1px solid var(--line);
    text-transform: uppercase;
    font-weight: 700;
    letter-spacing: 0.05em;
    font-size: 0.625rem;
    color: var(--dim);
  }
  .lvl.high {
    color: var(--wait);
    border-color: var(--wait);
  }
  .kind {
    color: var(--dim);
    font-weight: 600;
  }
  .proj {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }
  .pos {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  .one {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    margin-top: var(--s-5);
    padding-top: var(--s-3);
    border-top: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  .nav a {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--accent);
    text-decoration: none;
  }
  .keys {
    margin-left: auto;
    color: var(--faint);
    font-size: var(--t-xs);
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 0.25rem;
  }
  .said {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    margin: 0 0 var(--s-4);
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--accent);
    background: var(--panel);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .undo {
    font-size: var(--t-xs);
  }
  .close {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: var(--s-2);
    padding: var(--s-7) var(--s-4);
    color: var(--dim);
  }
  .close h1 {
    margin: 0;
    font-size: 1.5rem;
    color: var(--ink);
  }
  .close p {
    margin: 0;
    max-width: 40rem;
  }
  .ok {
    display: grid;
    place-items: center;
    width: 3rem;
    height: 3rem;
    border-radius: 50%;
    background: var(--panel);
    border: 1px solid var(--line);
    color: var(--dim);
  }
  .dim {
    color: var(--faint);
  }
  .hairline {
    font-size: var(--t-xs);
    color: var(--wait);
  }
  .stale {
    opacity: 0.75;
  }
</style>
