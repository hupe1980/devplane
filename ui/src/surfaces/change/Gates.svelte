<script lang="ts">
  import NoCheck, { declaresNoCheck } from "../../lib/NoCheck.svelte";
  // Every gate run against this change: each command, exit code, duration and
  // output (its own bytes), and the certificate a reviewer can re-run. The
  // newest attempt is open; older ones fold with their verdict showing.
  import Skeleton from "../../lib/Skeleton.svelte";
  import { untrack } from "svelte";
  import { resource, copyText } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import { blocks } from "../../lib/md";
  import Inline from "../../lib/ui/Inline.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { api, Refused } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";
  import { exit, took, ago, countSays, type Detail, type GateReport } from "./types";

  let {
    d,
    id,
    /// Called after a write the host acted on, so the change is read again.
    reload = () => {},
    /// A certificate already read (the render harness); the host's read replaces it.
    certificate = null,
    /// The weakened rows the review found, each joined back to its line.
    weakened = [],
    /// Opens the review on a weakened row's file.
    toLine = null,
  }: {
    d: Detail;
    id: string;
    reload?: () => void | Promise<void>;
    certificate?: { finished: boolean; markdown?: string } | null;
    weakened?: Array<{ path: string; why: string; matched: string; seen: boolean; gate_says?: string | null }>;
    toLine?: ((path: string) => void) | null;
  } = $props();

  const bc = $derived(d.base_checks ?? null);
  let asked = $state("");
  let asking = $state(false);
  /// *Run base checks*: drawn only when the host says a run can start.
  async function runBaseChecks() {
    if (asking) return;
    asking = true;
    try {
      const r = await api<{ started?: boolean; current?: boolean }>(`/api/changes/${encodeURIComponent(id)}/base-checks`, { method: "POST" });
      asked = r?.current ? "Already current for this tree — nothing ran." : "Base checks started — the base's tests on this change, then this change's new tests on the base.";
      await reload();
    } catch (e) {
      const body = e instanceof Refused ? (e.body as { says?: string } | null) : null;
      asked = body?.says ?? `That did not land: ${failure(e).says}`;
    } finally {
      asking = false;
    }
  }

  const attempts = $derived([...(d.gates ?? [])].reverse());
  let open = $state<Record<number, boolean>>({});
  const passed = (g: GateReport) => g.commands.every((c) => c.outcome.outcome === "exited" && c.outcome.code === 0);

  const key = $derived(id);
  const read = resource<{ finished: boolean; markdown?: string }>(
    () => (key ? `/api/changes/${encodeURIComponent(key)}/certificate` : null),
    { tell: () => `devplane change export ${key}` },
  );
  const cert = $derived(read.data ?? certificate);
  let said = $state("");
  // A new attempt makes a new certificate: read it again when one lands.
  const newest = $derived(d.gates?.[d.gates.length - 1]?.at ?? "");
  let seenNewest: string | null = null;
  $effect(() => {
    const n = newest;
    untrack(() => {
      if (seenNewest !== null && n !== seenNewest) void read.reload();
      seenNewest = n;
    });
  });
  async function copy() {
    if (!cert?.markdown) return;
    said = (await copyText(cert.markdown))
      ? "Copied — paste it into the pull request."
      : `Nothing was copied — this page has no clipboard. Run: devplane change export ${key}`;
  }
</script>

<div class="gates">
  {#if weakened.length}
    <!-- What the gates ran over includes a check this change weakened: each
         row goes back to its line in the review. -->
    <section class="weakened" aria-label="checks this change weakened">
      {#each weakened as w (w.path + w.matched)}
        <p><Icon name="alert" size={13} /> <b>check weakened</b> in <button class="line" onclick={() => toLine?.(w.path)}><code>{w.path}</code></button> — <Inline text={w.why} />{#if w.gate_says}; <Inline text={w.gate_says} />{/if} · {w.seen ? "read" : "not yet read"}</p>
      {/each}
    </section>
  {/if}
  {#if declaresNoCheck(d)}
    <NoCheck project={d.project_id} />
  {:else if attempts.length === 0}
    <p class="quiet">No gate has run against this change yet. The gates run when the agent says it has finished, or when you press <b>Run gates</b>.</p>
  {:else}
    {#each attempts as g, i (g.at)}
      {@const ok = passed(g)}
      {@const shown = open[i] ?? i === 0}
      <section class="attempt" class:ok class:bad={!ok}>
        <button class="head" onclick={() => (open = { ...open, [i]: !shown })} aria-expanded={shown}>
          <Icon name={shown ? "down" : "right"} size={12} />
          <Icon name={ok ? "check" : "x"} size={15} />
          <b>{g.gate}</b>
          <span>attempt {g.attempt}</span>
          <span class="verdict">{ok ? "passed" : "failed"}</span>
          <span class="meta">{g.commands.length} commands · {took(g.duration_ms)} · {ago(g.at)}</span>
          {#if g.commit?.tree}<code class="tree" title="the tree this ran against">tree {g.commit.tree.slice(0, 10)}</code>{/if}
        </button>
        {#if shown}
          <ol class="cmds">
            {#each g.commands as c, ci (ci)}
              {@const cok = c.outcome.outcome === "exited" && c.outcome.code === 0}
              <li class:cbad={!cok}>
                <div class="row">
                  <Icon name={cok ? "check" : "x"} size={13} />
                  <code class="cmd">{c.command}</code>
                  <span class="exit">{exit(c.outcome)}</span>
                  {#if c.tests}<span class="tests">{countSays(c.tests)}</span>{/if}
                  <span class="dur">{took(c.duration_ms)}</span>
                </div>
                {#if c.failures?.length}
                  <ul class="fails">{#each c.failures as f (f)}<li>{f}</li>{/each}</ul>
                {/if}
                {#if c.output_tail}
                  <pre class="out">{c.output_tail}</pre>
                  <span class="bytes">{c.output_bytes ?? "?"} bytes · digest {c.output_digest ?? "—"}</span>
                {/if}
              </li>
            {/each}
          </ol>
        {/if}
      </section>
    {/each}
  {/if}

  {#if bc}
    <section class="base">
      <header>
        <h2><Icon name="gate" size={14} /> Base checks</h2>
        {#if bc.can_run}<button onclick={runBaseChecks} disabled={asking}><Icon name="play" size={13} /> Run base checks</button>{/if}
      </header>
      <p class="counts">{bc.counts_says}</p>
      {#each bc.junit_notes as n, ni (ni)}<p class="w-warn">{n}</p>{/each}
      {#if asked}<p class="said">{asked}</p>{/if}
      {#if bc.stale && bc.on_change}<p class="quiet">These runs describe tree {bc.tree?.slice(0, 10)}, not this one.</p>{/if}
      {#if bc.on_change}
        <h3>The base's tests on this change</h3>
        <p class="w-{bc.on_change.weight}">{bc.on_change.says}</p>
        {#if bc.on_change.restored.length}<p class="files">Put back as the base has them: {#each bc.on_change.restored as f, fi (fi)}<code>{f}</code>{" "}{/each}</p>{/if}
        {#if bc.on_change.removed.length}<p class="files">Removed, as the base lacks them: {#each bc.on_change.removed as f, fi (fi)}<code>{f}</code>{" "}{/each}</p>{/if}
        {#each bc.on_change.junit_notes as n, ni (ni)}<p class="quiet">{n}</p>{/each}
        {#each bc.on_change.commands as c, ci (ci)}<p class="cmdline"><code>{c.command}</code> {exit(c.outcome)} · {countSays(c.tests)}</p>{/each}
        {#if bc.on_change.failing.length}
          <ul class="names">{#each bc.on_change.failing as n, ni (ni)}<li>{n}</li>{/each}</ul>
          {#if bc.on_change.failing_more}<p class="quiet">and {bc.on_change.failing_more} more — <code>devplane change show {id} --json</code> lists every one</p>{/if}
        {/if}
        {#if bc.on_change.named?.not_run_on_change.length}
          <p>{bc.on_change.named.says}</p>
          <ul class="names">{#each bc.on_change.named.not_run_on_change as n, ni (ni)}<li>{n}</li>{/each}</ul>
          {#if bc.on_change.named.not_run_on_change_more}<p class="quiet">and {bc.on_change.named.not_run_on_change_more} more — <code>devplane change show {id} --json</code> lists every one</p>{/if}
        {/if}
      {/if}
      {#if bc.on_base}
        <h3>This change's new tests on the base</h3>
        <p class="w-{bc.on_base.weight}">{bc.on_base.says}</p>
        {#if bc.on_base.placed.length}<p class="files">Placed: {#each bc.on_base.placed as f, fi (fi)}<code>{f.path}</code>{#if f.blob} <span class="quiet">blob {f.blob.slice(0, 10)}</span>{/if}{" "}{/each}</p>{/if}
        {#if bc.on_base.not_separable.length}<p class="files">Not separable: {#each bc.on_base.not_separable as f, fi (fi)}<code>{f}</code>{" "}{/each}</p>{/if}
        {#each bc.on_base.commands as c, ci (ci)}<p class="cmdline"><code>{c.command}</code> {exit(c.outcome)} · {countSays(c.tests)}</p>{/each}
        {#if bc.on_base.passing.length}<p>Passing without the change:</p><ul class="names">{#each bc.on_base.passing as n, ni (ni)}<li>{n}</li>{/each}</ul>{/if}
        {#if bc.on_base.passing_more}<p class="quiet">and {bc.on_base.passing_more} more — <code>devplane change show {id} --json</code> lists every one</p>{/if}
      {/if}
    </section>
  {/if}

  <section class="cert">
    <header>
      <h2><Icon name="shield" size={14} /> Certificate</h2>
      {#if cert?.markdown}<button onclick={copy}><Icon name="file" size={13} /> Copy as markdown</button>{/if}
    </header>
    {#if said}<p class="said"><Inline text={said} /></p>{/if}
    {#if !cert && read.failure}
      <Failed what="the certificate" failure={read.failure} />
    {:else if !cert}
      <Skeleton rows={3} />
    {:else}
      <!-- The host's markdown, rendered as text: inline code and emphasis, never literal markup. -->
      <div class="md">
        {#each blocks(cert.markdown) as b, bi (bi)}
          {#if b.kind === "pre"}<pre>{b.text}</pre>
          {:else if b.kind === "h"}<h3><Inline segs={b.segs} /></h3>
          {:else if b.kind === "li"}<p class="li">• <Inline segs={b.segs} /></p>
          {:else if b.kind === "row"}<p class="row"><Inline segs={b.segs} /></p>
          {:else}<p><Inline segs={b.segs} /></p>{/if}
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .weakened {
    margin-bottom: var(--s-3);
    padding: var(--s-2) var(--s-3);
    border-left: 3px solid var(--fail);
    background: var(--panel);
    font-size: var(--t-sm);
  }
  .weakened p {
    margin: 0;
    color: var(--fail);
  }
  .weakened .line {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
    text-decoration: underline;
  }
  .gates {
    display: grid;
    gap: var(--s-3);
    max-width: 80rem;
  }
  .attempt {
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    overflow: hidden;
  }
  .attempt.ok .head :global(svg:nth-of-type(2)) {
    color: var(--done);
  }
  .attempt.bad .head :global(svg:nth-of-type(2)) {
    color: var(--fail);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    width: 100%;
    padding: var(--s-2) var(--s-3);
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
    text-align: start;
  }
  .verdict {
    font-weight: 600;
  }
  .ok .verdict {
    color: var(--done);
  }
  .bad .verdict {
    color: var(--fail);
  }
  .meta {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  .tree {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 0.6875rem;
    color: var(--faint);
  }
  .cmds {
    list-style: none;
    margin: 0;
    padding: 0 var(--s-3) var(--s-3);
    display: grid;
    gap: var(--s-2);
  }
  .cmds li {
    border-top: 1px solid var(--line);
    padding-top: var(--s-2);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
    color: var(--done);
  }
  .cbad .row {
    color: var(--fail);
  }
  .cmd {
    flex: 1;
    color: var(--ink);
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .exit {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .dur {
    color: var(--faint);
    font-size: var(--t-xs);
    min-width: 4rem;
    text-align: end;
  }
  .fails {
    margin: var(--s-1) 0 0 var(--s-5);
    padding: 0;
    color: var(--fail);
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .out,
  .md {
    margin: var(--s-2) 0 0;
    padding: var(--s-2) var(--s-3);
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-family: var(--mono);
    font-size: var(--t-xs);
    color: var(--dim);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 18rem;
    overflow: auto;
  }
  .bytes {
    font-size: 0.6875rem;
    color: var(--faint);
  }
  .cert {
    margin-top: var(--s-4);
  }
  .base {
    display: grid;
    gap: var(--s-1);
    font-size: var(--t-sm);
  }
  .base header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .base h3 {
    margin: var(--s-2) 0 0;
    font-size: var(--t-sm);
  }
  .base p {
    margin: 0;
  }
  .base button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: var(--t-sm);
  }
  .counts,
  .tests {
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
  .tests {
    font-size: var(--t-xs);
  }
  .w-warn {
    color: var(--wait);
    font-weight: 600;
  }
  .w-absent {
    color: var(--faint);
  }
  .files code,
  .cmdline code,
  .names {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .names {
    margin: 0 0 0 var(--s-4);
    padding: 0;
  }
  .cert header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
    margin: 0;
  }
  .cert button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: var(--t-sm);
  }
  .md {
    max-height: 28rem;
    color: var(--ink);
    font-family: inherit;
    font-size: var(--t-sm);
    white-space: normal;
  }
  .md h3 {
    margin: 0 0 var(--s-2);
    font-size: var(--t-sm);
  }
  .md p {
    margin: 0 0 var(--s-2);
  }
  .md .row,
  .md pre {
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
  }
  .md :global(code) {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .said {
    color: var(--dim);
    font-size: var(--t-sm);
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
  }
</style>
