// The host's recorded responses, found by the change's title: file names
// carry ids that change on every capture.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;

const all = import.meta.glob("../fixtures/*.json", { eager: true, import: "default" }) as Record<string, Json>;

/// The recording of `kind` (`change`, `review`, `certificate`) whose title is `title`.
export function recorded(kind: "change" | "review", title: string): Json {
  for (const [path, body] of Object.entries(all)) {
    const name = path.split("/").pop() ?? "";
    if (name.startsWith(`${kind}-`) && body?.title === title) return body;
  }
  throw new Error(`no recorded ${kind} titled "${title}" — run scripts/capture-fixtures.sh`);
}
