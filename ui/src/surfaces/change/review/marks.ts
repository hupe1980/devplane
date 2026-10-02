// A reader's *seen* mark on a hunk, kept in `localStorage` and never sent to
// the host: a reading aid, not a record of the person.
//
// A mark is keyed by the hunk's content — its header and every line — so a
// hunk the agent rewrote is unseen again, even when its `@@` header (and so
// its line counts) stayed the same. Marks for hunks that are no longer in the
// change are pruned when its review is read.

export type Mark = "seen";

type Lines = ReadonlyArray<readonly [string, string]>;

/// FNV-1a over the text, as eight hex digits: enough to tell two versions of
/// one hunk apart, with no dependency and no async crypto.
function digest(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(16).padStart(8, "0");
}

const prefix = (change: string) => `review:${change}:`;

/// The key of one hunk's mark: the change, the file, and the hunk's content.
export function markKey(change: string, path: string, header: string, lines: Lines): string {
  const body = lines.map(([kind, text]) => `${kind}${text}`).join("\n");
  return `${prefix(change)}${path}:${digest(`${header}\n${body}`)}`;
}

export function readMark(key: string): Mark | null {
  try {
    return localStorage.getItem(key) === "seen" ? "seen" : null;
  } catch {
    return null;
  }
}

export function writeMark(key: string, mark: Mark): void {
  try {
    localStorage.setItem(key, mark);
  } catch {
    // Storage refused: the mark lasts the page.
  }
}

/// Drops this change's marks whose hunk is gone (rewritten or reverted), so
/// a count of marks is a count of hunks that are still there.
export function prune(change: string, live: ReadonlySet<string>): void {
  try {
    const gone: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(prefix(change)) && !live.has(k)) gone.push(k);
    }
    gone.forEach((k) => localStorage.removeItem(k));
  } catch {
    // Storage refused: nothing was kept to prune.
  }
}

/// How many hunks of one change this browser has marked, for a row that has
/// no hunks of its own to key by. Bounded by the change's hunks by the caller.
export function markedFor(change: string): number {
  try {
    let n = 0;
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k?.startsWith(prefix(change)) && readMark(k)) n++;
    }
    return n;
  } catch {
    return 0;
  }
}
