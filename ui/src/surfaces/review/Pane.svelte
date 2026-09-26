<script lang="ts">
  // Deciding whether to merge one change: its files, the diff, and what covers
  // each file. The order is the project's (roles from `devplane.toml`, or
  // *unordered*), or by the run that wrote them. Marks stay in this browser;
  // *request a fix* is a message to the agent, not a verdict.
  import { api } from "../../lib/api";
  import { resource, failure } from "../../lib/resource.svelte";
  import { writer } from "../../lib/write.svelte";
  import Failed from "../../lib/Failed.svelte";
  import { onAction } from "../../lib/keys";
  import Icon from "../../lib/ui/Icon.svelte";
  import Split from "../../lib/ui/Split.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import Files from "./Files.svelte";
  import Diff from "./Diff.svelte";
  import { markKey, readMark, writeMark, prune, type Mark } from "./marks";
  import Inline from "../../lib/ui/Inline.svelte";
  import type { ReviewBody, File, Hunk, WeakRow } from "./types";

  let {
    id,
    review: planted = null,
    /// A path to open on, e.g. from an offer's refusal naming it.
    focus = "",
  }: { id: string; review?: ReviewBody | null; focus?: string } = $props();

  /// Keyed on the id's value: a re-rendered parent does not re-read the
  /// worktree or flash the diff away.
  const key = $derived(id);
  const read = resource<ReviewBody>(() => (key ? `/api/changes/${encodeURIComponent(key)}/review` : null), {
    tell: () => `devplane change review ${key}`,
  });
  const r = $derived(read.data ?? planted);

  // ── how it is read ────────────────────────────────────────────────────────
  function pref<T extends string>(key: string, dflt: T): T {
    try {
      return (localStorage.getItem(key) as T) || dflt;
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
  const byPath = $derived(new Map(files.map((f) => [f.path, f])));
  const sections = $derived.by(() => {
    if (!r) return [];
    if (by === "risk")
      return r.groups.map((g) => ({ says: g.says, files: g.files, tone: g.weakened ? ("fail" as const) : undefined }));
    const out = r.intent.groups.map((g) => ({
      says: `${g.title}`,
      files: g.files.map((p) => byPath.get(p)).filter((f): f is File => !!f),
    }));
    if (r.intent.not_asked_for.length)
      out.push({
        says: r.intent.not_asked_for_heading,
        files: r.intent.not_asked_for.map((n) => byPath.get(n.path)).filter((f): f is File => !!f),
        tone: "wait",
      } as never);
    return out;
  });
  const order = $derived(sections.flatMap((s) => s.files.map((f) => f.path)));

  /// The file picked, else the first in reading order.
  let picked = $state("");
  const selected = $derived(order.includes(picked) ? picked : (order[0] ?? ""));
  /// The weakened rows per file, when it has any. Seen marks on these are
  /// durable, in the host, because an offer waits for every one.
  const weakRows = $derived.by(() => {
    const m = new Map<string, WeakRow[]>();
    for (const w of (r?.groups ?? []).flatMap((g) => g.weakened ?? [])) m.set(w.path, [...(m.get(w.path) ?? []), w]);
    return m;
  });
  $effect(() => {
    if (focus) picked = focus;
  });
  let seenSaid = $state("");
  /// Records that a person read these rows; the host refuses a row that is
  /// not there, and the review is re-read so the marks show what it holds.
  async function markRead(rows: WeakRow[]) {
    const todo = rows.filter((w) => !w.seen);
    if (!todo.length || !id) return;
    try {
      for (const w of todo)
        await api(`/api/changes/${encodeURIComponent(id)}/review/seen`, {
          method: "POST",
          body: JSON.stringify({ path: w.path, matched: w.matched }),
        });
      seenSaid = `Marked read, as yours: ${todo.length === 1 ? todo[0].path : `${todo.length} rows`}.`;
      await read.reload();
    } catch (e) {
      seenSaid = `That did not land: ${failure(e).says}`;
    }
  }
  const file = $derived(byPath.get(selected) ?? null);
  let current = $state(0);
  $effect(() => {
    void selected;
    current = 0;
  });
  let expanded = $state(new Set<string>());

  // ── marks, per hunk, in this browser ─────────────────────────────────────
  // Keyed by content (`marks.ts`): a rewritten hunk is unseen again.
  const keyOf = (f: File, h: Hunk) => markKey(r?.change ?? "", f.path, h.header, h.lines);
  let marks = $state<Record<string, Mark>>({});
  $effect(() => {
    const next: Record<string, Mark> = {};
    const live = new Set<string>();
    for (const f of files)
      for (const h of f.hunks) {
        const k = keyOf(f, h);
        live.add(k);
        const m = readMark(k);
        if (m) next[k] = m;
      }
    // Only over a whole review: a truncated one does not name every hunk.
    if (r?.change && files.length && !r.truncated_says) prune(r.change, live);
    marks = next;
  });
  const markOf = (f: File) => (h: Hunk) => marks[keyOf(f, h)] ?? null;
  /// Whether a weakened row's matched line is an added or removed line of
  /// this hunk — the only way marking a hunk can be reading that row.
  const inHunk = (h: Hunk, w: WeakRow) =>
    w.kind === "skip" && h.lines.some(([k, t]) => k !== "context" && t.trim() === w.matched);
  const marked = (f: File) => f.hunks.filter((h) => markOf(f)(h)).length;
  const totalHunks = $derived(files.reduce((n, f) => n + f.hunks.length, 0));
  const totalMarked = $derived(Object.keys(marks).length);
  function setMark(m: Mark) {
    const h = file?.hunks[current];
    if (!file || !h) return;
    const k = keyOf(file, h);
    marks = { ...marks, [k]: m };
    writeMark(k, m);
    // A hunk marked here reads only the weakened rows whose matched line is
    // in that hunk; a deleted file or a changed definition is read with
    // *I have read this*, never by marking some other hunk of its file.
    void markRead((weakRows.get(file.path) ?? []).filter((w) => inHunk(h, w)));
    move(1);
  }

  // ── moving ───────────────────────────────────────────────────────────────
  function reveal() {
    queueMicrotask(() => document.getElementById(`hunk-${current}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  }
  function move(step: 1 | -1) {
    if (!file) return;
    const next = current + step;
    if (next >= 0 && next < file.hunks.length) {
      current = next;
      reveal();
      return;
    }
    const at = order.indexOf(selected) + step;
    if (at >= 0 && at < order.length) {
      picked = order[at];
      queueMicrotask(() => {
        current = step === 1 ? 0 : Math.max(0, (byPath.get(order[at])?.hunks.length ?? 1) - 1);
        reveal();
      });
    }
  }
  function moveFile(step: 1 | -1) {
    const at = order.indexOf(selected) + step;
    if (at >= 0 && at < order.length) picked = order[at];
  }

  // ── a fix, as a message to the agent ─────────────────────────────────────
  let fixing = $state(false);
  let draft = $state("");
  let said = $state("");
  const sending = writer();
  function requestFix() {
    const h = file?.hunks[current];
    if (!file || !h) return;
    draft = `In \`${file.path}\` ${h.header}:\n`;
    fixing = true;
  }
  async function sendFix() {
    const run = r?.latest_run;
    if (!run || !draft.trim()) return;
    await sending.run("Sending to the agent", async () => {
      try {
        const res = await api<{ says?: string }>(`/api/runs/${encodeURIComponent(run)}/prompt`, {
          method: "POST",
          body: JSON.stringify({ text: draft.trim() }),
        });
        said = res.says ?? "Sent to the agent.";
        fixing = false;
        await read.reload();
      } catch (e) {
        said = `That did not land: ${failure(e).says}`;
      }
    });
  }
  function fixKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      fixing = false;
    } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void sendFix();
    }
  }

  $effect(() => {
    const offs = [
      onAction("hunk-next", () => (move(1), true)),
      onAction("hunk-prev", () => (move(-1), true)),
      onAction("file-next", () => (moveFile(1), true)),
      onAction("file-prev", () => (moveFile(-1), true)),
      onAction("hunk-seen", () => (setMark("seen"), true)),
      onAction("hunk-fix", () => (requestFix(), true)),
      onAction("hunk-expand", () => (file && (expanded = new Set([...expanded, file.path])), true)),
      onAction("tab-risk", () => ((by = "risk"), true)),
      onAction("tab-intent", () => ((by = "intent"), true)),
      onAction("diff-mode", () => ((mode = mode === "unified" ? "split" : "unified"), true)),
    ];
    return () => offs.forEach((off) => off());
  });
</script>

{#if read.failure && !r}
  <Failed what="the review" failure={read.failure} />
{:else if !r}
  <div class="loading">Reading the worktree against its base…</div>
{:else if files.length === 0}
  <Empty icon="split" title="Nothing to review yet" body={r.empty_says ?? "The worktree has no changes against its base."} />
{:else}
  <div class="review">
    <header class="bar">
      <span class="base">against <code>{r.base}</code></span>
      <span class="shape">{r.shape_says}</span>
      <span class="gap"></span>
      <span class="progress" title="hunks you have marked in this browser">{totalMarked} of {totalHunks} hunks marked</span>
      <div class="seg" role="group" aria-label="order">
        <button class:on={by === "risk"} onclick={() => (by = "risk")}>By risk</button>
        <button class:on={by === "intent"} onclick={() => (by = "intent")}>By intent</button>
      </div>
      <div class="seg" role="group" aria-label="diff layout">
        <button class:on={mode === "unified"} onclick={() => (mode = "unified")} title="unified"><Icon name="unified" size={14} /></button>
        <button class:on={mode === "split"} onclick={() => (mode = "split")} title="side by side"><Icon name="split" size={14} /></button>
      </div>
    </header>
    {#if read.phase === "stale" && read.failure}<div class="stale"><Failed what="the review" failure={read.failure} at={read.at} stale /></div>{/if}
    {#each [r.unordered, r.coverage_absent, r.truncated_says, by === "intent" ? r.intent.unavailable : null] as s, i (i)}
      {#if s}<p class="finding"><Icon name="alert" size={13} /> {s}</p>{/if}
    {/each}

    <div class="body">
      <Split id="review-files" size={300} min={200} max={520}>
        {#snippet pane()}
          <div class="tree"><Files {sections} {selected} {marked} pick={(p) => (picked = p)} /></div>
        {/snippet}
        <div class="diff">
          {#if file}
            <header class="fh">
              <div class="path"><code>{file.path}</code> <span class="st">{file.status_says}</span> <span class="add">+{file.added}</span> <span class="del">−{file.removed}</span></div>
              <dl class="why">
                {#if weakRows.get(file.path)}
                  <dt class="weak">Check weakened</dt>
                  <dd class="weak">
                    {#each weakRows.get(file.path) ?? [] as w (w.why + w.matched)}
                      <span class="wrow"><Inline text={w.why} /> <code class="matched">{w.matched}</code> <span class="seen">{w.seen ? "marked read" : "not yet read"}</span></span>
                    {/each}
                    {#if (weakRows.get(file.path) ?? []).some((w) => !w.seen)}
                      <button class="read" onclick={() => markRead(weakRows.get(file!.path) ?? [])}><Icon name="eye" size={13} /> I have read this</button>
                    {/if}
                  </dd>
                {/if}
                <dt>Role</dt><dd>{file.role_says}</dd>
                <dt>Covered by</dt><dd class:quiet={file.coverage?.coverage !== "covered"}>{file.coverage_says ?? "no mapping declared"}</dd>
                <dt>Asked for</dt><dd class:wait={file.task_says.startsWith("not asked")}>{file.task_says}</dd>
                {#if file.decisions.length}<dt>Decided while writing</dt><dd>{file.decisions.map((x) => x.says).join(" · ")}</dd>{/if}
                {#if file.marker_commands.length}<dt>Check it yourself</dt><dd>{#each file.marker_commands as c (c)}<code class="cmd">{c}</code>{/each}</dd>{/if}
              </dl>
              <div class="acts">
                <button onclick={() => setMark("seen")} title="marks this hunk in this browser; a weakened line inside it is recorded as read"><Icon name="eye" size={13} /> Mark <kbd>s</kbd></button>
                <button onclick={requestFix} disabled={!r.latest_run}><Icon name="agent" size={13} /> Request a fix <kbd>f</kbd></button>
                <span class="hint"><kbd>j</kbd>/<kbd>k</kbd> hunks · <kbd>n</kbd>/<kbd>p</kbd> files</span>
              </div>
              {#if fixing}
                <form class="fix" onsubmit={(e) => { e.preventDefault(); void sendFix(); }}>
                  <!-- svelte-ignore a11y_autofocus -->
                  <textarea bind:value={draft} rows="3" aria-label="a message to the latest run" onkeydown={fixKey} autofocus></textarea>
                  <div>
                    <button type="submit" class="primary" disabled={!!sending.busy}>{sending.busy ? `${sending.busy}… ${sending.elapsed}` : "Send to the agent"}</button>
                    <button type="button" onclick={() => (fixing = false)}>Cancel <kbd>Esc</kbd></button>
                  </div>
                </form>
              {/if}
              {#if said}<p class="said" role="status">{said}</p>{/if}
              {#if seenSaid}<p class="said" role="status">{seenSaid}</p>{/if}
            </header>
            {#if file.body_says}<p class="finding">{file.body_says}</p>{/if}
            <Diff
              hunks={file.hunks}
              {mode}
              {current}
              expanded={expanded.has(file.path)}
              mark={markOf(file)}
              onexpand={() => (expanded = new Set([...expanded, file!.path]))}
              onpick={(i) => (current = i)}
            />
          {/if}
        </div>
      </Split>
    </div>
  </div>
{/if}

<style>
  .review {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    flex-wrap: wrap;
    font-size: var(--t-sm);
    color: var(--dim);
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
  .stale {
    padding: 0 var(--s-3);
  }
  .finding {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--wait);
    border-radius: var(--radius);
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .body {
    display: flex;
    height: calc(100vh - 16rem);
    min-height: 26rem;
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
  .diff {
    flex: 1;
    overflow: auto;
    padding: var(--s-3) var(--s-4);
    min-width: 0;
  }
  .fh {
    display: grid;
    gap: var(--s-2);
    margin-bottom: var(--s-3);
  }
  .path {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .path code {
    font-size: var(--t-sm);
    color: var(--ink);
    font-weight: 600;
  }
  .st {
    color: var(--faint);
    font-size: var(--t-xs);
  }
  .add {
    color: var(--add);
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .del {
    color: var(--del);
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
  .why {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 0.2rem var(--s-4);
    margin: 0;
    font-size: var(--t-xs);
  }
  .why dt {
    color: var(--faint);
  }
  .why dd {
    margin: 0;
    color: var(--ink);
  }
  .cmd {
    display: inline-block;
    margin-right: var(--s-2);
    padding: 0 0.3rem;
    border-radius: 4px;
    background: var(--panel);
  }
  .quiet {
    color: var(--faint) !important;
  }
  .weak {
    color: var(--fail) !important;
    font-weight: 600;
  }
  .wrow {
    display: block;
  }
  .matched {
    font-weight: 400;
    color: var(--ink);
  }
  .seen {
    font-weight: 400;
    color: var(--dim);
    font-size: var(--t-xs);
  }
  .read {
    margin-top: var(--s-1);
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.6rem;
    padding: 0 0.5rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-xs);
    cursor: pointer;
  }
  .wait {
    color: var(--wait) !important;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .acts button {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    height: 1.7rem;
    font-size: var(--t-xs);
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
    color: var(--faint);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 0.25rem;
  }
  .hint {
    margin-left: auto;
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .fix {
    display: grid;
    gap: var(--s-2);
  }
  .fix textarea {
    width: 100%;
    background: var(--bg);
    color: var(--ink);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: var(--s-2);
    font: inherit;
    font-size: var(--t-sm);
  }
  .fix div {
    display: flex;
    gap: var(--s-2);
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
  }
  .said {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--dim);
  }
  .loading {
    color: var(--faint);
    padding: var(--s-5);
  }
</style>
