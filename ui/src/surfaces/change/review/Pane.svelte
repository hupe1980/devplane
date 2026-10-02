<script lang="ts">
  // Deciding whether to merge one change: every hunk of every file in one
  // column, in the review's order — weakened checks first, then the declared
  // roles — each file's facts in a strip that stays in view while its hunks
  // scroll. `j`/`k` move hunk to hunk across files, `n`/`p` file to file;
  // scrolling marks nothing. A hunk is *seen* only by `s`, kept by the host.
  // Comments collect into one pending review, sent as one turn.
  import { untrack } from "svelte";
  import { api, Refused } from "../../../lib/api";
  import { resource, failure, type Resource } from "../../../lib/resource.svelte";
  import Failed from "../../../lib/Failed.svelte";
  import { onAction } from "../../../lib/keys";
  import Icon from "../../../lib/ui/Icon.svelte";
  import Split from "../../../lib/ui/Split.svelte";
  import Empty from "../../../lib/ui/Empty.svelte";
  import Skeleton from "../../../lib/Skeleton.svelte";
  import Inline from "../../../lib/ui/Inline.svelte";
  import { shape } from "../../../lib/text";
  import Files from "./Files.svelte";
  import FileSection from "./FileSection.svelte";
  import Comments from "./Comments.svelte";
  import Composer from "./Composer.svelte";
  import { markSeen, unread, unreadIn, unreadSays } from "./unread.svelte";
  import type { Comment, File, ReviewBody, WeakRow } from "./types";

  let {
    id,
    review: planted = null,
    /// Where to open: a path (an offer's refusal naming it) or `path@digest`
    /// (the address of a hunk).
    focus = "",
    /// The review already being read by the change around this pane, so its
    /// header and this pane count one review; without it the pane reads its own.
    source = null,
    /// The change around this pane says the unread count; the pane does not
    /// say it again.
    counted = false,
    /// Puts the hunk under the cursor in the address.
    onaddress = null,
  }: {
    id: string;
    review?: ReviewBody | null;
    focus?: string;
    source?: Resource<ReviewBody> | null;
    counted?: boolean;
    onaddress?: ((at: string) => void) | null;
  } = $props();

  const key = $derived(id);
  const own = resource<ReviewBody>(() => (key && !source ? `/api/changes/${encodeURIComponent(key)}/review` : null), {
    tell: () => `devplane change review ${key}`,
  });
  const read = $derived(source ?? own);
  const r = $derived(read.data ?? planted);
  const change = $derived(r?.change ?? key);
  const unreadNow = $derived(counted ? "" : unreadSays(unread(r)));

  // ── how it is read ────────────────────────────────────────────────────────
  function pref<T extends string>(k: string, dflt: T): T {
    try {
      return (localStorage.getItem(k) as T) || dflt;
    } catch {
      return dflt;
    }
  }
  let by = $state<"risk" | "intent">(pref("vp-review-by", "risk"));
  let mode = $state<"unified" | "split">(pref("vp-review-mode", "unified"));
  $effect(() => {
    try {
      localStorage.setItem("vp-review-by", by);
      localStorage.setItem("vp-review-mode", mode);
    } catch {
      /* lasts the tab */
    }
  });

  const files = $derived((r?.groups ?? []).flatMap((g) => g.files));
  const findings = $derived([r?.unordered, r?.coverage_absent, r?.truncated_says, by === "intent" ? r?.intent.unavailable : null].filter((f): f is string => !!f));
  const byPath = $derived(new Map(files.map((f) => [f.path, f])));
  type Section = { says: string; files: File[]; tone?: "wait" | "fail" };
  const sections = $derived.by((): Section[] => {
    if (!r) return [];
    if (by === "risk" || r.intent.unavailable)
      return r.groups.map((g) => ({ says: g.says, files: g.files, tone: g.weakened ? ("fail" as const) : undefined }));
    const out: Section[] = r.intent.groups.map((g) => ({
      says: g.title,
      files: g.files.map((p) => byPath.get(p)).filter((f): f is File => !!f),
    }));
    if (r.intent.not_asked_for.length)
      out.push({
        says: r.intent.not_asked_for_heading,
        files: r.intent.not_asked_for.map((n) => byPath.get(n.path)).filter((f): f is File => !!f),
        tone: "wait",
      });
    return out;
  });
  /// The column: each file once, in reading order, with its first hunk's
  /// number across the change; and every hunk, in that order.
  const placed = $derived.by(() => {
    const seen = new Set<string>();
    let n = 0;
    return sections.map((s) => ({
      ...s,
      files: s.files
        .filter((f) => !seen.has(f.path) && (seen.add(f.path), true))
        .map((f) => {
          const at = n;
          n += f.hunks.length;
          return { file: f, at };
        }),
    }));
  });
  const flat = $derived(placed.flatMap((s) => s.files.flatMap(({ file, at }) => file.hunks.map((h, i) => ({ file, h, n: at + i })))));
  const firstOf = $derived(new Map(placed.flatMap((s) => s.files.map(({ file, at }) => [file.path, at] as const))));
  const order = $derived(placed.flatMap((s) => s.files.map(({ file }) => file.path)));

  /// The weakened rows per file. A row is read when its hunks are seen.
  const weakRows = $derived.by(() => {
    const m = new Map<string, WeakRow[]>();
    for (const w of (r?.groups ?? []).flatMap((g) => g.weakened ?? [])) m.set(w.path, [...(m.get(w.path) ?? []), w]);
    return m;
  });
  const commentsBy = $derived.by(() => {
    const m = new Map<string, Comment[]>();
    for (const c of r?.comments ?? []) m.set(c.path, [...(m.get(c.path) ?? []), c]);
    return m;
  });

  // ── the cursor ───────────────────────────────────────────────────────────
  let cur = $state(0);
  const here = $derived(flat[Math.min(cur, Math.max(0, flat.length - 1))]);
  /// The file whose strip is at the top of the column, for the tree.
  let inView = $state("");
  const selected = $derived(inView || here?.file.path || order[0] || "");
  let expanded = $state(new Set<string>());
  let said = $state("");
  let col = $state<HTMLElement | null>(null);

  function reveal(n: number) {
    document.getElementById(`hunk-${n}`)?.scrollIntoView({ block: "nearest" });
  }
  let addressing: ReturnType<typeof setTimeout> | undefined;
  /// The address follows the cursor, once it rests: one replace, no history.
  function address() {
    clearTimeout(addressing);
    if (!onaddress) return;
    addressing = setTimeout(() => {
      const at = flat[cur];
      if (at) onaddress?.(`${at.file.path}@${at.h.digest ?? ""}`);
    }, 400);
  }
  function to(n: number) {
    if (n < 0 || n >= flat.length) return false;
    performance.mark("review:move");
    cur = n;
    inView = "";
    queueMicrotask(() => {
      reveal(n);
      try {
        performance.measure("review: hunk to hunk", "review:move");
      } catch {
        /* marks cleared */
      }
    });
    address();
    return true;
  }
  function moveFile(step: 1 | -1) {
    const at = order.indexOf(here?.file.path ?? "") + step;
    for (let i = at; i >= 0 && i < order.length; i += step) {
      const first = firstOf.get(order[i]);
      if (first !== undefined && byPath.get(order[i])?.hunks.length) return to(first);
    }
    return false;
  }

  // The tree moved under the reader: the cursor stays on the hunk it was on
  // (by content), the column says so once, and the scroll stays put.
  let onKey = "";
  let onPath = "";
  let lastFlat: typeof flat = [];
  $effect(() => {
    const now = flat;
    untrack(() => {
      const was = lastFlat;
      lastFlat = now;
      if (!was.length || was === now) return;
      const at = now.findIndex((x) => x.file.path === onPath && x.h.digest === onKey);
      if (onKey && at >= 0 && at !== cur) cur = at;
      const before = new Set(was.map((x) => `${x.file.path}@${x.h.digest}`));
      const moved = new Set(now.filter((x) => !before.has(`${x.file.path}@${x.h.digest}`)).map((x) => x.file.path));
      if (moved.size && was[0]?.h.digest) said = `This change moved — ${moved.size === 1 ? "1 file" : `${moved.size} files`} updated; what changed is unread again.`;
    });
  });
  $effect(() => {
    onKey = flat[cur]?.h.digest ?? "";
    onPath = flat[cur]?.file.path ?? "";
  });

  // Where to open: a hunk's address, a path, else the first hunk.
  let opened = "";
  $effect(() => {
    const want = focus;
    const ready = flat.length;
    if (!ready || untrack(() => opened) === `${change}|${want}`) return;
    untrack(() => {
      opened = `${change}|${want}`;
      if (!want) return;
      const [path, digest] = want.includes("@") ? [want.slice(0, want.lastIndexOf("@")), want.slice(want.lastIndexOf("@") + 1)] : [want, ""];
      const exact = flat.find((x) => x.file.path === path && digest && x.h.digest === digest);
      const first = firstOf.get(path);
      if (exact) to(exact.n);
      else if (first !== undefined && byPath.get(path)?.hunks.length) {
        to(first);
        if (digest) said = `The hunk the address names has changed since; showing \`${path}\` from its first hunk.`;
      } else if (path) {
        said = `\`${path}\` is not in this change now; showing the first hunk.`;
        to(0);
      }
    });
  });

  // ── seen ─────────────────────────────────────────────────────────────────
  let rereading: ReturnType<typeof setTimeout> | undefined;
  /// Reads the review again once marks stop arriving, for the weakened rows
  /// and the qualifier they complete.
  function reread() {
    clearTimeout(rereading);
    rereading = setTimeout(() => void read.reload(), 900);
  }
  async function markHere() {
    const x = flat[cur];
    if (!x) return;
    if (!x.h.digest) {
      said = "This recording carries no hunk digests, so nothing can be marked.";
      return;
    }
    to(cur + 1);
    try {
      await markSeen(change, x.file.path, x.h.digest);
      reread();
    } catch (e) {
      said = `That mark did not land: ${failure(e).says}`;
    }
  }
  async function readPath(path: string) {
    try {
      await markSeen(change, path, null);
      said = `Marked \`${path}\` seen, as yours.`;
      await read.reload();
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    }
  }

  // ── comments ─────────────────────────────────────────────────────────────
  let drafting = $state<{ path: string; side: "new" | "old"; line: number } | null>(null);
  let busy = $state("");
  // Another change: the cursor starts at its first hunk.
  $effect.pre(() => {
    void key;
    untrack(() => {
      cur = 0;
      inView = "";
      drafting = null;
      said = "";
    });
  });

  function startComment(path: string, side: "new" | "old", line: number) {
    drafting = { path, side, line };
  }
  /// `f`: a comment on the current hunk's first changed line.
  function commentHere() {
    const x = flat[cur];
    if (!x) return;
    const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(x.h.header);
    let o = m ? Number(m[1]) : 0;
    let n = m ? Number(m[2]) : 0;
    for (const [kind] of x.h.lines) {
      if (kind === "added") return startComment(x.file.path, "new", n);
      if (kind === "removed") return startComment(x.file.path, "old", o);
      o++;
      n++;
    }
  }
  async function saveComment(body: string) {
    const d = drafting;
    if (!d || !body.trim() || busy) return;
    busy = "Keeping the comment";
    try {
      await api(`/api/changes/${encodeURIComponent(change)}/comments`, {
        method: "POST",
        body: JSON.stringify({ path: d.path, line: d.line, side: d.side, body: body.trim() }),
      });
      drafting = null;
      said = "";
      await read.reload();
    } catch (e) {
      said = `The comment was not kept: ${failure(e).says}`;
    } finally {
      busy = "";
    }
  }
  async function sendReview() {
    if (busy || !(r?.comments ?? []).some((c) => !c.sent_at)) return false;
    busy = "Sending";
    try {
      const res = await api<{ says?: string }>(`/api/changes/${encodeURIComponent(change)}/comments/send`, { method: "POST" });
      said = res.says ?? "Sent.";
      await read.reload();
    } catch (e) {
      const body = e instanceof Refused ? (e.body as { says?: string } | null) : null;
      said = `Nothing was sent: ${body?.says ?? failure(e).says}. The comments are kept.`;
    } finally {
      busy = "";
    }
    return true;
  }
  async function removeComment(cid: string) {
    try {
      await api(`/api/changes/${encodeURIComponent(change)}/comments/${encodeURIComponent(cid)}`, { method: "DELETE" });
      await read.reload();
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    }
  }
  function jumpTo(c: Comment) {
    const x = flat.find((y) => y.file.path === c.path && y.h.digest === c.hunk_digest) ?? flat.find((y) => y.file.path === c.path);
    if (x) to(x.n);
  }

  // ── the file in view ─────────────────────────────────────────────────────
  let framing = 0;
  function scrolled() {
    if (framing || !col) return;
    framing = requestAnimationFrame(() => {
      framing = 0;
      if (!col) return;
      const top = col.getBoundingClientRect().top + 8;
      for (const s of col.querySelectorAll<HTMLElement>("section.file")) {
        if (s.getBoundingClientRect().bottom > top) {
          inView = s.dataset.path ?? "";
          return;
        }
      }
    });
  }

  $effect(() => {
    const offs = [
      onAction("hunk-next", () => to(cur + 1) || true),
      onAction("hunk-prev", () => to(cur - 1) || true),
      onAction("file-next", () => (moveFile(1), true)),
      onAction("file-prev", () => (moveFile(-1), true)),
      onAction("hunk-seen", () => (void markHere(), true)),
      onAction("hunk-fix", () => (commentHere(), true)),
      // ⌘Enter inside the composer adds the comment (the field takes it);
      // anywhere else in the review it sends the pending review.
      onAction("review-send", () => (drafting ? false : (void sendReview(), true))),
      onAction("hunk-expand", () => (here && (expanded = new Set([...expanded, here.file.path])), true)),
      onAction("tab-risk", () => ((by = "risk"), true)),
      onAction("tab-intent", () => ((by = "intent"), true)),
      onAction("diff-mode", () => ((mode = mode === "unified" ? "split" : "unified"), true)),
    ];
    return () => offs.forEach((off) => off());
  });
</script>

{#snippet composer()}
  <Composer busy={!!busy} save={(body) => void saveComment(body)} cancel={() => (drafting = null)} />
{/snippet}

{#if read.failure && !r}
  <Failed what="the review" failure={read.failure} />
{:else if !r}
  <Skeleton rows={5} />
{:else if files.length === 0}
  <Empty icon="split" title="Nothing to review yet" body={r.empty_says ?? "The worktree has no changes against its base."} />
{:else}
  <div class="review">
    <header class="bar">
      <span class="base">against <code>{r.base}</code></span>
      <span class="shape">{shape(r.shape_says)}</span>
      {#if unreadNow}<span class="progress">{unreadNow}</span>{/if}
      <span class="gap"></span>
      <Comments comments={r.comments ?? []} {busy} send={() => void sendReview()} jump={jumpTo} remove={removeComment} />
      <div class="seg" role="group" aria-label="order">
        <button class:on={by === "risk"} onclick={() => (by = "risk")}>By risk</button>
        <button class:on={by === "intent"} onclick={() => (by = "intent")}>By intent</button>
      </div>
      <div class="seg" role="group" aria-label="diff layout">
        <button class:on={mode === "unified"} onclick={() => (mode = "unified")} title="unified" aria-label="unified"><Icon name="unified" size={14} /></button>
        <button class:on={mode === "split"} onclick={() => (mode = "split")} title="side by side" aria-label="side by side"><Icon name="split" size={14} /></button>
      </div>
    </header>
    {#if said}<p class="said" role="status"><Inline text={said} /></p>{/if}
    {#if read.phase === "stale" && read.failure}<Failed what="the review" failure={read.failure} at={read.at} stale />{/if}
    {#if findings.length}
      <!-- What the order and the coverage cannot say, in one line. -->
      <p class="finding"><Icon name="alert" size={12} /> {#each findings as f, i (i)}{#if i}{" · "}{/if}<Inline text={f} />{/each}</p>
    {/if}

    <div class="body">
      <Split id="review-files" size={260} min={180} max={520}>
        {#snippet pane()}
          <div class="tree">
            <Files {sections} {selected} unreadOf={(f) => unreadIn(change, f)} pick={(p) => { const n = firstOf.get(p); if (n !== undefined && byPath.get(p)?.hunks.length) to(n); else document.querySelector(`section.file[data-path="${CSS.escape(p)}"]`)?.scrollIntoView({ block: "start" }); }} />
          </div>
        {/snippet}
        <div class="column" bind:this={col} onscroll={scrolled}>
          {#each placed as s (s.says)}
            {#if placed.length > 1 || s.tone}<h3 class="sec {s.tone ?? ''}"><Inline text={s.says} /> <span class="n">{s.files.length}</span></h3>{/if}
            {#each s.files as { file, at } (file.path)}
              <FileSection
                {change}
                {file}
                {at}
                current={cur >= at && cur < at + file.hunks.length ? cur : -1}
                {mode}
                expanded={expanded.has(file.path)}
                weak={weakRows.get(file.path) ?? []}
                comments={commentsBy.get(file.path) ?? []}
                drafting={drafting?.path === file.path ? drafting : null}
                {composer}
                pick={(n) => to(n)}
                expand={() => (expanded = new Set([...expanded, file.path]))}
                comment={(side, line) => startComment(file.path, side, line)}
                remove={removeComment}
                readRows={() => void readPath(file.path)}
              />
            {/each}
          {/each}
        </div>
      </Split>
    </div>
  </div>
{/if}

<style>
  .review {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--s-2);
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    min-height: 1.8rem;
    font-size: var(--t-sm);
    color: var(--dim);
    white-space: nowrap;
  }
  .shape {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gap {
    flex: 1;
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .progress {
    font-size: var(--t-xs);
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .seg {
    display: inline-flex;
    flex: none;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.7rem;
    padding: 0 0.6rem;
    border: 0;
    border-radius: 0;
    background: var(--panel);
    color: var(--dim);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    background: var(--select);
    color: var(--ink);
  }
  .finding {
    margin: 0;
    color: var(--wait);
    font-size: var(--t-xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .said {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .body {
    display: flex;
    flex: 1;
    height: calc(100vh - 13rem);
    min-height: 12rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .tree {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    background: var(--side);
  }
  .column {
    flex: 1;
    overflow: auto;
    padding: 0 var(--s-3) var(--s-6);
    min-width: 0;
    overscroll-behavior: contain;
  }
  .sec {
    margin: var(--s-3) 0 var(--s-1);
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--dim);
  }
  .sec.fail {
    color: var(--fail);
  }
  .sec.wait {
    color: var(--wait);
  }
  .sec .n {
    color: var(--faint);
    font-weight: 400;
  }
</style>
