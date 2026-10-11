<script lang="ts" module>
  /// One edit an agent reported for one tool call, as the ledger row carries it.
  export type Edit = {
    call_id?: string;
    path: string;
    old_text?: string | null;
    new_text: string;
    omitted_bytes?: number;
    says?: string;
  };
</script>

<script lang="ts">
  // The edit a driven call reported, shown beside the decision about that call.
  // The agent's own claim, quoted as text: nothing in it becomes a link, markup
  // or an action, and it is never what a gate or *verified* reads — the
  // worktree is.
  let { edits, none = null }: { edits: Edit[] | null | undefined; none?: string | null } = $props();

  /// Lines of one side, each marked as the agent said it was removed or added.
  const lines = (text: string | null | undefined, mark: string) =>
    (text ?? "")
      .replace(/\n$/, "")
      .split("\n")
      .filter((_, __, all) => all.length > 1 || all[0] !== "")
      .map((l) => `${mark} ${l}`);
  const short = (p: string) => p.split("/").slice(-3).join("/");
</script>

{#if edits && edits.length > 0}
  <section class="edits" aria-label="the agent's report of its edit">
    <p class="says">{edits[0].says ?? "the agent's report of its edit"} — the worktree, not this, is what the gates read</p>
    {#each edits as e, i (`${e.path}-${i}`)}
      <figure>
        <figcaption title={e.path}>{short(e.path)}{e.old_text == null ? " · new file" : ""}</figcaption>
        <pre>{#each lines(e.old_text, "-") as l}<span class="del">{l}</span>{"\n"}{/each}{#each lines(e.new_text, "+") as l}<span class="add">{l}</span>{"\n"}{/each}</pre>
        {#if (e.omitted_bytes ?? 0) > 0}<p class="cut">{e.omitted_bytes} bytes not kept</p>{/if}
      </figure>
    {/each}
  </section>
{:else if none}
  <p class="none">{none}</p>
{/if}

<style>
  .edits {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    margin-top: var(--s-2);
  }
  .says,
  .none,
  .cut {
    margin: 0;
    color: var(--faint);
    font-size: var(--t-xs);
  }
  figure {
    margin: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  figcaption {
    padding: var(--s-1) var(--s-2);
    font-family: var(--mono);
    font-size: var(--t-xs);
    color: var(--dim);
    border-bottom: 1px solid var(--line);
  }
  pre {
    margin: 0;
    padding: var(--s-2);
    max-height: 18rem;
    overflow: auto;
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre;
  }
  .del {
    color: var(--del);
    background: var(--del-bg);
  }
  .add {
    color: var(--add);
    background: var(--add-bg);
  }
  .cut {
    padding: 0 var(--s-2) var(--s-1);
  }
</style>
