<script lang="ts">
  // The ledger: everything Devplane recorded as decided, and on whose
  // authority. *Decided for you* is one click. No rate, no score, no count per
  // person — the chips count rows.
  import { api } from "../../lib/api";
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Pill, { type Tone } from "../../lib/ui/Pill.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";

  type Decision = {
    id: string;
    at: string;
    authority: string;
    action: string;
    subject: string;
    outcome: string;
    reason?: string | null;
    tool?: string | null;
    project_id?: string | null;
    run_id?: string | null;
    change_id?: string | null;
  };
  let { about = "" }: { about?: string } = $props();

  let rows = $state<Decision[] | null>(null);
  let error = $state("");
  $effect(() => {
    const want = about;
    let live = true;
    api<Decision[]>(want ? `/api/decisions?about=${encodeURIComponent(want)}&limit=1000` : "/api/decisions?limit=1000")
      .then((r) => {
        if (live) rows = Array.isArray(r) ? r : [];
      })
      .catch((e) => {
        if (live) error = e instanceof Error ? e.message : String(e);
      });
    return () => {
      live = false;
    };
  });

  const AUTH = ["person", "rule", "timer", "nobody", "devplane"];
  const FOR_YOU = new Set(["rule", "timer", "nobody"]);
  let only = $state<string | null>(null);
  let forYou = $state(false);
  let text = $state("");
  let selected = $state<string | null>(null);

  const counts = $derived(AUTH.map((a) => [a, (rows ?? []).filter((r) => r.authority === a).length] as const));
  const shown = $derived(
    (rows ?? []).filter(
      (r) =>
        (!only || r.authority === only) &&
        (!forYou || FOR_YOU.has(r.authority)) &&
        (!text.trim() || `${r.action} ${r.subject} ${r.reason ?? ""} ${r.outcome}`.toLowerCase().includes(text.trim().toLowerCase())),
    ),
  );
  const pick = $derived(shown.find((r) => r.id === selected) ?? null);
  const tone = (a: string): Tone => (a === "person" ? "work" : a === "rule" ? "wait" : a === "devplane" ? "none" : "fail");
  const day = (r: Decision) => new Date(r.at).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
  const project = (p?: string | null) => (p ? (p.split("/").filter(Boolean).pop() ?? p) : "—");

  const columns: Column<Decision>[] = [
    { key: "at", label: "Time", width: 80, sort: (r) => r.at, mono: true },
    { key: "authority", label: "Authority", width: 120, sort: (r) => r.authority },
    { key: "outcome", label: "Outcome", width: 100, sort: (r) => r.outcome },
    { key: "action", label: "Action", width: 150, sort: (r) => r.action, mono: true },
    { key: "project", label: "Project", width: 110, sort: (r) => project(r.project_id) },
    { key: "subject", label: "About", width: 340, mono: true },
    { key: "reason", label: "Why" },
  ];
</script>

<div class="ledger">
  <header class="head">
    <h1>Ledger</h1>
    <button class="foryou" class:on={forYou} onclick={() => (forYou = !forYou)} title="rule, timer and nobody — what was decided instead of you">
      <Icon name="person" size={13} /> Decided for you <span>{(rows ?? []).filter((r) => FOR_YOU.has(r.authority)).length}</span>
    </button>
    <div class="chips" role="group" aria-label="by authority">
      {#each counts as [a, n] (a)}
        <button class:on={only === a} disabled={n === 0} onclick={() => (only = only === a ? null : a)}>{a} <span>{n}</span></button>
      {/each}
    </div>
    <span class="gap"></span>
    <label class="find"><Icon name="search" size={13} /><input bind:value={text} placeholder="Filter" aria-label="filter the ledger" /></label>
  </header>
  {#if about}<p class="about">Narrowed to <code>{about}</code> · <a href="#why">show everything</a></p>{/if}

  {#if error}
    <Empty icon="alert" title="The ledger could not be read" body={error} />
  {:else if rows && rows.length === 0}
    <Empty icon="ledger" title="Nothing has been decided yet" body="Every refusal a rule made, every question a person answered, every gate Devplane ran — each lands here with the authority that decided it." />
  {:else}
    <div class="frame">
      <Grid id="ledger" {columns} rows={shown} key={(r) => r.id} group={day} bind:selected label="decisions">
        {#snippet cell(r, c)}
          {#if c.key === "at"}{new Date(r.at).toTimeString().slice(0, 5)}
          {:else if c.key === "authority"}<Pill word={r.authority} as={tone(r.authority)} />
          {:else if c.key === "outcome"}<span class="out {r.outcome}">{r.outcome}</span>
          {:else if c.key === "action"}{r.action}
          {:else if c.key === "project"}{project(r.project_id)}
          {:else if c.key === "subject"}{r.subject}
          {:else}<span class="dim">{r.reason ?? ""}</span>{/if}
        {/snippet}
        {#snippet empty()}<p class="quiet">{rows === null ? "Reading…" : "Nothing matches."}</p>{/snippet}
      </Grid>
      {#if pick}
        <aside class="drawer">
          <header><Pill word={pick.authority} as={tone(pick.authority)} /> <b>{pick.action}</b> <button class="x" aria-label="close" onclick={() => (selected = null)}><Icon name="x" size={13} /></button></header>
          <p class="when">{new Date(pick.at).toLocaleString()} · {pick.outcome}</p>
          <pre>{pick.subject}</pre>
          {#if pick.reason}<p>{pick.reason}</p>{/if}
          <div class="links">
            {#if pick.change_id}<a href={`#change/${encodeURIComponent(pick.change_id)}`}><Icon name="change" size={13} /> the change</a>{/if}
            {#if pick.run_id}<a href={`#why/${encodeURIComponent(pick.run_id)}`}><Icon name="sessions" size={13} /> everything about this run</a>{/if}
          </div>
        </aside>
      {/if}
    </div>
  {/if}
</div>

<style>
  .ledger {
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
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .foryou,
  .chips button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.6rem;
    padding: 0 0.6rem;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--panel);
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .foryou span,
  .chips span {
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .foryou.on,
  .chips button.on {
    border-color: var(--accent);
    background: var(--select);
    color: var(--ink);
  }
  .chips {
    display: flex;
    gap: var(--s-1);
  }
  .chips button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .gap {
    flex: 1;
  }
  .find {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: 0 var(--s-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--faint);
  }
  .find input {
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    padding: 0.25rem 0;
    outline: none;
    box-shadow: none;
    width: 14rem;
  }
  .about {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .out {
    font-size: var(--t-xs);
    font-weight: 600;
  }
  .out.deny,
  .out.fail {
    color: var(--fail);
  }
  /* An allow is a permission, not a proof: not the verified green. */
  .out.allow {
    color: var(--ink);
  }
  .out.pass {
    color: var(--done);
  }
  .dim {
    color: var(--faint);
  }
  .drawer {
    width: 24rem;
    flex: none;
    border-left: 1px solid var(--line);
    background: var(--side);
    padding: var(--s-3) var(--s-4);
    overflow: auto;
    font-size: var(--t-sm);
  }
  .drawer header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .x {
    margin-left: auto;
    border: 0;
    background: none;
    color: var(--dim);
    padding: 0;
    cursor: pointer;
  }
  .when {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  pre {
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: var(--s-2) var(--s-3);
  }
  .links {
    display: grid;
    gap: var(--s-2);
  }
  .links a {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--accent);
    text-decoration: none;
  }
  .quiet {
    padding: var(--s-4);
    color: var(--faint);
    margin: 0;
  }
</style>
