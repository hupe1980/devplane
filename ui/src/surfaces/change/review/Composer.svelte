<script lang="ts">
  // A comment being written on one line: kept by the host when added, sent
  // only with the rest of the pending review. ⌘Enter adds it; Esc lets it go.
  import { spell } from "../../../lib/keys";

  let {
    busy = false,
    save,
    cancel,
  }: { busy?: boolean; save: (body: string) => void; cancel: () => void } = $props();

  let draft = $state("");
  let field = $state<HTMLTextAreaElement | null>(null);
  // Opened by a key or a click on a line number: the words go here at once.
  $effect(() => {
    field?.focus();
  });
  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      cancel();
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      e.stopPropagation();
      if (draft.trim()) save(draft.trim());
    }
  }
</script>

<form class="compose" onsubmit={(e) => { e.preventDefault(); if (draft.trim()) save(draft.trim()); }}>
  <textarea bind:this={field} bind:value={draft} rows="2" aria-label="a comment on this line, for the pending review" onkeydown={key}></textarea>
  <div>
    <button type="submit" class="primary" disabled={!draft.trim() || busy}>Add to the review <kbd>{spell("Mod+Enter")}</kbd></button>
    <button type="button" onclick={cancel}>Cancel <kbd>Esc</kbd></button>
    <span class="hint">nothing is sent until you send the review</span>
  </div>
</form>

<style>
  .compose {
    display: grid;
    gap: var(--s-1);
  }
  textarea {
    width: 100%;
    background: var(--bg);
    color: var(--ink);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: var(--s-1) var(--s-2);
    font: inherit;
    font-size: var(--t-sm);
  }
  div {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  button {
    height: 1.6rem;
    font-size: var(--t-xs);
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
  }
  .hint {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
  }
</style>
