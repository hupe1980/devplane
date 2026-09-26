// The inbox narrowed to one project, read once for the list and the item
// beside it. The narrowing is the host's (`core::attention::narrow`), so the
// window and the terminal narrow by one computation. A narrowed read that
// fails says so; it never falls back to the whole inbox under a chip that
// still names the project.

import { api } from "../../lib/api";
import { failure, same, type Failure, type Phase } from "../../lib/resource.svelte";
import type { Item } from "./Item.svelte";
import type { Narrowed } from "../../wire/Narrowed";

export type Summary = { kind: string; project: string | null; count: number; level: string };
export type Inhibited = { cause: string; count: number; because: string };
/// The inbox as `/api/inbox?project=` answers it.
export type NarrowedInbox = {
  items: Item[];
  folded: Summary[];
  inhibited: Inhibited[];
  narrowed?: Narrowed | null;
};

/// As often as the feed: a narrowed inbox is the same list, filtered.
const EVERY_MS = 2_000;

const shared = $state<{ project: string; data: NarrowedInbox | null; failure: Failure | null; at: number | null }>({
  project: "",
  data: null,
  failure: null,
  at: null,
});
let inflight = 0;
/// Every request is numbered; an answer older than one already shown is
/// dropped, so a poll sent before an answer never brings the answered item
/// back after the re-read that followed it.
let seq = 0;
let landed = 0;
let users = 0;
let timer: ReturnType<typeof setInterval> | null = null;

/// A poll waits for any read still out; a re-read after a write does not.
async function read(project: string, force = false) {
  if (inflight > 0 && !force) return;
  const n = ++seq;
  inflight += 1;
  try {
    // Not a look: a narrowed view must not advance the close's boundary.
    const r = await api<NarrowedInbox>(`/api/inbox?project=${encodeURIComponent(project)}`);
    if (shared.project !== project || n < landed) return;
    landed = n;
    if (!same($state.snapshot(shared.data), r)) shared.data = r;
    shared.failure = null;
    shared.at = Date.now();
  } catch (e) {
    if (shared.project !== project || n < landed) return;
    landed = n;
    shared.failure = failure(e, `devplane inbox --project ${project}`);
  } finally {
    inflight = Math.max(0, inflight - 1);
  }
}

/// The narrowed inbox for `project()`, shared by every caller on screen: one
/// request per poll however many regions show it. Call during setup.
export function narrowed(project: () => string) {
  const key = $derived(project().trim());
  $effect(() => {
    const want = key;
    if (!want) return;
    if (shared.project !== want) {
      shared.project = want;
      shared.data = null;
      shared.failure = null;
      shared.at = null;
      landed = seq;
    }
    users += 1;
    if (users === 1 || shared.data === null) void read(want);
    if (timer === null) {
      timer = setInterval(() => {
        if (!document.hidden && shared.project) void read(shared.project);
      }, EVERY_MS);
    }
    return () => {
      users -= 1;
      if (users === 0 && timer !== null) {
        clearInterval(timer);
        timer = null;
      }
    };
  });
  const mine = () => !!key && shared.project === key;
  return {
    /// Whether a narrowing is asked for at all.
    get on() {
      return !!key;
    },
    get data(): NarrowedInbox | null {
      return mine() ? shared.data : null;
    },
    get failure(): Failure | null {
      return mine() ? shared.failure : null;
    },
    get at(): number | null {
      return mine() ? shared.at : null;
    },
    get phase(): Phase {
      const d = mine() ? shared.data : null;
      const f = mine() ? shared.failure : null;
      return d ? (f ? "stale" : "ok") : f ? "failed" : "loading";
    },
    /// The host found no project by that name: a failure, never *Clear.*
    get missing(): boolean {
      return mine() && shared.data?.narrowed?.no_such_project === true;
    },
    reload: () => (key ? read(key, true) : Promise.resolve()),
  };
}
