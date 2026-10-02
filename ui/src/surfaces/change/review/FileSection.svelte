<script lang="ts">
  // One file in the review's column: its facts as one strip that stays at the
  // top while its hunks scroll, then its hunks. Off screen, the browser skips
  // laying it out (`content-visibility`), sized from its line count, so a
  // forty-file change moves hunk to hunk as fast as a one-file change.
  import type { Snippet } from "svelte";
  import Icon from "../../../lib/ui/Icon.svelte";
  import Inline from "../../../lib/ui/Inline.svelte";
  import Diff from "./Diff.svelte";
  import { isSeen, unitSeen, unreadIn } from "./unread.svelte";
  import type { Comment, File, Hunk, WeakRow } from "./types";

  let {
    change,
    file,
    at,
    current,
    mode,
    expanded,
    weak = [],
    comments = [],
    drafting = null,
    composer,
    pick,
    expand,
    comment,
    remove,
    readRows,
  }: {
    change: string;
    file: File;
    /// The number of this file's first hunk across the change.
    at: number;
    /// The cursor, across the change.
    current: number;
    mode: "unified" | "split";
    expanded: boolean;
    weak?: WeakRow[];
    comments?: Comment[];
    drafting?: { side: string; line: number } | null;
    composer?: Snippet;
    pick: (n: number) => void;
    expand: () => void;
    comment: (side: "new" | "old", line: number) => void;
    remove: (id: string) => void;
    /// Marks every unit at this path seen: *I have read this*.
    readRows: () => void;
  } = $props();

  const unread = $derived(unreadIn(change, file));
  const unreadWeak = $derived(weak.filter((w) => !w.seen));
  const lines = $derived(file.hunks.reduce((n, h) => n + h.lines.length + 2, 0));
  /// The comments whose line is gone, said at the file rather than lost.
  const adrift = $derived(comments.filter((c) => c.anchor.is === "changed"));
  const covered = $derived(file.coverage?.coverage === "covered");
  /// Over a thousand changed lines: collapsed in place, counted, expandable.
  const huge = $derived(file.added + file.removed > 1000);
  /// The strip's facts in one run, in the words the host composed.
  const facts = $derived(
    [file.status_says, file.role_says, file.coverage_says ?? "no mapping declared", file.task_says, file.decisions.length ? `${file.decisions.length} ${file.decisions.length === 1 ? "decision" : "decisions"} while writing` : ""]
      .filter(Boolean)
      .map((x) => x.replace(/`/g, ""))
      .join(" · "),
  );
</script>

<section class="file" id="file-{at}" data-path={file.path} style="contain-intrinsic-size: auto {huge && !expanded ? 90 : lines * 19 + 48}px">
  <header class="strip" class:weak={unreadWeak.length > 0}>
    <code class="path" title={file.path}>{file.path}</code>
    <span class="add">+{file.added}</span><span class="del">−{file.removed}</span>
    {#if weak.length}
      <span class="weakw"><Icon name="alert" size={12} /> check weakened{unreadWeak.length ? "" : " · read"}</span>
    {/if}
    {#if file.changed_since_seen}<span class="moved">changed since you marked it</span>{/if}
    <!-- The facts in one run, cut at the strip's end; whole in *facts*. -->
    <span class="facts" title={facts}>{facts}</span>
    <span class="unread">{unread ? `${unread} unread` : file.hunks.length || file.unit_seen != null ? "all seen" : ""}</span>
    {#if unreadWeak.length || (!file.hunks.length && file.unit_seen === false && !unitSeen(change, file))}
      <button class="read" onclick={readRows}><Icon name="eye" size={12} /> I have read this</button>
    {/if}
    <details class="check">
      <summary>facts</summary>
      <dl class="card">
        <dt>Status</dt><dd><Inline text={file.status_says} /></dd>
        <dt>Role</dt><dd><Inline text={file.role_says} /></dd>
        <dt>Covered by</dt><dd class:quiet={!covered}><Inline text={file.coverage_says ?? "no mapping declared"} /></dd>
        {#if file.task_says}<dt>Asked for</dt><dd class:wait={file.task_says.startsWith("not asked")}><Inline text={file.task_says} /></dd>{/if}
        {#if file.decisions.length}<dt>Decided while writing</dt><dd>{file.decisions.map((x) => x.says).join(" · ")}</dd>{/if}
        {#each weak as w (w.why + w.matched)}<dt class="wait">Check weakened</dt><dd><Inline text={w.why} />{#if w.gate_says}; <Inline text={w.gate_says} />{/if}</dd>{/each}
        {#if file.marker_commands.length}<dt>Check it yourself</dt><dd class="cmds">{#each file.marker_commands as c (c)}<code>{c}</code>{/each}</dd>{/if}
      </dl>
    </details>
  </header>
  {#if file.body_says}<p class="finding"><Inline text={file.body_says} /></p>{/if}
  {#each adrift as c (c.id)}
    <p class="adrift"><b>{c.sent_at ? "sent" : "pending"}</b> on line {c.line} — the line changed; written against: <code>{c.line_text}</code> — {c.body}</p>
  {/each}
  {#if huge && !expanded}
    <!-- A generated file: folded with its count, so moving elsewhere stays fast. -->
    <button class="huge" onclick={expand}>{file.added + file.removed} changed lines in {file.hunks.length} {file.hunks.length === 1 ? "hunk" : "hunks"} — collapsed. Show them <kbd>x</kbd></button>
  {:else}
  <div class="hunks">
    <Diff
      hunks={file.hunks}
      {mode}
      {at}
      current={current - at}
      {expanded}
      seen={(h: Hunk) => isSeen(change, file.path, h)}
      {weak}
      {comments}
      {drafting}
      {composer}
      onexpand={expand}
      onpick={(i) => pick(at + i)}
      oncomment={comment}
      ondelete={remove}
    />
  </div>
  {/if}
</section>

<style>
  .file {
    content-visibility: auto;
    margin-bottom: var(--s-3);
  }
  .strip {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: var(--s-2);
    min-width: 0;
    padding: 0.3rem var(--s-2);
    margin: 0 calc(-1 * var(--s-2)) var(--s-2);
    background: var(--bg);
    border-bottom: 1px solid var(--line);
    font-size: var(--t-xs);
    white-space: nowrap;
    /* Cut sideways, never downwards: the commands open below the strip. */
    overflow-x: clip;
    overflow-y: visible;
  }
  .strip.weak {
    box-shadow: inset 3px 0 0 var(--fail);
  }
  .path {
    font-family: var(--mono);
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink);
    flex: 0 0 auto;
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    /* A long path loses its start, never its file name. */
    direction: rtl;
    text-align: left;
  }
  .facts {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--dim);
  }
  .moved,
  .wait {
    color: var(--wait);
    flex: none;
  }
  .quiet {
    color: var(--faint);
  }
  .add {
    color: var(--add);
    font-family: var(--mono);
  }
  .del {
    color: var(--del);
    font-family: var(--mono);
  }
  .weakw {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--fail);
    font-weight: 600;
    flex: none;
  }
  .unread {
    color: var(--dim);
    font-variant-numeric: tabular-nums;
    flex: none;
  }
  .read {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    height: 1.4rem;
    padding: 0 0.45rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    cursor: pointer;
    flex: none;
  }
  .check {
    flex: none;
    position: relative;
  }
  .check summary {
    cursor: pointer;
    color: var(--dim);
  }
  .check[open] .card {
    position: absolute;
    right: 0;
    top: 1.4rem;
    z-index: 3;
    display: grid;
    grid-template-columns: max-content minmax(0, 28rem);
    gap: 0.2rem var(--s-3);
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.15);
    white-space: normal;
  }
  .card dt {
    color: var(--faint);
  }
  .card dd {
    margin: 0;
    color: var(--ink);
  }
  .cmds {
    display: grid;
    gap: 0.2rem;
  }
  .cmds code {
    font-family: var(--mono);
    overflow-wrap: anywhere;
  }
  .huge {
    width: 100%;
    padding: var(--s-2) var(--s-3);
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-sm);
    text-align: start;
    cursor: pointer;
  }
  .finding,
  .adrift {
    margin: 0 0 var(--s-2);
    font-size: var(--t-xs);
    color: var(--wait);
  }
  .adrift code {
    font-family: var(--mono);
  }
</style>
