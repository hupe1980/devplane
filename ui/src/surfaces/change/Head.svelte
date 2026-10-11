<script lang="ts">
  // A change's header: one line — title, state with its qualifier, the
  // actions that can do something, and the rest under *More*. Where the
  // change is in its life, and its facts (project, branch, specification,
  // size), open from the state. Outside the review a second line carries the
  // host's sentence about the check and the base checks' counts; in the
  // review the diff starts right under the view tabs. Each count is said
  // once on the screen: the unread hunks on the Review tab, not here. An
  // agent's stop claim sits between the two lines: after the weakened checks
  // the state carries, before the check's sentence — never first, and never
  // in the gates' words.
  import ChangeState from "../../lib/ChangeState.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Pill from "../../lib/ui/Pill.svelte";
  import { shape } from "../../lib/text";
  import Stepper from "./Stepper.svelte";
  import Claim from "./Claim.svelte";
  import NoCheck, { declaresNoCheck } from "../../lib/NoCheck.svelte";
  import type { Detail } from "./types";

  type Verb = "verify" | "finish" | "offer" | "resume" | "retry" | "archive";
  let {
    d,
    view,
    unseen,
    verified,
    leads,
    standing,
    offerHeld,
    busy,
    notOpened = [],
    act,
    openIn,
    copy,
    review,
    reload = () => {},
  }: {
    d: Detail;
    view: string;
    unseen: number;
    verified: boolean;
    leads: boolean;
    standing: string;
    offerHeld: string;
    busy: string;
    /// The changed files nobody opened, by path, named beside the offer.
    notOpened?: string[];
    act: (verb: Verb) => void;
    openIn: (place: "editor" | "terminal") => void;
    copy: (text: string, what: string) => void;
    review: () => void;
    /// Reads the change again after the claim's own actions.
    reload?: () => void;
  } = $props();

  const project = $derived((d.project_id ?? "").split("/").filter(Boolean).pop() ?? "");
  /// No `check` declared: the change can never read verified, so *Declare a
  /// check* stands where *Run gates* would, and nothing else is primary.
  const noCheck = $derived(declaresNoCheck(d));
  const reviewing = $derived(view === "review");
  const more = $derived(!!d.worktree || (!d.completion && !d.archived_at && !!d.runs?.length) || (!d.archived_at && (!!d.completion || !!d.stopped)));
  let menu = $state<HTMLDetailsElement | null>(null);
  const shut = () => menu?.removeAttribute("open");
</script>

<header class="head">
  <div class="line">
    <h1 title={d.title || d.id}>{d.title || d.id}</h1>
    <details class="pop state">
      <summary title="where this change is, and its facts"><ChangeState state={d.state} q={d.qualifier} /><Icon name="down" size={11} /></summary>
      <div class="card">
        <Stepper state={d.state} archived={!!d.archived_at} offered={!!d.pull_request} weakUnread={unseen} {noCheck} qualified={d.qualifier?.says ?? ""} />
        <dl>
          {#if project}<dt>Project</dt><dd>{project}</dd>{/if}
          {#if d.branch}<dt>Branch</dt><dd><button class="link mono" title="copy the branch name" onclick={() => copy(d.branch!, "the branch name")}>{d.branch}</button></dd>{/if}
          {#if d.spec}<dt>Specification</dt><dd class="mono">{d.spec}</dd>{/if}
          {#if d.shape_says}<dt>Size</dt><dd>{shape(d.shape_says)}</dd>{/if}
          {#if d.worktree}<dt>Worktree</dt><dd class="mono">{d.worktree}</dd>{/if}
        </dl>
      </div>
    </details>
    {#if d.waiting_says}<span class="waiting"><Pill word={d.waiting_says} as="wait" /></span>{/if}
    {#if d.in_place}<span class="warn"><Icon name="alert" size={12} /> in place — no parallel safety</span>{/if}
    {#if d.branch}<button class="branch mono" title="copy the branch name" onclick={() => copy(d.branch!, "the branch name")}><Icon name="change" size={12} /> {d.branch}</button>{/if}
    <span class="gap"></span>
    <div class="acts" class:held-offer={!!offerHeld} role="toolbar" aria-label="what you can do to this change">
      {#if !reviewing}
        <button class:primary={!noCheck && (unseen > 0 || !verified)} onclick={review}><Icon name="split" size={14} /> Review</button>
      {/if}
      {#if noCheck}
        <NoCheck project={d.project_id} compact />
      {:else if d.worktree}
        <button onclick={() => act("verify")} disabled={!!busy} aria-label="Run gates" title="Run gates"><Icon name="gate" size={14} /> <span class="label">Run gates</span></button>
      {/if}
      {#if verified}
        <!-- Held while a weakened row is unread, with why beside it: the host
             refuses that offer, so the button does not pretend otherwise. -->
        {#if offerHeld}
          <button disabled aria-label="Offer as pull request" aria-describedby="offer-held" title={offerHeld}><Icon name="forge" size={14} /> Offer</button>
          <span class="held" id="offer-held">{offerHeld}</span>
        {:else}
          <button class:primary={unseen === 0} onclick={() => act("offer")} disabled={!!busy}><Icon name="forge" size={14} /> <span class="full">Offer as pull request</span><span class="short" aria-hidden="true">Offer</span></button>
          {#if notOpened.length}
            <details class="pop opened">
              <summary class="held">{notOpened.length === 1 ? "1 file not opened" : `${notOpened.length} files not opened`}</summary>
              <ul class="card">{#each notOpened as p (p)}<li class="mono">{p}</li>{/each}</ul>
            </details>
          {/if}
        {/if}
      {/if}
      {#if d.stopped}
        <button onclick={() => act("resume")} disabled={!!busy}><Icon name="play" size={14} /> Pick it back up</button>
        {#if d.can_retry}<button onclick={() => act("retry")} disabled={!!busy}><Icon name="refresh" size={14} /> Try again</button>{/if}
      {/if}
      {#if more}
        <details class="pop more" bind:this={menu}>
          <summary aria-label="More actions" title="More actions"><Icon name="more" size={14} /> <span class="label">More</span></summary>
          <div class="card menu" role="menu">
            {#if !d.completion && !d.archived_at && d.runs?.length}
              <button role="menuitem" onclick={() => (shut(), act("finish"))} disabled={!!busy}><Icon name="check" size={14} /> Finish</button>
            {/if}
            {#if d.worktree}
              <button role="menuitem" onclick={() => (shut(), openIn("editor"))}><Icon name="external" size={14} /> Open in the editor</button>
              <button role="menuitem" onclick={() => (shut(), openIn("terminal"))}><Icon name="terminal" size={14} /> Open a terminal</button>
            {/if}
            {#if !d.archived_at && (d.completion || d.stopped)}
              <button role="menuitem" disabled={!!busy} onclick={() => (shut(), act("archive"))} title="remove the worktree; the branch and the record are kept"><Icon name="folder" size={14} /> Archive</button>
            {/if}
          </div>
        </details>
      {/if}
    </div>
  </div>
  {#if !reviewing && d.stop_claim}
    <Claim claim={d.stop_claim} change={d.id} done={reload} />
  {/if}
  {#if !reviewing && (standing || d.base_checks)}
    <p class="sub">
      {#if standing}<span class="standing {verified && !leads ? 'done' : ''}"><Inline text={standing} /></span>{/if}
      <!-- Beside the standing, never in it: counts carry no status colour. -->
      {#if d.base_checks}<span class="counts">{d.base_checks.counts_says}{#if d.base_checks.headline}<span class="w-{d.base_checks.headline.weight}"> · {d.base_checks.headline.says}</span>{:else if d.base_checks.on_base && !d.base_checks.stale}<span class="w-{d.base_checks.on_base.weight}"> · {d.base_checks.on_base.says}</span>{/if}</span>{/if}
    </p>
  {/if}
</header>

<style>
  .head {
    container: head / inline-size;
    padding: var(--s-2) var(--s-6) var(--s-1);
    display: grid;
    gap: var(--s-1);
    background: var(--bg);
    min-width: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    min-width: 0;
    white-space: nowrap;
  }
  h1 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 650;
    letter-spacing: -0.015em;
    min-width: 6rem;
    flex: 0 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gap {
    flex: 1;
  }
  .mono {
    font-family: var(--mono);
  }
  .branch {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.4rem;
    padding: 0 0.5rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--dim);
    font-size: var(--t-xs);
    cursor: pointer;
    flex: none;
  }
  .waiting {
    display: inline-flex;
    flex: none;
  }
  .warn {
    color: var(--wait);
    font-size: var(--t-xs);
    flex: none;
  }
  .pop {
    position: relative;
    flex: none;
  }
  .pop summary {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    list-style: none;
    cursor: pointer;
  }
  .pop summary::-webkit-details-marker {
    display: none;
  }
  .card {
    position: absolute;
    top: calc(100% + 0.3rem);
    left: 0;
    z-index: 20;
    min-width: 16rem;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.18);
    white-space: normal;
    font-size: var(--t-xs);
  }
  .card dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.2rem var(--s-3);
    margin: var(--s-2) 0 0;
  }
  .card dt {
    color: var(--faint);
  }
  .card dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .more .card,
  .opened .card {
    left: auto;
    right: 0;
  }
  .opened ul {
    margin: 0;
    padding-left: var(--s-4);
  }
  .menu {
    display: grid;
    gap: 0.15rem;
    min-width: 13rem;
    padding: var(--s-1);
  }
  .menu button {
    justify-content: flex-start;
    border-color: transparent !important;
    background: none !important;
    width: 100%;
  }
  .menu button:hover:not(:disabled) {
    background: var(--select) !important;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex: none;
  }
  .acts button,
  .more summary {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.8rem;
    padding: 0 0.7rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    cursor: pointer;
    white-space: nowrap;
    flex: none;
  }
  .acts button:hover:not(:disabled) {
    border-color: var(--edge);
    background: var(--raise);
  }
  .acts button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .acts .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .held {
    font-size: var(--t-xs);
    color: var(--wait);
    white-space: nowrap;
  }
  .sub {
    margin: 0;
    display: flex;
    gap: var(--s-3);
    min-width: 0;
    font-size: var(--t-xs);
    color: var(--dim);
  }
  .standing {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .standing.done {
    color: var(--done);
  }
  .counts {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .counts .w-warn {
    color: var(--wait);
    font-weight: 600;
  }
  .counts .w-absent {
    color: var(--faint);
  }
  /* Narrower, the branch moves into the state's card and *More* keeps its
     icon; a held Offer's reason stays tied to the button for a reader. */
  @container head (max-width: 1000px) {
    .branch {
      display: none;
    }
    /* Still the button's description for a reader; Review says the rest. */
    .held:not(summary) {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }
  }
  /* Narrower still: *Offer* keeps its verb, *More* and *Run gates* their
     icon and name for a reader — nothing is cut at the edge. */
  .short {
    display: none;
  }
  @container head (max-width: 1100px) {
    .short {
      display: inline;
    }
    .full {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }
    .more .label {
      display: none;
    }
    .more summary {
      padding: 0 0.45rem;
    }
  }
  /* The primary action says what the change waits on; the title keeps its words. */
  @container head (max-width: 900px) {
    .waiting {
      display: none;
    }
  }
  @container head (max-width: 820px) {
    .acts .label {
      display: none;
    }

    .acts button:has(> .label) {
      padding: 0 0.45rem;
    }
  }
</style>
