<script lang="ts">
  // Every open issue and pull request across every project.
  //
  // **Half of what is waiting on you is not a session.** The forge is read
  // through the person's own `gh`, and nothing here is ever written to it:
  // every action is a link, because opening an issue under somebody's name
  // from a list is a write they did not review.
  //
  // **This surface fetches, and until 2026-09-21 it did not.** Its `select`
  // returned `{}`, so the list rendered its own defaults — two empty tabs and
  // a sentence saying nothing is open — on every machine, for ever, while the
  // daemon served the rows on `/api/forge` and nothing asked. The rows are not
  // in the poll feed on purpose: they are a `gh` read the daemon refreshes on
  // its own schedule, and carrying them in a two-second poll would send a
  // payload nobody is looking at on every board refresh.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import GithubList from "./GithubList.svelte";
  import type { Row } from "./GithubList.svelte";

  let issues = $state<Row[]>([]);
  let pulls = $state<Row[]>([]);
  /// **Three states, not two.** Before the first answer there is nothing to
  /// say; after it, an empty list and a failed read are different facts and
  /// only one of them is good news.
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

<GithubList {issues} {pulls} {loaded} {failed} />
