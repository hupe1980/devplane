<script lang="ts">
  // What is configured, and why there is nothing to edit.
  //
  // **A person who came looking for a settings form deserves an answer rather
  // than a missing button.** Every value here is a file: Devplane reads them
  // and never writes them, because the rules are committed and reviewed like
  // code — and an agent on this machine runs as the same user, so a route that
  // edited them would be reachable by the party they exist to bound.
  //
  // **It fetches, and until 2026-09-21 it did not.** Its `select` returned
  // `{}`, so the component rendered its own defaults and told every reader
  // *"Nothing is configured for this project yet"* — on a machine with hooks
  // installed, telemetry running and gates declared. A surface that renders a
  // false statement is worse than one that is missing: the missing one sends
  // somebody to the CLI, and this one answered their question wrongly.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";

  type Connect = {
    settings_path?: string;
    hooks_installed?: string[];
    telemetry_endpoint?: string | null;
    telemetry_is_ours?: boolean;
    allowlist_blocks_us?: boolean;
    gate_is_stale?: boolean;
  };
  type ProjectRow = {
    id: string;
    name: string;
    root: string;
    trusted: boolean;
    config?: { path: string; exists: boolean };
    verified?: boolean;
    error?: string;
  };
  type Setup = {
    machine?: { version?: string; home?: string; database?: string };
    provider?: { name?: string; because?: string; vendor_supervision?: boolean };
    connect?: Connect | null;
    machine_policy?: { path?: string; exists?: boolean; deny?: string[]; ask?: string[]; error?: string } | null;
    projects?: ProjectRow[];
  };

  let data = $state<Setup | null>(null);
  let failed = $state("");

  onMount(async () => {
    try {
      data = await api<Setup>("/api/setup");
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    }
  });

  const hooks = $derived(data?.connect?.hooks_installed ?? []);
  const projects = $derived(data?.projects ?? []);
  const policy = $derived(data?.machine_policy ?? null);
</script>

<section aria-labelledby="setup-head">
  <h2 id="setup-head">What is configured</h2>

  {#if failed}
    <p class="empty" role="status">Devplane could not say what is configured: {failed}</p>
  {:else if !data}
    <p class="empty">Reading the files…</p>
  {:else}
    <h3>This machine</h3>
    <dl>
      <dt>version</dt>
      <dd>{data.machine?.version ?? "unknown"}</dd>
      {#if data.machine?.home}<dt>home</dt><dd><code>{data.machine.home}</code></dd>{/if}
      {#if data.provider?.name}
        <dt>provider</dt>
        <!-- The sentence is the daemon's: whether a vendor supervision surface
             exists at all is a fact about somebody's subscription, and a page
             that worded it would be guessing about their plan. -->
        <dd>{data.provider.name}{#if data.provider.because} — {data.provider.because}{/if}</dd>
      {/if}
    </dl>

    <h3>Your agent</h3>
    {#if !data.connect}
      <p class="empty">
        No agent settings file was found. <code>devplane connect claude</code> writes one.
      </p>
    {:else}
      <dl>
        <dt>settings</dt>
        <dd><code>{data.connect.settings_path}</code></dd>
        <dt>hooks</dt>
        <dd>
          {#if hooks.length === 0}
            none installed — <code>devplane connect claude</code>
          {:else}
            {hooks.join(", ")}
          {/if}
        </dd>
        <dt>telemetry</dt>
        <dd>
          {#if !data.connect.telemetry_endpoint}
            not exporting
          {:else if data.connect.telemetry_is_ours}
            to Devplane
          {:else}
            to somebody else's collector — left alone
          {/if}
        </dd>
      </dl>
      <!-- Two states that look like working and are not. Both are the same
           failure shape: the hooks are installed and decide nothing. -->
      {#if data.connect.allowlist_blocks_us}
        <p class="warn">
          An <code>allowedHttpHookUrls</code> list is set and does not cover loopback, so every
          hook Devplane installed is silently disabled.
        </p>
      {/if}
      {#if data.connect.gate_is_stale}
        <p class="warn">
          These hooks were installed before the gate became two events, so prohibitions do not
          reach a session in auto mode. <code>devplane connect claude</code> again.
        </p>
      {/if}
    {/if}

    <h3>Machine-wide rules</h3>
    {#if !policy || !policy.exists}
      <p class="empty">
        No <code>policy.toml</code>. Rules live in each project's <code>devplane.toml</code>.
      </p>
    {:else if policy.error}
      <p class="warn">{policy.path} will not parse: {policy.error}</p>
    {:else}
      <dl>
        <dt>file</dt>
        <dd><code>{policy.path}</code></dd>
        {#if (policy.deny ?? []).length > 0}<dt>never</dt><dd>{(policy.deny ?? []).join(" · ")}</dd>{/if}
        {#if (policy.ask ?? []).length > 0}<dt>always ask</dt><dd>{(policy.ask ?? []).join(" · ")}</dd>{/if}
      </dl>
    {/if}

    <h3>Projects ({projects.length})</h3>
    {#if projects.length === 0}
      <p class="empty">
        No project is registered yet. <code>devplane trust .</code> in a repository adds it.
      </p>
    {:else}
      <ul role="list">
        {#each projects as p (p.id)}
          <li>
            <b>{p.name}</b>
            <code class="path">{p.root}</code>
            {#if !p.trusted}<span class="warn">not trusted</span>{/if}
            {#if p.error}
              <span class="warn">its devplane.toml will not parse: {p.error}</span>
            {:else if !p.config?.exists}
              <span class="dim">no devplane.toml</span>
            {:else if p.verified === false}
              <span class="dim">declares no checks — nothing is verified</span>
            {:else if p.verified}
              <span class="ok">gates declared</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <p class="foot">
    Every value here is a file. Devplane reads them and never writes them: the rules are
    committed and reviewed like code, and an agent on this machine runs as you.
  </p>
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .3rem; }
  h3 { font-size: .85rem; color: var(--dim); margin: .9rem 0 .2rem; font-weight: 600; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: .15rem .8rem; margin: .3rem 0; }
  dt { color: var(--dim); font-size: .82rem; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  ul { list-style: none; margin: .2rem 0; padding: 0; }
  li { display: flex; flex-wrap: wrap; gap: .5rem; align-items: baseline; padding: .1rem 0; }
  .path { color: var(--dim); font-size: .82rem; overflow-wrap: anywhere; }
  .dim, .empty, .foot { color: var(--dim); }
  .warn { color: var(--wait); }
  .ok { color: var(--done); }
  .foot { font-size: .82rem; margin-top: .9rem; max-width: 70ch; }
  .empty { max-width: 70ch; }
</style>
