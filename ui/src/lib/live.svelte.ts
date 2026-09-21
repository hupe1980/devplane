// What the daemon currently says, polled.
//
// **A poll is not a look**, which is why `?read=true` is sent by the CLI and by
// this page only when the inbox becomes visible after being away — the board
// fetches every couple of seconds, and marking on every fetch would erase the
// boundary it is drawing and leave the hairline permanently reading `0m`.
//
// One store, because two would be two clocks: a board and an inbox fetched
// independently disagree about the same moment, and the disagreement shows up
// as a count that does not match the rows under it.

import { api, Unauthorised } from "./api";

/// How often to ask. The daemon is on loopback and the answer is a projection
/// it already holds, so this is cheap — but it is still a poll, and every one
/// of them is a decision not to have built a stream.
const EVERY_MS = 2_000;

export type Live = {
  board: unknown | null;
  inbox: unknown | null;
  /// What went wrong last, if anything. **Shown rather than swallowed**: a
  /// board that silently stops refreshing looks like a quiet machine, which is
  /// the worst way for this page to be wrong.
  error: string | null;
  /// Set when the tab has no usable token. It has its own answer — open it
  /// again from the shell — and retrying does not help.
  unauthorised: boolean;
};

export function live(): { state: Live; start: () => () => void } {
  const state = $state<Live>({ board: null, inbox: null, error: null, unauthorised: false });

  async function tick(read: boolean) {
    try {
      const [board, inbox] = await Promise.all([
        api<unknown>("/api/board"),
        // The query string is a parameter, not part of the route — built as
        // one it reads as `/api/inbox{}`, which is a route nobody serves.
        api<unknown>(read ? "/api/inbox?read=true" : "/api/inbox"),
      ]);
      state.board = board;
      state.inbox = inbox;
      state.error = null;
      state.unauthorised = false;
    } catch (e) {
      if (e instanceof Unauthorised) {
        state.unauthorised = true;
        state.error = e.message;
        return;
      }
      state.error = e instanceof Error ? e.message : String(e);
    }
  }

  function start(): () => void {
    // The first fetch counts as a look: somebody opened the page.
    void tick(true);
    const id = setInterval(() => void tick(false), EVERY_MS);
    // **Marks a look when the tab comes back**, not on every repaint. A person
    // returning to a tab they left this morning has read it; a page refreshing
    // behind a different window has not.
    const onVisible = () => {
      if (document.visibilityState === "visible") void tick(true);
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }

  return { state, start };
}
