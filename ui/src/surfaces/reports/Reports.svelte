<script lang="ts">
  // Reports between projects, and a form to file one. A person files through
  // the agents' route with `as_person` as the origin.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import ReportList from "./ReportList.svelte";
  import type { Row, Project, Filing } from "./ReportList.svelte";

  let { projects = [] }: { projects?: Project[] } = $props();

  const reportsRead = resource<Row[]>(() => "/api/reports?all=true", { tell: () => "devplane report ls --all" });
  const reports = $derived(Array.isArray(reportsRead.data) ? reportsRead.data : []);
  const loaded = $derived(reportsRead.phase !== "loading");
  const failed = $derived(
    reportsRead.failure ? `${reportsRead.failure.says} — \`${reportsRead.failure.tell}\` tells more${reportsRead.data ? " (showing the last read)" : ""}` : "",
  );
  let said = $state("");
  const read = () => reportsRead.reload();

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
      said = `not filed: ${failure(e).says}`;
      return false;
    }
  }
</script>

<ReportList {reports} {projects} {loaded} {failed} {said} {file} reload={read} />
