<script lang="ts">
  // What a region says when it has nothing to show: one sentence of fact, at
  // most one next step. `limit` — what this view cannot see — renders apart,
  // so *nothing here* and *nothing visible here* never look alike.
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    icon = "dot",
    title,
    body = "",
    limit = "",
    action,
  }: { icon?: string; title: string; body?: string; limit?: string; action?: Snippet } = $props();
</script>

<div class="empty">
  <span class="glyph"><Icon name={icon} size={22} /></span>
  <p class="title">{title}</p>
  {#if body}<p class="body">{body}</p>{/if}
  {#if limit}<p class="limit"><Icon name="eye" size={13} /> {limit}</p>{/if}
  {#if action}<div class="action">{@render action()}</div>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: var(--s-2);
    padding: var(--s-7) var(--s-5);
    color: var(--dim);
  }
  .glyph {
    color: var(--faint);
    display: grid;
    place-items: center;
    width: 2.75rem;
    height: 2.75rem;
    border-radius: 50%;
    background: var(--panel);
    border: 1px solid var(--line);
  }
  p {
    margin: 0;
    max-width: 34rem;
  }
  .title {
    color: var(--ink);
    font-weight: 600;
    font-size: var(--t-md);
  }
  .body {
    font-size: var(--t-sm);
  }
  .limit {
    font-size: var(--t-xs);
    color: var(--wait);
  }
  .action {
    margin-top: var(--s-2);
  }
</style>
