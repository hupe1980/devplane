<script lang="ts" module>
  export type Row = { title: string; url: string; project_name: string; needs_you: boolean; number?: number; author?: string };
  export type Coverage = { projects: number; configured: number };
</script>

<script lang="ts">
  // Every open issue and pull request, what needs you first, read with your
  // own `gh`. Projects the forge cannot see are counted above the list, so an
  // empty list never reads as *nothing open*.
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";

  let {
    issues = [],
    pulls = [],
    loaded = false,
    failed = "",
    tab = "issues",
    coverage = null,
  }: {
    issues?: Row[];
    pulls?: Row[];
    loaded?: boolean;
    failed?: string;
    tab?: "issues" | "pulls";
    coverage?: Coverage | null;
  } = $props();

  let showing = $state<string>("issues");
  $effect(() => {
    showing = tab;
  });
  const shown = $derived(showing === "issues" ? issues : pulls);
  const absent = $derived(coverage ? Math.max(0, coverage.projects - coverage.configured) : 0);
  let selected = $state<string | null>(null);
  const columns: Column<Row>[] = [
    { key: "you", label: "", width: 36 },
    { key: "title", label: "Title", width: 520, sort: (r) => r.title },
    { key: "project", label: "Project", width: 160, sort: (r) => r.project_name },
    { key: "open", label: "", align: "end" },
  ];
</script>

<div class="page">
  <header class="head">
    <h1>Issues and pull requests</h1>
    {#if coverage}<span class="cov">{coverage.configured} of {coverage.projects} projects are on GitHub{absent ? ` — ${absent} not seen here` : ""}</span>{/if}
  </header>
  <Tabs
    tabs={[
      { id: "issues", label: "Issues", icon: "dot", count: issues.length },
      { id: "pulls", label: "Pull requests", icon: "forge", count: pulls.length },
    ]}
    bind:active={showing}
    label="what to show"
  />
  {#if !loaded}
    <p class="quiet">Asking GitHub through your own <code>gh</code>…</p>
  {:else if failed}
    <Empty icon="alert" title="The forge could not be read" body={failed} />
  {:else if shown.length === 0}
    <Empty icon="forge" title={showing === "issues" ? "No open issue" : "No open pull request"} body="Across every registered project that has a GitHub remote." limit={absent ? `${absent} projects have no GitHub remote and are not counted.` : ""} />
  {:else}
    <div class="frame">
      <Grid id="forge" {columns} rows={shown} key={(r) => r.url} group={(r) => (r.needs_you ? "Needs you" : "Everything else")} bind:selected open={(r) => window.open(r.url, "_blank", "noopener")} label={showing}>
        {#snippet cell(r, c)}
          {#if c.key === "you"}{#if r.needs_you}<span class="you" title="needs you"><Icon name="person" size={13} /></span>{/if}
          {:else if c.key === "title"}<span class="t">{r.title}</span>
          {:else if c.key === "project"}<span class="dim">{r.project_name}</span>
          {:else}<a href={r.url} target="_blank" rel="noopener noreferrer" class="open"><Icon name="external" size={13} /> open</a>{/if}
        {/snippet}
      </Grid>
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
    align-items: baseline;
    gap: var(--s-4);
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .cov {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .you {
    color: var(--wait);
  }
  .t {
    color: var(--ink);
  }
  .dim {
    color: var(--faint);
  }
  .open {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--accent);
    text-decoration: none;
    font-size: var(--t-xs);
  }
  .quiet {
    color: var(--faint);
  }
  code {
    font-family: var(--mono);
  }
</style>
