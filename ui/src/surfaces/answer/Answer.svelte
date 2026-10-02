<script lang="ts">
  // The topmost thing that needs you, answerable, and nothing else — what the
  // shortcut raises. The inbox's first row (same `Item.svelte`, same routes);
  // `Esc` gives focus back.
  import Inline from "../../lib/ui/Inline.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";
  import { ago } from "../../lib/text";
  import { hideWindow } from "../../lib/frame";
  import { onAction } from "../../lib/keys";
  import { cursor } from "../../lib/cursor";
  import { resource } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import Commands from "../../lib/Commands.svelte";
  import Row from "../inbox/Item.svelte";
  import type { Answer, Item } from "../inbox/Item.svelte";
  import * as actions from "../inbox/actions";

  let {
    /// A planted list, for a harness; otherwise read from the host.
    items = null,
    blind = "",
  }: { items?: Item[] | null; blind?: string } = $props();

  /// The host's needs-you list (as `devplane inbox --needs-you`), so this
  /// window agrees with the tray's number.
  const read = resource<{ items?: Item[]; held?: actions.Held | null; landed?: actions.Landed | null }>(() => "/api/inbox?needs_you=true", {
    every: 2_000,
    tell: () => "devplane inbox --needs-you",
  });
  let said = $state("");
  let commands = $state<string[]>([]);
  const shown = $derived(read.data?.items ?? items ?? []);
  /// Which waiting item is on screen: the first, until `j`/`k` move. Kept by
  /// id, so an answered item leaving puts the next one here.
  let at = $state("");
  const index = $derived(Math.max(0, shown.findIndex((x) => x.id === at)));
  const first = $derived(shown[index] ?? shown[0] ?? null);
  const held = $derived(read.data?.held ?? null);
  const landed = $derived(read.data?.landed ?? null);
  $effect(() =>
    cursor({
      scope: "answer",
      size: () => shown.length,
      at: () => index,
      go: (i) => (at = shown[i].id),
      open: () => (document.querySelector(".answer .one") as HTMLElement | null)?.focus(),
    }),
  );
  /// The ask to put back on screen once the undo returns it to the list.
  let returning = $state("");
  $effect(() => {
    if (!returning) return;
    const back = shown.find((x) => x.ask === returning || x.request_id === returning);
    if (back) {
      returning = "";
      at = back.id;
    }
  });
  async function takeBack() {
    const h = held;
    if (!h) return;
    const r = await pending.run("undo", () => actions.takeBack({ where: h.undo }));
    if (r) said = r.said;
    returning = h.ask;
    await read.reload();
  }
  // `z` takes back the answer the host is holding; nothing else here is
  // reversible, and it says so.
  $effect(() =>
    onAction("item-undo", (surface) => {
      if (surface !== "answer") return false;
      if (held) void takeBack();
      else said = landed?.ok ? "that answer was already sent — it can no longer be taken back" : "nothing to undo";
      return true;
    }),
  );
  let tick = $state(Date.now());
  $effect(() => {
    if (!held) return;
    const id = setInterval(() => (tick = Date.now()), 250);
    return () => clearInterval(id);
  });
  const left = $derived(held ? Math.max(0, Math.ceil((Date.parse(held.until) - tick) / 1000)) : 0);
  /// Nothing was read and the read failed: not *Nothing needs you*.
  const failed = $derived(read.phase === "failed" && items === null);
  const loaded = $derived(read.data !== null || items !== null);

  // `Esc` hides the window; in a browser tab it does nothing.
  $effect(() =>
    onAction("leave", () => {
      hideWindow();
      return true;
    }),
  );

  const pending = writer();
  async function report(work: () => Promise<actions.Outcome>) {
    const r = await pending.run("action", work);
    if (!r) return;
    said = r.said;
    commands = r.commands ?? [];
    await read.reload();
  }
  const answer = (item: Item, what: Answer) => report(() => actions.answer(item, what));
  const act = (item: Item, action: string) => report(() => actions.act(item, action));
  const snooze = (item: Item) => report(() => actions.snooze(item));
  async function copyRule(rule: string) {
    said = (await actions.copyRule(rule)).said;
  }

  let now = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });
</script>

<section class="answer" aria-labelledby="answer-head">
  <h2 id="answer-head">The one thing that needs you</h2>
  <p class="said" role="status" aria-live="polite"><Inline text={said} /></p>
  {#if commands.length}<Commands {commands} />{/if}
  {#if held}
    <p class="held" role="status">
      <b>{held.title}</b>: {held.says} · {left > 0 ? `sending in ${left} s` : "sending now"} · <kbd>z</kbd> to undo
      <button onclick={takeBack} disabled={!!pending.busy}>take it back <kbd>z</kbd></button>
    </p>
  {:else if landed && !landed.ok}
    <p class="held failed" role="status"><b>{landed.title}</b>: {landed.says}</p>
  {/if}
  {#if failed && read.failure}
    <Failed what="what needs you" failure={read.failure} />
  {:else if !loaded}
    <Skeleton rows={2} />
  {:else if first}
    {#if read.phase === "stale" && read.failure}<Failed what="what needs you" failure={read.failure} at={read.at} stale />{/if}
    <fieldset class="bare" disabled={!!pending.busy}>
      <ul role="list" class="one" tabindex="-1" aria-label="the item: {first.title}">
        <!-- Keyed: a reply typed for one item never carries to the next. -->
        {#key first.id}
          <Row
            item={first}
            current={true}
            age={first.since ? ago(Math.max(0, (now - Date.parse(first.since)) / 1000)) : ""}
            {answer}
            {act}
            {snooze}
            {copyRule}
            say={(s) => (said = s)}
            keys="answer"
          />
        {/key}
      </ul>
    </fieldset>
    {#if shown.length > 1}
      <p class="dim more">item {index + 1} of {shown.length} · <kbd>j</kbd> <kbd>k</kbd> move between them</p>
    {/if}
  {:else}
    <!-- Zero, said as a fact: the list was read and it is empty — and, if
         the re-read since failed, marked as the old answer it is. -->
    {#if read.phase === "stale" && read.failure}
      <Failed what="what needs you" failure={read.failure} at={read.at} stale />
    {:else}
      <p class="dim">Nothing needs you.</p>
      {#if blind}<p class="dim"><Inline text={blind} /></p>{/if}
    {/if}
  {/if}
  <p class="hint dim">Esc hides this · ? lists every key</p>
</section>

<style>
  .answer { padding: var(--s-3) var(--s-4); }
  h2 { font-size: var(--t-md); margin: 0 0 var(--s-1); }
  .said { color: var(--dim); font-size: var(--t-xs); margin: 0; min-height: 1.2em; }
  .said:empty { display: none; }
  ul { list-style: none; margin: var(--s-2) 0 0; padding: 0; border-top: 1px solid var(--line); }
  .dim { color: var(--dim); }
  fieldset.bare { border: 0; margin: 0; padding: 0; min-width: 0; }
  .more { font-size: var(--t-xs); margin: var(--s-2) 0 0; }
  .hint { font-size: var(--t-xs); margin: var(--s-3) 0 0; }
  .held { font-size: var(--t-xs); margin: var(--s-2) 0 0; padding: var(--s-1) var(--s-2); border-left: 2px solid var(--wait); color: var(--dim); font-variant-numeric: tabular-nums; }
  .held.failed { border-left-color: var(--fail); color: var(--ink); }
  kbd { font-family: var(--mono); font-size: 0.625rem; border: 1px solid var(--line); border-radius: 3px; padding: 0 0.25rem; }
</style>
