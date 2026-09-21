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

/// One request, with the auth and the error handling in one place.
///
/// **401 is its own error type** because it has its own answer: every other
/// failure is *try again*, and this one is *the tab lost its token*, which no
/// amount of retrying fixes.
export async function api<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const token = claimToken();
  const res = await fetch(path, {
    ...opts,
    headers: { ...(opts.headers ?? {}), Authorization: `Bearer ${token}` },
  });
  if (res.status === 401) throw new Unauthorised();
  if (!res.ok) throw new Error(`${path} → ${res.status}`);
  return (await res.json()) as T;
}
