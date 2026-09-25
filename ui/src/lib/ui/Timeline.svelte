<script lang="ts" module>
  export type Mark = { at: string; lane: string; label: string; tone?: "work" | "wait" | "fail" | "done" | "none" };
</script>

<script lang="ts">
  // Events on one time axis, in lanes: start, every gate attempt, every
  // decision, now. A picture of when, never of how good: no line, trend or
  // total. HTML, not a stretched SVG, so text stays text.
  let { marks, lanes }: { marks: Mark[]; lanes: string[] } = $props();

  const times = $derived(marks.map((m) => new Date(m.at).getTime()).filter((t) => !Number.isNaN(t)));
  const lo = $derived(times.length ? Math.min(...times) : 0);
  const hi = $derived(times.length ? Math.max(Date.now(), ...times) : 1);
  const span = $derived(Math.max(1, hi - lo));
  const pct = (at: string) => ((new Date(at).getTime() - lo) / span) * 100;

  function clock(t: number): string {
    const d = new Date(t);
    const sameDay = new Date().toDateString() === d.toDateString();
    return sameDay ? d.toTimeString().slice(0, 5) : `${d.toLocaleDateString()} ${d.toTimeString().slice(0, 5)}`;
  }
  let hover = $state<Mark | null>(null);
</script>

{#if marks.length === 0}
  <p class="none">Nothing has happened to it yet.</p>
{:else}
  <figure class="tl" aria-label="timeline of {marks.length} events">
    {#each lanes as lane (lane)}
      <div class="lane">
        <span class="name">{lane}</span>
        <div class="track">
          {#each marks.filter((m) => m.lane === lane) as m, i (i)}
            <button
              class="mark {m.tone ?? 'none'}"
              style:left="{pct(m.at)}%"
              title="{m.label} — {new Date(m.at).toLocaleString()}"
              aria-label="{m.label} at {new Date(m.at).toLocaleString()}"
              onmouseenter={() => (hover = m)}
              onmouseleave={() => (hover = null)}
              onfocus={() => (hover = m)}
              onblur={() => (hover = null)}
            ></button>
          {/each}
          <span class="now" aria-hidden="true"></span>
        </div>
      </div>
    {/each}
    <div class="axis"><span></span><span class="ticks"><span>{clock(lo)}</span><span>now · {clock(hi)}</span></span></div>
    <figcaption aria-live="polite">{hover ? `${hover.lane} — ${hover.label}` : " "}</figcaption>
  </figure>
{/if}

<style>
  .tl {
    margin: 0;
    display: grid;
    gap: 2px;
  }
  .lane,
  .axis {
    display: grid;
    grid-template-columns: 6rem 1fr;
    align-items: center;
    gap: var(--s-3);
  }
  .name {
    font-size: var(--t-xs);
    color: var(--faint);
    text-align: end;
  }
  .track {
    position: relative;
    height: 1.5rem;
    margin: 0 0.5rem;
  }
  .track::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 1px;
    background: var(--line);
  }
  .now {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    border-right: 1px dashed var(--accent);
  }
  .mark {
    position: absolute;
    top: 50%;
    width: 0.8rem;
    height: 0.8rem;
    padding: 0;
    margin: 0;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    border: 2px solid var(--bg);
    background: var(--dim);
    cursor: default;
    z-index: 1;
  }
  .mark:hover,
  .mark:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    z-index: 2;
  }
  .mark.work { background: var(--work); }
  .mark.wait { background: var(--wait); }
  .mark.fail { background: var(--fail); }
  .mark.done { background: var(--done); }
  .ticks {
    display: flex;
    justify-content: space-between;
    margin: 0 0.5rem;
    font-size: 0.6875rem;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  figcaption {
    font-size: var(--t-xs);
    color: var(--dim);
    min-height: 1.3em;
    padding-left: calc(6rem + var(--s-3));
  }
  .none {
    color: var(--faint);
    font-size: var(--t-sm);
    margin: 0;
  }
</style>
