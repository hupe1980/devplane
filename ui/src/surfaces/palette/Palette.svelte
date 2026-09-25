<script lang="ts" module>
  export type Kind = "change" | "surface" | "action" | "project";
  export type Entry = { kind: Kind; label: string; hint?: string; icon: string; go: () => void };

  /// Letters in order; a lower score is a better match, `null` is no match.
  /// A prefix of the label, then the start of a word, then anywhere
  /// contiguous, then letters in order with the fewest gaps.
  export function score(q: string, text: string): number | null {
    const t = text.toLowerCase();
    if (!q) return 0;
    const at = t.indexOf(q);
    if (at === 0) return 0;
    if (at > 0) return /\W/.test(t[at - 1]) ? 1 : 2;
    let i = 0;
    let gaps = 0;
    let last = -1;
    for (const ch of q) {
      const j = t.indexOf(ch, i);
      if (j === -1) return null;
      if (last !== -1 && j !== last + 1) gaps++;
      last = j;
      i = j + 1;
    }
    return 3 + gaps;
  }

  export const ORDER: Kind[] = ["change", "surface", "action", "project"];

  /// What the palette shows for what was typed: grouped by kind in `ORDER`,
  /// best match first within each. A leading `>` narrows to actions.
  export function rank(entries: Entry[], raw: string): Entry[] {
    const typed = raw.trim().toLowerCase();
    const actions = typed.startsWith(">");
    const q = actions ? typed.slice(1).trim() : typed;
    return entries
      .filter((e) => !actions || e.kind === "action")
      .map((e) => ({ e, s: score(q, e.label) }))
      .filter((x) => x.s !== null)
      .sort((a, b) => ORDER.indexOf(a.e.kind) - ORDER.indexOf(b.e.kind) || a.s! - b.s!)
      .map((x) => x.e);
  }
</script>

<script lang="ts">
  // The command palette: changes, places, actions and projects by name,
  // floating over the current page. Fuzzy (see `score`); `>` narrows to
  // actions; every action shows its key.
  import { api } from "../../lib/api";
  import { all, run, help } from "../../lib/keys";
  import { landing, listed, surfaces } from "../../lib/surfaces";
  import { go } from "../../lib/route";
  import Icon from "../../lib/ui/Icon.svelte";

  let {
    from = "",
    changes = null,
    projects = null,
  }: {
    from?: string;
    changes?: Array<{ id: string; title: string; state?: string }> | null;
    projects?: Array<{ id: string; name: string }> | null;
  } = $props();

  let query = $state("");
  let unread = $state("");
  let fetchedChanges = $state<Array<{ id: string; title: string; state?: string }> | null>(null);
  let fetchedProjects = $state<Array<{ id: string; name: string }> | null>(null);
  $effect(() => {
    api<Array<{ id: string; title: string; state?: string }>>("/api/changes")
      .then((r) => (fetchedChanges = Array.isArray(r) ? r : []))
      .catch((e) => {
        fetchedChanges = [];
        unread = `The changes could not be read (${e instanceof Error ? e.message : String(e)}), so none are listed.`;
      });
    api<Array<{ id: string; name: string }>>("/api/projects")
      .then((r) => (fetchedProjects = Array.isArray(r) ? r : []))
      .catch(() => (fetchedProjects = []));
    // A host that did not answer is said as such, not as "no changes".
  });

  const over = $derived(from.replace(/^#/, "").split("/")[0] || landing()?.id || "");
  const opensChange = $derived(surfaces().find((s) => s.link === "change"));

  const entries = $derived.by((): Entry[] => {
    const out: Entry[] = [];
    for (const c of fetchedChanges ?? changes ?? []) {
      const to = opensChange?.id;
      if (!to) continue;
      out.push({ kind: "change", icon: "change", label: c.title || c.id, hint: c.state, go: () => go(`#${to}/${encodeURIComponent(c.id)}`) });
    }
    for (const s of listed()) {
      out.push({ kind: "surface", icon: s.icon ?? "right", label: `Go to ${s.title}`, go: () => go(`#${s.id}`) });
    }
    for (const b of help(over)) {
      if (b.action === "open-palette" || b.action === "leave") continue;
      out.push({
        kind: "action",
        icon: "keyboard",
        label: b.label.charAt(0).toUpperCase() + b.label.slice(1),
        hint: b.combo,
        go: () => {
          go(from || "");
          setTimeout(() => run(b.action, over), 0);
        },
      });
    }
    for (const p of fetchedProjects ?? projects ?? []) {
      // The landing surface narrows to a project by name: `#<landing>/<name>`.
      const to = landing()?.id;
      if (!to) continue;
      out.push({ kind: "project", icon: "folder", label: p.name, hint: "narrow to this project", go: () => go(`#${to}/${encodeURIComponent(p.name)}`) });
    }
    return out;
  });

  const TITLES: Record<Kind, string> = { change: "Changes", surface: "Go to", action: "Actions", project: "Projects" };
  /// The surface that searches every session, if one is registered.
  const searcher = $derived(surfaces().find((s) => s.takesQuery));
  const shown = $derived.by(() => {
    const raw = query.trim();
    const actions = raw.startsWith(">");
    return rank(entries, raw).concat(
      // Anything typed can also be looked for in what the agents did.
      raw && !actions && searcher
        ? [{ kind: "surface" as Kind, icon: "search", label: `Search every session for “${raw}”`, go: () => go(`#${searcher.id}/${encodeURIComponent(raw)}`) }]
        : [],
    );
  });

  let at = $state(0);
  $effect(() => {
    void query;
    at = 0;
  });
  function typed(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      at = Math.min(shown.length - 1, at + 1);
      e.preventDefault();
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      at = Math.max(0, at - 1);
      e.preventDefault();
    } else if (e.key === "Enter") {
      shown[at]?.go();
      e.preventDefault();
    }
    queueMicrotask(() => document.querySelector(".palette li[aria-selected='true']")?.scrollIntoView({ block: "nearest" }));
  }
  const bound = $derived(all().length);
</script>

<section class="palette" aria-labelledby="palette-head">
  <h2 id="palette-head" class="sr-only">Everything, by name</h2>
  <label class="field">
    <Icon name="search" size={16} />
    <!-- svelte-ignore a11y_autofocus -->
    <input bind:value={query} onkeydown={typed} placeholder="Type a change, a place, an action — or > for actions only" aria-label="find by name" autofocus />
  </label>
  {#if unread}<p class="unread" role="status">{unread}</p>{/if}
  <ul role="listbox" aria-label="matches">
    {#each shown as e, i (i)}
      {#if i === 0 || shown[i - 1].kind !== e.kind}
        <li class="sec" role="presentation">{TITLES[e.kind]}</li>
      {/if}
      <li role="option" aria-selected={i === at}>
        <button onclick={e.go} onmouseenter={() => (at = i)}>
          <Icon name={e.icon} size={14} />
          <span class="label">{e.label}</span>
          {#if e.hint}<span class="hint" class:kbd={e.kind === "action"}>{e.hint}</span>{/if}
        </button>
      </li>
    {/each}
    {#if shown.length === 0}
      <li class="none">Nothing by that name — {entries.length} things and {bound} keys are here.</li>
    {/if}
  </ul>
  <footer><span><kbd>↑</kbd><kbd>↓</kbd> move</span><span><kbd>↵</kbd> open</span><span><kbd>esc</kbd> close</span></footer>
</section>

<style>
  .palette {
    display: flex;
    flex-direction: column;
    max-height: 72vh;
  }
  .field {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border-bottom: 1px solid var(--line);
    color: var(--faint);
  }
  .field input {
    flex: 1;
    border: 0;
    box-shadow: none;
    padding: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--t-md);
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: var(--s-1) 0;
    overflow: auto;
    flex: 1;
  }
  .sec {
    padding: var(--s-2) var(--s-4) var(--s-1);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--faint);
  }
  li button {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: 100%;
    padding: 0.4rem var(--s-4);
    border: 0;
    border-radius: 0;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: var(--t-sm);
    text-align: start;
    cursor: pointer;
  }
  li[aria-selected="true"] button {
    background: var(--select);
    color: var(--ink);
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .hint {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .hint.kbd {
    font-family: var(--mono);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 0.35rem;
  }
  .none {
    padding: var(--s-4);
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .unread {
    margin: 0;
    padding: var(--s-2) var(--s-4);
    color: var(--wait);
    font-size: var(--t-xs);
    border-bottom: 1px solid var(--line);
  }
  footer {
    display: flex;
    gap: var(--s-4);
    padding: var(--s-2) var(--s-4);
    border-top: 1px solid var(--line);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 0.25rem;
    margin-right: 0.15rem;
  }
</style>
