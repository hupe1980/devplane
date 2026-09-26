<script lang="ts" module>
  export type Target = { asked: string; name: string; root: string | null; refusal: string | null; says: string; notes: string[] };

  /// Whether Start can do what it says: the host answered for every project
  /// and refused none.
  export function startable(targets: Target[] | null, checking: boolean, failed: string): boolean {
    return !!targets && targets.length > 0 && targets.every((t) => !t.refusal) && !checking && !failed;
  }
</script>

<script lang="ts">
  // Starting a change: where, what, by whom, and what it would do in each
  // project. The live preflight sends the exact start body, writes nothing,
  // and lists every refusal by project; Start is enabled only when none is
  // refused. A fresh worktree's cost is said before you commit.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import { go } from "../../lib/route";
  import { run } from "../../lib/keys";
  import Icon from "../../lib/ui/Icon.svelte";

  type Project = { id: string; name: string; root: string; trusted: boolean; default_agent?: string | null };
  type Agent = { id: string; name?: string; command?: string };
  type Plan = { plan: { path: string; progress?: { done: number; total: number } | null }; change_id: string | null };
  type SpecProject = { project_id: string; project: string; root: string; plans: Plan[] };

  let {
    changeSurface = "change",
    targets: planted = null,
  }: {
    changeSurface?: string;
    /// A preflight already answered — what a harness hands the form.
    targets?: Target[] | null;
  } = $props();

  // Three reads, each with its own failure: a host that did not answer is
  // never *No project is registered yet*.
  const projectsRead = resource<Project[]>(() => "/api/projects", { tell: () => "devplane doctor" });
  const agentsRead = resource<Agent[]>(() => "/api/agents", { tell: () => "devplane agents" });
  const specsRead = resource<{ projects: SpecProject[] }>(() => "/api/specs", { tell: () => "devplane doctor" });
  const projects = $derived(Array.isArray(projectsRead.data) ? projectsRead.data : []);
  const agents = $derived(Array.isArray(agentsRead.data) ? agentsRead.data : []);
  const specs = $derived(specsRead.data?.projects ?? []);

  let chosen = $state<string[]>([]);
  let title = $state("");
  let prompt = $state("");
  let agent = $state("");
  let spec = $state("");
  let worktree = $state(true);
  let filter = $state("");

  const shownProjects = $derived(
    projects.filter((p) => !filter.trim() || p.name.toLowerCase().includes(filter.trim().toLowerCase())),
  );
  const one = $derived(chosen.length === 1 ? projects.find((p) => p.id === chosen[0]) : null);
  const plans = $derived(one ? (specs.find((s) => s.root === one.root || s.project_id === one.id)?.plans ?? []) : []);
  function toggle(id: string) {
    chosen = chosen.includes(id) ? chosen.filter((x) => x !== id) : [...chosen, id];
    if (chosen.length !== 1) spec = "";
  }

  const body = $derived({
    projects: chosen,
    title: title.trim(),
    prompt: prompt.trim() || null,
    agent: agent || null,
    spec: spec || null,
    worktree,
  });

  // ── the preflight, debounced ─────────────────────────────────────────────
  let fetchedTargets = $state<Target[] | null>(null);
  const targets = $derived(fetchedTargets ?? planted);
  let checking = $state(false);
  let checkError = $state("");
  $effect(() => {
    const b = body;
    if (b.projects.length === 0 || !b.title) {
      fetchedTargets = null;
      checkError = "";
      return;
    }
    checking = true;
    let live = true;
    const t = setTimeout(() => {
      api<{ targets: Target[] }>("/api/changes/preflight", { method: "POST", body: JSON.stringify(b) })
        .then((r) => {
          if (live) {
            fetchedTargets = r.targets ?? [];
            checkError = "";
          }
        })
        .catch((e) => {
          if (live) checkError = e instanceof Error ? e.message : String(e);
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
  const refused = $derived((targets ?? []).filter((t) => t.refusal));
  const ready = $derived(startable(targets, checking, checkError));

  /// Creating a worktree and installing into it takes as long as it takes:
  /// no timeout, the elapsed time on the button, and no second start.
  const starting = writer();
  let said = $state("");
  async function start(e: Event) {
    e.preventDefault();
    if (!ready || starting.busy) return;
    await starting.run("start", async () => {
      try {
        const r = await api<{ change_id?: string; changes?: Array<{ change_id: string }>; error?: string }>("/api/changes", {
          method: "POST",
          body: JSON.stringify(body),
        });
        const first = r.change_id ?? r.changes?.[0]?.change_id;
        if (r.error) said = r.error;
        if (first) go(`#${changeSurface}/${encodeURIComponent(first)}`);
      } catch (err) {
        const f = failure(err, "devplane change list");
        said = `Nothing was started: ${f.says}. \`${f.tell}\` tells more.`;
      }
    });
  }
  const close = () => run("leave", "new");
</script>

<form class="new" onsubmit={start} aria-label="start a new change">
  <header>
    <Icon name="change" size={16} />
    <h1>New change</h1>
    <button type="button" class="x" aria-label="close" onclick={close}><Icon name="x" size={14} /></button>
  </header>

  <div class="body">
    <section>
      <span class="label">Where <span class="hint">one project, or the same change in several</span></span>
      {#if projects.length > 6}
        <input class="filter" bind:value={filter} placeholder="Filter projects" aria-label="filter projects" />
      {/if}
      <div class="projects" role="group" aria-label="projects">
        {#each shownProjects as p (p.id)}
          <button type="button" class="proj" class:on={chosen.includes(p.id)} aria-pressed={chosen.includes(p.id)} onclick={() => toggle(p.id)}>
            <Icon name={chosen.includes(p.id) ? "check" : "folder"} size={13} />
            {p.name}
            {#if !p.trusted}<span class="untrusted">not trusted</span>{/if}
          </button>
        {:else}
          {#if projectsRead.phase === "failed" && projectsRead.failure}
            <Failed what="the projects" failure={projectsRead.failure} />
          {:else if projectsRead.phase === "loading"}
            <p class="quiet">Reading the projects…</p>
          {:else}
            <p class="quiet">No project is registered yet. <code>devplane trust &lt;path&gt;</code> adds one.</p>
          {/if}
        {/each}
      </div>
    </section>

    {#if one && specsRead.failure}
      <Failed what="the specifications, so none can be picked" failure={specsRead.failure} />
    {/if}
    {#if one && plans.length > 0}
      <section>
        <label class="label" for="spec">Specification <span class="hint">optional — the change works to it, and its tasks are traced</span></label>
        <select id="spec" bind:value={spec}>
          <option value="">none — just the words below</option>
          {#each plans as p (p.plan.path)}
            <option value={p.plan.path} disabled={!!p.change_id}>{p.plan.path}{p.plan.progress ? ` · ${p.plan.progress.done} ticked · ${p.plan.progress.total} tasks` : ""}{p.change_id ? " · a change already works to it" : ""}</option>
          {/each}
        </select>
      </section>
    {/if}

    <section>
      <label class="label" for="title">Title <span class="hint">also the branch name</span></label>
      <!-- svelte-ignore a11y_autofocus -->
      <input id="title" bind:value={title} placeholder="Rate-limit the login route" autofocus />
    </section>

    <section>
      <label class="label" for="prompt">What to do <span class="hint">the agent's first prompt; the title is used when empty</span></label>
      <textarea id="prompt" bind:value={prompt} rows="5" placeholder="Five failed logins a minute per account; the sixth is rejected with Retry-After."></textarea>
    </section>

    <div class="row">
      <section>
        <label class="label" for="agent">Agent</label>
        <select id="agent" bind:value={agent}>
          <option value="">{agentsRead.failure ? "the project's default (the agents could not be read)" : "the project's default"}</option>
          {#each agents as a (a.id)}<option value={a.id}>{a.name ?? a.id}</option>{/each}
        </select>
      </section>
      <section>
        <span class="label">Isolation</span>
        <label class="check">
          <input type="checkbox" bind:checked={worktree} />
          Its own worktree <span class="hint">{worktree ? "your checkout is never touched" : "in place — no parallel safety, and the diff is against your working tree"}</span>
        </label>
      </section>
    </div>

    <section class="preflight" aria-live="polite">
      <span class="label">Before anything is created</span>
      {#if !targets && (chosen.length === 0 || !title.trim())}
        <p class="quiet">Pick where and give it a title, and what starting it would do is shown here.</p>
      {:else if checkError}
        <p class="fail"><Icon name="alert" size={13} /> {checkError}</p>
      {:else if !targets}
        <p class="quiet">Checking…</p>
      {:else}
        <ul>
          {#each targets as t (t.asked)}
            <li class:bad={!!t.refusal}>
              <Icon name={t.refusal ? "x" : "check"} size={14} />
              <b>{t.name}</b>
              <span>{t.says}</span>
              {#each t.notes as n, i (i)}<span class="note">{n}</span>{/each}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
    {#if said}<p class="fail">{said}</p>{/if}
  </div>

  <footer>
    <span class="quiet">{refused.length ? `${refused.length} refused — nothing will be created until every project can start` : chosen.length > 1 ? `${chosen.length} changes, one per project` : ""}</span>
    <button type="button" onclick={close}>Cancel</button>
    <button type="submit" class="primary" disabled={!ready || !!starting.busy}>
      <Icon name="play" size={13} /> {starting.busy ? `Starting · ${starting.elapsed}` : chosen.length > 1 ? `Start ${chosen.length} changes` : "Start the change"}
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
  .x {
    margin-left: auto;
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
  input:not([type="checkbox"]),
  textarea,
  select {
    width: 100%;
    font: inherit;
    font-size: var(--t-sm);
  }
  .projects {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .proj {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.9rem;
    padding: 0 0.7rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--dim);
    font-size: var(--t-sm);
    cursor: pointer;
  }
  .proj.on {
    border-color: var(--accent);
    background: var(--select);
    color: var(--ink);
  }
  .untrusted {
    font-size: 0.625rem;
    color: var(--wait);
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-4);
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .preflight ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-2);
  }
  .preflight li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .preflight li b,
  .preflight li span {
    color: var(--ink);
  }
  .preflight li.bad {
    color: var(--fail);
    border-color: var(--fail);
  }
  .note {
    flex-basis: 100%;
    color: var(--faint) !important;
    font-size: var(--t-xs);
    padding-left: 1.4rem;
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
  code {
    font-family: var(--mono);
  }
</style>
