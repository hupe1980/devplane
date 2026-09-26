// The review as `/api/changes/{id}/review` serves it. Every sentence is the host's.
import type { Coverage } from "../../wire/Coverage";
import type { ReviewRole } from "../../wire/ReviewRole";
import type { SentTask } from "../../wire/SentTask";
import type { Weakened } from "../../wire/Weakened";

export type Kind = "context" | "added" | "removed";
export type Hunk = { header: string; lines: [Kind, string][]; formatter_only: boolean };
export type Decision = { authority: string; action: string; outcome: string; at: string; subject: string; says: string };
export type File = {
  path: string;
  status_says: string;
  added: number;
  removed: number;
  role: ReviewRole | null;
  role_says: string;
  coverage: Coverage;
  coverage_says: string | null;
  decisions: Decision[];
  task_group: number | null;
  task_says: string;
  body_says: string | null;
  hunks: Hunk[];
  marker_commands: string[];
};
/// A group of files. The first is *checks weakened or changed* when the change
/// skipped, deleted or redefined a check; `weakened` says what matched per file.
/// Each weakened row names what it matched, and whether a person marked it
/// seen (durable, in the host: an offer waits for every one).
export type WeakRow = Weakened & { seen: boolean };
export type Group = { role: ReviewRole | null; says: string; files: File[]; weakened?: WeakRow[] };
export type IntentGroup = { run: string; tasks: SentTask[]; title: string; says: string; files: string[] };
export type Intent = {
  heading: string;
  groups: IntentGroup[];
  not_asked_for: Array<{ path: string; says: string }>;
  not_asked_for_heading: string;
  unavailable: string | null;
};
export type ReviewBody = {
  change: string;
  title: string;
  base: string;
  standing_says: string;
  unordered: string | null;
  coverage_absent: string | null;
  shape_says: string;
  groups: Group[];
  intent: Intent;
  formatter_only_collapsed: number;
  formatter_only_says: string | null;
  truncated_says: string | null;
  empty_says: string | null;
  latest_run: string | null;
  qualifier?: import("../../wire/Qualifier").Qualifier | null;
};

/// One row of the risk tab: a role's heading, a file, or one of its hunks.
/// `n` numbers the hunks across the change, which is what `j`/`k` move by.
export type Row =
  | { kind: "group"; says: string }
  | { kind: "file"; file: File; fi: number }
  | { kind: "hunk"; file: File; fi: number; hunk: Hunk; n: number };

/// The change flattened into rows, in reading order.
export function rows(groups: Group[]): Row[] {
  const out: Row[] = [];
  let fi = 0;
  let n = 0;
  for (const g of groups) {
    out.push({ kind: "group", says: g.says });
    for (const file of g.files) {
      out.push({ kind: "file", file, fi });
      for (const hunk of file.hunks) out.push({ kind: "hunk", file, fi, hunk, n: n++ });
      fi += 1;
    }
  }
  return out;
}
