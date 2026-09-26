// Inline markup for sentences the host composed: `code` and **strong**, and
// nothing else — no links, no images, no raw HTML. The result is a list of
// segments rendered as text nodes by `Inline.svelte`, so nothing here ever
// becomes markup: text quoted inside a sentence stays inert data. Never used
// for untrusted content on its own (a report, a spec line): that stays plain.

/// One run of a host sentence: plain text, a code span, or strong text.
export type Seg = { k: "t" | "code" | "strong"; s: string };

/// A host sentence as segments: `x` as code and **x** as strong. An unpaired
/// backtick or `**` is kept as the character it is.
export function inline(text: string | null | undefined): Seg[] {
  if (!text) return [];
  const out: Seg[] = [];
  // Code spans first, so `**` inside one is shown as written.
  for (const p of text.split(/(`[^`\n]+`)/g)) {
    if (!p) continue;
    if (p.length > 2 && p.startsWith("`") && p.endsWith("`")) {
      out.push({ k: "code", s: p.slice(1, -1) });
      continue;
    }
    for (const q of p.split(/(\*\*[^*\n]+\*\*)/g)) {
      if (!q) continue;
      if (q.length > 4 && q.startsWith("**") && q.endsWith("**")) {
        out.push({ k: "strong", s: q.slice(2, -2) });
      } else {
        out.push({ k: "t", s: q });
      }
    }
  }
  return out;
}

/// The same sentence with the markup removed, for a `title` or plain text.
export function plain(text: string | null | undefined): string {
  if (!text) return "";
  return text.replace(/`([^`\n]+)`/g, "$1").replace(/\*\*([^*\n]+)\*\*/g, "$1");
}

/// One block of a host-composed document. `pre` carries its text as written.
export type Block =
  | { kind: "h" | "p" | "li" | "row"; segs: Seg[] }
  | { kind: "pre"; text: string };

/// A host-composed markdown document (the certificate) as blocks: headings,
/// paragraphs, list items, table rows and fenced code. Nothing else.
export function blocks(text: string | null | undefined): Block[] {
  const out: Block[] = [];
  let fence: string[] | null = null;
  let para: string[] = [];
  const flush = () => {
    if (para.length) out.push({ kind: "p", segs: inline(para.join(" ")) });
    para = [];
  };
  for (const line of (text ?? "").split("\n")) {
    if (line.trimStart().startsWith("```")) {
      if (fence) {
        out.push({ kind: "pre", text: fence.join("\n") });
        fence = null;
      } else {
        flush();
        fence = [];
      }
      continue;
    }
    if (fence) {
      fence.push(line);
      continue;
    }
    const t = line.trim();
    if (!t) flush();
    else if (/^#{1,6}\s/.test(t)) {
      flush();
      out.push({ kind: "h", segs: inline(t.replace(/^#+\s*/, "")) });
    } else if (/^[-*]\s/.test(t)) {
      flush();
      out.push({ kind: "li", segs: inline(t.slice(2)) });
    } else if (t.startsWith("|")) {
      flush();
      if (!/^\|[\s|:-]+\|$/.test(t)) {
        const cells = t.replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
        out.push({ kind: "row", segs: inline(cells.join("  ·  ")) });
      }
    } else para.push(t);
  }
  if (fence) out.push({ kind: "pre", text: fence.join("\n") });
  flush();
  return out;
}
