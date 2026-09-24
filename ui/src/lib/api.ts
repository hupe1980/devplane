// Talking to the daemon on loopback.
//
// **The token arrives once in the URL from `devplane open`, is kept for the
// tab, and is stripped from the address bar** so it does not end up in a
// screenshot or a bookmark. That is the behaviour the page being replaced has
// and it is carried over unchanged — it is the only thing standing between a
// bearer token and somebody's screen recording.

const KEY = "vp_token";

/// Reads the token out of the URL once, then out of the tab thereafter.
///
/// Called for its side effect at start-up. Safe to call twice: the second call
/// finds no token in the URL and reads the stored one.
export function claimToken(): string {
  // `sessionStorage` throws in a private window with site data blocked, and a
  // board that fails to load because storage is unavailable is worse than one
  // that asks for the link again.
  try {
    const url = new URL(location.href);
    const fromUrl = url.searchParams.get("token");
    if (fromUrl) {
      sessionStorage.setItem(KEY, fromUrl);
      url.searchParams.delete("token");
      history.replaceState({}, "", url);
      return fromUrl;
    }
    return sessionStorage.getItem(KEY) ?? "";
  } catch {
    return "";
  }
}

export class Unauthorised extends Error {
  constructor() {
    super("unauthorised — run `devplane open` again");
  }
}

/// How long any one request may take before it is a failure rather than a wait.
///
/// **An eternal spinner is the worst state a surface can be in**, and it is the
/// one a dead daemon produces: the port in the address bar is the one Devplane
/// was on when the tab was opened, the daemon is restarted, it comes back on a
/// different port, and every fetch from that tab hangs for ever. The page sits
/// on *Reading…* — indistinguishable from a slow read, and saying nothing about
/// the one thing that would fix it.
///
/// Ten seconds: longer than any route here takes on a machine with sixty
/// projects, short enough that a person has not yet decided the product is
/// broken.
const TIMEOUT_MS = 10_000;

/// A request that did not come back.
///
/// Its own type because it has its own answer, and the answer is not *try
/// again*: the daemon this tab was opened against is gone, and only a new link
/// finds the new one.
export class Unreachable extends Error {
  constructor(path: string) {
    super(
      `${path} did not answer. The daemon may have restarted on another port — ` +
        "run `devplane open` for a fresh link.",
    );
  }
}

/// One request, with the auth and the error handling in one place.
///
/// **401 is its own error type** because it has its own answer: every other
/// failure is *try again*, and this one is *the tab lost its token*, which no
/// amount of retrying fixes.
export async function api<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const token = claimToken();
  // **Bounded, because the alternative is a spinner that never stops.** A tab
  // whose daemon has moved gets an answer it can act on instead of a page that
  // looks like it is still working.
  let res: Response;
  try {
    res = await fetch(path, {
      ...opts,
      headers: { ...(opts.headers ?? {}), Authorization: `Bearer ${token}` },
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
  } catch {
    // A timeout, a refused connection and a dropped network all mean the same
    // thing to somebody reading the page: nothing answered.
    throw new Unreachable(path);
  }
  if (res.status === 401) throw new Unauthorised();
  if (!res.ok) throw new Error(`${path} → ${res.status}`);
  return (await res.json()) as T;
}
