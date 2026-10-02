// The first-run checklist's shapes and the window's half of its state: which
// steps this window skipped. Kept apart from the component so the registry can
// ask whether the checklist is incomplete without loading the page.

export type Connection = {
  agent: string;
  name: string;
  settings_path: string;
  connected: boolean;
  gate_off: string[];
  present: boolean;
};

export type Step = {
  id: string;
  state: "done" | "next" | "waiting";
  says: string;
  writes?: string;
  advances_without_input: boolean;
};
export type FirstRun = {
  project: string | null;
  trusted: Array<{ id: string; name: string; root: string }>;
  latest: { id: string; title: string; state: string; created_at: string; check_failed: string | null } | null;
  agents: Connection[];
  steps: Step[];
};

const KEY = "first-run:skipped";

/// The steps this window skipped: a per-window convenience, never sent to
/// the host. Lost storage only shows the checklist again.
export function skippedSteps(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}
export function keepSkipped(ids: string[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(ids));
  } catch {
    /* lasts the page */
  }
}

/// Whether a step still asks for something: neither done nor skipped.
export function incomplete(steps: Step[], skipped: string[]): boolean {
  return steps.some((s) => s.state !== "done" && !skipped.includes(s.id));
}

export const TITLES: Record<string, string> = {
  sessions: "Sessions already running",
  updates: "Update checks",
  connect: "Connect an agent",
  trust: "Trust a repository",
  github: "Sign in to GitHub",
  declare: "Declare a check",
  dialect: "Spec dialect",
  first_change: "A first verified change",
};
