<script lang="ts">
  // Reports between projects, and a form to file one. A person files through
  // the agents' route with `as_person` as the origin.
  import { onMount } from "svelte";
  import { api } from "../../lib/api";
  import ReportList from "./ReportList.svelte";
  import type { Row, Project, Filing } from "./ReportList.svelte";

  let { projects = [] }: { projects?: Project[] } = $props();

  let reports = $state<Row[]>([]);
  let loaded = $state(false);
  let failed = $state("");
  let said = $state("");

  async function read() {
    try {
      reports = await api<Row[]>("/api/reports?all=true");
      failed = "";
    } catch (e) {
      failed = e instanceof Error ? e.message : String(e);
    } finally {
      loaded = true;
    }
  }
  onMount(read);

  async function file(f: Filing) {
    try {
      const r = await api<{ says?: string; report?: { id: string } }>("/api/reports", {
        method: "POST",
        body: JSON.stringify({
          kind: f.kind,
          title: f.title,
          finding: f.words,
          evidence: { command: f.command || null, output: f.output || null },
          to: f.to,
          as_person: true,
          from: f.from,
        }),
      });
      said = `filed ${r.report?.id ?? ""} — ${r.says ?? ""}`;
      await read();
      return true;
    } catch (e) {
      said = `not filed: ${e instanceof Error ? e.message : String(e)}`;
      return false;
    }
  }
</script>

<ReportList {reports} {projects} {loaded} {failed} {said} {file} reload={read} />
