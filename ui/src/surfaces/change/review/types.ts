// The review as `/api/changes/{id}/review` serves it. Every sentence is the host's.
import type { Coverage } from "../../../wire/Coverage";
import type { ReviewRole } from "../../../wire/ReviewRole";
import type { SentTask } from "../../../wire/SentTask";
import type { Weakened } from "../../../wire/Weakened";

export type Kind = "context" | "added" | "removed";
/// `digest` is the hunk's content (a seen mark and a comment are keyed by it);
/// `seen` is the host's mark. Both are absent in recordings made before them.
export type Hunk = { header: string; lines: [Kind, string][]; formatter_only: boolean; digest?: string; seen?: boolean };
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
  /// A file with no hunks is one unit to mark: whether it is marked.
  unit_seen?: boolean | null;
  /// A hunk here was marked seen and has since been rewritten.
  changed_since_seen?: boolean;
};
/// A group of files. The first is *checks weakened or changed* when the change
/// skipped, deleted or redefined a check; `weakened` says what matched per file.
/// Each weakened row names what it matched, and whether a person marked it
/// seen (durable, in the host: an offer waits for every one).
export type WeakRow = Weakened & { seen: boolean; gate_says?: string | null };
/// A comment as the host keeps it, with where its line is now and, once its
/// run has finished a turn since it was sent, whether its hunk changed.
export type Comment = import("../../../wire/ReviewComment").ReviewComment & {
  anchor: import("../../../wire/Anchor").Anchor;
  hunk_changed: boolean | null;
};
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
  qualifier?: import("../../../wire/Qualifier").Qualifier | null;
  comments?: Comment[];
  /// Files none of whose hunks is marked seen, by path; never a share.
  not_opened?: string[];
  not_opened_says?: string | null;
};

/// A hunk's key for marks and the cursor: its content digest from the host,
/// or — in a recording made before digests — its place in the file.
export const hunkKey = (h: Hunk, i: number): string => h.digest ?? `${i}:${h.header}`;

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
