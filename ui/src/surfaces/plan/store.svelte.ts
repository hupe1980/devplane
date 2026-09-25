// `/api/specs`, read once and shared by the sidebar and the document, and read
// again every fifteen seconds while either is on screen — a specification
// changes when somebody saves a file, not on every poll.
import { api } from "../../lib/api";
import type { Plan } from "../../wire/Plan";
import type { Counts } from "../../wire/Counts";
import type { TokenRow } from "../../wire/TokenRow";

export type Task = { path: string; line: number; text: string; done: boolean; cites: string[] };
export type Trace = {
  edges: Array<[string, Task[]]>;
  orphan_requirements: string[];
  tasks_citing_nothing: Task[];
  unrecognised: boolean;
};
export type PlanRow = {
  change_id: string | null;
  title: string | null;
  state: string | null;
  contradicts_done: boolean;
  drifted: boolean | null;
  plan: Plan;
  trace?: Trace | null;
  counts?: Counts | null;
  counts_says?: string | null;
  token_rows?: Array<TokenRow & { says?: string }>;
};
export type ProjectRow = {
  project_id: string;
  project: string;
  root: string;
  declares_markers: boolean;
  declares_plans: boolean;
  layouts?: string[];
  no_layout?: string | null;
  plans: PlanRow[];
};

export const specs = $state<{ projects: ProjectRow[] | null; omitted: number; error: string }>({
  projects: null,
  omitted: 0,
  error: "",
});

let users = 0;
let timer: ReturnType<typeof setInterval> | null = null;

async function read() {
  try {
    const r = await api<{ projects: ProjectRow[]; omitted?: number }>("/api/specs");
    specs.projects = r.projects ?? [];
    specs.omitted = r.omitted ?? 0;
    specs.error = "";
  } catch (e) {
    specs.error = e instanceof Error ? e.message : String(e);
  }
}

/// Call from an `$effect`: reads now, keeps reading while anybody is watching.
export function watch(): () => void {
  users += 1;
  if (users === 1) {
    void read();
    timer = setInterval(read, 15_000);
  }
  return () => {
    users -= 1;
    if (users === 0 && timer) {
      clearInterval(timer);
      timer = null;
    }
  };
}

/// A specification's address: its project and its folder.
export const key = (p: ProjectRow, r: PlanRow) => `${p.project}/${r.plan.path}`;
