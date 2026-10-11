<script lang="ts" module>
  /// What the host answers a write previewed with `confirm: false`: the file
  /// and the exact change, nothing written yet.
  export type Pending = {
    path: string;
    diff: string;
    writes: boolean;
    uncommitted?: boolean;
    new_file?: boolean;
    removes?: boolean;
  };
</script>

<script lang="ts">
  // Every configuration write in the window: the file, the exact diff, and
  // Confirm / Cancel. Confirm sends the same request with `confirm: true` and
  // the diff that was shown; the host writes only if the file still produces
  // it, and otherwise answers the new diff, which replaces this one and is
  // asked about again. Nothing is written before Confirm.
  import { api, Refused } from "./api";
  import Icon from "./ui/Icon.svelte";

  let {
    pending,
    url,
    body,
    confirmLabel = "Confirm",
    step = "",
    oncancel,
    ondone,
  }: {
    pending: Pending;
    /// The route the preview came from; the confirm goes to the same one.
    url: string;
    body: Record<string, unknown>;
    confirmLabel?: string;
    /// A first-run step this confirm completes, for the checklist's walk.
    step?: string;
    oncancel?: () => void;
    ondone?: (answer: unknown) => void;
  } = $props();

  let shown = $state<Pending | null>(null);
  const p = $derived(shown ?? pending);
  let busy = $state(false);
  let said = $state("");

  async function confirm() {
    if (busy) return;
    busy = true;
    said = "";
    try {
      const r = await api<unknown>(url, { method: "POST", body: JSON.stringify({ ...body, confirm: true, diff: p.diff }) });
      ondone?.(r);
    } catch (e) {
      if (e instanceof Refused && e.status === 409) {
        const b = e.body as Partial<Pending> & { error?: string };
        if (typeof b?.diff === "string") {
          shown = { path: b.path ?? p.path, diff: b.diff, writes: b.writes ?? true, uncommitted: b.uncommitted };
          said = "The file changed since this was shown; this is the change now. Nothing was written.";
          return;
        }
      }
      said = `Nothing was written: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      busy = false;
    }
  }
  const lines = $derived(p.diff.replace(/\n$/, "").split("\n"));
</script>

<div class="pending" role="group" aria-label="the change to {p.path}">
  <p class="file">
    <Icon name="file" size={13} />
    <code>{p.path}</code>
    {#if p.new_file}<span class="tag">a new file</span>{/if}
    {#if p.removes}<span class="tag">removed</span>{/if}
    {#if p.uncommitted}<span class="tag warn">has uncommitted changes — the diff is against the file as it is on disk</span>{/if}
  </p>
  {#if !p.writes}
    <p class="quiet">Already exactly this; nothing to write.</p>
  {:else}
    <pre class="diff" data-quoted>{#each lines as l, i (i)}<span class:add={l.startsWith("+")} class:del={l.startsWith("-")}>{l}
</span>{/each}</pre>
  {/if}
  {#if said}<p class="said" role="status">{said}</p>{/if}
  <div class="actions">
    <button type="button" class="primary" disabled={busy || !p.writes} onclick={confirm} data-first-run-step={step || undefined}>
      <Icon name="check" size={13} /> {busy ? "Writing…" : confirmLabel}
    </button>
    <button type="button" disabled={busy} onclick={() => oncancel?.()}>Cancel — write nothing</button>
  </div>
</div>

<style>
  .pending {
    display: grid;
    gap: var(--s-2);
  }
  .file {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    margin: 0;
    font-size: var(--t-sm);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
    overflow-wrap: anywhere;
  }
  .tag {
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .tag.warn {
    color: var(--wait);
  }
  .diff {
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 22rem;
    overflow: auto;
  }
  .add {
    color: var(--add);
  }
  .del {
    color: var(--del);
  }
  .said {
    margin: 0;
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .quiet {
    margin: 0;
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
</style>
