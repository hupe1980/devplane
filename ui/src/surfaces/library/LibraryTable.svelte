<script lang="ts">
  // One prompt or skill, and which of your repositories has it.
  //
  // **Pure, so it can be rendered with a matrix and asserted on.** The fetch is
  // in the container beside this.
  import type { Drift } from "../../wire/Drift";

  export type Copy = { project: string; drift: Drift; present: boolean };
  export type Artefact = {
    name: string;
    digest: string;
    origin: string | null;
    copies: Copy[];
  };

  let {
    artefacts = [],
    loaded = true,
    failed = "",
  }: { artefacts?: Artefact[]; loaded?: boolean; failed?: string } = $props();

  /// **Six values, and never a boolean.** `copy_moved` and `library_moved` are
  /// the same yes-or-no and opposite instructions: one says your repository has
  /// an edit the library has not, the other says the library moved on without
  /// it. A tick and a cross cannot carry that, so every cell is a word.
  const says: Record<Drift, string> = {
    unchanged: "same",
    copy_moved: "edited here",
    library_moved: "library moved on",
    both_moved: "both moved",
    missing: "missing",
    unrecorded: "not installed by Devplane",
  };

  /// Which cells are worth a reader's eye. `missing` is the coverage answer
  /// rather than a fault, so it is dim rather than loud.
  const tone = (d: Drift): string =>
    d === "unchanged" ? "ok" : d === "missing" ? "dim" : "warn";

  const projects = $derived(
    [...new Set(artefacts.flatMap((a) => a.copies.map((c) => c.project)))].sort(),
  );
  const cell = (a: Artefact, p: string): Copy | undefined =>
    a.copies.find((c) => c.project === p);
</script>

<section aria-labelledby="lib-head">
  <h2 id="lib-head">What is installed where</h2>

  {#if failed}
    <p class="empty" role="status">The library could not be read: {failed}</p>
  {:else if !loaded}
    <p class="empty">Reading the library…</p>
  {:else if artefacts.length === 0}
    <p class="empty">
      Nothing in the library yet. <code>devplane library</code> lists what this machine can
      reach, and it reads your agent's own <code>SKILL.md</code> files — Devplane invents no
      format of its own.
    </p>
  {:else}
    <table>
      <thead>
        <tr>
          <th scope="col">artefact</th>
          {#each projects as p (p)}<th scope="col">{p}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each artefacts as a (a.name)}
          <tr>
            <th scope="row">
              {a.name}
              {#if a.origin}<span class="origin">{a.origin}</span>{/if}
            </th>
            {#each projects as p (p)}
              {@const c = cell(a, p)}
              <td class={c ? tone(c.drift) : "dim"}>{c ? says[c.drift] : "—"}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="foot">
      <code>devplane library diff &lt;name&gt;</code> shows what moved, and
      <code>devplane library sync</code> is the only thing that writes. Nothing on this page does.
    </p>
  {/if}
</section>

<style>
  h2 { font-size: 1rem; margin: 0 0 .4rem; }
  table { border-collapse: collapse; font-size: .86rem; }
  th, td { text-align: left; padding: .2rem .6rem; border-bottom: 1px solid var(--line); white-space: nowrap; }
  thead th { color: var(--dim); font-weight: 600; }
  tbody th { font-weight: 600; }
  .origin { color: var(--dim); font-weight: 400; margin-left: .4rem; font-size: .8rem; }
  .ok { color: var(--done); }
  .warn { color: var(--wait); }
  .dim, .empty, .foot { color: var(--dim); }
  .foot { font-size: .82rem; margin-top: .6rem; }
  .empty { max-width: 70ch; }
</style>
