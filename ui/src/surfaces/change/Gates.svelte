<script lang="ts">
  // Every gate run against this change: each command, exit code, duration and
  // output (its own bytes), and the certificate a reviewer can re-run. The
  // newest attempt is open; older ones fold with their verdict showing.
  import { untrack } from "svelte";
  import { resource, copyText } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import { blocks } from "../../lib/md";
  import Inline from "../../lib/ui/Inline.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { exit, took, ago, type Detail, type GateReport } from "./types";

  let {
    d,
    id,
    /// A certificate already read (the render harness); the host's read replaces it.
    certificate = null,
  }: { d: Detail; id: string; certificate?: { finished: boolean; markdown?: string } | null } = $props();

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
  {#if attempts.length === 0}
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

  <section class="cert">
    <header>
      <h2><Icon name="shield" size={14} /> Certificate</h2>
      {#if cert?.markdown}<button onclick={copy}><Icon name="file" size={13} /> Copy as markdown</button>{/if}
    </header>
    {#if said}<p class="said">{said}</p>{/if}
    {#if !cert && read.failure}
      <Failed what="the certificate" failure={read.failure} />
    {:else if !cert}
      <p class="quiet">Reading…</p>
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
