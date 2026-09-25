<script lang="ts">
  // One file's hunks, unified or split, numbered from the hunk header. Split
  // pairs a run of removals with the following additions line by line; the
  // remainder faces an empty cell. Formatter-only hunks fold with the reason.
  // The sign is on every line in both layouts, never colour alone.
  import Icon from "../../lib/ui/Icon.svelte";
  import type { Hunk, Kind } from "./types";
  import type { Mark } from "./marks";

  let {
    hunks,
    mode = "unified",
    current = -1,
    expanded = false,
    mark,
    onexpand,
    onpick,
  }: {
    hunks: Hunk[];
    mode?: "unified" | "split";
    current?: number;
    expanded?: boolean;
    mark: (h: Hunk) => Mark | null;
    onexpand?: () => void;
    onpick?: (i: number) => void;
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
</script>

{#each hunks as h, i (h.header + i)}
  {@const m = mark(h)}
  <section class="hunk" class:current={i === current} id="hunk-{i}">
    <button class="hh" onclick={() => onpick?.(i)}>
      <code>{h.header}</code>
      {#if m}<span class="mark {m}"><Icon name={m === "accepted" ? "check" : "eye"} size={12} /> {m}</span>{/if}
    </button>
    {#if h.formatter_only && !expanded}
      <button class="folded" onclick={onexpand}>
        <Icon name="right" size={12} /> Formatting only — {h.lines.length} lines whose text is unchanged apart from whitespace. Show them.
      </button>
    {:else if mode === "unified"}
      <table class="code unified">
        <tbody>
          {#each numbered(h) as l, li (li)}
            <tr class={l.kind}>
              <td class="no">{l.old ?? ""}</td>
              <td class="no">{l.new ?? ""}</td>
              <td class="sg">{sign(l.kind)}</td>
              <td class="tx">{l.text}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <table class="code split">
        <tbody>
          {#each paired(numbered(h)) as p, pi (pi)}
            <tr>
              <td class="no {p.left?.kind ?? 'none'}">{p.left?.old ?? ""}</td>
              <td class="sg {p.left?.kind ?? 'none'}">{p.left ? sign(p.left.kind) : ""}</td>
              <td class="tx {p.left?.kind === 'removed' ? 'removed' : p.left ? 'context' : 'none'}">{p.left?.text ?? ""}</td>
              <td class="no {p.right?.kind ?? 'none'}">{p.right?.new ?? ""}</td>
              <td class="sg {p.right?.kind ?? 'none'}">{p.right ? sign(p.right.kind) : ""}</td>
              <td class="tx {p.right?.kind === 'added' ? 'added' : p.right ? 'context' : 'none'}">{p.right?.text ?? ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/each}

<style>
  .hunk {
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
    padding: 0.25rem var(--s-3);
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
  /* A mark in this browser, not a verification: never the verified green. */
  .mark.accepted {
    color: var(--accent);
  }
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
</style>
