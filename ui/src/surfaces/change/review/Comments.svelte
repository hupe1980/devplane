<script lang="ts">
  // The pending review: the comments written and not yet sent, counted while
  // any exist, each reachable or deletable from the list, sent together as
  // one turn to the agent. Kept by the host, so a reload loses none.
  import Icon from "../../../lib/ui/Icon.svelte";
  import Inline from "../../../lib/ui/Inline.svelte";
  import { spell } from "../../../lib/keys";
  import type { Comment } from "./types";

  let {
    comments,
    busy = "",
    said = "",
    send,
    jump,
    remove,
  }: {
    comments: Comment[];
    busy?: string;
    said?: string;
    send: () => void;
    jump: (c: Comment) => void;
    remove: (id: string) => void;
  } = $props();

  const pending = $derived(comments.filter((c) => !c.sent_at));
  const n = $derived(pending.length);
</script>

{#if n}
  <div class="pending" role="group" aria-label="the pending review">
    <details class="list">
      <summary>{n === 1 ? "1 comment pending" : `${n} comments pending`}</summary>
      <ol>
        {#each pending as c (c.id)}
          <li>
            <button class="to" onclick={() => jump(c)}><code>{c.path}:{c.line}</code> {c.body}</button>
            <button class="x" onclick={() => remove(c.id)} aria-label="delete the comment on {c.path} line {c.line}"><Icon name="x" size={11} /></button>
          </li>
        {/each}
      </ol>
    </details>
    <button class="primary" onclick={send} disabled={!!busy} title="every pending comment, as one turn to the agent">
      {busy ? `${busy}…` : n === 1 ? "Send 1 comment" : `Send ${n} comments`} <kbd>{spell("Mod+Enter")}</kbd>
    </button>
  </div>
{/if}
{#if said}<p class="said" role="status"><Inline text={said} /></p>{/if}

<style>
  .pending {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
  }
  .list {
    position: relative;
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .list summary {
    cursor: pointer;
  }
  .list ol {
    position: absolute;
    right: 0;
    top: 1.5rem;
    z-index: 4;
    min-width: 22rem;
    max-width: 34rem;
    max-height: 18rem;
    overflow: auto;
    margin: 0;
    padding: var(--s-1);
    list-style: none;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.15);
  }
  .list li {
    display: flex;
    align-items: center;
    gap: var(--s-1);
  }
  .to {
    flex: 1;
    min-width: 0;
    padding: 0.25rem 0.4rem;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: start;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .to:hover {
    background: var(--select);
  }
  .to code {
    font-family: var(--mono);
    color: var(--dim);
  }
  .x {
    border: 0;
    background: none;
    color: var(--faint);
    cursor: pointer;
  }
  .primary {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.7rem;
    padding: 0 0.6rem;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--chrome);
    font: inherit;
    font-size: var(--t-xs);
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
    opacity: 0.85;
  }
  .said {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--dim);
  }
</style>
