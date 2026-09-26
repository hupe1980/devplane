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

/// How long a read may take before it is a failure rather than a wait, so a
/// wedged host never leaves a surface on a spinner. A write has no timeout:
/// running the gates, pushing and creating a worktree take as long as they
/// take, and the control that sent one shows its elapsed time instead.
const READ_TIMEOUT_MS = 10_000;

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

/// A request the host answered with a refusal: its words, its status, and the
/// whole body, for a refusal with structure (a 409 naming rows).
export class Refused extends Error {
  readonly status: number;
  readonly body: unknown;
  constructor(said: string, status: number, body: unknown) {
    super(said);
    this.status = status;
    this.body = body;
  }
}

/// Whether a request writes: anything but a GET or HEAD.
const writes = (opts: RequestInit) => !!opts.method && !/^(GET|HEAD)$/i.test(opts.method);

/// One request, with auth and error handling in one place. A 401 is its own
/// error: the tab lost its token, and retrying cannot fix that.
export async function api<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const token = claimToken();
  const timeout = writes(opts) ? {} : { signal: AbortSignal.timeout(READ_TIMEOUT_MS) };
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
      ...timeout,
    });
  } catch (e) {
    // A timeout rejects with `TimeoutError`; a refused connection with `TypeError`.
    const timedOut = e instanceof DOMException && e.name === "TimeoutError";
    throw new Unreachable(timedOut ? "timeout" : "refused");
  }
  if (res.status === 401) throw new Unauthorised();
  // A refusal is shown in the host's words (`{ "error": "…" }`, or a plain
  // text body from the router's own 404 and 422), not a status.
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    let body: unknown = null;
    try {
      body = text ? JSON.parse(text) : null;
    } catch {
      body = null;
    }
    const error = (body as { error?: unknown } | null)?.error;
    const said = typeof error === "string" ? error : body === null ? text.trim().slice(0, 300) : "";
    throw new Refused(said || `${path} → ${res.status}`, res.status, body);
  }
  return (await res.json()) as T;
}
