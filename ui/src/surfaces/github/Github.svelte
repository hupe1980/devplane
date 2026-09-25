<script lang="ts">
  // Every open issue and pull request across every project, read through the
  // person's own `gh`. Nothing is written: every action is a link.
  //
  // Fetched from `/api/forge` rather than the poll feed: the host refreshes
  // it on its own schedule.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import GithubList from "./GithubList.svelte";
  import type { Row, Coverage } from "./GithubList.svelte";

  /// How many projects the poll feed knows, and how many of them the forge
  /// covers — the number the empty state owes.
  let { coverage = null }: { coverage?: Coverage | null } = $props();

  let issues = $state<Row[]>([]);
  let pulls = $state<Row[]>([]);
  /// Not loaded, empty, and failed are three different facts.
  let loaded = $state(false);
  let failed = $state("");

  onMount(async () => {
    try {
      const r = await api<{ issues?: Row[]; pull_requests?: Row[] }>("/api/forge");
      issues = r.issues ?? [];
      pulls = r.pull_requests ?? [];
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      loaded = true;
    }
  });
</script>

<GithubList {issues} {pulls} {loaded} {failed} {coverage} />
