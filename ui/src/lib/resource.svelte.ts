// One way for a surface to read from the host, with one set of failure
// semantics. A read is `loading` until something came back, `ok` while the
// last read landed, `stale` when a re-read failed and the last good data is
// still on screen, and `failed` when nothing was ever read. A failure carries
// the host's reason and the command that tells more; it is never rendered as
// empty or calm. `lib/Failed.svelte` draws the failed and stale phases.

import { untrack } from "svelte";
import { api, Unauthorised, Unreachable } from "./api";

export type Phase = "loading" | "ok" | "stale" | "failed";

/// Why a read or a write did not land, and the command to run for more.
export type Failure = { says: string; tell: string };

/// The failure a thrown error is. `tell` is the surface's own command for
/// more (`devplane change show <id>`); a host that is down or a tab without
/// a token has a better one, whatever was being read.
export function failure(e: unknown, tell = "devplane doctor"): Failure {
  if (e instanceof Unauthorised) return { says: "this tab has no token", tell: "devplane open" };
  if (e instanceof Unreachable) {
    return { says: e.message, tell: e.kind === "refused" ? "devplane serve" : "devplane doctor" };
  }
  return { says: e instanceof Error ? e.message : String(e), tell };
}

/// Whether two answers are the same JSON. A re-read that changed nothing
/// keeps the old object, so nothing downstream wakes up.
export function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  try {
    return JSON.stringify(a) === JSON.stringify(b);
  } catch {
    return false;
  }
}

export type Resource<T> = {
  readonly phase: Phase;
  /// The last good answer, kept through a failed re-read.
  readonly data: T | null;
  readonly failure: Failure | null;
  /// When the last good answer landed (ms since the epoch).
  readonly at: number | null;
  /// Reads again now, keeping what is shown until the answer lands.
  reload: () => Promise<void>;
};

/// Reads `url()` while the calling component is mounted. Call it during
/// component setup. The read restarts only when the URL's *value* changes —
/// a prop object that is rebuilt with the same id does not re-read — and a
/// changed URL drops the old answer and the old failure, so one item never
/// shows another's. `every` re-reads on a timer while the page is visible.
export function resource<T>(
  url: () => string | null,
  opts: { every?: number; tell?: () => string } = {},
): Resource<T> {
  let data = $state.raw<T | null>(null);
  let fail = $state.raw<Failure | null>(null);
  let at = $state<number | null>(null);
  const key = $derived(url());
  /// Bumped on every URL change, so a late answer for the old URL is dropped.
  let gen = 0;
  /// Every request is numbered; an answer older than one already shown is
  /// dropped, so a slow poll never overwrites a re-read sent after a write
  /// (an answered ask coming back from the dead).
  let seq = 0;
  let landed = 0;
  /// Requests still out for the current URL; a timer tick waits for them.
  let pending = 0;

  async function read(want: string, mine: number) {
    const n = ++seq;
    pending += 1;
    try {
      const r = await api<T>(want);
      if (mine !== gen || n < landed) return;
      landed = n;
      if (!same(data, r)) data = r;
      fail = null;
      at = Date.now();
    } catch (e) {
      if (mine !== gen || n < landed) return;
      landed = n;
      fail = failure(e, opts.tell?.());
    } finally {
      if (mine === gen) pending = Math.max(0, pending - 1);
    }
  }

  $effect(() => {
    const want = key;
    const mine = untrack(() => {
      gen += 1;
      pending = 0;
      landed = seq;
      data = null;
      fail = null;
      at = null;
      return gen;
    });
    if (!want) return;
    void read(want, mine);
    if (!opts.every) return;
    const t = setInterval(() => {
      if (!document.hidden && pending === 0) void read(want, mine);
    }, opts.every);
    return () => clearInterval(t);
  });

  return {
    get phase(): Phase {
      return data !== null ? (fail ? "stale" : "ok") : fail ? "failed" : "loading";
    },
    get data() {
      return data;
    },
    get failure() {
      return fail;
    },
    get at() {
      return at;
    },
    async reload() {
      const want = untrack(() => key);
      if (want) await read(want, gen);
    },
  };
}

/// Copies text, and says whether it did. `navigator.clipboard` is absent on
/// a page that is not a secure origin, where optional chaining would resolve
/// to `undefined` and a caller would claim the copy landed.
export async function copyText(text: string): Promise<boolean> {
  try {
    if (!navigator.clipboard?.writeText) return false;
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
