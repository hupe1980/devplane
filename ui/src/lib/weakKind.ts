// A weakened row's kind in words: the same words the host's `WeakKind::says`
// gives, one map, so every surface names a kind alike. None says whether an
// edit is good or bad — it names what kind of file was edited.
import type { WeakKind } from "../wire/WeakKind";

export const weakKindSays: Record<WeakKind, string> = {
  skip: "check weakened",
  deleted: "test deleted",
  gate: "gate changed",
  agent_controls: "agent controls",
  ownership: "ownership",
  supply_chain: "supply chain",
  base_tests: "base tests",
};

/// The kind's words as a label at the start of a line.
export const weakKindLabel = (k: WeakKind): string => {
  const s = weakKindSays[k] ?? "";
  return s.charAt(0).toUpperCase() + s.slice(1);
};
