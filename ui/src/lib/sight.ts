// What this host cannot see, said beside an empty list so *nothing here*
// never reads like *nothing visible here*. A host that has never had a session
// report to it knows nothing about the agents on this machine: an empty inbox,
// ledger or search there is the limit of its sight, not calm.

type Board = {
  runs?: unknown[];
  summary?: { runs?: number } | null;
  watching?: { driven_only?: string[]; unproved?: string[] } | null;
};

/// The sentence for a host no session has reported to, else `""`. Nothing
/// before the board is read: the board's absence is not a fact about sight.
export function unseen(board: unknown): string {
  const b = board as Board | null;
  if (!b || typeof b !== "object") return "";
  if ((b.runs?.length ?? 0) > 0 || (b.summary?.runs ?? 0) > 0) return "";
  const only = b.watching?.driven_only ?? [];
  const unproved = b.watching?.unproved ?? [];
  const parts = [
    "No session has reported to Devplane on this machine yet, so this cannot say what your agents are doing.",
    only.length ? `${only.join(", ")} ${only.length === 1 ? "appears" : "appear"} only when Devplane starts ${only.length === 1 ? "it" : "them"};` : "",
    unproved.length ? `${unproved.join(", ")} ${unproved.length === 1 ? "is" : "are"} read once connected.` : "",
    "`devplane doctor` says what is watched here.",
  ];
  return parts.filter(Boolean).join(" ");
}
