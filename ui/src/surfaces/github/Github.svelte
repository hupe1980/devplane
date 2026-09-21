<script lang="ts">
  // Every open issue and pull request across every project.
  //
  // **Half of what is waiting on you is not a session.** The forge is read
  // through the person's own `gh`, and nothing here is ever written to it:
  // every action is a link, because opening an issue under somebody's name
  // from a list is a write they did not review.
  import { clip } from "../../lib/text";

  type Row = { title: string; url: string; project: string; needs_you: boolean };

  let {
    issues = [],
    pulls = [],
    tab = $bindable<"issues" | "pulls">("issues"),
  }: { issues?: Row[]; pulls?: Row[]; tab?: "issues" | "pulls" } = $props();

  const shown = $derived(tab === "issues" ? issues : pulls);
</script>

<section aria-labelledby="gh-head">
  <h2 id="gh-head">Issues and pull requests</h2>

  <div role="tablist" aria-label="what to show">
    <button role="tab" aria-selected={tab === "issues"} onclick={() => (tab = "issues")}>
      issues ({issues.length})
    </button>
    <button role="tab" aria-selected={tab === "pulls"} onclick={() => (tab = "pulls")}>
      pull requests ({pulls.length})
    </button>
  </div>

  {#if shown.length === 0}
    <p class="empty">
      Nothing open{tab === "issues" ? "" : " that is waiting"} across your projects. Devplane reads
      the forge through your own <code>gh</code>; a project with none configured is simply absent
      rather than empty.
    </p>
  {:else}
    <ul role="list">
      {#each shown as r (r.url)}
        <li>
          {#if r.needs_you}<span class="wait">needs you</span>{/if}
          <span class="where">{r.project}</span>
          <!-- A link, never a button: nothing is written to somebody else's
               repository from a list. -->
          <a href={r.url} target="_blank" rel="noreferrer">{clip(r.title, 90)}</a>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .3rem; }
  [role="tablist"] { display: flex; gap: .3rem; margin-bottom: .3rem; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; gap: .6rem; padding: .1rem 0; align-items: baseline; }
  .where { color: var(--dim); font-weight: 600; }
  .wait { color: var(--wait); }
  .empty { color: var(--dim); max-width: 60ch; }
</style>
