// The specification's tasks beside the agent's own plan, matched by key. A
// step pairs with a task only when its text contains the task's key (the line
// without its box); anything unmatched sits alone. Nothing is inferred.

export type Task = { text: string; path?: string; line?: number };
export type Step = { content: string; status: string };

/// One row of the two columns: a task, a step, or both when they match.
export type Row = { task: Task | null; step: Step | null };

/// Whether `step` contains `task`'s key, case-insensitive, whitespace folded.
export function names(step: Step, task: Task): boolean {
  const key = fold(task.text);
  return key.length > 0 && fold(step.content).includes(key);
}

function fold(s: string): string {
  return s.toLowerCase().replace(/\s+/g, " ").trim();
}

/// Tasks in their order, each with the first step that names it; then every
/// step that named nothing, alone, in the plan's own order.
export function pair(tasks: Task[], steps: Step[]): Row[] {
  const taken = new Set<number>();
  const rows: Row[] = tasks.map((task) => {
    const at = steps.findIndex((s, i) => !taken.has(i) && names(s, task));
    if (at === -1) return { task, step: null };
    taken.add(at);
    return { task, step: steps[at] };
  });
  steps.forEach((step, i) => {
    if (!taken.has(i)) rows.push({ task: null, step });
  });
  return rows;
}
