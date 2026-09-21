<script lang="ts">
  // One piece of work: what was checked, and what that proves.
  //
  // **The certificate is the point of this surface.** It existed behind
  // `devplane work export` — a command nobody types — while the page showed the
  // gate that produced it and never what it proved.
  //
  // Every sentence here comes from the daemon. The page renders and words
  // nothing: the certificate is the one artefact whose value is that a reviewer
  // can re-derive it, and a surface with its own phrasing would be a second
  // description of the same completion with nothing able to notice they had
  // drifted.
  import { clip } from "../../lib/text";
  import { api } from "../../lib/api";
  import Certificate from "./Certificate.svelte";

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

  type Brief = { id: string; title: string; phase: string };

  let {
    title = "",
    id = "",
    phase = "",
    all = [],
  }: { title?: string; id?: string; phase?: string; all?: Brief[] } = $props();

  let said = $state("");

  /// The certificate for the Work on screen.
  ///
  /// **Nothing fetched it until 2026-09-21.** It was a prop with a `null`
  /// default that the registry never filled, so the surface rendered *"Reading
  /// the evidence…"* on every visit, for ever — a loading line for a request
  /// that was never made, which is the most convincing way to look broken.
  let page = $state<Page | null>(null);
  let reading = $state(false);

  $effect(() => {
    const want = id;
    page = null;
    if (!want) return;
    reading = true;
    void (async () => {
      try {
        page = await api<Page>(`/api/work/${encodeURIComponent(want)}/certificate`);
        said = "";
      } catch (e) {
        said = `the evidence could not be read: ${e instanceof Error ? e.message : String(e)}`;
      } finally {
        reading = false;
      }
    })();
  });

  /// **Released, picked back up, tried again** — three verbs, one route each.
  ///
  /// `approve` is the only one that is a *decision*: it releases a pipeline
  /// held at a declared human step, and the record says it was the person's.
  /// The other two restart work that stopped. None of them is offered where it
  /// cannot do anything, because a button that cannot keep its promise is the
  /// one failure a control plane cannot afford.
  async function act(verb: "approve" | "resume" | "retry") {
    if (!id) {
      said = "there is no work here to act on";
      return;
    }
    // **The route is a literal per verb, not a path with the verb pasted in.**
    // Interpolating it means a surface can build a route the daemon does not
    // serve, and nobody finds out until somebody presses the button — so the
    // three are written out and a guard checks each against the daemon's own
    // registration.
    const where = {
      approve: `/api/work/${encodeURIComponent(id)}/approve`,
      resume: `/api/work/${encodeURIComponent(id)}/resume`,
      retry: `/api/work/${encodeURIComponent(id)}/retry`,
    }[verb];
    try {
      await api(where, { method: "POST" });
      said = { approve: "released — the pipeline continues, and the decision is recorded as yours",
               resume: "picked back up",
               retry: "trying again" }[verb];
    } catch (e) {
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }
</script>

<section aria-labelledby="work-head">
  <h2 id="work-head">{title || "Work"}</h2>
  <p class="said" role="status" aria-live="polite">{said}</p>

  <!-- **Offered only where it can do something.** `approve` appears for work
       actually held at a declared human step and nowhere else. -->
  <div class="acts">
    {#if phase === "human"}
      <button class="primary" onclick={() => act("approve")}>release this step</button>
    {/if}
    {#if phase === "stopped" || phase === "failed"}
      <button onclick={() => act("resume")}>pick it back up</button>
      <button onclick={() => act("retry")}>try again</button>
    {/if}
  </div>

  {#if !id}
    <!-- **No Work is not a Work that is loading.** A machine with none is the
         ordinary state on a first run, and a permanent loading line is the
         most convincing way for a surface to look broken. -->
    <p class="dim">
      No work has been started here. <code>devplane work start "…"</code> makes one — an isolated
      checkout, the project's own checks, and a certificate when it finishes.
    </p>
  {:else if reading}
    <p class="dim">Reading the evidence…</p>
  {:else if page === null}
    <p class="dim">There is no evidence to read for this one.</p>
  {:else}
    <Certificate {page} />
  {/if}
</section>
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
