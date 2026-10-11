<script lang="ts">
  // Drafting a specification by the project's own methodology. Two sentences
  // of intent go to the agent after the methodology's own command — Spec
  // Kit's `/speckit-specify` as that agent spells it, or OpenSpec's installed
  // propose workflow — and nothing else. What would be sent is shown, from
  // the host, before anything is created; the result is the change's diff.
  import { api } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import { go } from "../../lib/route";
  import Icon from "../../lib/ui/Icon.svelte";

  type Project = { id: string; name: string; root: string; trusted: boolean };
  type Agent = { id: string; name?: string };
  type Preview = { sent?: string; folder_says?: string; offered?: string[]; refused?: string; says?: string; error?: string };

  let {
    projects,
    agents,
    changeSurface = "change",
    implement,
    close,
  }: {
    projects: Project[];
    agents: Agent[];
    changeSurface?: string;
    /// Back to starting a change from a prompt.
    implement: () => void;
    close: () => void;
  } = $props();

  let project = $state("");
  let name = $state("");
  let intent = $state("");
  let agent = $state("");
  let methodology = $state("");
  $effect(() => {
    if (!project && projects.length === 1) project = projects[0].id;
  });

  const body = $derived({
    project,
    name: name.trim(),
    intent,
    agent: agent || null,
    methodology: methodology || null,
  });

  // What would be sent, asked of the host as the person types: nothing is made.
  let preview = $state<Preview | null>(null);
  let checking = $state(false);
  $effect(() => {
    const b = body;
    if (!b.project || !b.intent.trim()) {
      preview = null;
      return;
    }
    checking = true;
    let live = true;
    const t = setTimeout(() => {
      api<Preview>("/api/changes/draft", { method: "POST", body: JSON.stringify({ ...b, preview: true }) })
        .then((r) => {
          if (live) preview = r;
        })
        .catch((e) => {
          const said = e instanceof Error ? e.message : String(e);
          const refusal = (e as { body?: Preview }).body ?? null;
          if (live) preview = refusal ?? { error: said };
        })
        .finally(() => {
          if (live) checking = false;
        });
    }, 350);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });
  const offered = $derived(preview?.offered ?? []);
  const ready = $derived(!!preview?.sent && !preview?.error && !!body.name && !checking);

  const starting = writer();
  let said = $state("");
  async function start(e: Event) {
    e.preventDefault();
    if (!ready || starting.busy) return;
    await starting.run("draft", async () => {
      try {
        const r = await api<{ change?: string; error?: string }>("/api/changes/draft", {
          method: "POST",
          body: JSON.stringify(body),
        });
        if (r.error) said = r.error;
        if (r.change) go(`#${changeSurface}/${encodeURIComponent(r.change)}`);
      } catch (err) {
        const f = failure(err, "devplane change list");
        said = `Nothing was started: ${f.says}.`;
      }
    });
  }
</script>

<form class="new" onsubmit={start} aria-label="draft a specification">
  <header>
    <Icon name="spec" size={16} />
    <h1>Draft a specification</h1>
    <button type="button" class="mode" onclick={implement}>Start from a prompt instead</button>
    <button type="button" class="x" aria-label="close" onclick={close}><Icon name="x" size={14} /></button>
  </header>

  <div class="body">
    <p class="quiet">
      Your words go to the agent after the project's own specification command, and nothing else. Devplane writes no word of the
      specification; what the command writes is this change's diff, which you review and then accept or reject.
    </p>
    <section>
      <label class="label" for="draft-project">Where</label>
      <select id="draft-project" bind:value={project}>
        <option value="">choose a project</option>
        {#each projects as p (p.id)}<option value={p.id} disabled={!p.trusted}>{p.name}{p.trusted ? "" : " · not trusted"}</option>{/each}
      </select>
    </section>
    <section>
      <label class="label" for="draft-name">Name <span class="hint">the branch, and the folder Spec Kit writes into</span></label>
      <input id="draft-name" bind:value={name} placeholder="Rate limit" />
    </section>
    <section>
      <label class="label" for="draft-intent">What it is for <span class="hint">sent verbatim</span></label>
      <textarea id="draft-intent" bind:value={intent} rows="4" placeholder="Limit login attempts per account. Lock the account after five tries in a row."></textarea>
    </section>
    <div class="row">
      <section>
        <label class="label" for="draft-agent">Agent</label>
        <select id="draft-agent" bind:value={agent}>
          <option value="">the project's default</option>
          {#each agents as a (a.id)}<option value={a.id}>{a.name ?? a.id}</option>{/each}
        </select>
      </section>
      {#if offered.length > 1}
        <section>
          <label class="label" for="draft-methodology">Methodology <span class="hint">this project has both</span></label>
          <select id="draft-methodology" bind:value={methodology}>
            <option value="">choose one</option>
            <option value="speckit">Spec Kit</option>
            <option value="openspec">OpenSpec</option>
          </select>
        </section>
      {/if}
    </div>

    <section class="preflight" aria-live="polite">
      <span class="label">What the agent will be sent</span>
      {#if !project || !intent.trim()}
        <p class="quiet">Choose where and write what it is for, and the exact text sent is shown here.</p>
      {:else if checking && !preview}
        <p class="quiet">Checking…</p>
      {:else if preview?.sent}
        <pre class="sent">{preview.sent}</pre>
        {#if preview.folder_says}<p class="quiet">{preview.folder_says}</p>{/if}
      {:else if preview}
        <p class="fail"><Icon name="x" size={13} /> {preview.says ?? preview.error}</p>
      {/if}
    </section>
    {#if said}<p class="fail">{said}</p>{/if}
  </div>

  <footer>
    <span class="quiet">{body.name ? "" : "Name the change to draft it"}</span>
    <button type="button" onclick={close}>Cancel</button>
    <button type="submit" class="primary" disabled={!ready || !!starting.busy}>
      <Icon name="play" size={13} /> {starting.busy ? `Starting · ${starting.elapsed}` : "Draft it"}
    </button>
  </footer>
</form>

<style>
  .new {
    display: flex;
    flex-direction: column;
    max-height: 80vh;
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border-bottom: 1px solid var(--line);
  }
  h1 {
    margin: 0;
    font-size: var(--t-md);
  }
  .mode {
    margin-left: auto;
    font-size: var(--t-xs);
  }
  .x {
    border: 0;
    background: none;
    color: var(--dim);
    padding: 0.2rem;
  }
  .body {
    padding: var(--s-4);
    display: grid;
    gap: var(--s-4);
    overflow: auto;
  }
  section {
    display: grid;
    gap: var(--s-2);
  }
  .label {
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--dim);
  }
  .hint {
    font-weight: 400;
    color: var(--faint);
  }
  input,
  textarea,
  select {
    width: 100%;
    font: inherit;
    font-size: var(--t-sm);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-4);
  }
  .sent {
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    font-family: var(--mono);
    font-size: var(--t-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  footer {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border-top: 1px solid var(--line);
  }
  footer .quiet {
    margin-right: auto;
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
  .primary:disabled {
    opacity: 0.5;
  }
  .quiet {
    margin: 0;
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .fail {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: 0;
    color: var(--fail);
    font-size: var(--t-sm);
  }
</style>
