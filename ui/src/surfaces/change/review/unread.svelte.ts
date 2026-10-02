// One *unread*, said the same way wherever a change's review is counted: the
// change's header, its Review tab and the review pane all call `unreadSays`
// over the same review, so the three cannot disagree. Two counts, never one
// figure: hunks this browser has not marked, and weakened rows the host has
// no read mark for.
import { markKey, readMark, writeMark as keep, type Mark } from "./marks";
import type { ReviewBody } from "./types";

/// Bumped on every mark, so anything counting marks counts again.
const marks = $state({ n: 0 });

/// Marks a hunk and tells every count.
export function writeMark(key: string, mark: Mark): void {
  keep(key, mark);
  marks.n += 1;
}

export type Unread = { hunks: number | null; weakened: number };

/// What is unread in a review; `hunks` is `null` before the review is read,
/// when only the host's weakened count (`fallback`) is known.
export function unread(r: ReviewBody | null | undefined, fallback = 0): Unread {
  void marks.n;
  if (!r) return { hunks: null, weakened: fallback };
  let hunks = 0;
  for (const g of r.groups)
    for (const f of g.files)
      for (const h of f.hunks) if (!readMark(markKey(r.change, f.path, h.header, h.lines))) hunks += 1;
  const weakened = r.groups.flatMap((g) => g.weakened ?? []).filter((w) => !w.seen).length;
  return { hunks, weakened };
}

/// *2 hunks unread · 1 weakened*, *1 weakened*, or *0 unread*.
export function unreadSays(u: Unread): string {
  const parts = [
    ...(u.hunks ? [`${u.hunks} ${u.hunks === 1 ? "hunk" : "hunks"} unread`] : []),
    ...(u.weakened ? [`${u.weakened} weakened`] : []),
  ];
  return parts.length ? parts.join(" · ") : u.hunks === null ? "" : "0 unread";
}
