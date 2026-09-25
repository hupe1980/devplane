<script lang="ts">
  // The topmost thing that needs you, answerable, and nothing else — what the
  // shortcut raises. The inbox's first row (same `Item.svelte`, same routes);
  // `Esc` gives focus back.
  import { api } from "../../lib/api";
  import { ago } from "../../lib/text";
  import { hideWindow } from "../../lib/frame";
  import { onAction } from "../../lib/keys";
  import Row from "../inbox/Item.svelte";
  import type { Answer, Item } from "../inbox/Item.svelte";
  import * as actions from "../inbox/actions";

  let {
    /// A planted list, for a harness; otherwise read from the host.
    items = null,
  }: { items?: Item[] | null } = $props();

  let fetched = $state<Item[] | null>(null);
  let read = $state(false);
  let said = $state("");
  const shown = $derived(fetched ?? items ?? []);
  const first = $derived(shown[0] ?? null);
  const loaded = $derived(read || items !== null);

  /// The host's needs-you list (as `devplane inbox --needs-you`), so this
  /// window agrees with the tray's number.
  async function reread() {
    try {
      const r = await api<{ items?: Item[] }>("/api/inbox?needs_you=true");
      fetched = r.items ?? [];
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
      fetched = [];
    }
    read = true;
  }
  $effect(() => {
    void reread();
    const id = setInterval(() => void reread(), 2_000);
    return () => clearInterval(id);
  });

  // `Esc` hides the window; in a browser tab it does nothing.
  $effect(() =>
    onAction("leave", () => {
      hideWindow();
      return true;
    }),
  );

  async function answer(item: Item, what: Answer) {
    said = (await actions.answer(item, what)).said;
    await reread();
  }
  async function act(item: Item, action: string, reason?: string) {
    said = (await actions.act(item, action, reason)).said;
    await reread();
  }
  async function snooze(item: Item) {
    said = (await actions.snooze(item)).said;
    await reread();
  }
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
  {#if !loaded}
    <p class="dim">reading…</p>
  {:else if first}
    <ul role="list">
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
    </ul>
    {#if shown.length > 1}
      <p class="dim more">{shown.length - 1} more after this one</p>
    {/if}
  {:else}
    <!-- Zero, said as a fact: the list was read and it is empty. -->
    <p class="dim">Nothing needs you.</p>
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
  .more { font-size: var(--t-xs); margin: var(--s-2) 0 0; }
  .hint { font-size: var(--t-xs); margin: var(--s-3) 0 0; }
</style>
