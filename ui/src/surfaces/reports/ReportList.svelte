<script lang="ts" module>
  export type Project = { id: string; name: string };
  export type Row = {
    id: string;
    title: string;
    kind: string;
    quoted: string;
    state_says: string;
    age_says: string;
    provenance_says: string;
    target_says: string;
    provenance: { project: string; project_name: string };
    target: { to: string; project?: string };
    state?: string;
  };
  export type Filing = {
    to: string;
    from: string;
    kind: string;
    title: string;
    words: string;
    command: string;
    output: string;
  };
</script>

<script lang="ts">
  // Findings one project filed about another, and what became of each. A
  // report is somebody else's words, shown quoted as filed. It reaches the
  // target's owner (its agent only via `[reports] deliver_from`); a GitHub
  // target is a draft until a person opens it.
  import { api } from "../../lib/api";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";

  let {
    reports = [],
    projects = [],
    loaded = false,
    failed = "",
    said = "",
    file = async () => false,
    reload = async () => {},
  }: {
    reports?: Row[];
    projects?: Project[];
    loaded?: boolean;
    failed?: string;
    said?: string;
    file?: (f: Filing) => Promise<boolean>;
    reload?: () => Promise<void>;
  } = $props();

  let about = $state("");
  let selected = $state<string | null>(null);
  let filing = $state(false);
  let acted = $state("");

  const shown = $derived(
    about ? reports.filter((r) => r.target.project === about || r.provenance.project === about) : reports,
  );
  const pick = $derived(reports.find((r) => r.id === selected) ?? null);
  const columns: Column<Row>[] = [
    { key: "title", label: "Finding", width: 320, sort: (r) => r.title },
    { key: "kind", label: "Kind", width: 90, sort: (r) => r.kind },
    { key: "route", label: "From → to", width: 220, sort: (r) => r.provenance.project_name },
    { key: "state", label: "State", width: 200, sort: (r) => r.state_says },
    { key: "age", label: "Filed", align: "end", sort: (r) => r.age_says },
  ];

  async function act(r: Row, verb: "start" | "resolve" | "open", how?: string) {
    try {
      const id = encodeURIComponent(r.id);
      const route = verb === "start" ? `/api/reports/${id}/start` : verb === "open" ? `/api/reports/${id}/open` : `/api/reports/${id}/resolve`;
      const res = await api<{ says?: string; change?: { id: string } }>(route, {
        method: "POST",
        body: JSON.stringify(verb === "resolve" ? { as: how } : {}),
      });
      acted = res.says ?? (verb === "start" ? "A change was started from it." : verb === "open" ? "Opened on GitHub." : `Marked ${how}.`);
      await reload();
    } catch (e) {
      acted = `That did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  let f = $state<Filing>({ to: "", from: "", kind: "defect", title: "", words: "", command: "", output: "" });
  let refused = $state("");
  async function submit(e: Event) {
    e.preventDefault();
    if (!f.to.trim() || !f.from || !f.title.trim() || !f.words.trim()) {
      refused = "Say where it goes, where it comes from, a title and the finding.";
      return;
    }
    refused = "";
    if (await file({ ...f })) {
      f = { ...f, title: "", words: "", command: "", output: "" };
      filing = false;
    }
  }
</script>

<div class="page">
  <header class="head">
    <h1>Reports</h1>
    <select bind:value={about} aria-label="which project">
      <option value="">every project</option>
      {#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
    </select>
    <span class="gap"></span>
    <button class="primary" onclick={() => (filing = !filing)}><Icon name="plus" size={14} /> File a report</button>
  </header>
  {#if said || acted}<p class="said" role="status">{acted || said}</p>{/if}

  {#if filing}
    <form class="file" onsubmit={submit} aria-label="file a report">
      <div class="two">
        <label>From <select bind:value={f.from}><option value="">choose</option>{#each projects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}</select></label>
        <label>To <input bind:value={f.to} placeholder="a registered project, or owner/repo on GitHub" /></label>
        <label>Kind <select bind:value={f.kind}><option value="defect">defect</option><option value="question">question</option><option value="request">request</option><option value="breaking_change">breaking change</option></select></label>
      </div>
      <label>Title <input bind:value={f.title} placeholder="One line a stranger would understand" /></label>
      <label>Finding <textarea bind:value={f.words} rows="4" placeholder="What is wrong, and how you know"></textarea></label>
      <div class="two">
        <label>Command that shows it <input bind:value={f.command} placeholder="optional" /></label>
        <label>Its output <input bind:value={f.output} placeholder="optional" /></label>
      </div>
      {#if refused}<p class="warn">{refused}</p>{/if}
      <div class="row"><button type="submit" class="primary">File it</button><button type="button" onclick={() => (filing = false)}>Cancel</button></div>
    </form>
  {/if}

  {#if !loaded}
    <p class="quiet">Reading…</p>
  {:else if failed}
    <Empty icon="alert" title="The reports could not be read" body={failed} />
  {:else if reports.length === 0}
    <Empty icon="report" title="No report has been filed" body="An agent working on one project can file what it found about another — it reaches that project's person, quoted and with where it came from." />
  {:else}
    <div class="frame">
      <Grid id="reports" {columns} rows={shown} key={(r) => r.id} bind:selected label="reports">
        {#snippet cell(r, c)}
          {#if c.key === "title"}<b class="t">{r.title}</b>
          {:else if c.key === "kind"}<span class="dim">{r.kind}</span>
          {:else if c.key === "route"}{r.provenance.project_name} <span class="dim">→</span> {r.target_says}
          {:else if c.key === "state"}<Pill word={r.state_says} />
          {:else}<span class="dim">{r.age_says}</span>{/if}
        {/snippet}
      </Grid>
      {#if pick}
        <aside class="drawer">
          <header><b>{pick.title}</b><button class="x" aria-label="close" onclick={() => (selected = null)}><Icon name="x" size={13} /></button></header>
          <p class="dim">{pick.provenance_says}</p>
          <p><Pill word={pick.state_says} /></p>
          <pre class="quoted" aria-label="the report, quoted as it was filed">{pick.quoted}</pre>
          <div class="acts">
            <button onclick={() => act(pick!, "start")}><Icon name="play" size={13} /> Start a change from it</button>
            <button onclick={() => act(pick!, "resolve", "fixed")}><Icon name="check" size={13} /> Fixed</button>
            <button onclick={() => act(pick!, "resolve", "rejected")}><Icon name="x" size={13} /> Rejected</button>
            <button onclick={() => act(pick!, "resolve", "deferred")}><Icon name="clock" size={13} /> Deferred</button>
            {#if pick.target.to.includes("/")}<button onclick={() => act(pick!, "open")}><Icon name="forge" size={13} /> Open the drafted issue</button>{/if}
          </div>
        </aside>
      {/if}
    </div>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--s-4) var(--s-5);
    gap: var(--s-3);
    box-sizing: border-box;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s-3);
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .gap {
    flex: 1;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .said {
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--accent);
    background: var(--panel);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .file {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  .file label {
    display: grid;
    gap: 0.25rem;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
    gap: var(--s-3);
  }
  .row {
    display: flex;
    gap: var(--s-2);
  }
  .warn {
    color: var(--wait);
    margin: 0;
    font-size: var(--t-sm);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .t {
    font-weight: 600;
  }
  .dim {
    color: var(--faint);
  }
  .drawer {
    width: 26rem;
    flex: none;
    border-left: 1px solid var(--line);
    background: var(--side);
    padding: var(--s-3) var(--s-4);
    overflow: auto;
    font-size: var(--t-sm);
  }
  .drawer header {
    display: flex;
    align-items: flex-start;
    gap: var(--s-2);
  }
  .x {
    margin-left: auto;
    border: 0;
    background: none;
    color: var(--dim);
    padding: 0;
  }
  .quoted {
    margin: var(--s-3) 0;
    padding: var(--s-2) var(--s-3);
    border-left: 2px solid var(--line);
    color: var(--dim);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .acts {
    display: grid;
    gap: var(--s-2);
  }
  .acts button {
    justify-content: flex-start;
    font-size: var(--t-sm);
  }
  .quiet {
    color: var(--faint);
  }
</style>
