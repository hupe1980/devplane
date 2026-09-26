// A write in progress: one at a time per owner, with its elapsed time. The
// host runs the gates, pushes and creates worktrees inline, so a write takes
// as long as that work does; the control says how long it has been going and
// refuses a second press rather than starting the same work twice.

import { ago } from "./text";

export type Writer = {
  /// What is being written, or `""` when nothing is.
  readonly busy: string;
  /// How long it has been going, as `12s`; `""` when nothing is.
  readonly elapsed: string;
  /// Runs `work` as `what`, unless a write is already going (then `null`).
  run: <T>(what: string, work: () => Promise<T>) => Promise<T | null>;
};

export function writer(): Writer {
  let busy = $state("");
  let since = $state(0);
  let now = $state(0);
  let tick: ReturnType<typeof setInterval> | null = null;

  return {
    get busy() {
      return busy;
    },
    get elapsed() {
      return busy ? ago(Math.max(0, (now - since) / 1000)) : "";
    },
    async run<T>(what: string, work: () => Promise<T>): Promise<T | null> {
      if (busy) return null;
      busy = what;
      since = now = Date.now();
      tick = setInterval(() => (now = Date.now()), 1_000);
      try {
        return await work();
      } finally {
        if (tick !== null) clearInterval(tick);
        tick = null;
        busy = "";
      }
    },
  };
}
