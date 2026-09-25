// What the host currently says, polled. One store, so board and inbox are
// read at the same moment. A poll is not a look: `?read=true` is sent only
// when somebody is looking at the inbox.

import { api, Unauthorised } from "./api";

/// How often to ask. Cheap: the host is on loopback and the answer is a
/// projection it already holds.
const EVERY_MS = 2_000;

export type Live = {
  board: unknown | null;
  inbox: unknown | null;
  /// Whether a poll has ever come back. Before it has, the feed is unknown,
  /// not empty, and must never render as *Clear.*.
  loaded: boolean;
  /// When the last good read landed, kept only while reads are failing, so
  /// surfaces can say the data is from then.
  stale_since: string | null;
  /// What went wrong last, if anything — shown, never swallowed.
  error: string | null;
  /// Set when the tab has no usable token; polling stops.
  unauthorised: boolean;
};

/// Starts the poll. `looking` asks the shell whether the surface that marks a
/// look is showing; the store must not know.
export function live(looking: () => boolean): {
  state: Live;
  start: () => () => void;
  /// Fetches now, marking a look if `looking()` says so. The shell calls it
  /// when the looked-at surface comes on screen.
  look: () => void;
} {
  const state = $state<Live>({
    board: null,
    inbox: null,
    loaded: false,
    stale_since: null,
    error: null,
    unauthorised: false,
  });

  /// One request in flight at a time, so a stalled host is not queued up.
  let inflight = false;
  let lastGood: string | null = null;
  let timer: ReturnType<typeof setInterval> | null = null;

  async function tick(read: boolean) {
    if (inflight || state.unauthorised) return;
    inflight = true;
    try {
      const [board, inbox] = await Promise.all([
        api<unknown>("/api/board"),
        // Two whole literals, so the route guard reads `/api/inbox`.
        api<unknown>(read ? "/api/inbox?read=true" : "/api/inbox"),
      ]);
      state.board = board;
      state.inbox = inbox;
      state.loaded = true;
      state.error = null;
      state.stale_since = null;
      lastGood = new Date().toISOString();
    } catch (e) {
      if (e instanceof Unauthorised) {
        // Nothing this tab sends will be accepted again: stop and say why.
        state.unauthorised = true;
        state.error = "This tab has no token. Run `devplane open` again for a fresh link.";
        if (timer !== null) clearInterval(timer);
        timer = null;
        return;
      }
      state.error = e instanceof Error ? e.message : String(e);
      state.stale_since = lastGood;
    } finally {
      inflight = false;
    }
  }

  function look() {
    void tick(looking());
  }

  function start(): () => void {
    // The first fetch counts as a look only if the looked-at surface is the
    // one that opened.
    look();
    timer = setInterval(() => void tick(false), EVERY_MS);
    // A returning tab marks a look only if the inbox is what is showing.
    const onVisible = () => {
      if (document.visibilityState === "visible") look();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      if (timer !== null) clearInterval(timer);
      timer = null;
      document.removeEventListener("visibilitychange", onVisible);
    };
  }

  return { state, start, look };
}
