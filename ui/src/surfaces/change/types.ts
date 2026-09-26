// A change as `/api/changes/{id}` serves it — the fields this surface reads.
// Every `*_says` is a sentence the host composed; the surface prints it.
import type { Counts } from "../../wire/Counts";
import type { TokenRow } from "../../wire/TokenRow";
import type { SpecDrift } from "../../wire/SpecDrift";
import type { Heading } from "../../wire/Heading";
import type { SentTask } from "../../wire/SentTask";

export type Outcome = { outcome: string; code?: number; after_secs?: number; reason?: string };
export type Command = {
  command: string;
  outcome: Outcome;
  duration_ms: number;
  output_tail?: string;
  output_bytes?: number;
  output_digest?: string;
  failures?: string[];
};
export type Stamp = {
  commit?: string | null;
  tree?: string | null;
  branch?: string | null;
  clean?: boolean;
  changed_files?: number;
  reach?: string;
  remote?: string | null;
};
export type GateReport = {
  gate: string;
  at: string;
  duration_ms: number;
  commands: Command[];
  attempt: number;
  commit?: Stamp | null;
};
export type Detail = {
  id: string;
  project_id: string;
  title: string;
  prompt?: string | null;
  spec?: string | null;
  worktree?: string | null;
  branch?: string | null;
  runs?: string[];
  gates?: GateReport[];
  feedback_rounds?: number;
  cost_usd?: number;
  completion?: unknown | null;
  stopped?: unknown | null;
  waiting_says?: string | null;
  tree_now?: Stamp | null;
  archived_at?: string | null;
  pull_request?: { url?: string; number?: number; status?: string } | null;
  created_at?: string;
  updated_at?: string;
  state: string;
  /// What the change's own diff weakened, by kind, beside its state.
  qualifier?: import("../../wire/Qualifier").Qualifier | null;
  in_place?: boolean;
  in_place_says?: string | null;
  standing_says?: string | null;
  gate?: { name: string; passed: boolean; summary: string; attempt: number; commands?: Command[] } | null;
  can_retry?: boolean;
  stopped_summary?: string | null;
  plan?: {
    path: string;
    present: boolean;
    progress?: { done: number; total: number } | null;
    open_questions?: number;
    outline: Heading[];
  } | null;
  counts?: Counts | null;
  counts_says?: string | null;
  token_rows?: Array<TokenRow & { says?: string }>;
  drifts?: Array<SpecDrift & { says: string }>;
  run_rows?: Array<{ id: string; sent_says: string; sent?: SentTask[] | null }>;
  shape_says?: string | null;
  review_files?: number | null;
  reports?: Array<{ id: string; target_says: string; state_says: string; age_says: string; quoted: string }>;
};

/// How a command ended, in the reducer's words — an exit code, a timeout,
/// never started, or unknown: four facts, never one blank.
export function exit(o: Outcome): string {
  if (o.outcome === "exited") return `exit ${o.code}`;
  if (o.outcome === "timed_out") return `timed out after ${o.after_secs}s`;
  return `${o.outcome.replace(/_/g, " ")}${o.reason ? `: ${o.reason}` : ""}`;
}

/// A duration a person reads: 840 ms, 3.2 s, 4 m 10 s.
export function took(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60_000)} m ${Math.round((ms % 60_000) / 1000)} s`;
}

/// When, relative to now, in the words a status line uses.
export function ago(at?: string | null): string {
  if (!at) return "—";
  const t = new Date(at).getTime();
  if (Number.isNaN(t)) return "—";
  const s = Math.max(0, Math.round((Date.now() - t) / 1000));
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return `${Math.round(s / 86400)}d ago`;
}
