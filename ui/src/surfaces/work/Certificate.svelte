<script lang="ts">
  // The certificate, rendered.
  //
  // **Separate from the surface that fetches it.** `Work` reads
  // `/api/work/<id>/certificate`; this renders whatever comes back, so the
  // rendering can be checked without a network — which is how the harness
  // checks it, and how a glyph losing its word beside it gets caught.
  //
  // Every sentence here comes from the daemon. This words nothing: the
  // certificate is the one artefact whose value is that a reviewer can
  // re-derive it, and a second description of the same completion is a second
  // thing that can drift.
  import { clip } from "../../lib/text";

  type Command = {
    command: string;
    shown: string;
    truncated: boolean;
    outcome: string;
    passed: boolean;
  };

  type Page = {
    finished: boolean;
    unfinished?: string;
    last_gate?: string | null;
    basis?: string;
    checked?: boolean;
    unchecked?: string | null;
    evidence?: {
      "gen_ai.evidence.origin": string;
      commit?: string | null;
      no_commit?: string | null;
      commands: Command[];
    } | null;
    no_evidence?: string | null;
    claim?: { "gen_ai.evidence.origin": string; text: string; caveat: string } | null;
    signing?: { signed: boolean; says: string };
    limits?: string;
  };


  let { page }: { page: Page } = $props();
</script>

  {#if !page.finished}
    <!-- Exporting unfinished work is a fair question, and the honest answer is
         where it is — not an error, and not a certificate. -->
    <p class="dim">
      {page.unfinished}
      {#if page.last_gate}Its last gate said: {page.last_gate}{/if}
    </p>
  {:else}
    <dl class="cert">
      <dt>basis</dt>
      <dd>{page.basis}</dd>

      {#if page.unchecked}
        <!-- A completion nothing checked says so in its own sentence, rather
             than by an absence a reader has to notice. -->
        <dt>warning</dt>
        <dd class="warn">{page.unchecked}</dd>
      {/if}

      {#if page.evidence}
        <dt>origin</dt>
        <dd><code>{page.evidence["gen_ai.evidence.origin"]}</code></dd>
        {#if page.evidence.commit}<dt>commit</dt><dd>{page.evidence.commit}</dd>{/if}
        {#if page.evidence.no_commit}<dt>commit</dt><dd class="dim">{page.evidence.no_commit}</dd>{/if}

        <dt>what was checked</dt>
        <dd>
          <ul>
            {#each page.evidence.commands as c (c.command)}
              <li>
                <span class="mark {c.passed ? 'ok' : 'bad'}" aria-hidden="true"
                  >{c.passed ? "✓" : "✗"}</span
                >
                <span class="sr">{c.passed ? "met" : "did not meet"}</span>
                <!-- Verbatim in the title, shortened on the row: a reformatted
                     command is one a reviewer cannot paste, which is the whole
                     differentiator. -->
                <code title={c.command}>{c.shown}</code>
                <span class="outcome">{c.outcome}</span>
              </li>
            {/each}
          </ul>
        </dd>
      {/if}

      {#if page.no_evidence}<dt>what was checked</dt><dd class="dim">{page.no_evidence}</dd>{/if}

      {#if page.claim}
        <dt>the agent said</dt>
        <dd>
          <code>{page.claim["gen_ai.evidence.origin"]}</code>
          <span class="claim">{clip(page.claim.text, 400)}</span>
          <p class="dim">{page.claim.caveat}</p>
        </dd>
      {/if}

      {#if page.signing}
        <dt>signed</dt>
        <dd class="dim">{page.signing.says}</dd>
      {/if}

      {#if page.limits}
        <dt>what this does not establish</dt>
        <dd class="dim">{page.limits}</dd>
      {/if}
    </dl>
  {/if}
<style>
  h2 { font-size: 1rem; margin: 0 0 .5rem; }
  .cert { display: grid; grid-template-columns: max-content 1fr; gap: .2rem .8rem; margin: 0; }
  dt { color: var(--dim); font-size: .82rem; }
  dd { margin: 0; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { display: flex; gap: .5rem; align-items: baseline; }
  .mark.ok { color: var(--done); }
  .mark.bad { color: var(--fail); }
  .outcome, .dim { color: var(--dim); }
  .warn { color: var(--wait); }
  .said { color: var(--dim); font-size: .82rem; margin: .1rem 0; }
  .acts { display: flex; gap: .3rem; margin-bottom: .4rem; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden;
        clip-path: inset(50%); white-space: nowrap; }
</style>
