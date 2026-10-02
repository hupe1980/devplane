<script lang="ts">
  // One file's hunks, unified or split, numbered from the hunk header. Split
  // pairs a run of removals with the following additions line by line; the
  // remainder faces an empty cell. Formatter-only hunks fold with the reason.
  // The sign is on every line in both layouts, never colour alone. A line a
  // weakened check was found on carries a glyph and the word in its gutter,
  // and a note naming the gate that runs it; a comment sits under its line.
  // Pressing a line's number starts a comment on it.
  import type { Snippet } from "svelte";
  import Icon from "../../../lib/ui/Icon.svelte";
  import Inline from "../../../lib/ui/Inline.svelte";
  import { hunkKey, type Comment, type Hunk, type Kind, type WeakRow } from "./types";

  let {
    hunks,
    mode = "unified",
    current = -1,
    expanded = false,
    /// The first hunk's number across the change: hunk ids are `hunk-<n>`.
    at = 0,
    seen = () => false,
    weak = [],
    comments = [],
    /// The line a comment is being written on, and the composer to show there.
    drafting = null,
    composer,
    onexpand,
    onpick,
    oncomment,
    ondelete,
  }: {
    hunks: Hunk[];
    mode?: "unified" | "split";
    current?: number;
    expanded?: boolean;
    at?: number;
    seen?: (h: Hunk) => boolean;
    weak?: WeakRow[];
    comments?: Comment[];
    drafting?: { side: string; line: number } | null;
    composer?: Snippet;
    onexpand?: () => void;
    onpick?: (i: number) => void;
    oncomment?: (side: "new" | "old", line: number) => void;
    ondelete?: (id: string) => void;
  } = $props();

  type Line = { kind: Kind; text: string; old: number | null; new: number | null };

  function numbered(h: Hunk): Line[] {
    const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(h.header);
    let o = m ? Number(m[1]) : 0;
    let n = m ? Number(m[2]) : 0;
    return h.lines.map(([kind, text]) => {
      if (kind === "added") return { kind, text, old: null, new: n++ };
      if (kind === "removed") return { kind, text, old: o++, new: null };
      return { kind, text, old: o++, new: n++ };
    });
  }

  type Pair = { left: Line | null; right: Line | null };
  function paired(lines: Line[]): Pair[] {
    const out: Pair[] = [];
    let i = 0;
    while (i < lines.length) {
      const l = lines[i];
      if (l.kind === "context") {
        out.push({ left: l, right: l });
        i++;
        continue;
      }
      const del: Line[] = [];
      const add: Line[] = [];
      while (i < lines.length && lines[i].kind === "removed") del.push(lines[i++]);
      while (i < lines.length && lines[i].kind === "added") add.push(lines[i++]);
      for (let k = 0; k < Math.max(del.length, add.length); k++) out.push({ left: del[k] ?? null, right: add[k] ?? null });
    }
    return out;
  }

  const sign = (k: Kind) => (k === "added" ? "+" : k === "removed" ? "−" : " ");
  /// The side and number a comment on this line is kept under.
  const place = (l: Line): { side: "new" | "old"; line: number } =>
    l.kind === "removed" ? { side: "old", line: l.old ?? 0 } : { side: "new", line: l.new ?? 0 };
  /// The weakened row found on this line: a changed line whose text is what it matched.
  const weakOn = (l: Line | null) => (l && l.kind !== "context" ? weak.find((w) => w.matched === l.text.trim()) : undefined);
  /// The comments under a line: where it was written, or where its text moved to.
  const under = (l: Line | null) => {
    if (!l) return [];
    const p = place(l);
    return comments.filter((c) => c.side === p.side && (c.anchor.is === "moved" ? c.anchor.line : c.line) === p.line && c.anchor.is !== "changed");
  };
  const isDrafting = (l: Line | null) => !!l && !!drafting && drafting.side === place(l).side && drafting.line === place(l).line;
  /// A line number pressed: a comment on that line.
  function pressed(e: MouseEvent) {
    const cell = (e.target as HTMLElement).closest("td.no[data-line]") as HTMLElement | null;
    if (!cell || !oncomment) return;
    oncomment(cell.dataset.side === "old" ? "old" : "new", Number(cell.dataset.line));
  }
  const when = (c: Comment) =>
    c.anchor.is === "changed"
      ? `changed — written against: ${c.line_text}`
      : c.anchor.is === "moved"
        ? `moved from line ${c.line}`
        : "";
</script>

{#snippet notes(l: Line | null, span: number)}
  {@const w = weakOn(l)}
  {#if w}
    <tr class="note weak"><td colspan={span}><Icon name="alert" size={12} /> <b>check weakened here</b> — <Inline text={w.why} />{#if w.gate_says}; <Inline text={w.gate_says} />{/if}{w.seen ? " · read" : " · not yet read"}</td></tr>
  {/if}
  {#each under(l) as c (c.id)}
    <tr class="note comment" class:sent={!!c.sent_at}>
      <td colspan={span}>
        <span class="who"><Icon name="person" size={12} /> {c.sent_at ? "sent" : "pending"}</span>
        <span class="body">{c.body}</span>
        {#if when(c)}<span class="moved">{when(c)}</span>{/if}
        {#if c.hunk_changed !== null && c.hunk_changed !== undefined}<span class="outcome">{c.hunk_changed ? "the agent changed this hunk since" : "not changed since"}</span>{/if}
        {#if !c.sent_at && ondelete}<button class="del-c" onclick={() => ondelete?.(c.id)} aria-label="delete this comment">delete</button>{/if}
      </td>
    </tr>
  {/each}
  {#if isDrafting(l) && composer}
    <tr class="note draft"><td colspan={span}>{@render composer()}</td></tr>
  {/if}
{/snippet}

{#each hunks as h, i (hunkKey(h, i))}
  {@const m = seen(h)}
  <section class="hunk" class:current={i === current} id="hunk-{at + i}" data-hunk={at + i}>
    <button class="hh" onclick={() => onpick?.(i)}>
      <code>{h.header}</code>
      {#if m}<span class="mark seen" title="marked seen; kept by Devplane until this hunk changes"><Icon name="eye" size={12} /> seen</span>{/if}
    </button>
    {#if h.formatter_only && !expanded}
      <button class="folded" onclick={onexpand}>
        <Icon name="right" size={12} /> Formatting only — {h.lines.length} lines whose text is unchanged apart from whitespace. Show them.
      </button>
    {:else if mode === "unified"}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <table class="code unified" onclick={pressed}>
        <tbody>
          {#each numbered(h) as l, li (li)}
            {@const p = place(l)}
            <tr class={l.kind} class:weakline={!!weakOn(l)}>
              <td class="no" data-side={p.side} data-line={p.line} title="comment on this line">{l.old ?? ""}</td>
              <td class="no" data-side={p.side} data-line={p.line} title="comment on this line">{l.new ?? ""}</td>
              <td class="sg">{weakOn(l) ? "⚠" : sign(l.kind)}</td>
              <td class="tx">{l.text}</td>
            </tr>
            {@render notes(l, 4)}
          {/each}
        </tbody>
      </table>
    {:else}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
      <table class="code split" onclick={pressed}>
        <tbody>
          {#each paired(numbered(h)) as pr, pi (pi)}
            <tr class:weakline={!!weakOn(pr.left) || !!weakOn(pr.right)}>
              <td class="no {pr.left?.kind ?? 'none'}" data-side="old" data-line={pr.left?.old ?? ""}>{pr.left?.old ?? ""}</td>
              <td class="sg {pr.left?.kind ?? 'none'}">{pr.left ? (weakOn(pr.left) ? "⚠" : sign(pr.left.kind)) : ""}</td>
              <td class="tx {pr.left?.kind === 'removed' ? 'removed' : pr.left ? 'context' : 'none'}">{pr.left?.text ?? ""}</td>
              <td class="no {pr.right?.kind ?? 'none'}" data-side="new" data-line={pr.right?.new ?? ""}>{pr.right?.new ?? ""}</td>
              <td class="sg {pr.right?.kind ?? 'none'}">{pr.right ? (weakOn(pr.right) ? "⚠" : sign(pr.right.kind)) : ""}</td>
              <td class="tx {pr.right?.kind === 'added' ? 'added' : pr.right ? 'context' : 'none'}">{pr.right?.text ?? ""}</td>
            </tr>
            {@render notes(pr.right ?? pr.left, 6)}
            {#if pr.left && pr.right && pr.left !== pr.right}{@render notes(pr.left, 6)}{/if}
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/each}

<style>
  .hunk {
    /* Moved to, it stops below its file's strip, never under it. */
    scroll-margin-top: 2.4rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    margin-bottom: var(--s-3);
    background: var(--bg);
  }
  .hunk.current {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .hh {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    width: 100%;
    padding: 0.15rem var(--s-3);
    border: 0;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    color: var(--faint);
    font: inherit;
    cursor: pointer;
    text-align: start;
  }
  .hh code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .mark {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: var(--t-xs);
  }
  /* A person's mark, not a verification: never the verified green. */
  .mark.seen {
    color: var(--dim);
  }
  .folded {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    width: 100%;
    padding: var(--s-2) var(--s-3);
    border: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
    text-align: start;
  }
  .code {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    font-family: var(--mono);
    font-size: 0.78rem;
    line-height: 1.55;
  }
  .unified .no {
    width: 3.2rem;
  }
  .sg {
    width: 1.3rem;
    text-align: center;
    color: var(--faint);
  }
  .split .no {
    width: 3.2rem;
  }
  .no {
    padding: 0 0.5rem;
    text-align: end;
    color: var(--faint);
    user-select: none;
    vertical-align: top;
    border-right: 1px solid var(--line);
    cursor: pointer;
  }
  .no:hover {
    color: var(--accent);
  }
  .tx {
    padding: 0 var(--s-3);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--ink);
    vertical-align: top;
  }
  tr.added td,
  td.added {
    background: var(--add-bg);
  }
  tr.removed td,
  td.removed {
    background: var(--del-bg);
  }
  tr.added .sg,
  td.sg.added,
  td.no.added {
    color: var(--add);
  }
  tr.removed .sg,
  td.sg.removed,
  td.no.removed {
    color: var(--del);
  }
  td.none {
    background: var(--panel);
  }
  .split .tx:nth-child(3) {
    border-right: 1px solid var(--line);
  }
  /* The weakened line: the warning weight in the gutter, glyph and word. */
  tr.weakline .sg {
    color: var(--fail);
    font-weight: 700;
    box-shadow: inset 3px 0 0 var(--fail);
  }
  tr.note td {
    padding: 0.25rem var(--s-3) 0.3rem calc(6.4rem + var(--s-3));
    font-family: var(--font);
    font-size: var(--t-xs);
    line-height: 1.45;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
  }
  tr.note.weak td {
    color: var(--fail);
    background: var(--panel);
  }
  tr.note.comment td {
    background: var(--raise, var(--panel));
    color: var(--ink);
  }
  tr.note.comment.sent td {
    color: var(--dim);
  }
  .who {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    margin-right: var(--s-2);
    color: var(--accent);
    font-weight: 600;
  }
  .sent .who {
    color: var(--dim);
  }
  .body {
    white-space: pre-wrap;
  }
  .moved,
  .outcome {
    margin-left: var(--s-2);
    color: var(--wait);
  }
  .del-c {
    margin-left: var(--s-2);
    padding: 0 0.3rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  tr.note.draft td {
    background: var(--panel);
  }
</style>
