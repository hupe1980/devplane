<script lang="ts">
  // The fetching half. `/api/specs` walks every in-flight work's specification
  // folder on disk, so it is read when somebody opens this page rather than on
  // the poll every surface shares.
  //
  // **A surface gets its data from exactly one of two places and must name
  // which.** Three shipped with neither and rendered their components' defaults
  // for ever, with every test green — the render tests hand props to a component
  // directly, so they prove the component and never the wiring.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import PlanList from "./PlanList.svelte";
  import type { ProjectRow } from "./PlanList.svelte";

  let projects = $state<ProjectRow[]>([]);
  let omitted = $state(0);
  let loaded = $state(false);
  let failed = $state("");

  /// **One function, called on mount and by the retry control.**
  ///
  /// A page whose only way out of a failure is a full reload is a page that
  /// loses the tab's token: `devplane open` puts it in the URL once and the
  /// address bar is stripped of it immediately, so reloading is not free here.
  async function read() {
    loaded = false;
    failed = "";
    try {
      const r = await api<{ projects: ProjectRow[]; omitted?: number }>("/api/specs");
      projects = r.projects ?? [];
      omitted = r.omitted ?? 0;
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      loaded = true;
    }
  }

  onMount(read);
</script>

<PlanList {projects} {omitted} {loaded} {failed} retry={read} />
