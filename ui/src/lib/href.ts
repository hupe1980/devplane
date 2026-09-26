// Host-supplied links, allowed by scheme in one place. A `javascript:` or
// `data:` value would run in this page's origin, where the bearer token is;
// the host builds these links today, but nothing else enforces that.

/// The schemes a link from the host may carry: the web, Devplane's own deep
/// links, and the two vendor deep links the host builds to open an agent
/// (`core::deeplink`). An in-page `#…` address is always allowed.
const SCHEMES = new Set(["http:", "https:", "devplane:", "vscode:", "claude-cli:"]);

/// The link if its scheme is allowed, else `null` (render no link).
export function safeHref(url: string | null | undefined): string | null {
  if (!url) return null;
  const s = url.trim();
  if (s.startsWith("#")) return s;
  const m = /^([a-z][a-z0-9+.-]*:)/i.exec(s);
  if (!m) return null;
  return SCHEMES.has(m[1].toLowerCase()) ? s : null;
}
