<script lang="ts">
  // Trusting a repository: one found from a running session, or one added by
  // its path. Before the person trusts it, what trusting would allow — its
  // hooks, MCP servers, skills and allow rules — is shown as quoted repository
  // text. Reading it registers nothing and writes nothing; only Trust does.
  import { api } from "../../lib/api";
  import { resource } from "../../lib/resource.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Inline from "../../lib/ui/Inline.svelte";

  type Finding = { source: string; kind: string; subject: string; detail: string };
  type Scan = {
    root: string;
    name: string | null;
    repository: boolean;
    trusted: boolean;
    findings: Finding[];
    skills: number;
    unreadable: string[];
  };
  type Project = { id: string; name: string; root: string; trusted: boolean; sessions?: number };

  let { ontrusted, firstRun = false }: { ontrusted?: (root: string) => void; firstRun?: boolean } = $props();

  const projects = resource<Project[]>(() => "/api/projects", { tell: () => "devplane doctor" });
  const found = $derived((Array.isArray(projects.data) ? projects.data : []).filter((p) => !p.trusted));

  let path = $state("");
  let scan = $state<Scan | null>(null);
  let reading = $state(false);
  let trusting = $state(false);
  let said = $state("");
  async function read(p: string) {
    const want = p.trim();
    if (!want || reading) return;
    reading = true;
    said = "";
    scan = null;
    try {
      scan = await api<Scan>(`/api/projects/scan?path=${encodeURIComponent(want)}`);
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
    } finally {
      reading = false;
    }
  }
  async function trust() {
    if (!scan || trusting) return;
    trusting = true;
    said = "";
    try {
      await api("/api/projects/trust", { method: "POST", body: JSON.stringify({ path: scan.root }) });
      const root = scan.root;
      scan = null;
      path = "";
      void projects.reload();
      ontrusted?.(root);
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
    } finally {
      trusting = false;
    }
  }
  const kinds: Array<[string, string]> = [
    ["hook", "Hooks it runs"],
    ["mcp", "MCP servers it starts"],
    ["skill", "Skills that pre-approve tools"],
    ["policy", "Allow rules it brings"],
  ];
</script>

<div class="trust">
  {#if found.length}
    <div class="found">
      <p class="label">Found from sessions on this machine</p>
      {#each found as p (p.id)}
        <div class="row">
          <Icon name="folder" size={13} /> <b>{p.name}</b> <code>{p.root}</code>
          <button type="button" onclick={() => read(p.root)} disabled={reading}>Read what it would allow</button>
        </div>
      {/each}
    </div>
  {/if}
  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void read(path);
    }}
  >
    <label for="trust-path" class="label">Add a repository by its path</label>
    <div class="line">
      <input
        id="trust-path"
        bind:value={path}
        placeholder="/path/to/your/repository"
        autocomplete="off"
        spellcheck="false"
        data-first-run-step={firstRun ? "add-repository" : undefined}
      />
      <button type="submit" disabled={!path.trim() || reading}>{reading ? "Reading…" : "Read it"}</button>
    </div>
  </form>

  {#if scan}
    <div class="scan" aria-label="what trusting {scan.root} would allow">
      <p><b>{scan.name ?? scan.root}</b> <code>{scan.root}</code>{#if !scan.repository} <span class="warn">not a git repository — a change here cannot be isolated</span>{/if}</p>
      {#if scan.trusted}
        <p class="quiet">Already trusted.</p>
      {:else}
        <p class="quiet">
          Trusting lets an agent Devplane starts here run this repository's own configuration without asking.
          {#if scan.findings.length === 0 && scan.unreadable.length === 0}
            Nothing in it asks to run anything{scan.skills ? `; it ships ${scan.skills} skill${scan.skills === 1 ? "" : "s"}, none pre-approving tools` : ""}.
          {/if}
        </p>
        {#each kinds as [k, title] (k)}
          {@const rows = scan.findings.filter((f) => f.kind === k)}
          {#if rows.length}
            <div class="kind">
              <p class="label">{title}</p>
              <ul>
                {#each rows as f, i (i)}
                  <li>
                    <code data-quoted>{f.subject}</code>
                    <span class="src">{f.source}</span>
                    <span class="detail"><Inline text={f.detail} /></span>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        {/each}
        {#if scan.unreadable.length}
          <p class="warn">Could not be read, so what they run is not shown here and is still run by the agent: {scan.unreadable.join(", ")}</p>
        {/if}
        <div>
          <button type="button" class="primary" onclick={trust} disabled={trusting} data-first-run-step={firstRun ? "trust" : undefined}>
            <Icon name="shield" size={13} /> {trusting ? "Trusting…" : `Trust ${scan.name ?? "this repository"}`}
          </button>
        </div>
      {/if}
    </div>
  {/if}
  {#if said}<p class="fail" role="alert"><Inline text={said} /></p>{/if}
</div>

<style>
  .trust {
    display: grid;
    gap: var(--s-3);
  }
  p {
    margin: 0;
  }
  .label {
    display: block;
    font-size: var(--t-xs);
    color: var(--faint);
    margin-bottom: var(--s-1);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .line {
    display: flex;
    gap: var(--s-2);
  }
  .line input {
    flex: 1;
    min-width: 0;
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
    overflow-wrap: anywhere;
  }
  .scan {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    font-size: var(--t-sm);
  }
  ul {
    margin: 0;
    padding-left: var(--s-4);
    display: grid;
    gap: var(--s-1);
  }
  .src {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  .detail {
    display: block;
    color: var(--dim);
    font-size: var(--t-xs);
  }
  .warn {
    color: var(--wait);
  }
  .quiet {
    color: var(--dim);
  }
  .fail {
    color: var(--fail);
    font-size: var(--t-sm);
  }
  .primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
</style>
