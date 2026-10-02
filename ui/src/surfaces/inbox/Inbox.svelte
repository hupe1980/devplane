<script lang="ts">
  // What needs a person, and answering it: the item the address names.
  import Inline from "../../lib/ui/Inline.svelte";
  import { ago } from "../../lib/text";
  import { cursor } from "../../lib/cursor";
  import { onAction, spell } from "../../lib/keys";
  import { place, remember, itemFocus } from "./place.svelte";
  import { reread } from "../../lib/live.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Commands from "../../lib/Commands.svelte";
  import { narrowed, type Summary, type Inhibited } from "./narrow.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";
  import Row from "./Item.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import type { Answer, Item } from "./Item.svelte";
  import * as actions from "./actions";

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
    item = "",
    wanted = "",
    loaded = false,
    stale_since = null,
    error = null,
    watching = null,
    blind = "",
    held = null,
    landed = null,
    open = () => {},
  }: {
    /// What this host cannot see, when no session has reported to it.
    blind?: string;
    items?: Item[];
    folded?: Summary[];
    inhibited?: Inhibited[];
    close?: Close | null;
    /// The project this list is narrowed to, out of `#inbox/<project>`.
    project?: string;
    /// The row the address names, out of `#inbox/item=<id>`.
    item?: string;
    /// The ask a link named, out of `#inbox/ask=<id>`: the cursor lands on it.
    wanted?: string;
    /// Whether the feed has ever arrived. Until it has, an empty list means
    /// *nothing read*, not *nothing needs you*.
    loaded?: boolean;
    /// When the data on screen was read, while the host is not answering.
    stale_since?: string | null;
    /// Why the feed is not the present, when it is not.
    error?: string | null;
    watching?: Watching | null;
    /// The answer the host is holding before it reaches the agent; its item
    /// is out of `items` while it waits.
    held?: actions.Held | null;
    /// What the last held answer came to once it was sent.
    landed?: actions.Landed | null;
    /// Moves the address to a row (the shell's `open`).
    open?: (focus: string) => void;
  } = $props();

  /// The vendors whose sessions never reach this inbox unless Devplane
  /// started them, joined as a person would say them.
  const unseen = $derived.by(() => {
    const v = [...(watching?.driven_only ?? []), ...(watching?.unproved ?? [])];
    return v.length <= 1 ? v.join("") : `${v.slice(0, -1).join(", ")} and ${v[v.length - 1]}`;
  });

  /// The narrowed list comes from the host, not from filtering the feed, and
  /// is read once for this and the list beside it (`narrow.svelte.ts`).
  const narrow = narrowed(() => project);

  /// A narrowed inbox shows only what the host narrowed: while it is being
  /// read or could not be, nothing from the whole inbox stands in for it.
  const shown = $derived(narrow.on ? (narrow.data?.items ?? []) : items);
  const shownFolded = $derived(narrow.on ? (narrow.data?.folded ?? []) : folded);
  const shownInhibited = $derived(narrow.on ? (narrow.data?.inhibited ?? []) : inhibited);
  /// No close over a narrowed list: a day is not one project. The host
  /// withholds it too.
  const shownClose = $derived(narrow.on ? null : close);
  const ready = $derived(narrow.on ? narrow.phase !== "loading" : loaded);
  /// The host knows no project by the name in the address: a failure, never
  /// an empty inbox.
  const missing = $derived(narrow.missing);

  /// What the last action did, and which item it was about, so the sentence
  /// never sits above a different item as if it were that one's.
  let said = $state<{ text: string; id: string; title: string; commands?: string[] } | null>(null);

  /// The one action that can be taken back, and the route that does it. Set by
  /// the reversible action, cleared by every other, so *undo* is only offered
  /// when it can be delivered.
  let undo = $state<{ says: string; where: string; id: string } | null>(null);

  /// One request at a time: the controls are disabled while it is out, so an
  /// item cannot be answered twice.
  const pending = writer();

  /// The controls' work lives in `actions.ts`, shared with the answer window;
  /// this surface reports what each did, keeps the one undo, and reads the
  /// inbox again at once so the answered item does not stay live.
  async function report(item: Item, work: () => Promise<actions.Outcome>) {
    const r = await pending.run("action", work);
    if (!r) return;
    said = { text: r.said, id: item.id, title: item.title, commands: r.commands };
    undo = r.undo ? { ...r.undo, id: item.id } : null;
    reread();
    void narrow.reload();
  }
  const act = (item: Item, action: string) => report(item, () => actions.act(item, action));
  const answer = (item: Item, what: Answer) => report(item, () => actions.answer(item, what));
  const snooze = (item: Item) => report(item, () => actions.snooze(item));
  /// Takes back the last snooze; cleared whether or not it succeeds.
  async function takeBack() {
    if (!undo) return;
    const u = undo;
    undo = null;
    const r = await pending.run("undo", () => actions.takeBack(u));
    if (r) said = { text: r.said, id: u.id, title: said?.title ?? "" };
    reread();
    void narrow.reload();
  }
  async function copyRule(rule: string) {
    const item = current;
    const r = await actions.copyRule(rule);
    if (item) said = { text: r.said, id: item.id, title: item.title };
  }
  function say(text: string) {
    if (current) said = { text, id: current.id, title: current.title };
  }

  const nothingRaised = $derived(
    shown.length === 0 && shownFolded.length === 0 && shownInhibited.length === 0,
  );

  /// The item on screen: the one the address names; when that has left the
  /// list, the row that took its place, and a sentence saying so.
  const placed = $derived(place(shown, { item, wanted }));
  const current = $derived(placed.current);
  const position = $derived(Math.max(0, shown.findIndex((x) => x.id === current?.id)));
  $effect(() => {
    if (current && current.id === item) remember(item, position);
  });
  /// Why the item on screen is not the one the address named. Not said over
  /// an answer given here: that item's own sentence already says it went.
  const goneSays = $derived(
    !ready || !placed.gone
      ? ""
      : placed.gone === "ask"
        ? "The ask you followed was already answered."
        : said?.id === item
          ? ""
          : "That item was resolved — this is the next one.",
  );

  /// The sentence, if it is about the item on screen, or about one that has
  /// left the list (answered, or resolved elsewhere) — then named.
  const sentence = $derived.by(() => {
    if (!said) return "";
    if (said.id === current?.id) return said.text;
    return shown.some((x) => x.id === said!.id) ? "" : `${said.title}: ${said.text}`;
  });
  /// The undo, over the item it is about, or once that item has left the list
  /// (a snooze hides it, so the next item is on screen).
  const undoHere = $derived(!held && undo && (undo.id === current?.id || !shown.some((x) => x.id === undo!.id)) ? undo : null);

  // The list keys: j/k and the arrows move the address, Enter moves into the
  // item's controls, g g and G go to the ends.
  $effect(() =>
    cursor({
      scope: "inbox",
      size: () => shown.length,
      at: () => shown.findIndex((x) => x.id === current?.id),
      go: (i) => open(itemFocus(shown[i].id)),
      // Into the item, never onto its first control: Enter twice must not
      // answer anything. Tab reaches the controls; `a` and `d` answer.
      open: () => (document.querySelector(".detail .one") as HTMLElement | null)?.focus(),
    }),
  );

  /// The held answer while the address is on the full inbox; a narrowed list
  /// shows it too, since it may be the very item that left.
  const heldHere = $derived(held);
  /// The asks this page held, so the sentence about what one came to is
  /// said here and nowhere it would be news about somebody else's answer.
  let mine = $state<string[]>([]);
  $effect(() => {
    const h = held?.ask;
    if (h && !mine.includes(h)) mine = [...mine.slice(-9), h];
  });
  /// What the last held answer came to: always when it failed — an answer
  /// that did not land is never silent — and when it was this page's.
  const landedHere = $derived(!held && landed && (!landed.ok || mine.includes(landed.ask)) ? landed : null);

  /// The ask to select once it is back in the list after an undo.
  let returning = $state("");
  $effect(() => {
    if (!returning) return;
    const back = shown.find((x) => x.ask === returning || x.request_id === returning);
    if (back) {
      returning = "";
      open(itemFocus(back.id));
    }
  });
  /// Takes the held answer back: nothing reached the agent, and the item is
  /// back in the list, selected.
  async function takeBackHeld() {
    const h = held;
    if (!h) return;
    const r = await pending.run("undo", () => actions.takeBack({ where: h.undo }));
    if (r) said = { text: r.said, id: "", title: h.title };
    returning = h.ask;
    reread();
    void narrow.reload();
  }

  // `z`: the held answer first — it is the one about to become a decision —
  // then the last snooze; with neither, it says so.
  $effect(() =>
    onAction("item-undo", (surface) => {
      if (surface !== "inbox") return false;
      if (pending.busy) return true;
      if (held) void takeBackHeld();
      else if (undoHere) void takeBack();
      else say(landedHere?.ok ? "that answer was already sent — it can no longer be taken back" : "nothing to undo");
      return true;
    }),
  );

  /// The keys that apply to the item on screen, as the footer prints them.
  const keysHere = $derived.by(() => {
    const a = current?.actions ?? [];
    const out: Array<[string, string]> = [];
    const opts = (current?.options ?? []).filter((o) => o.id).length || (current?.form?.[0]?.options.length ?? 0);
    if (opts) out.push([opts > 1 ? `1–${Math.min(opts, 9)}` : "1", "choose"]);
    if (a.includes("reply") || current?.form?.some((q) => q.custom_field)) out.push(["t", "type an answer"]);
    if (a.includes("allow")) out.push(["a", "allow"]);
    if (a.includes("deny")) out.push(["d", "deny"]);
    if (current?.answer_in && (current.session_link || current.resume_command))
      out.push(["w", current.session_link ? "open the session" : "copy the resume command"]);
    if (a.includes("snooze")) out.push(["s", "snooze"]);
    if ((a.includes("open") || a.includes("review")) && (current?.change_id || current?.run_id)) out.push(["o", "open"]);
    if (a.includes("review") && current?.change_id) out.push(["r", "review"]);
    return out;
  });

  /// Seconds left on the held answer, from the host's `until`.
  let tick = $state(Date.now());
  $effect(() => {
    if (!held) return;
    const id = setInterval(() => (tick = Date.now()), 250);
    return () => clearInterval(id);
  });
  const heldLeft = $derived(held ? Math.max(0, Math.ceil((Date.parse(held.until) - tick) / 1000)) : 0);

  let now = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });

</script>

<section class="detail" aria-label="the item">
  <!-- Mounted always, so a screen reader hears the first sentence too. -->
  <p class="said" class:empty={!sentence && !undoHere} role="status" aria-live="polite">
    <Inline text={sentence} />
    {#if undoHere}<button class="undo" onclick={takeBack} disabled={!!pending.busy}>{undoHere.says} <kbd>z</kbd></button>{/if}
  </p>
  <!-- The answer waiting out its hold: what it is, the seconds left, and the
       key that takes it back — in words. -->
  {#if heldHere}
    <p class="held" role="status">
      <span><b>{heldHere.title}</b>: {heldHere.says} · {heldLeft > 0 ? `sending in ${heldLeft} s` : "sending now"} · <kbd>z</kbd> to undo</span>
      <button class="undo" onclick={takeBackHeld} disabled={!!pending.busy}>take it back <kbd>z</kbd></button>
    </p>
  {:else if landedHere}
    <p class="held" class:failed={!landedHere.ok} role="status"><span><b>{landedHere.title}</b>: {landedHere.says}</span></p>
  {/if}
  {#if sentence && said?.commands?.length}<Commands commands={said.commands} />{/if}

  {#if narrow.on && narrow.failure && !narrow.data}
    <Failed what={`the inbox narrowed to ${project}`} failure={narrow.failure} />
  {:else if missing}
    <div class="missing" role="alert">
      <p><b>No project is named “{project}”.</b> Nothing here is narrowed, and nothing here says the inbox is clear.</p>
      <button onclick={() => open("")}>Show the whole inbox</button>
    </div>
  {:else if !ready && nothingRaised}
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
        {#each shownClose?.sentences ?? [] as s, si (si)}<p><Inline text={s} /></p>{/each}
      {/if}
      {#if shownClose?.next}<p class="dim">Next · {shownClose.next}</p>{/if}
      {#if shownClose?.keeps_running}<p class="dim">{shownClose.keeps_running}</p>{/if}
      {#if blind}<p class="dim blind"><Inline text={blind} /></p>
      {:else if unseen}<p class="dim">Sessions in {unseen} are not on this board unless Devplane started them.</p>{/if}
    </div>
  {:else if current}
    {#if narrow.on && narrow.failure}
      <Failed what={`the inbox narrowed to ${project}`} failure={narrow.failure} at={narrow.at} stale />
    {:else if error}
      <p class="hairline stale">
        stale · as read {stale_since ? ago(Math.max(0, (Date.now() - Date.parse(stale_since)) / 1000)) + " ago" : "before Devplane stopped answering"}
      </p>
    {/if}
    {#if goneSays}<p class="gone" role="status">{goneSays}</p>{/if}
    <header class="top">
      <span class="lvl {current.level}">{current.level}</span>
      <span class="kind">{current.kind.replace(/_/g, " ")}</span>
      {#if current.project_name}<span class="proj"><Icon name="folder" size={12} /> {current.project_name}</span>{/if}
      {#if current.since}<span class="age" title={current.since}>{ago(Math.max(0, (now - Date.parse(current.since)) / 1000))} ago</span>{/if}
      <span class="pos">item {position + 1} · {shown.length} in the list</span>
    </header>
    <!-- Keyed by the item: what was typed, picked or given as a reason
         belongs to one item and never carries to the next. The fieldset
         disables every control while an answer is out. -->
    <fieldset class="bare" disabled={!!pending.busy}>
      <ul role="list" class="one" class:stale={!!error} tabindex="-1" aria-label="the item: {current.title}">
        {#key current.id}
          <Row item={current} current={true} age="" {answer} {act} {snooze} {copyRule} {say} keys="inbox" />
        {/key}
      </ul>
    </fieldset>
    <footer class="nav">
      {#if current.change_id}<a href={`#change/${encodeURIComponent(current.change_id)}`}><Icon name="change" size={13} /> Open the change</a>{/if}
      <span class="keys"
        ><kbd>j</kbd> <kbd>k</kbd> move between items · <kbd>Enter</kbd> moves into this one, <kbd>Tab</kbd> to its controls{#each keysHere as [k, what] (k)}
          · <kbd>{k}</kbd> {what}{/each}{#if heldHere || undoHere} · <kbd>z</kbd> undo{/if} · <kbd>{spell("?")}</kbd> every key</span
      >
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
  .said.empty {
    margin: 0;
    padding: 0;
    border: 0;
  }
  fieldset.bare {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }
  .undo {
    font-size: var(--t-xs);
  }
  .held {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    margin: 0 0 var(--s-4);
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--wait);
    background: var(--panel);
    font-size: var(--t-sm);
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
  .held.failed {
    border-left-color: var(--fail);
    color: var(--ink);
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
  .gone {
    margin: 0 0 var(--s-3);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .missing {
    padding: var(--s-4);
    border: 1px solid var(--fail);
    border-radius: var(--radius-lg);
    display: grid;
    gap: var(--s-2);
    justify-items: start;
  }
  .missing p {
    margin: 0;
  }
  .one:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .hairline {
    font-size: var(--t-xs);
    color: var(--wait);
  }
  .stale {
    opacity: 0.75;
  }
</style>
