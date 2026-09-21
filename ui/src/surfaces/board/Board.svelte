<script lang="ts">
  // The board surface. **A port, so it owes every control the page it replaces
  // had** — a rebuild that loses one is a regression with a new coat of paint.
  //
  // This is the first surface through the registry, and it exists to prove the
  // contract before the other three follow: it registers itself, declares its
  // keys, renders every outside value as text, and has an empty state that is
  // a *result* rather than a blank.
  import { clip, ago } from "../../lib/text";
  import { api } from "../../lib/api";
  import Counts from "./Counts.svelte";
  import Supervision from "./Supervision.svelte";

  type Run = {
    id: string;
    project_name: string | null;
    agent: string;
    state: string;
    summary: string | null;
    cost_usd: number;
    context_percent: number | null;
    idle_seconds: number;
    permission_mode?: string | null;
    asks_a_person?: boolean | null;
  };

  /// Which projects this board is actually about.
  ///
  /// **The list's promise is *across everything*.** When a project cannot be
  /// read — its forge poll failed, its configuration will not parse — its rows
  /// are simply absent, and an empty list that silently covers four projects
  /// out of six reads as good news. That is the worst way for this page to be
  /// wrong, because it is wrong in the **reassuring** direction.
  type Coverage = { projects: number; unreadable: Array<{ name: string; why: string }> };

  /// Which vendors this board can see at all.
  ///
  /// **The same obligation `Coverage` carries, one level up.** An empty board
  /// on a machine running three Codex sessions is not reporting quiet; it is
  /// reporting the limit of its own sight, and a person reading it as *nothing
  /// is happening* has been misled by an interface that was technically
  /// correct. Composed by the daemon so this and `devplane ls` cannot disagree.
  type Watching = { watched: string[]; unproved: string[]; driven_only: string[] };

  /// The numbers that decide when a session is worth looking at.
  ///
  /// **Read from the daemon, never chosen here.** They are configurable, so a
  /// figure written into this page would disagree with the one `devplane ls`
  /// uses the moment somebody changes it — and the two surfaces would call the
  /// same session crowded and fine.
  type Thresholds = { context_high_percent?: number; rate_limit_percent?: number };

  type Summary = {
    projects: number; runs: number; working: number; needs_you: number;
    idle: number; failed: number; dormant: number; cost_usd: number;
    open_issues: number; open_prs: number; forge_needs_you: number; asks_waiting: number;
  };

  const NOTHING: Summary = {
    projects: 0, runs: 0, working: 0, needs_you: 0, idle: 0, failed: 0, dormant: 0,
    cost_usd: 0, open_issues: 0, open_prs: 0, forge_needs_you: 0, asks_waiting: 0,
  };

  /// What the last action did, so a refusal is never silent.
  let said = $state("");

  /// **Raises the editor window that owns a run.** The one control here that
  /// reaches outside the browser, and it is the daemon's to perform: a page
  /// cannot focus somebody else's window.
  async function focus(run: Run) {
    try {
      await api(`/api/runs/${encodeURIComponent(run.id)}/focus`, { method: "POST" });
      said = "raised the window that owns it";
    } catch (e) {
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  /// **Copies the command rather than attaching.** Attaching a terminal is
  /// something a terminal does; a page that claimed to would be a button that
  /// cannot keep its promise. The fallback is real — the command is on screen
  /// as text — because a browser may withhold the clipboard.
  async function attach(run: Run) {
    const cmd = `devplane attach ${run.id}`;
    try {
      await navigator.clipboard?.writeText(cmd);
      said = `copied: ${cmd}`;
    } catch {
      said = `no clipboard here — run: ${cmd}`;
    }
  }

  // **There is no selected row.** One was carried over with the keyboard
  // model: `j` and `k` moved it and the highlight said where they would act.
  // With the keys gone it marked whichever session happened to be first in an
  // unsorted list — a cursor nobody placed and nobody could move, which reads
  // as *this one is special* and means nothing.
  let {
    runs = [],
    summary = NOTHING,
    coverage = null,
    thresholds = null,
    watching = null,
  }: {
    runs?: Run[];
    summary?: Summary;
    coverage?: Coverage | null;
    thresholds?: Thresholds | null;
    watching?: Watching | null;
  } = $props();

  /// English for a list, because "Codex, OpenCode, Gemini CLI" in a sentence
  /// reads as a fragment and this is a sentence.
  function listed(xs: string[]): string {
    if (xs.length <= 1) return xs[0] ?? "";
    return `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`;
  }

  /// Whether a context window is close enough to compaction to say so.
  ///
  /// **`false` when the daemon has not said.** No threshold means no opinion,
  /// and a default invented here would be this page deciding what "crowded"
  /// means on its own.
  function crowded(pct: number | null): boolean {
    const at = thresholds?.context_high_percent;
    return pct !== null && typeof at === "number" && pct >= at;
  }

  // Grouped by project, because that is the unit a person thinks in: nine
  // sessions on one repository are one line of context, not nine rows that
  // differ by a hash.
  const grouped = $derived(
    Object.entries(
      runs.reduce<Record<string, Run[]>>((acc, r) => {
        const k = r.project_name ?? "(no project)";
        (acc[k] ??= []).push(r);
        return acc;
      }, {}),
    ).sort(([a], [b]) => a.localeCompare(b)),
  );
</script>

<section aria-labelledby="board-head">
  <h2 id="board-head">What is happening</h2>
  <Counts {summary} />

  <!-- **Above the list, because it is a fact about the list rather than a row
       in it** — a reader who has started down the rows has already decided the
       list is complete. -->
  {#if coverage && coverage.unreadable.length > 0}
    <p class="coverage" role="status">
      This is {coverage.projects - coverage.unreadable.length} of {coverage.projects} projects.
      {#each coverage.unreadable as u (u.name)}
        <span class="miss"><b>{u.name}</b> — {u.why}</span>
      {/each}
    </p>
  {/if}
  <p class="said" role="status" aria-live="polite">{said}</p>

  {#if runs.length === 0}
    <!-- **An empty state is a result, not a blank** — and it has to be the
         *right* result. This said "No agent session is running on this
         machine", which is a claim about the machine and not about what
         Devplane can see: on a machine running three Codex sessions it was
         false, in the reassuring direction, on the surface people trust to tell
         them nothing needs them. `devplane ls` had always said "no **Claude
         Code** sessions are running". -->
    <div class="empty">
      {#if watching && watching.watched.length > 0}
        <p><b>Nothing is running that Devplane can see.</b></p>
        <p class="seen">
          It watches {listed(watching.watched)} sessions you started yourself — start one and it
          appears here with no configuration.
        </p>
        {#if watching.unproved.length > 0}
          <p class="seen">
            {listed(watching.unproved)}: the channels are read and that path has not been proved
            end to end yet.
          </p>
        {/if}
        {#if watching.driven_only.length > 0}
          <!-- **Named, because this is the gap a person cannot otherwise
               discover.** A session you opened yourself in one of these never
               appears, and an empty board that does not say so is the whole
               failure this block exists to prevent. -->
          <p class="unseen">
            {listed(watching.driven_only)} appear only when Devplane starts them. A session you
            opened yourself in one of those is not on this board.
          </p>
        {/if}
      {:else}
        <p><b>Nothing is running that Devplane can see.</b></p>
      {/if}
    </div>
  {:else}
    {#each grouped as [project, rows] (project)}
      <h3>{project} <span class="n">{rows.length}</span></h3>
      <ul role="list">
        {#each rows as r, i (r.id)}
          <li role="listitem">
            <!-- **The state leads the row.** The identifier led it before, and
                 a hash is the least useful thing about a session: what it is
                 doing and whether it needs somebody are what a person came
                 for. The word carries it and the dot only repeats it. -->
            <span class="state s-{r.state}">
              <span class="dot" aria-hidden="true"></span>{r.state}
            </span>

            <span class="what">
              <!-- **The row is reachable.** Every session had a decision log
                   behind it and no way to open one, so *why this is here*
                   rendered an instruction to open a row that nothing could
                   carry out. An anchor rather than a click handler: it works
                   from the keyboard, it can be opened in a new tab, and the
                   hash is the router. -->
              <a class="says" href="#why/{encodeURIComponent(r.id)}"
                >{r.summary ?? "—"}<span class="sr"> — why this is here</span></a
              >
              <span class="sub">
                <span class="who">{clip(r.id, 8)}</span>
                <span>{r.agent}</span>
                <Supervision mode={r.permission_mode ?? null} asksAPerson={r.asks_a_person ?? null} />
              </span>
            </span>

            <span class="nums">
              <span class="cost">{r.cost_usd > 0 ? `$${r.cost_usd.toFixed(2)}` : "–"}</span>
              <!-- **The word, not only the colour.** A percentage turning amber
                   says nothing to somebody who cannot separate it from the ones
                   above, so the row close to compaction says so. -->
              <span class="ctx" class:crowded={crowded(r.context_percent ?? null)}>
                {r.context_percent === null ? "–" : `${r.context_percent}%`}
                {#if crowded(r.context_percent ?? null)}<span class="sr">context nearly full</span>{/if}
              </span>
              <span class="idle">{ago(r.idle_seconds)}</span>
            </span>

            <span class="acts">
              <button onclick={() => focus(r)} title="raise the window that owns this run">focus</button>
              <button onclick={() => attach(r)} title="copy `devplane attach`">attach</button>
            </span>
            <span class="sr">row {i + 1} of {rows.length}</span>
          </li>
        {/each}
      </ul>
    {/each}
  {/if}
</section>

<style>
  h2 { font-size: var(--t-lg); margin: 0; }

  h3 {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
    font-weight: 600;
    margin: var(--s-5) 0 var(--s-2);
    color: var(--ink);
  }
  .n {
    color: var(--dim);
    font-weight: 400;
    font-size: var(--t-xs);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0 var(--s-2);
  }

  ul { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-1); }

  /* **A row, not a table cell.** Nine equal columns made every field shout at
     the same volume, so nothing told a reader where to look. Now the state
     leads, what it is doing is the subject, and the numbers are metadata
     against the right edge. */
  li {
    display: grid;
    grid-template-columns: 7.5rem minmax(0, 1fr) auto auto;
    gap: var(--s-4);
    align-items: center;
    padding: var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
  }
  li:hover { border-color: var(--edge); }

  .state {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .state .dot { width: 7px; height: 7px; border-radius: 50%; background: currentColor; flex: none; }
  .s-working { color: var(--work); }
  .s-waiting { color: var(--wait); font-weight: 600; }
  .s-failed { color: var(--fail); font-weight: 600; }

  .what { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .says {
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-decoration: none;
    display: block;
  }
  .says:hover { text-decoration: underline; text-underline-offset: 2px; }
  .sub { display: flex; gap: var(--s-2); color: var(--dim); font-size: var(--t-xs); align-items: center; }
  .who { font-family: var(--mono); }

  .nums {
    display: flex;
    gap: var(--s-4);
    color: var(--dim);
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
  }
  .nums .cost, .nums .ctx { min-width: 3.2rem; text-align: right; }
  .nums .idle { min-width: 2.5rem; text-align: right; }
  .ctx.crowded { color: var(--wait); font-weight: 600; }

  /* **Quiet until the row is pointed at.** Twenty always-lit buttons told a
     reader every row was equally worth acting on, which is the opposite of
     what this list is for. They stay in the document and in the tab order —
     hiding them would take them from the people who cannot hover. */
  .acts { display: inline-flex; gap: var(--s-1); }
  .acts button { font-size: var(--t-xs); padding: 2px var(--s-2); opacity: .55; }
  li:hover .acts button,
  .acts button:focus-visible { opacity: 1; }

  .empty, .said { color: var(--dim); }
  .said { font-size: var(--t-xs); margin: var(--s-1) 0; }
  .said:empty { display: none; }

  .empty p { margin: 0 0 var(--s-2); }
  .empty p:last-child { margin-bottom: 0; }
  .empty .seen { color: var(--dim); }
  .empty .unseen { color: var(--wait); }
  .empty {
    max-width: 68ch;
    border: 1px dashed var(--line);
    border-radius: var(--radius-lg);
    padding: var(--s-6) var(--s-5);
    margin-top: var(--s-4);
    text-align: center;
  }

  .coverage {
    color: var(--wait);
    font-size: var(--t-sm);
    margin: var(--s-3) 0;
    border: 1px solid currentColor;
    border-radius: var(--radius);
    padding: var(--s-2) var(--s-3);
  }
  .coverage .miss { display: block; color: var(--ink); }

  /* On a phone the numbers wrap under the subject rather than competing with
     it for a 390-point line. */
  @media (max-width: 52rem) {
    li { grid-template-columns: 1fr auto; gap: var(--s-2); }
    .state { grid-column: 1; }
    .acts { grid-column: 2; grid-row: 1; }
    .acts button { opacity: 1; }
    .what { grid-column: 1 / -1; }
    .nums { grid-column: 1 / -1; justify-content: flex-start; }
    .nums .cost, .nums .ctx, .nums .idle { min-width: 0; text-align: left; }
  }

  /* Visible to a screen reader, and to nothing else. Position matters: a
     `display:none` here would take the row count away from the people it is
     for. */
  .sr {
    position: absolute; width: 1px; height: 1px;
    overflow: hidden; clip-path: inset(50%); white-space: nowrap;
  }
</style>
