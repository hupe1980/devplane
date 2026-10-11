// One *unread*, from the host's seen marks: the CLI, the inbox row and the
// offer read the same marks, so every surface counts one number. A mark made
// here counts at once, before the review is read again; the host keeps it.
import { api } from "../../../lib/api";
import type { File, Hunk, ReviewBody } from "./types";

/// Marks posted from this page, by `change path digest`, until the host's
/// next read carries them. Bumps every count that reads it.
const posted = $state<{ keys: Set<string> }>({ keys: new Set() });
const keyOf = (change: string, path: string, digest: string) => `${change}\u0000${path}\u0000${digest}`;

/// Whether a hunk is seen: by the host, or by a mark just posted from here.
export function isSeen(change: string, path: string, h: Hunk): boolean {
  return !!h.seen || (!!h.digest && posted.keys.has(keyOf(change, path, h.digest)));
}

/// Whether a file with no hunks (one unit) is seen.
export function unitSeen(change: string, f: File): boolean {
  return !!f.unit_seen || posted.keys.has(keyOf(change, f.path, "*"));
}

/// The hunks of a file not yet seen, and its one unit when it has no hunks.
export function unreadIn(change: string, f: File): number {
  if (!f.hunks.length) return f.unit_seen === false && !unitSeen(change, f) ? 1 : 0;
  let n = 0;
  for (const h of f.hunks) if (!isSeen(change, f.path, h)) n += 1;
  return n;
}

/// Marks one hunk seen (or, with no digest, everything at the path) in the
/// host. Throws what the host said when it refused.
export async function markSeen(change: string, path: string, digest: string | null): Promise<void> {
  const k = keyOf(change, path, digest ?? "*");
  posted.keys = new Set([...posted.keys, k]);
  try {
    await api(`/api/changes/${encodeURIComponent(change)}/review/seen`, {
      method: "POST",
      body: JSON.stringify(digest ? { path, digest } : { path }),
    });
  } catch (e) {
    const next = new Set(posted.keys);
    next.delete(k);
    posted.keys = next;
    throw e;
  }
}

export type Unread = { hunks: number | null; weakened: number };

/// What is unread in a review; `hunks` is `null` before the review is read,
/// when only the host's weakened count (`fallback`) is known.
export function unread(r: ReviewBody | null | undefined, fallback = 0): Unread {
  void posted.keys;
  if (!r) return { hunks: null, weakened: fallback };
  let hunks = 0;
  for (const g of r.groups) for (const f of g.files) hunks += unreadIn(r.change, f);
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
