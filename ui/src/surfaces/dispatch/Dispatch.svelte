<script lang="ts">
  // Compose one prompt and open it in several projects.
  //
  // **The panel before the button is the feature.** Every other launcher in
  // this category sends and then tells you what went wrong; this one names
  // every refusal *before* anything is written, because a fan-out that half
  // fires is the one failure a control plane cannot take back.
  //
  // **It had no button at all until 2026-09-21**, and no panel either: the
  // preflight lived only in the CLI, so `preflight` stayed on its default —
  // empty — and the surface titled *Start work* could not start work. The
  // panel is `/api/dispatch/preflight` now, which is where the rule belongs:
  // *draft above three* is a number with one home, and a page re-deriving it
  // is the second place it would live.
  //
  // **Nothing here writes.** The default position is a draft, so what this
  // produces is one deep link per project — the vendor's own window, opened
  // with the prompt typed and not sent. Starting six agents from a page is the
  // `--apply` path and it stays in the CLI, where the person is already in a
  // terminal that can show them what happened.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";

  type Target = { id: string; name: string };
  /// One prompt the project already has: a portable prompt, a project skill or
  /// a personal one. `core::templates` has listed these with their description
  /// and `argument-hint` since before this surface existed, `/api/projects`
  /// has served them, and nothing read them — so a launcher whose whole job is
  /// picking one made you retype it.
  type Template = {
    name: string;
    source: string;
    description: string | null;
    hint: string | null;
  };
  type Row = {
    project: string;
    name: string;
    refusal: string | null;
    says: string | null;
    link: string | null;
    would_lose_fields?: string[];
  };

  let {
    projects = [],
    prompt = $bindable(""),
    chosen = $bindable<string[]>([]),
  }: {
    projects?: Target[];
    prompt?: string;
    chosen?: string[];
  } = $props();

  let templates = $state<Record<string, Template[]>>({});
  /// The picked template's own name, so the picker can show what is selected
  /// and so choosing a second one replaces rather than appends.
  let picked = $state("");
  /// What the artefact said it expects, as a **field**. A hint rendered as
  /// prose is a paragraph to retype, which is the thing this page exists not
  /// to make you do.
  let argument = $state("");

  /// The prompts of the chosen projects, deduplicated by name.
  ///
  /// **Union rather than intersection.** A fan-out to six repositories where
  /// five have the skill is the ordinary case; hiding it because one does not
  /// would make the picker useless exactly when it is most useful. The
  /// preflight already names, per target, what will happen there.
  const offered = $derived.by(() => {
    const seen = new Map<string, Template>();
    for (const id of chosen) {
      for (const t of templates[id] ?? []) {
        if (!seen.has(t.name)) seen.set(t.name, t);
      }
    }
    return [...seen.values()];
  });

  const pickedTemplate = $derived(offered.find((t) => t.name === picked) ?? null);

  function choose(t: Template) {
    if (picked === t.name) {
      picked = "";
      argument = "";
      return;
    }
    picked = t.name;
    argument = "";
    prompt = `/${t.name}`;
  }

  // The argument goes on the end of the invocation, which is how every one of
  // these formats takes one. Devplane owns no format here: it is typing what
  // you would have typed.
  $effect(() => {
    if (pickedTemplate) prompt = `/${picked}${argument.trim() ? ` ${argument.trim()}` : ""}`;
  });

  onMount(async () => {
    try {
      const ps = await api<Array<{ id: string; templates?: Template[] }>>("/api/projects");
      templates = Object.fromEntries(ps.map((p) => [p.id, p.templates ?? []]));
    } catch {
      // A launcher that cannot list what is written down still launches. The
      // textarea is the path that always works, and a failure here must not
      // take it away.
      templates = {};
    }
  });

  type Resolved = {
    project: string;
    name: string;
    text: string | null;
    cannot: string[];
  };

  let rows = $state<Row[]>([]);
  /// **The prompt as each target will actually receive it.**
  ///
  /// Six projects have six sets of facts; that is the point of the feature.
  /// Shown before the button is live because it is the only one of the four
  /// panels the person can be *wrong* about — a placeholder they thought said
  /// one thing and says another.
  let resolved = $state<Resolved[]>([]);
  let says = $state("");
  let forced = $state(false);
  let asked = $state(false);
  let failed = $state("");
  let busy = $state(false);

  const ready = $derived(rows.filter((r) => r.refusal === null));
  const refused = $derived(rows.filter((r) => r.refusal !== null));
  /// **A warning, never a refusal.** The artefact still works in the tool that
  /// wrote it; the documented error is about leaving it. So a target that would
  /// drop a field is still ready, and the sentence sits beside it.
  const losing = $derived(rows.filter((r) => (r.would_lose_fields ?? []).length > 0));

  /// **Asked for, never automatic.** The preflight shells out to `git` per
  /// target to see whether the worktree is dirty, so running it on every
  /// keystroke would put a subprocess per project behind a text field.
  async function check() {
    if (chosen.length === 0) return;
    busy = true;
    failed = "";
    try {
      const r = await api<{
        targets?: Row[];
        resolved?: Resolved[];
        says?: string;
        forced?: boolean;
      }>("/api/dispatch/preflight", {
        method: "POST",
        body: JSON.stringify({ projects: chosen, agent: "claude", prompt }),
      });
      rows = r.targets ?? [];
      resolved = r.resolved ?? [];
      says = r.says ?? "";
      forced = r.forced ?? false;
      asked = true;
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section aria-labelledby="dispatch-head">
  <h2 id="dispatch-head">Start work</h2>

  <textarea
    bind:value={prompt}
    rows="3"
    placeholder="what should the agent do?"
    aria-label="what should the agent do?"
  ></textarea>

  {#if offered.length > 0}
    <!-- **What is already written down.** A launcher that makes you retype the
         thing you do every Tuesday is a launcher you stop opening. -->
    <fieldset>
      <legend>or something this project already has</legend>
      <div class="templates">
        {#each offered as t (t.name)}
          <button
            type="button"
            class="template"
            class:on={picked === t.name}
            aria-pressed={picked === t.name}
            onclick={() => choose(t)}
          >
            <span class="name">/{t.name}</span>
            <span class="source">{t.source}</span>
            {#if t.description}<span class="desc">{t.description}</span>{/if}
          </button>
        {/each}
      </div>
      {#if pickedTemplate?.hint}
        <label class="arg">
          <span>{pickedTemplate.hint}</span>
          <input
            type="text"
            bind:value={argument}
            placeholder={pickedTemplate.hint}
            aria-label={`argument for /${picked}: ${pickedTemplate.hint}`}
          />
        </label>
      {/if}
    </fieldset>
  {/if}

  <fieldset>
    <legend>where</legend>
    {#if projects.length === 0}
      <p class="dim">
        No project is registered. <code>devplane trust .</code> in a repository adds it.
      </p>
    {:else}
      <!-- **A chosen project has to look chosen.** These were inline labels
           whose checkboxes the global input rule had squashed, so six projects
           read as a run of bare names with no way to tell which were picked. -->
      <div class="where">
        {#each projects as p (p.id)}
          <label class:on={chosen.includes(p.id)}>
            <input type="checkbox" value={p.id} bind:group={chosen} />
            <span>{p.name}</span>
          </label>
        {/each}
      </div>
      <p class="picked dim" aria-live="polite">
        {chosen.length === 0
          ? `none of ${projects.length} chosen`
          : `${chosen.length} of ${projects.length} chosen`}
      </p>
    {/if}
  </fieldset>

  <button
    class="primary"
    onclick={check}
    disabled={busy || chosen.length === 0 || prompt.trim() === ""}
  >
    {busy ? "checking…" : "what will happen?"}
  </button>

  {#if resolved.length > 0}
    <!-- **What each target will actually receive.** A prompt that names a fact
         about the project resolves differently in each one, and a placeholder
         somebody was wrong about is the failure this panel exists to catch. -->
    <section class="resolved">
      <h3>what each project will be sent</h3>
      {#each resolved as r (r.project)}
        <article class:refused={r.cannot.length > 0}>
          <h4>{r.name}</h4>
          {#if r.cannot.length > 0}
            <!-- **Named individually, and it is a refusal rather than a
                 blank.** A confidently wrong prompt sent to an agent is worse
                 than one that was not sent. -->
            <ul class="cannot">
              {#each r.cannot as why, wi (wi)}<li>{why}</li>{/each}
            </ul>
          {:else}
            <pre>{r.text}</pre>
          {/if}
        </article>
      {/each}
    </section>
  {/if}

  <!-- **What will happen, before it happens.** Named per target, so a person
       can fix one rather than being told the batch failed. -->
  <div class="will" aria-live="polite">
    {#if failed}
      <p class="refused">The preflight could not run: {failed}</p>
    {:else if !asked}
      <p class="dim">Choose a project, write a prompt, and this says what will happen.</p>
    {:else}
      <p>
        <b>{ready.length}</b>
        {ready.length === 1 ? "project is" : "projects are"} ready.
        {#if refused.length > 0}
          <b class="refused">{refused.length}</b> cannot take this.
        {/if}
      </p>
      <!-- The position is the daemon's and it says what it will not do. -->
      {#if says}<p class="dim">{says}{#if forced} — chosen for you, above three projects{/if}</p>{/if}

      {#if refused.length > 0}
        <ul role="list">
          {#each refused as r (r.project)}
            <li class="refused">{r.says}</li>
          {/each}
        </ul>
      {/if}

      {#each losing as l (l.project)}
        <p class="warn">
          <b>{l.name}</b> would drop {(l.would_lose_fields ?? []).join(", ")} — it still runs
          there.
        </p>
      {/each}

      {#if ready.length > 0}
        <ul role="list">
          {#each ready as r (r.project)}
            <li>
              <b>{r.name}</b>
              {#if r.link}
                <!-- A link, never a fetch: it opens the vendor's own window
                     with the prompt typed. You send it. -->
                <a href={r.link}>open a draft</a>
              {:else}
                <span class="dim">this path cannot be opened by a deep link</span>
              {/if}
            </li>
          {/each}
        </ul>
        <p class="dim">Each opens with the prompt typed. You send it.</p>
      {/if}
    {/if}
  </div>
</section>

<style>
  .resolved h3 {
    font-size: 0.85rem;
    margin: 0.8rem 0 0.3rem;
  }
  .resolved h4 {
    font-size: 0.8rem;
    margin: 0 0 0.2rem;
    color: var(--dim);
  }
  .resolved article {
    border-left: 2px solid var(--line);
    padding-left: 0.6rem;
    margin-bottom: 0.5rem;
  }
  .resolved article.refused {
    border-left-color: var(--fail);
  }
  .resolved pre {
    margin: 0;
    white-space: pre-wrap;
    font-size: 0.8rem;
  }
  .resolved .cannot {
    margin: 0;
    padding-left: 1rem;
    color: var(--fail);
    font-size: 0.8rem;
  }

  .templates {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .template {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.1rem;
    text-align: left;
    padding: 0.35rem 0.6rem;
    max-width: 22rem;
    border: 1px solid var(--line);
    background: var(--panel);
    color: var(--ink);
    cursor: pointer;
  }
  .template.on {
    border-color: var(--accent);
  }
  .template .name {
    font-family: monospace;
  }
  .template .source,
  .template .desc {
    color: var(--dim);
    font-size: 0.78rem;
  }
  .arg {
    display: block;
    margin-top: 0.5rem;
  }
  .arg span {
    display: block;
    color: var(--dim);
    font-size: 0.78rem;
  }

  h2 { font-size: 1rem; margin: 0 0 .5rem; }
  textarea { width: 100%; max-width: 44rem; font: inherit; padding: .4rem;
             background: var(--panel); color: var(--ink); border: 1px solid var(--line); }
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    margin: var(--s-3) 0;
    padding: var(--s-2) var(--s-3) var(--s-3);
  }
  legend { color: var(--dim); font-size: var(--t-xs); padding: 0 var(--s-1); }

  /* Wrapped rather than inline: a project is a target somebody points at, and
     six of them in a sentence is a sentence. */
  .where { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .where label {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-1) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    cursor: pointer;
    line-height: 1.4;
  }
  .where label:hover { background: var(--panel); }
  /* Chosen is carried by the ground and the weight as well as the tick, so it
     survives greyscale — nothing here may be distinguishable by colour alone. */
  .where label.on {
    background: var(--panel);
    border-color: var(--accent);
    font-weight: 600;
  }
  .picked { font-size: var(--t-xs); margin: var(--s-2) 0 0; }
  .will { margin-top: .6rem; }
  .will ul { list-style: none; margin: .2rem 0; padding: 0; }
  .will li { padding: .1rem 0; display: flex; gap: .6rem; align-items: baseline; }
  .refused { color: var(--fail); }
  .warn { color: var(--wait); }
  .dim { color: var(--dim); }
</style>
