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
  /// How many tests ran, in the runner's own nouns, or why that is not known.
  tests?: CommandCount;
};
/// One runner's summary from one command.
export type Tally = { runner: string; counts: [string, number][]; ran: number; lines: number };
export type CommandCount =
  | { count: "read"; tallies: Tally[] }
  | { count: "not_recognised"; runner: string; says: string }
  | { count: "no_runner" }
  | { count: "no_exit" };
/// How a base-checks sentence is weighed: never the verified colour.
export type Weight = "warn" | "plain" | "absent";
export type BaseRunOutcome = { ran: "passed" | "failed" | "timed_out" | "could_not_run" | "skipped"; why?: string; after_s?: number };
/// Base checks as the host composed them: every sentence is its.
export type BaseChecksView = {
  counts_says: string;
  absent?: { absent: string; says: string } | null;
  stale: boolean;
  base?: string | null;
  tree?: string | null;
  at?: string | null;
  can_run: boolean;
  certificate: string;
  junit_notes: string[];
  /// Whether the evidence tells this change from its base: the section's
  /// first sentence, for a current result.
  headline?: { says: string; weight: Weight; named: string[]; named_more: number } | null;
  /// Why the last attempt stopped before any result.
  did_not_run?: string | null;
  on_change?: {
    says: string;
    weight: Weight;
    outcome: BaseRunOutcome;
    restored: string[];
    removed: string[];
    inline: string[];
    commands: Command[];
    failing: string[];
    failing_more: number;
    junit_notes: string[];
    named?: {
      not_run_on_change: string[];
      not_run_on_change_more: number;
      became_skipped: string[];
      became_skipped_more: number;
      changed: string[];
      changed_more: number;
      says: string;
    } | null;
  } | null;
  on_base?: {
    says: string;
    weight: Weight;
    outcome: BaseRunOutcome;
    placed: { path: string; renamed_from?: string | null; blob?: string | null }[];
    not_separable: string[];
    commands: Command[];
    passing: string[];
    passing_more: number;
    failing: string[];
    failing_more: number;
  } | null;
};
/// A command's count in words, as the host would say it.
export function countSays(t: CommandCount | undefined): string {
  if (!t || t.count === "no_runner") return "no test runner's summary in this output";
  if (t.count === "no_exit") return "did not finish, so no count was read";
  if (t.count === "not_recognised") return t.says;
  return t.tallies
    .map((x) => [`${x.runner} ran ${x.ran}`, ...x.counts.filter(([n, c]) => c > 0 && !["run", "total", "tests"].includes(n)).map(([n, c]) => `${n} ${c}`)].join(" · "))
    .join(" · ");
}
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
  /// A specification drafted by the project's own methodology: what was
  /// sent, where it was written, and where it got to.
  draft?: { methodology: "speckit" | "openspec"; agent: string; sent: string; folder: string; outcome: "pending" | "produced" | "none_produced" | "accepted" | "rejected" } | null;
  draft_says?: string[];
  /// After a draft, when the project declares no unresolved-question words.
  open_questions_offer?: { says: string; diff?: string; base?: string } | null;
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
  pull_request?: {
    url?: string;
    number?: number;
    status?: string;
    /// GitHub's own review decision (`APPROVED`, …), quoted as GitHub's.
    review_decision?: string | null;
    /// Who the approvals came from, as last read from the forge.
    approvals?: Approvals | null;
  } | null;
  created_at?: string;
  updated_at?: string;
  state: string;
  /// What the change's own diff weakened, by kind, beside its state.
  qualifier?: import("../../wire/Qualifier").Qualifier | null;
  in_place?: boolean;
  in_place_says?: string | null;
  standing_says?: string | null;
  gate?: { name: string; passed: boolean; summary: string; attempt: number; commands?: Command[]; ran_at?: Stamp | null } | null;
  can_retry?: boolean;
  stopped_summary?: string | null;
  plan?: {
    path: string;
    present: boolean;
    progress?: { done: number; total: number } | null;
    /// Why there is no count: the layout keeps no task file, or a schema not read.
    no_progress?: string | null;
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
  /// How many tests ran and the base checks, beside the standing.
  base_checks?: BaseChecksView | null;
  /// The open stop claim: the agent says the spec and the tests conflict.
  stop_claim?: ClaimView | null;
  /// Every stop claim, newest first, open or closed.
  claim_history?: ClaimView[];
  /// GSD's gitignored `.planning/` is not in this change's worktree; names `.worktreeinclude`.
  planning_absent?: string | null;
  /// What the current run says about its session's permission mode, in the
  /// agent's own words. *Decides without you* is shown whole, never folded.
  mode_statement?: ModeStatement | null;
};

/// A driven session's mode statement, as the host composes it.
export type ModeStatement = {
  sentence: string;
  mode?: string | null;
  name?: string | null;
  reading: "asks" | "decides_without_you" | "not_known";
  decides?: string | null;
  why: string;
  detail?: string | null;
  at?: string | null;
};

/// What an agent said through the stop tool, as the host renders it. The
/// agent's words: shown as text, never markup, never a link.
export type ClaimView = {
  id: string;
  says: string;
  reason: string;
  paths: string[];
  agent: string;
  run: string;
  at: string;
  origin_says: string;
  quoted: string;
  ending: string;
  state: "open" | "closed";
  /// Why it closed, as the host names it; `closed_says` is the sentence.
  closed_by?: string | null;
  closed_says?: string | null;
  can_tell?: boolean;
};

/// The tasks, as one line of counts side by side, never one over another:
/// *5 tasks · 3 sent · 2 ticked · 2 seen by a passing check*. *Sent* is how
/// many of them any run of this change was handed. *No gates declared* is
/// said only when the host says so (`seen_by_pass: null`); a count the host
/// did not send at all is left out rather than guessed.
export function taskLine(d: Detail): string {
  const c = d.counts;
  if (!c) return "";
  const sent = new Set((d.run_rows ?? []).flatMap((r) => (r.sent ?? []).map((t) => `${t.path}:${t.text}`))).size;
  const seen =
    c.seen_by_pass === null ? "no gates declared" : typeof c.seen_by_pass === "number" ? `${c.seen_by_pass} seen by a passing check` : "";
  return [`${c.tasks} ${c.tasks === 1 ? "task" : "tasks"}`, `${sent} sent`, `${c.ticked} ticked`, seen].filter(Boolean).join(" · ");
}

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

/// One reviewer's standing on a pull request, as the forge's review list said.
export type BearingReview = {
  account: { id: number; login: string; type: string } | null;
  kind: "person" | "model" | "bot" | "unclassified";
  product?: string;
  received?: string;
  state: string;
  commit: string | null;
  onHead: "head" | "older" | "gone";
  submittedAt?: string | null;
};

/// One read of a pull request's approvals; `sentence` is the one every
/// surface prints.
export type ApprovalsReading = {
  number: number;
  url: string;
  head: string;
  base: string;
  requiredSays?: string | null;
  bearing: BearingReview[];
  unreadPages?: number | null;
  sentence: string;
  modelWord: "unattributed" | "model";
  readAt: string;
  afterMerge: boolean;
};

export type Approvals =
  | { state: "current"; reading: ApprovalsReading }
  | { state: "stale"; last: ApprovalsReading; why: string }
  | { state: "not_read"; why: string };
