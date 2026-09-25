// A reader's marks on a hunk — *seen*, *accepted* — kept in `localStorage`
// and never sent to the host: a reading aid, not a record of the person.

export type Mark = "seen" | "accepted";

export function markKey(change: string, path: string, header: string): string {
  return `review:${change}:${path}:${header}`;
}

export function readMark(key: string): Mark | null {
  try {
    const v = localStorage.getItem(key);
    return v === "seen" || v === "accepted" ? v : null;
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
