<script lang="ts">
  // The first run, in order, ending at a change that reads *verified*. Every
  // step's state is read from the machine on each poll — hooks present, a
  // repository trusted, a check declared, a change verified — never from a
  // remembered tick. Every step can be skipped; a skipped step stays listed
  // with a way back. Each write is a diff confirmed in its own step.
  import { resource } from "../../lib/resource.svelte";
  import { run } from "../../lib/keys";
  import { ago } from "../../lib/text";
  import Failed from "../../lib/Failed.svelte";
  import Skeleton from "../../lib/Skeleton.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import Connect from "./Connect.svelte";
  import Trust from "./Trust.svelte";
  import Declare from "./Declare.svelte";
  import { TITLES, incomplete, keepSkipped, skippedSteps, type FirstRun, type Step } from "./checklist";

  let { changeSurface = "", ongithub }: { changeSurface?: string; ongithub?: () => void } = $props();

  let chosen = $state("");
  const read = resource<FirstRun>(() => (chosen ? `/api/first-run?project=${encodeURIComponent(chosen)}` : "/api/first-run"), {
    every: 3_000,
    tell: () => "devplane doctor",
  });
  const data = $derived(read.data);

  let skipped = $state<string[]>(skippedSteps());
  function skip(id: string) {
    skipped = [...new Set([...skipped, id])];
    keepSkipped(skipped);
  }
  function back(id: string) {
    skipped = skipped.filter((x) => x !== id);
    keepSkipped(skipped);
  }
  /// The step the window opens: the first neither done nor skipped.
  const open = $derived(data?.steps.find((s) => s.state !== "done" && !skipped.includes(s.id))?.id ?? "");
  const doneCount = $derived(data ? data.steps.filter((s) => s.state === "done").length : 0);
  /// Every step done or skipped: the checklist folds to one line, and stays
  /// reachable from it.
  const complete = $derived(!!data && !incomplete(data.steps, skipped));
  let expanded = $state(false);

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 1_000);
    return () => clearInterval(t);
  });

  function newChange() {
    try {
      if (data?.project) sessionStorage.setItem("vp-new-project", data.project);
    } catch {
      /* the form asks for the project */
    }
    run("new-change", "");
  }
  const reload = () => void read.reload();
  const word = (s: Step) => (skipped.includes(s.id) && s.state !== "done" ? "skipped" : s.id === open ? "next" : s.state);
</script>

<section class="checklist" aria-labelledby="first-run-title">
  <header>
    <h2 id="first-run-title"><Icon name="check" size={14} /> First run</h2>
    {#if data}<span class="count">{doneCount} of {data.steps.length} done{skipped.length ? ` · ${data.steps.filter((s) => skipped.includes(s.id) && s.state !== "done").length} skipped` : ""}</span>{/if}
    <span class="gap"></span>
    {#if data && data.trusted.length > 1}
      <label class="pick">
        Project
        <select bind:value={chosen} aria-label="the project the last steps are about">
          {#each data.trusted as p (p.id)}<option value={p.id} selected={p.id === data.project}>{p.name}</option>{/each}
        </select>
      </label>
    {/if}
  </header>
  {#if !complete || expanded}<p class="lead">Nothing is configured until you confirm the exact change it makes. Every step can be skipped.</p>{/if}

  {#if read.phase === "failed" && read.failure}
    <Failed what="the first-run checklist" failure={read.failure} />
  {:else if !data}
    <Skeleton rows={7} />
  {:else if complete && !expanded}
    <p class="quiet">Every step is done or skipped. <button type="button" class="small" onclick={() => (expanded = true)}>Show the first-run checklist</button></p>
  {:else}
    <ol class="steps">
      {#each data.steps as s, i (s.id)}
        {@const w = word(s)}
        <li class="step {w}" data-first-run-row={s.id}>
          <div class="line">
            <span class="mark" aria-hidden="true">
              {#if w === "done"}<Icon name="check" size={13} />{:else if w === "skipped"}<Icon name="skip" size={13} />{:else}{i + 1}{/if}
            </span>
            <span class="title">{TITLES[s.id] ?? s.id}</span>
            <span class="state">{w}</span>
            <span class="says"><Inline text={s.says} /></span>
            {#if s.writes && w !== "done"}<span class="writes">would write {s.writes}</span>{/if}
            <span class="gap"></span>
            {#if w === "skipped"}
              <button type="button" class="small" onclick={() => back(s.id)}>Go back</button>
            {:else if w !== "done" && !s.advances_without_input && s.id !== "connect"}
              <button type="button" class="small" onclick={() => skip(s.id)}>Skip</button>
            {/if}
          </div>
          {#if s.id === "updates" && s.state === "done"}
            <!-- This build has no updater, so there is no question to answer:
                 the row says so and moves on without an input. -->
            <span class="sr-only" data-first-run-step="update" data-first-run-done="true">update checks: {s.says}</span>
          {/if}
          {#if s.id === open}
            <div class="body">
              {#if s.id === "connect"}
                <Connect agents={data.agents} firstRun onchanged={reload} onskip={() => skip("connect")} />
              {:else if s.id === "trust"}
                <Trust firstRun ontrusted={reload} />
              {:else if s.id === "github"}
                <p>A registered project's remote is on GitHub. Signing in lets Devplane read its issues and pull requests; skipping holds nothing else.</p>
                <div><button type="button" onclick={() => ongithub?.()}>Open the GitHub sign-in</button></div>
              {:else if s.id === "declare" && data.project}
                <Declare project={data.project} firstRun ondone={reload} />
              {:else if s.id === "first_change"}
                {#if data.latest}
                  <p class="latest">
                    {#if changeSurface}<a href={`#${changeSurface}/${encodeURIComponent(data.latest.id)}`}>{data.latest.title}</a>{:else}<b>{data.latest.title}</b>{/if}
                    · {data.latest.state}
                    {#if data.latest.check_failed}· not verified — {data.latest.check_failed}; the gate's output is on the change's Gates tab{:else if data.latest.state !== "verified" && data.latest.created_at}· started {ago(Math.max(0, (now - Date.parse(data.latest.created_at)) / 1000))} ago{/if}
                  </p>
                {/if}
                <div class="go">
                  <button type="button" class="primary" onclick={newChange}><Icon name="plus" size={13} /> New change</button>
                  <span class="quiet">The checklist ends when the change reads verified.</span>
                </div>
              {/if}
            </div>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .checklist {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--edge);
    border-radius: var(--radius-lg);
    background: var(--panel);
  }
  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: var(--t-md);
  }
  .count,
  .lead,
  .quiet {
    color: var(--dim);
    font-size: var(--t-sm);
  }
  .lead {
    margin: 0;
  }
  .gap {
    flex: 1;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .step {
    border-radius: var(--radius);
    padding: var(--s-2);
  }
  .step.next {
    background: var(--bg);
    border: 1px solid var(--line);
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .mark {
    display: inline-grid;
    place-items: center;
    width: 1.3rem;
    height: 1.3rem;
    border-radius: 50%;
    border: 1px solid var(--line);
    font-size: var(--t-xs);
    color: var(--dim);
    align-self: center;
  }
  .done .mark {
    color: var(--ink);
  }
  .title {
    font-weight: 600;
  }
  .state {
    font-size: var(--t-xs);
    color: var(--faint);
    text-transform: lowercase;
  }
  .next .state {
    color: var(--accent);
  }
  .skipped .state,
  .skipped .title {
    color: var(--faint);
  }
  .says {
    color: var(--dim);
  }
  .writes {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .body {
    display: grid;
    gap: var(--s-2);
    margin: var(--s-2) 0 0 1.9rem;
  }
  .body p {
    margin: 0;
    font-size: var(--t-sm);
  }
  .go {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
  }
  .small {
    font-size: var(--t-xs);
    padding: 0.15rem 0.5rem;
  }
  .primary:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
</style>
