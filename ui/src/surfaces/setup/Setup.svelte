<script lang="ts">
  // What is configured on this machine: Devplane, its agent, your rules, and
  // each project. Read-only: each section names the file it read and the
  // command that changes it.
  import { api } from "../../lib/api";
  import Props from "../../lib/ui/Props.svelte";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import Empty from "../../lib/ui/Empty.svelte";

  type ProjectRow = {
    id: string;
    name: string;
    root: string;
    trusted: boolean;
    config?: { path: string; exists: boolean };
    declares_gates?: boolean;
    error?: string;
  };
  type Setup = {
    machine?: { version?: string; home?: string; database?: string };
    provider?: { name?: string; because?: string; vendor_supervision?: boolean };
    connect?: {
      settings_path?: string;
      hooks_installed?: string[];
      telemetry_endpoint?: string | null;
      telemetry_is_ours?: boolean;
    } | null;
    machine_policy?: { path?: string; exists?: boolean; deny?: string[]; ask?: string[]; error?: string } | null;
    projects?: ProjectRow[];
  };

  let data = $state<Setup | null>(null);
  let failed = $state("");
  $effect(() => {
    let live = true;
    api<Setup>("/api/setup")
      .then((r) => {
        if (live) data = r;
      })
      .catch((e) => {
        if (live) failed = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });

  const hooks = $derived(data?.connect?.hooks_installed ?? []);
  const policy = $derived(data?.machine_policy ?? null);
  let selected = $state<string | null>(null);
  const columns: Column<ProjectRow>[] = [
    { key: "name", label: "Project", width: 180, sort: (r) => r.name },
    { key: "trusted", label: "Trusted", width: 110, sort: (r) => (r.trusted ? 0 : 1) },
    { key: "gates", label: "Gates", width: 130, sort: (r) => (r.declares_gates ? 0 : 1) },
    { key: "config", label: "devplane.toml", width: 150 },
    { key: "root", label: "Where", mono: true },
  ];
</script>

<div class="page">
  <h1>Setup</h1>
  {#if failed}
    <Empty icon="alert" title="Devplane could not say what is configured" body={failed} />
  {:else if !data}
    <p class="quiet">Reading the files…</p>
  {:else}
    <div class="grid">
      <section class="card">
        <h2><Icon name="settings" size={14} /> This machine</h2>
        <Props
          rows={[
            { label: "Version", value: data.machine?.version ?? null, missing: "unknown" },
            { label: "Home", value: data.machine?.home ?? null, mono: true },
            { label: "Record", value: data.machine?.database ?? null, mono: true },
            { label: "Sessions from", value: data.provider?.name ? `${data.provider.name}${data.provider.because ? ` — ${data.provider.because}` : ""}` : null },
          ]}
        />
      </section>

      <section class="card">
        <h2><Icon name="agent" size={14} /> Your agent</h2>
        {#if !data.connect}
          <p class="quiet">No agent settings file was found. <code>devplane connect claude</code> writes the hooks, and <code>devplane disconnect claude</code> removes exactly what it wrote.</p>
        {:else}
          <Props
            rows={[
              { label: "Settings", value: data.connect.settings_path ?? null, mono: true },
              { label: "Hooks", value: hooks.length ? hooks.join(", ") : null, missing: "none installed — devplane connect claude" },
              {
                label: "Telemetry",
                value: !data.connect.telemetry_endpoint ? "not exporting" : data.connect.telemetry_is_ours ? "to Devplane" : "to somebody else's collector — left alone",
              },
            ]}
          />
        {/if}
      </section>

      <section class="card wide">
        <h2><Icon name="shield" size={14} /> Rules for every project</h2>
        {#if !policy || !policy.exists}
          <p class="quiet">No <code>~/.devplane/policy.toml</code>. Rules live in each project's <code>devplane.toml</code>; a rule you want everywhere goes in that file, in the same shape.</p>
        {:else if policy.error}
          <p class="fail"><Icon name="alert" size={13} /> {policy.path} will not parse — every gated call asks until it does: {policy.error}</p>
        {:else}
          <p class="file"><code>{policy.path}</code></p>
          <div class="rules">
            <div>
              <h3>Never</h3>
              {#each policy.deny ?? [] as r (r)}<code class="rule deny">{r}</code>{:else}<span class="quiet">none</span>{/each}
            </div>
            <div>
              <h3>Always ask</h3>
              {#each policy.ask ?? [] as r (r)}<code class="rule ask">{r}</code>{:else}<span class="quiet">none</span>{/each}
            </div>
          </div>
        {/if}
      </section>
    </div>

    <section class="projects">
      <h2><Icon name="folder" size={14} /> Projects <span>{data.projects?.length ?? 0}</span></h2>
      {#if (data.projects ?? []).length === 0}
        <p class="quiet">None registered. A project is registered the first time an agent works in it, or with <code>devplane trust &lt;path&gt;</code>.</p>
      {:else}
        <div class="frame">
          <Grid id="setup-projects" {columns} rows={data.projects ?? []} key={(r) => r.id} bind:selected label="projects" dense>
            {#snippet cell(r, c)}
              {#if c.key === "name"}<b>{r.name}</b>
              {:else if c.key === "trusted"}<Pill word={r.trusted ? "trusted" : "not trusted"} as={r.trusted ? "none" : "wait"} />
              {:else if c.key === "gates"}{#if r.error}<span class="fail">unreadable</span>{:else if r.declares_gates}<span class="ok">declared</span>{:else}<span class="quiet">none declared</span>{/if}
              {:else if c.key === "config"}{r.config?.exists ? "present" : "absent"}
              {:else}{r.root}{/if}
            {/snippet}
          </Grid>
        </div>
        {#each (data.projects ?? []).filter((p) => p.error) as p (p.id)}
          <p class="fail"><Icon name="alert" size={13} /> {p.name}: {p.error}</p>
        {/each}
      {/if}
    </section>
  {/if}
</div>

<style>
  .page {
    padding: var(--s-4) var(--s-5) var(--s-6);
    display: grid;
    gap: var(--s-4);
    max-width: 90rem;
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(24rem, 1fr));
    gap: var(--s-3);
  }
  .card {
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    display: grid;
    gap: var(--s-2);
    align-content: start;
  }
  .card.wide {
    grid-column: 1 / -1;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
  }
  h2 span {
    font-weight: 400;
  }
  h3 {
    margin: 0 0 var(--s-2);
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .rules {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-4);
  }
  .rules > div {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-1);
    align-content: start;
  }
  .rules h3 {
    flex-basis: 100%;
  }
  .rule {
    font-family: var(--mono);
    font-size: var(--t-xs);
    padding: 0.1rem 0.45rem;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--bg);
  }
  .rule.deny {
    color: var(--fail);
  }
  .rule.ask {
    color: var(--wait);
  }
  .projects {
    display: grid;
    gap: var(--s-2);
  }
  .frame {
    height: min(50vh, 26rem);
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .file {
    margin: 0;
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
    margin: 0;
  }
  .fail {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    color: var(--fail);
    font-size: var(--t-sm);
    margin: 0;
  }
  /* A declared gate is a promise, not a pass: not the verified green. */
  .ok {
    color: var(--ink);
  }
</style>
