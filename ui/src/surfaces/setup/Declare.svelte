<script lang="ts" module>
  export type Candidate = { command: string; source: string; line: number | null; joins: string };
  export type Candidates = {
    project: { id: string; name: string; root: string };
    current: { check: string[]; named: Record<string, string[]> };
    candidates: Candidate[];
    looked_for: string[];
    config: { path: string; exists: boolean; uncommitted: boolean; parses: boolean; line?: number | null; error?: string };
  };
</script>

<script lang="ts">
  // Declare a check: the one thing *verified* needs. Commands the repository
  // already has are offered as quoted text with where they were found; the
  // person picks, edits, reorders or types, sees the exact change to
  // devplane.toml, and confirms. Nothing is run, and nothing is written
  // before the confirm. A declined candidate is simply not sent: nothing
  // remembers it, so it is offered again next time.
  import { resource } from "../../lib/resource.svelte";
  import { untrack } from "svelte";
  import { api } from "../../lib/api";
  import Failed from "../../lib/Failed.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";
  import PendingWrite, { type Pending } from "../../lib/PendingWrite.svelte";
  import Icon from "../../lib/ui/Icon.svelte";

  let {
    project,
    ondone,
    firstRun = false,
    candidates: planted = null,
  }: {
    /// The project's id (its root) or name.
    project: string;
    ondone?: () => void;
    /// Inside the first-run checklist: its controls are marked for the walk.
    firstRun?: boolean;
    /// An answer already read — what a harness hands the flow.
    candidates?: Candidates | null;
  } = $props();

  const read = resource<Candidates>(() => (planted ? null : `/api/projects/${encodeURIComponent(project)}/gates/candidates`), {
    tell: () => "devplane config check",
  });
  const data = $derived(read.data ?? planted);

  type Row = { key: number; command: string; source: string; line: number | null; picked: boolean; editing: boolean };
  let n = 0;
  /// The rows a reading offers: the current check first, picked; then each
  /// candidate, none picked. OpenSpec apart, as a named gate.
  function seed(d: Candidates | null): { rows: Row[]; named: Row[] } {
    if (!d) return { rows: [], named: [] };
    const current = d.current.check.map((c) => ({ key: n++, command: c, source: "devplane.toml", line: null, picked: true, editing: false }));
    const offered = d.candidates
      .filter((c) => c.joins === "check" && !d.current.check.includes(c.command))
      .map((c) => ({ key: n++, command: c.command, source: c.source, line: c.line, picked: false, editing: false }));
    const named = d.candidates
      .filter((c) => c.joins.startsWith("named:"))
      .map((c) => ({ key: n++, command: c.command, source: c.source, line: c.line, picked: !!d.current.named.openspec, editing: false }));
    return { rows: [...current, ...offered], named };
  }
  // A planted answer seeds the rows once; a read answer seeds them below.
  const first = untrack(() => seed(planted));
  let rows = $state<Row[]>(first.rows);
  let named = $state<Row[]>(first.named);
  let typed = $state("");
  let seeded = $state(untrack(() => planted?.project.id ?? ""));
  $effect(() => {
    if (!data || seeded === data.project.id) return;
    seeded = data.project.id;
    ({ rows, named } = seed(data));
  });

  function move(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= rows.length) return;
    const next = [...rows];
    [next[i], next[j]] = [next[j], next[i]];
    rows = next;
  }
  function add(e?: Event) {
    e?.preventDefault();
    const c = typed.trim();
    if (!c) return;
    rows = [...rows, { key: n++, command: c, source: "typed here", line: null, picked: true, editing: false }];
    typed = "";
  }

  const chosen = $derived({
    check: rows.filter((r) => r.picked && r.command.trim()).map((r) => r.command),
    named: named.some((r) => r.picked) ? { openspec: named.filter((r) => r.picked).map((r) => r.command) } : {},
  });
  const anything = $derived(chosen.check.length > 0 || Object.keys(chosen.named).length > 0);

  let pending = $state<Pending | null>(null);
  let said = $state("");
  let done = $state(false);
  let asking = $state(false);
  const url = $derived(`/api/projects/${encodeURIComponent(project)}/gates`);
  async function preview() {
    if (!anything || asking) return;
    asking = true;
    said = "";
    try {
      pending = await api<Pending>(url, { method: "POST", body: JSON.stringify({ ...chosen, confirm: false }) });
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
    } finally {
      asking = false;
    }
  }
  function written() {
    pending = null;
    done = true;
    void read.reload();
    ondone?.();
  }
  const where = (r: Row) => (r.source === "typed here" ? "typed here" : `found: ${r.source}${r.line ? `:${r.line}` : ""}`);
  const mark = (s: string) => (firstRun ? s : undefined);
  const firstOffered = $derived(rows.findIndex((r) => r.source !== "devplane.toml"));
</script>

<div class="declare">
  {#if read.phase === "failed" && read.failure}
    <Failed what="what this repository offers as a check" failure={read.failure} retry={read.reload} />
  {:else if !data}
    <Skeleton rows={3} />
  {:else}
    {#if firstRun}
      <!-- The checklist's row already says what this project lacks. -->
    {:else if data.current.check.length === 0}
      <p class="head"><Icon name="alert" size={13} /> {data.project.name}: no gates declared — changes here cannot become verified until a check is declared.</p>
    {:else}
      <p class="head ok">{data.project.name}: check is declared. Change it below; the edit is shown as a diff before anything is written.</p>
    {/if}

    {#if done}
      <p class="done" role="status"><Icon name="check" size={13} /> Check declared — nothing has run yet. It runs when a change's agent finishes, or when you run the gates.</p>
    {/if}

    {#if !data.config.parses}
      <p class="fail" role="alert">
        <Icon name="alert" size={13} />
        <code>{data.config.path}</code> does not parse{data.config.line ? ` at line ${data.config.line}` : ""}: {data.config.error}. Nothing can be written until it does; fix it by hand.
      </p>
    {:else if pending}
      <PendingWrite {pending} {url} body={chosen} confirmLabel="Write devplane.toml" step={firstRun ? "confirm-diff" : ""} oncancel={() => (pending = null)} ondone={written} />
    {:else}
      {#if rows.length === 0}
        <p class="quiet">Found nothing to offer — looked for: {data.looked_for.join(" · ")}. Type the command that says this project is done.</p>
      {:else}
        <ol class="rows" aria-label="commands for check, in order">
          {#each rows as r, i (r.key)}
            <li class:on={r.picked}>
              <input
                type="checkbox"
                bind:checked={r.picked}
                aria-label="use {r.command} in check"
                data-first-run-step={i === firstOffered ? mark("pick-check") : undefined}
              />
              <div class="cmd">
                {#if r.editing}
                  <textarea bind:value={r.command} rows={Math.max(1, r.command.split("\n").length)} aria-label="the command"></textarea>
                {:else}
                  <code data-quoted>{r.command}</code>
                {/if}
                <span class="src">{where(r)}</span>
              </div>
              <div class="tools">
                <button type="button" onclick={() => (r.editing = !r.editing)}>{r.editing ? "Done" : "Edit"}</button>
                <button type="button" aria-label="move up" disabled={i === 0} onclick={() => move(i, -1)}>↑</button>
                <button type="button" aria-label="move down" disabled={i === rows.length - 1} onclick={() => move(i, 1)}>↓</button>
              </div>
            </li>
          {/each}
        </ol>
      {/if}
      <form class="add" onsubmit={add}>
        <input bind:value={typed} placeholder="Add a command — e.g. make test" aria-label="add a command to check" />
        <button type="submit" disabled={!typed.trim()}><Icon name="plus" size={12} /> Add</button>
      </form>

      {#if named.length}
        <div class="named">
          <p class="label">A named gate — evidence on the change, never part of check</p>
          {#each named as r (r.key)}
            <label class="row">
              <input type="checkbox" bind:checked={r.picked} />
              <code data-quoted>{r.command}</code>
              <span class="src">{where(r)}</span>
            </label>
          {/each}
        </div>
      {/if}

      {#if said}<p class="fail">{said}</p>{/if}
      <div class="go">
        <button type="button" class="primary" disabled={!anything || asking} onclick={preview} data-first-run-step={mark("show-diff")}>
          {asking ? "Reading…" : "Continue — show the change to devplane.toml"}
        </button>
        <span class="quiet">Offering runs nothing. Only the commands you confirm become the gate.</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .declare {
    display: grid;
    gap: var(--s-2);
  }
  p {
    margin: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .head.ok {
    color: var(--dim);
  }
  .done {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: var(--t-sm);
  }
  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  .rows li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: var(--s-2);
    align-items: start;
    padding: var(--s-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
  .rows li.on {
    border-color: var(--edge);
  }
  .cmd {
    display: grid;
    gap: 0.15rem;
    min-width: 0;
  }
  code,
  textarea {
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
  }
  .src {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  .tools {
    display: flex;
    gap: var(--s-1);
  }
  .add {
    display: flex;
    gap: var(--s-2);
  }
  .add input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .named {
    display: grid;
    gap: var(--s-1);
    padding: var(--s-2);
    border: 1px dashed var(--line);
    border-radius: var(--radius);
  }
  .label {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
  }
  .primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .fail {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--fail);
    font-size: var(--t-sm);
  }
</style>
