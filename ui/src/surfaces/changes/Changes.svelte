<script lang="ts">
  // What a Work actually changed.
  //
  // **Rendered from the structured change set, never from HTML.** The daemon
  // used to send a rendered `html` field beside it, for a page that no longer
  // exists; this surface reads the parsed shape, because `{@html}` is the one
  // thing that would put an outside value into the document unescaped and no
  // surface here uses it. A diff is the densest concentration of somebody
  // else's text this product renders — file names, commit content, whatever an
  // agent wrote — so it is the worst possible place to make an exception.
  import { api } from "../../lib/api";
  import { plural } from "../../lib/text";

  type Kind = "context" | "added" | "removed";
  type Hunk = { header: string; lines: [Kind, string][] };
  type Body =
    | { hunks: Hunk[] }
    | { binary: { bytes: number | null } }
    | { skipped: { why: string } };
  type Status = "added" | "modified" | "deleted" | { renamed: { from: string } };
  type FileChange = { path: string; status: Status; added: number; removed: number; body: Body };
  type Truncation = { files_shown: number; files_total: number; command: string };
  type ChangeSet = { base: string; files: FileChange[]; truncated?: Truncation | null };

  type Work = { id: string; title?: string | null };

  let { works = [] }: { works?: Work[] } = $props();

  /// Which Work is being read. Nothing is fetched until one is chosen: a diff
  /// is a `git` call per Work, and a surface that fetches every one on open
  /// would run the whole list to render a heading.
  let chosen = $state("");
  let set = $state<ChangeSet | null>(null);
  let said = $state("");
  let loading = $state(false);

  async function load(id: string) {
    chosen = id;
    set = null;
    said = "";
    if (!id) return;
    loading = true;
    try {
      const body = await api<{ changes: ChangeSet }>(
        `/api/work/${encodeURIComponent(id)}/changes`,
      );
      set = body.changes;
    } catch (e) {
      // **The checkout being gone is not the same as nothing having changed**,
      // and the daemon says which. Passing its sentence through beats inventing
      // a friendlier one that loses the distinction.
      said = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  const totals = $derived(
    (set?.files ?? []).reduce(
      (acc, f) => ({ added: acc.added + f.added, removed: acc.removed + f.removed }),
      { added: 0, removed: 0 },
    ),
  );

  function statusWord(s: Status): string {
    return typeof s === "string" ? s : `renamed from ${s.renamed.from}`;
  }

  function hunks(b: Body): Hunk[] {
    return "hunks" in b ? b.hunks : [];
  }
</script>

<section aria-labelledby="changes-head">
  <h2 id="changes-head">What changed</h2>

  <p class="pick">
    <label for="which">Work</label>
    <select id="which" value={chosen} onchange={(e) => load(e.currentTarget.value)}>
      <option value="">choose one</option>
      {#each works as w (w.id)}
        <option value={w.id}>{w.title ?? w.id}</option>
      {/each}
    </select>
  </p>

  <p class="said" role="status" aria-live="polite">{said}</p>

  {#if loading}
    <p class="dim">reading the checkout…</p>
  {:else if !chosen}
    <p class="dim">Pick a Work to see the change it made against its base branch.</p>
  {:else if set}
    <p class="base">
      against <code>{set.base}</code> ·
      <b class="add">+{totals.added}</b>
      <b class="del">−{totals.removed}</b>
      · {plural(set.files.length, "file")}
    </p>

    {#if set.files.length === 0}
      <!-- **A finding, not an empty state.** A gate that passed over no change
           verified nothing, and a reviewer told "no diff" has been told
           something quite different from "nothing to show". -->
      <p class="finding">
        This branch changed nothing against <code>{set.base}</code>. Any check that passed here
        passed over no change.
      </p>
    {/if}

    {#each set.files as f (f.path)}
      <article class="file">
        <h3>
          <span class="path">{f.path}</span>
          <span class="status">{statusWord(f.status)}</span>
          <span class="counts"><b class="add">+{f.added}</b> <b class="del">−{f.removed}</b></span>
        </h3>

        {#if "binary" in f.body}
          <p class="dim">
            binary{f.body.binary.bytes === null ? "" : `, ${f.body.binary.bytes} bytes`}
          </p>
        {:else if "skipped" in f.body}
          <!-- *Nothing to show* and *we chose not to show it* are different
               sentences, and a reviewer deciding whether to approve is entitled
               to know which one they are reading. -->
          <p class="dim">not shown — {f.body.skipped.why}</p>
        {:else}
          {#each hunks(f.body) as h (h.header)}
            <pre class="hunk"><code><span class="hdr">{h.header}</span>
{#each h.lines as [kind, text], i (i)}<span class={kind}>{kind === "added" ? "+" : kind === "removed" ? "−" : " "}{text}</span>
{/each}</code></pre>
          {/each}
        {/if}
      </article>
    {/each}

    {#if set.truncated}
      <!-- The exact command that shows the whole thing. A reviewer told only
           that something is missing has been told half of what they need. -->
      <p class="finding">
        Showing {set.truncated.files_shown} of {set.truncated.files_total} files.
        For all of it: <code>{set.truncated.command}</code>
      </p>
    {/if}
  {/if}
</section>

<style>
  h2 { font-size: var(--t-lg); margin: 0 0 var(--s-3); }
  .pick { display: flex; align-items: center; gap: var(--s-2); margin-bottom: var(--s-3); }
  .pick label { color: var(--dim); font-size: var(--t-sm); }
  .said { color: var(--fail); font-size: var(--t-sm); margin: 0 0 var(--s-2); }
  .said:empty { display: none; }
  .dim { color: var(--dim); }

  .base { color: var(--dim); font-size: var(--t-sm); margin-bottom: var(--s-4); }
  .add { color: var(--done); }
  .del { color: var(--fail); }

  .finding {
    border: 1px solid var(--wait);
    border-radius: var(--radius);
    padding: var(--s-3) var(--s-4);
    margin: var(--s-3) 0;
    max-width: 70ch;
  }

  .file {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    margin-bottom: var(--s-3);
    overflow: hidden;
  }
  .file h3 {
    display: flex;
    align-items: baseline;
    gap: var(--s-3);
    margin: 0;
    padding: var(--s-2) var(--s-3);
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  .path { font-family: var(--mono); font-weight: 600; word-break: break-all; }
  .status { color: var(--dim); font-size: var(--t-xs); }
  .counts { margin-left: auto; font-variant-numeric: tabular-nums; flex: none; }

  .hunk {
    margin: 0;
    padding: var(--s-2) 0;
    overflow-x: auto;
    font-size: var(--t-xs);
    line-height: 1.45;
  }
  .hunk code { display: block; }
  /* A line's own background, so a long line stays marked to its right edge
     rather than losing its colour where the text stops. */
  .hunk span { display: block; padding: 0 var(--s-3); white-space: pre; }
  .hdr { color: var(--dim); }
  /* **The sign carries it and the colour helps.** Red and green are the one
     pair that cannot be separated under deuteranopia, and a diff is exactly
     where that matters — so every line starts with `+`, `−` or a space. */
  .added { color: var(--done); }
  .removed { color: var(--fail); }
  .context { color: var(--dim); }
</style>
