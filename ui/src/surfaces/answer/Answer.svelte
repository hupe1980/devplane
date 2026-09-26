<script lang="ts">
  // The topmost thing that needs you, answerable, and nothing else — what the
  // shortcut raises. The inbox's first row (same `Item.svelte`, same routes);
  // `Esc` gives focus back.
  import { ago } from "../../lib/text";
  import { hideWindow } from "../../lib/frame";
  import { onAction } from "../../lib/keys";
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
  }: { items?: Item[] | null } = $props();

  /// The host's needs-you list (as `devplane inbox --needs-you`), so this
  /// window agrees with the tray's number.
  const read = resource<{ items?: Item[] }>(() => "/api/inbox?needs_you=true", {
    every: 2_000,
    tell: () => "devplane inbox --needs-you",
  });
  let said = $state("");
  let commands = $state<string[]>([]);
  const shown = $derived(read.data?.items ?? items ?? []);
  const first = $derived(shown[0] ?? null);
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
  const act = (item: Item, action: string, reason?: string) => report(() => actions.act(item, action, reason));
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
  <p class="said" role="status" aria-live="polite">{said}</p>
  {#if commands.length}<Commands {commands} />{/if}
  {#if failed && read.failure}
    <Failed what="what needs you" failure={read.failure} />
  {:else if !loaded}
    <p class="dim">reading…</p>
  {:else if first}
    {#if read.phase === "stale" && read.failure}<Failed what="what needs you" failure={read.failure} at={read.at} stale />{/if}
    <fieldset class="bare" disabled={!!pending.busy}>
      <ul role="list">
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
          />
        {/key}
      </ul>
    </fieldset>
    {#if shown.length > 1}
      <p class="dim more">{shown.length - 1} more after this one</p>
    {/if}
  {:else}
    <!-- Zero, said as a fact: the list was read and it is empty — and, if
         the re-read since failed, marked as the old answer it is. -->
    {#if read.phase === "stale" && read.failure}
      <Failed what="what needs you" failure={read.failure} at={read.at} stale />
    {:else}
      <p class="dim">Nothing needs you.</p>
    {/if}
  {/if}
  <p class="hint dim">Esc hides this</p>
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
</style>
