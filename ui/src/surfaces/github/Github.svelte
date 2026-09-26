<script lang="ts">
  // Every open issue and pull request across every project, read by
  // Devplane's own GitHub sign-in. Nothing is written: every action is a link.
  //
  // Fetched from `/api/forge` rather than the poll feed: the host refreshes
  // it on its own schedule, and this page re-reads what the host holds.
  import { resource } from "../../lib/resource.svelte";
  import Failed from "../../lib/Failed.svelte";
  import GithubList from "./GithubList.svelte";
  import type { Row, Coverage, SignIn, ProjectState } from "./GithubList.svelte";

  /// How many projects the poll feed knows, and how many of them the forge
  /// covers — the number the empty state owes.
  let { coverage = null }: { coverage?: Coverage | null } = $props();

  const read = resource<{
    issues?: Row[];
    pull_requests?: Row[];
    error?: string | null;
    github?: SignIn | null;
    projects?: ProjectState[];
    fetched_at?: string | null;
  }>(() => "/api/forge", {
    every: 30_000,
    tell: () => "devplane forge issues",
  });
  const issues = $derived(read.data?.issues ?? []);
  const pulls = $derived(read.data?.pull_requests ?? []);
  /// Not loaded, empty, and failed are three different facts. A sign-in
  /// that is not data (not signed in, expired, …) is said by the list in its
  /// own words; a host that could not be read is a failure.
  const loaded = $derived(read.phase !== "loading");
  const failed = $derived(read.data?.error ?? (read.phase === "failed" ? (read.failure?.says ?? "") : ""));
</script>

{#if read.phase === "stale" && read.failure}
  <div class="stale"><Failed what="the forge" failure={read.failure} at={read.at} stale /></div>
{/if}
<GithubList {issues} {pulls} {loaded} {failed} {coverage} github={read.data?.github ?? null} projects={read.data?.projects ?? []} readAt={read.data?.fetched_at ?? null} />

<style>
  .stale {
    padding: 0 var(--s-5);
  }
</style>
