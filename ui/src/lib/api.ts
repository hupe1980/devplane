// Talking to the host on loopback. The token arrives once in the URL from
// `devplane open`, is kept for the tab, and is stripped from the address bar
// so it never lands in a screenshot or a bookmark.

const KEY = "vp_token";

/// Reads the token out of the URL once, then out of the tab thereafter.
/// Safe to call repeatedly.
export function claimToken(): string {
  // `sessionStorage` can throw (blocked site data); then ask for the link again.
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

/// How long any one request may take before it is a failure rather than a
/// wait, so a wedged host never leaves a surface on a spinner.
const TIMEOUT_MS = 10_000;

/// A request that did not come back. Refused means nothing is listening
/// (start it); a timeout means it is listening but slow or wedged.
export type UnreachableKind = "refused" | "timeout";

export class Unreachable extends Error {
  readonly kind: UnreachableKind;
  constructor(kind: UnreachableKind) {
    super(
      kind === "timeout"
        ? "Devplane is not answering (slow or wedged)"
        : "Devplane is not running — start it with `devplane serve`",
    );
    this.kind = kind;
  }
}

/// One request, with auth and error handling in one place. A 401 is its own
/// error: the tab lost its token, and retrying cannot fix that.
export async function api<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const token = claimToken();
  let res: Response;
  try {
    res = await fetch(path, {
      ...opts,
      // Every body is JSON, so the header is set here rather than per call.
      headers: {
        ...(opts.body ? { "content-type": "application/json" } : {}),
        ...(opts.headers ?? {}),
        Authorization: `Bearer ${token}`,
      },
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
  } catch (e) {
    // A timeout rejects with `TimeoutError`; a refused connection with `TypeError`.
    const timedOut = e instanceof DOMException && e.name === "TimeoutError";
    throw new Unreachable(timedOut ? "timeout" : "refused");
  }
  if (res.status === 401) throw new Unauthorised();
  // A refusal is shown in the host's words (`{ "error": "…" }`), not a status.
  if (!res.ok) {
    const said = await res
      .json()
      .then((b: { error?: unknown }) => (typeof b?.error === "string" ? b.error : ""))
      .catch(() => "");
    throw new Error(said || `${path} → ${res.status}`);
  }
  return (await res.json()) as T;
}
