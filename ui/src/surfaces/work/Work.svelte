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

  let {
    title = "",
    id = "",
    phase = "",
    /// Every Work, newest first.
    ///
    /// **The registry computed this and nothing read it**, so the surface showed
    /// the Work named in the address or the most recent, and any other one was
    /// reachable only by editing the URL. *Two clicks from a finished Work to
    /// the clipboard* was true of exactly one Work.
    all = [],
  }: {
    title?: string;
    id?: string;
    phase?: string;
    all?: Array<{ id: string; title: string; phase: string }>;
  } = $props();

  let said = $state("");

  /// The certificate for the Work on screen.
  ///
  /// **Nothing fetched it until 2026-09-21.** It was a prop with a `null`
  /// default that the registry never filled, so the surface rendered *"Reading
  /// the evidence…"* on every visit, for ever — a loading line for a request
  /// that was never made, which is the most convincing way to look broken.
  let page = $state<Page | null>(null);
  let reading = $state(false);
  /// The same bytes `devplane work export` writes.
  ///
  /// **The daemon has served this beside `page` since the certificate shipped
  /// and nothing read it.** The quickstart said *"and one button that copies
  /// it"*, the roadmap counted the item as narrowed to nothing, and no file in
  /// `ui/src/` mentioned the clipboard on this surface — so the one sentence
  /// this feature is sold on, *the others hand you a verdict and this one hands
  /// you the commands*, had no way off the page.
  let markdown = $state("");

  $effect(() => {
    const want = id;
    page = null;
    markdown = "";
    if (!want) return;
    reading = true;
    void (async () => {
      try {
        const r = await api<Page & { markdown?: string }>(
          `/api/work/${encodeURIComponent(want)}/certificate`,
        );
        page = r;
        markdown = r.markdown ?? "";
        said = "";
      } catch (e) {
        said = `the evidence could not be read: ${e instanceof Error ? e.message : String(e)}`;
      } finally {
        reading = false;
      }
    })();
  });

  /// **Copies what the command writes, byte for byte.** Not a rendering of the
  /// page: the point of the certificate is that a reviewer can re-run the
  /// commands without trusting this tool, and a second composition of it here
  /// would be a second thing that can drift from the one `devplane work
  /// export` produces.
  async function copyCertificate() {
    if (!markdown) return;
    try {
      await navigator.clipboard?.writeText(markdown);
      said = "copied — paste it into the pull request";
    } catch {
      said = "no clipboard here — run: devplane work export " + id;
    }
  }

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
  <h2 id="work-head">{title || "Finished work"}</h2>
  <!-- A diff is a detail of this Work, so it is reached from here rather than
       from a nav entry with its own picker. -->
  {#if id}<p class="also"><a href={`#changes/${id}`}>what changed</a></p>{/if}
  <p class="said" role="status" aria-live="polite">{said}</p>

  <!-- **Anchors, not a handler.** Each is `#work/<id>`, which the shell already
       reads — so they work from the keyboard, open in a new tab, and survive a
       reload, which a click handler would not. Shown only where there is a
       choice to make. -->
  {#if all.length > 1}
    <nav class="picker" aria-label="which work">
      {#each all as w (w.id)}
        <a href="#work/{encodeURIComponent(w.id)}" aria-current={w.id === id ? "page" : undefined}>
          {w.title || w.id}
        </a>
      {/each}
    </nav>
  {/if}

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
    <Certificate {page} {markdown} oncopy={copyCertificate} />
  {/if}
</section>
<style>
  .also { margin: 0 0 var(--s-3); font-size: var(--t-sm); }
  h2 { font-size: var(--t-lg); margin: 0 0 var(--s-3); }
  .said { color: var(--dim); font-size: var(--t-xs); margin: var(--s-1) 0; }
  .said:empty { display: none; }
  .acts { display: flex; gap: var(--s-2); margin-bottom: var(--s-3); }
  .acts:empty { display: none; }
  .dim { color: var(--dim); max-width: 70ch; }
  .picker { display: flex; flex-wrap: wrap; gap: var(--s-2); margin-bottom: var(--s-3); }
  .picker a { color: var(--dim); font-size: var(--t-sm); text-decoration: none; }
  .picker a:hover { color: var(--ink); }
  .picker a[aria-current="page"] { color: var(--ink); font-weight: 600; }
</style>
