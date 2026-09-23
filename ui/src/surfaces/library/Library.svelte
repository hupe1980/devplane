<script lang="ts">
  // The library, across every registered project.
  //
  // **The last surface the shell was rebuilt for.** `/api/library` has served
  // this matrix since the library shipped and nothing has ever asked it: the
  // verb `diff` answered *which of my six copies drifted* in a terminal, and
  // the page that was supposed to show it was never built. One artefact per
  // row, one project per column, and a word rather than a tick in every cell.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import LibraryTable from "./LibraryTable.svelte";
  import type { Artefact } from "./LibraryTable.svelte";

  let artefacts = $state<Artefact[]>([]);
  let loaded = $state(false);
  let failed = $state("");

  onMount(async () => {
    try {
      artefacts = await api<Artefact[]>("/api/library");
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      loaded = true;
    }
  });
</script>

<LibraryTable {artefacts} {loaded} {failed} />
