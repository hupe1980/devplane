<script lang="ts">
  // The triage list: everything that stops without you, in the host's ranking,
  // never re-sorted here. One dense row each; the selected item opens beside
  // it. What was folded or held back is counted at the foot.
  import Icon from "../../lib/ui/Icon.svelte";
  import { ago } from "../../lib/text";
  import type { Item } from "./Item.svelte";
  import { narrowed, type Summary, type Inhibited } from "./narrow.svelte";
  import { address, place, itemFocus } from "./place.svelte";

  let {
    items = [],
    folded = [],
    inhibited = [],
    close = null,
    project = "",
    focus = "",
    loaded = false,
    error = null,
    open,
  }: {
    items?: Item[];
    folded?: Summary[];
    inhibited?: Inhibited[];
    close?: { since_last_look?: string | null } | null;
    /// The project the address narrows to, out of `#inbox/<project>`.
    project?: string;
    focus?: string;
    loaded?: boolean;
    error?: string | null;
    open: (id: string, pin?: boolean) => void;
  } = $props();

  /// The narrowed list comes from the host (`core::attention::narrow`), so the
  /// window and the terminal narrow identically; read once for this list and
  /// the item beside it.
  const narrow = narrowed(() => project);

  const projects = $derived([...new Set(items.map((i) => i.project_name).filter((p): p is string => !!p))]);
  // Narrowed, only what the host narrowed: never the whole inbox under a chip
  // that names one project.
  // While it is being read or could not be, the narrowed feed is empty, not
  // absent.
  const NONE = { items: [] as Item[], folded: [] as Summary[], inhibited: [] as Inhibited[] };
  const narrowedFeed = $derived(narrow.on ? (narrow.data ?? NONE) : null);
  const shown = $derived(narrowedFeed?.items ?? items);
  const shownFolded = $derived(narrowedFeed?.folded ?? folded);
  const shownInhibited = $derived(narrowedFeed?.inhibited ?? inhibited);
  const ready = $derived(narrow.on ? narrow.phase !== "loading" : loaded);
  const nothingRaised = $derived(shown.length === 0 && shownFolded.length === 0 && shownInhibited.length === 0);
  /// The host knows no project by that name: said, never *Nothing needs you*.
  const missing = $derived(narrow.missing);
  /// The same row the item pane shows, by the same rule.
  const selected = $derived(place(shown, address(focus, items)).current?.id ?? "");
  const optionId = (id: string) => `inbox-row-${id.replace(/[^\w-]/g, "_")}`;

  const icon = (k: string) =>
    k === "permission" ? "shield" : k === "question" ? "question" : k.includes("gate") || k.includes("fail") ? "x" : k.includes("ready") ? "check" : k.includes("conflict") ? "alert" : k.includes("context") ? "clock" : "dot";

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const age = (since?: string) => (since ? ago(Math.max(0, (now - Date.parse(since)) / 1000)) : "");

</script>

<div class="list">
  <header>
    <span class="t">What needs you</span>
    <!-- No count until one is known: a list being read has no size yet. -->
    <span class="n">{ready && !missing && !(narrow.on && narrow.failure && !narrow.data) ? shown.length : ""}</span>
  </header>
  {#if close?.since_last_look}<p class="since"><Icon name="clock" size={12} /> since you last looked · {close.since_last_look}</p>{/if}
  {#if projects.length > 1 || project}
    <div class="chips" role="group" aria-label="narrow to one project">
      <button class:on={!project} aria-pressed={!project} onclick={() => open("")}>all</button>
      {#each projects as p (p)}<button class:on={project === p} aria-pressed={project === p} onclick={() => open(project === p ? "" : p)}>{p}</button>{/each}
    </div>
  {/if}

  <!-- The list keys are the registry's (`bindList`, answered in the item
       pane through `lib/cursor`), so they work with focus here or anywhere. -->
  <div
    class="rows"
    role="listbox"
    aria-label="what needs you"
    tabindex="0"
    aria-activedescendant={selected ? optionId(selected) : undefined}
  >
    {#if narrow.on && narrow.failure && !narrow.data}
      <p class="quiet failed">Could not read the inbox narrowed to {project}: {narrow.failure.says}.</p>
    {:else if missing}
      <p class="quiet failed">No project is named “{project}”.</p>
    {:else if !ready && shown.length === 0}
      {#each [0, 1, 2] as i (i)}<div class="skel" aria-busy="true"></div>{/each}
    {:else if nothingRaised && error}
      <p class="quiet">The last read had nothing for you, and Devplane has not answered since.</p>
    {:else if nothingRaised}
      <p class="quiet">{project ? `Nothing needs you in ${project}.` : "Nothing needs you."}</p>
    {/if}
    {#each shown as i (i.id)}
      <div
        class="row"
        class:high={i.level === "high"}
        role="option"
        id={optionId(i.id)}
        tabindex="-1"
        aria-selected={i.id === selected}
        onclick={() => open(itemFocus(i.id))}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            open(itemFocus(i.id));
          }
        }}
      >
        <span class="ic"><Icon name={icon(i.kind)} size={14} /></span>
        <span class="title">{i.title}</span>
        {#if i.new_to_you}<span class="new">new</span>{/if}
        <span class="meta">{i.kind.replace(/_/g, " ")}{i.project_name ? ` · ${i.project_name}` : ""}</span>
        <span class="age">{age(i.since)}</span>
      </div>
    {/each}
    {#each shownFolded as f (f.kind + (f.project ?? ""))}
      <div class="folded"><Icon name="more" size={13} /> {f.count} × {f.kind.replace(/_/g, " ")} <span>{f.project ?? "across projects"} · folded</span></div>
    {/each}
    {#each shownInhibited as s (s.cause)}
      <div class="folded"><Icon name="more" size={13} /> {s.count} more counted <span>{s.because}</span></div>
    {/each}
  </div>
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    height: 2.2rem;
    padding: 0 var(--s-4);
    flex: none;
  }
  .t {
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--dim);
  }
  .n {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .since {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin: 0 var(--s-4) var(--s-2);
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1);
    padding: 0 var(--s-3) var(--s-2);
    flex: none;
  }
  .chips button {
    height: 1.4rem;
    padding: 0 0.5rem;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: none;
    color: var(--dim);
    font: inherit;
    font-size: 0.6875rem;
    cursor: pointer;
  }
  .chips button.on {
    border-color: var(--accent);
    background: var(--select);
    color: var(--ink);
  }
  .rows {
    flex: 1;
    min-height: 0;
    overflow: auto;
    outline: none;
  }
  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas: "ic title age" "ic meta meta";
    column-gap: var(--s-2);
    row-gap: 0.1rem;
    padding: 0.5rem var(--s-3);
    border-left: 2px solid transparent;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .row:hover {
    background: var(--raise);
  }
  .row[aria-selected="true"] {
    background: var(--select);
    border-left-color: var(--accent);
  }
  .rows:focus-visible .row[aria-selected="true"] {
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }
  .ic {
    grid-area: ic;
    color: var(--faint);
    padding-top: 0.1rem;
  }
  .high .ic {
    color: var(--wait);
  }
  .title {
    grid-area: title;
    font-size: var(--t-sm);
    color: var(--ink);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .new {
    grid-area: title;
    justify-self: end;
    align-self: center;
    margin-right: 2.5rem;
    font-size: 0.625rem;
    color: var(--accent);
    font-weight: 700;
    text-transform: uppercase;
  }
  .meta {
    grid-area: meta;
    font-size: var(--t-xs);
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .age {
    grid-area: age;
    font-size: var(--t-xs);
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .folded {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.45rem var(--s-3);
    font-size: var(--t-xs);
    color: var(--dim);
    border-bottom: 1px solid var(--line);
  }
  .folded span {
    color: var(--faint);
  }
  .quiet {
    margin: var(--s-3) var(--s-4);
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .quiet.failed {
    color: var(--fail);
  }
  .skel {
    height: 3rem;
    margin: var(--s-1) var(--s-3);
    border-radius: var(--radius);
    background: var(--raise);
    opacity: 0.6;
  }
</style>
