<script lang="ts" module>
  import { tick } from "svelte";
  import { run, spell, type Binding } from "../../lib/keys";
  import { go } from "../../lib/route";
  import type { Surface } from "../../lib/surfaces";
  import type { Command } from "../../lib/offered.svelte";
  export type Kind = "here" | "change" | "surface" | "action" | "project";
  /// `id` names the command, not the row: two rows that would do the same
  /// thing share it, and the palette lists the first of them once.
  export type Entry = { id: string; kind: Kind; label: string; hint?: string; icon: string; go: () => void; qualifier?: import("../../wire/Qualifier").Qualifier | null };

  /// Each command once, in the order first listed; a later row with the same
  /// id lends the kept one its key if that one has none.
  export function unique(entries: Entry[]): Entry[] {
    const kept = new Map<string, Entry>();
    for (const e of entries) {
      const have = kept.get(e.id);
      if (!have) kept.set(e.id, e);
      else if (!have.hint && e.hint) kept.set(e.id, { ...have, hint: e.hint });
    }
    return [...kept.values()];
  }

  /// Focuses a field once it is on the page. `autofocus` only takes when
  /// nothing else has focus, and the palette opens over a focused page.
  export function focusOnMount(node: HTMLElement): void {
    void tick().then(() => node.focus());
  }

  /// A lower score is a better match, `null` is no match. A prefix of the
  /// label, then the start of a word, then anywhere contiguous, then letters
  /// that each continue the last one or start a word (*rlr* for *Rate-limit
  /// the login route*), fewest jumps first. Letters scattered inside words
  /// match nothing: *rate* is not *show or hide the activity panel*.
  export function score(q: string, text: string): number | null {
    const t = text.toLowerCase();
    if (!q) return 0;
    const at = t.indexOf(q);
    if (at === 0) return 0;
    if (at > 0) return /\W/.test(t[at - 1]) ? 1 : 2;
    const starts = (k: number) => k === 0 || /[^\p{L}\p{N}]/u.test(t[k - 1]);
    let last = -1;
    let jumps = 0;
    for (const ch of q.replace(/\s+/g, "")) {
      if (last !== -1 && t[last + 1] === ch) {
        last += 1;
        continue;
      }
      let j = -1;
      for (let k = last + 1; k < t.length; k++)
        if (t[k] === ch && starts(k)) {
          j = k;
          break;
        }
      if (j === -1) return null;
      jumps += 1;
      last = j;
    }
    return 3 + jumps;
  }

  /// The list keys move a list on the page under the palette; they are not
  /// commands to run from it.
  const LIST_ACTIONS = new Set(["next", "prev", "first", "last", "open"]);

  /// Every row the palette can show, each command once: the page's own
  /// actions, the changes, the places, the keys bound where it was opened,
  /// the projects. A key that goes to a listed place (*go to the sessions*,
  /// `g b`) is that place's row with its chord beside it, not a second row.
  export function commands(on: {
    here: Command[];
    changes: Array<{ id: string; title: string; state?: string; qualifier?: import("../../wire/Qualifier").Qualifier | null }>;
    opensChange: string | undefined;
    places: Surface[];
    keys: Binding[];
    projects: Array<{ id: string; name: string }>;
    landing: string | undefined;
    over: string;
    back: () => void;
  }): Entry[] {
    const out: Entry[] = [];
    // The page under the palette's own actions, each run by that page (a
    // change offers itself only through its own guard).
    for (const c of on.here) {
      out.push({
        id: `here:${c.label}`,
        kind: "here",
        icon: c.icon,
        label: c.label,
        hint: c.hint,
        go: () => {
          on.back();
          setTimeout(c.run, 0);
        },
      });
    }
    for (const c of on.changes) {
      const to = on.opensChange;
      if (!to) continue;
      out.push({ id: `change:${c.id}`, kind: "change", icon: "change", label: c.title || c.id, hint: c.state, qualifier: c.qualifier, go: () => go(`#${to}/${encodeURIComponent(c.id)}`) });
    }
    const place = (title: string) => `go to the ${title.toLowerCase()}`;
    for (const s of on.places) {
      const key = on.keys.find((b) => b.surface === "global" && b.label === place(s.title));
      out.push({ id: `place:${s.id}`, kind: "surface", icon: s.icon ?? "right", label: `Go to ${s.title}`, hint: key ? spell(key.combo) : undefined, go: () => go(`#${s.id}`) });
    }
    for (const b of on.keys) {
      if (b.action === "open-palette" || b.action === "leave" || LIST_ACTIONS.has(b.action)) continue;
      const to = b.surface === "global" ? on.places.find((s) => b.label === place(s.title)) : undefined;
      out.push({
        id: to ? `place:${to.id}` : `action:${b.action}`,
        kind: "action",
        icon: "keyboard",
        label: b.label.charAt(0).toUpperCase() + b.label.slice(1),
        hint: spell(b.combo),
        go: () => {
          on.back();
          setTimeout(() => run(b.action, on.over), 0);
        },
      });
    }
    for (const p of on.projects) {
      // The landing surface narrows to a project by name: `#<landing>/<name>`.
      const to = on.landing;
      if (!to) continue;
      out.push({ id: `project:${p.id}`, kind: "project", icon: "folder", label: p.name, hint: "narrow to this project", go: () => go(`#${to}/${encodeURIComponent(p.name)}`) });
    }
    return unique(out);
  }

  export const ORDER: Kind[] = ["here", "change", "surface", "action", "project"];

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
  import ChangeState from "../../lib/ChangeState.svelte";
  import { here } from "../../lib/offered.svelte";
  // The command palette: changes, places, actions and projects by name,
  // floating over the current page. Fuzzy (see `score`); `>` narrows to
  // actions; every action and every place with a key shows it.
  import { resource } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import { all, help } from "../../lib/keys";
  import { landing, listed, surfaces } from "../../lib/surfaces";
  import Icon from "../../lib/ui/Icon.svelte";

  let {
    from = "",
    changes = null,
    projects = null,
  }: {
    from?: string;
    changes?: Array<{ id: string; title: string; state?: string; qualifier?: import("../../wire/Qualifier").Qualifier | null }> | null;
    projects?: Array<{ id: string; name: string }> | null;
  } = $props();

  let query = $state("");
  // A host that did not answer is said as such, not as "no changes".
  const changesRead = resource<Array<{ id: string; title: string; state?: string; qualifier?: import("../../wire/Qualifier").Qualifier | null }>>(
    () => "/api/changes",
    { tell: () => "devplane change list" },
  );
  const projectsRead = resource<Array<{ id: string; name: string }>>(() => "/api/projects", { tell: () => "devplane doctor" });
  const fetchedChanges = $derived(Array.isArray(changesRead.data) ? changesRead.data : null);
  const fetchedProjects = $derived(Array.isArray(projectsRead.data) ? projectsRead.data : null);
  const over = $derived(from.replace(/^#/, "").split("/")[0] || landing()?.id || "");
  const opensChange = $derived(surfaces().find((s) => s.link === "change"));

  const entries = $derived(
    commands({
      here: here.list,
      changes: fetchedChanges ?? changes ?? [],
      opensChange: opensChange?.id,
      places: listed(),
      keys: help(over),
      projects: fetchedProjects ?? projects ?? [],
      landing: landing()?.id,
      over,
      back: () => go(from || ""),
    }),
  );

  const TITLES = $derived<Record<Kind, string>>({ here: here.title ? `This change: ${here.title}` : "Here", change: "Changes", surface: "Go to", action: "Actions", project: "Projects" });
  /// The surface that searches every session, if one is registered.
  const searcher = $derived(surfaces().find((s) => s.takesQuery));
  const shown = $derived.by(() => {
    const raw = query.trim();
    const actions = raw.startsWith(">");
    return rank(entries, raw).concat(
      // Anything typed can also be looked for in what the agents did.
      raw && !actions && searcher
        ? [{ id: "search", kind: "surface" as Kind, icon: "search", label: `Search every session for “${raw}”`, go: () => go(`#${searcher.id}/${encodeURIComponent(raw)}`) }]
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
    <input
      bind:value={query}
      onkeydown={typed}
      placeholder="Type a change, a place, an action — or > for actions only"
      aria-label="find by name"
      role="combobox"
      aria-expanded="true"
      aria-controls="palette-matches"
      aria-autocomplete="list"
      aria-activedescendant={shown[at] ? `palette-opt-${at}` : undefined}
      use:focusOnMount
    />
  </label>
  {#if changesRead.failure && !changesRead.data}<div class="unread"><Failed what="the changes, so none are listed" failure={changesRead.failure} /></div>{/if}
  {#if projectsRead.failure && !projectsRead.data}<div class="unread"><Failed what="the projects, so none are listed" failure={projectsRead.failure} /></div>{/if}
  <ul role="listbox" aria-label="matches" id="palette-matches">
    {#each shown as e, i (e.id)}
      {#if i === 0 || shown[i - 1].kind !== e.kind}
        <li class="sec" role="presentation">{TITLES[e.kind]}</li>
      {/if}
      <li role="option" id="palette-opt-{i}" aria-selected={i === at}>
        <button tabindex="-1" onclick={e.go} onmouseenter={() => (at = i)}>
          <Icon name={e.icon} size={14} />
          <span class="label">{e.label}</span>
          {#if e.kind === "change" && e.hint}<ChangeState state={e.hint} q={e.qualifier} />{:else if e.hint}<span class="hint" class:kbd={e.kind === "action" || e.kind === "surface"}>{e.hint}</span>{/if}
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
