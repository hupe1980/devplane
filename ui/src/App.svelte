<script lang="ts">
  // The workbench frame. It knows the registry, never a surface by name.
  //
  // The address is the source of truth: `#<surface>` or `#<surface>/<focus>`.
  // Tabs are a memory laid over it. Deep links arrive as a query (`?surface=`,
  // `?change=`, `?ask=`, `?run=`), read once and folded into the hash.
  import { surfaces, listed, landing, type Feed } from "./lib/surfaces";
  import { loadSurfaces } from "./lib/load";
  import { live } from "./lib/live.svelte";
  import { theme } from "./lib/theme.svelte";
  import { claimToken } from "./lib/api";
  import { untrack } from "svelte";
  import { dispatch, help, onAction, run, spell } from "./lib/keys";
  import { same } from "./lib/resource.svelte";
  import { go } from "./lib/route";
  import Stale from "./lib/Stale.svelte";
  import { trap } from "./lib/trap";
  import Split from "./lib/ui/Split.svelte";
  import Icon from "./lib/ui/Icon.svelte";
  import TitleBar from "./shell/TitleBar.svelte";
  import ActivityBar from "./shell/ActivityBar.svelte";
  import EditorTabs from "./shell/EditorTabs.svelte";
  import StatusBar from "./shell/StatusBar.svelte";
  import Panel from "./shell/Panel.svelte";
  import { tabs, show, close, pin, hashOf, type Tab } from "./shell/tabs.svelte";
  import "./shell/keys";

  loadSurfaces();

  let current = $state(landing()?.id ?? "");
  let focus = $state("");
  let notice = $state("");
  let helpOpen = $state(false);
  const showing = $derived(surfaces().find((s) => s.id === current));

  /// Whether a person is looking at the surface that marks a look.
  const looking = () => showing?.marksLook === true && document.visibilityState === "visible";
  const { state: feed, start, look } = live(looking);
  // One poll for the life of the page: started untracked, so moving between
  // surfaces (which `look` reads) never tears it down and starts another.
  $effect(() => untrack(start));
  $effect(() => {
    if (showing?.marksLook) untrack(look);
  });

  const { state: themeState, restore, cycle } = theme();
  $effect(restore);

  // ── the regions a person arranges, remembered per browser ────────────────
  function flag(key: string, dflt: boolean): boolean {
    try {
      const v = localStorage.getItem(key);
      return v === null ? dflt : v === "1";
    } catch {
      return dflt;
    }
  }
  function keepFlag(key: string, v: boolean) {
    try {
      localStorage.setItem(key, v ? "1" : "0");
    } catch {
      /* lasts the tab */
    }
  }
  let sideOpen = $state(flag("vp-side", true));
  let panelOpen = $state(flag("vp-panel", false));
  $effect(() => keepFlag("vp-side", sideOpen));
  $effect(() => keepFlag("vp-panel", panelOpen));

  // ── the address ───────────────────────────────────────────────────────────
  function read() {
    const raw = location.hash.slice(1);
    const cut = raw.indexOf("/");
    const id = cut === -1 ? raw : raw.slice(0, cut);
    if (!surfaces().some((s) => s.id === id)) {
      if (!raw) {
        const home = landing()?.id;
        if (home) go(`#${home}`);
      }
      return;
    }
    current = id;
    focus = cut === -1 ? "" : decodeURIComponent(raw.slice(cut + 1));
    helpOpen = false;
    const s = surfaces().find((x) => x.id === id);
    // The palette and the answer window never become tabs.
    if (s && !s.transient && !s.bare) show(id, focus);
  }

  function readQuery() {
    claimToken();
    const url = new URL(location.href);
    const q = url.searchParams;
    if ([...q.keys()].length === 0) return;
    let id = "";
    let f = "";
    const want = q.get("surface");
    if (want && surfaces().some((s) => s.id === want)) id = want;
    for (const s of surfaces()) {
      const v = s.link ? q.get(s.link) : null;
      if (s.link && v) {
        id = s.id;
        f = s.linkFocus ? s.linkFocus(v) : v;
      }
    }
    const link = q.get("link");
    if (link) {
      notice = `${link} names nothing this app opens; links are change, review, ask, run, inbox`;
      id = landing()?.id ?? "";
      f = "";
    }
    const hash = id ? `#${id}${f ? `/${encodeURIComponent(f)}` : ""}` : url.hash;
    history.replaceState({}, "", `${url.pathname}${hash}`);
  }

  $effect(() => {
    readQuery();
    read();
    addEventListener("hashchange", read);
    return () => removeEventListener("hashchange", read);
  });

  /// A surface picked from the activity bar: its most recent tab, or itself.
  function pick(id: string) {
    const last = [...tabs.list].reverse().find((t) => t.surface === id);
    if (id === current && sideOpen && surfaces().find((s) => s.id === id)?.side) {
      sideOpen = false;
      return;
    }
    sideOpen = true;
    go(last ? hashOf(last) : `#${id}`);
  }
  function pickTab(i: number) {
    const t = tabs.list[i];
    if (t) go(hashOf(t));
  }
  function closeTab(i: number) {
    const next = close(i);
    go(next ? hashOf(next) : `#${landing()?.id ?? ""}`);
  }

  // ── keys ─────────────────────────────────────────────────────────────────
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      dispatch(current, e);
    };
    addEventListener("keydown", onKey);
    const offs = [
      onAction("help", () => {
        helpOpen = !helpOpen;
        return true;
      }),
      onAction("toggle-side", () => {
        sideOpen = !sideOpen;
        return true;
      }),
      onAction("toggle-panel", () => {
        panelOpen = !panelOpen;
        return true;
      }),
      onAction("close-tab", () => {
        if (tabs.active !== -1) closeTab(tabs.active);
        return true;
      }),
      onAction("next-tab", () => {
        if (tabs.list.length) pickTab((tabs.active + 1) % tabs.list.length);
        return true;
      }),
      onAction("prev-tab", () => {
        if (tabs.list.length) pickTab((tabs.active - 1 + tabs.list.length) % tabs.list.length);
        return true;
      }),
      onAction("leave", () => {
        if (helpOpen) {
          helpOpen = false;
          return true;
        }
        if (notice) {
          notice = "";
          return true;
        }
        return false;
      }),
    ];
    return () => {
      removeEventListener("keydown", onKey);
      offs.forEach((off) => off());
    };
  });

  // ── what the frame shows ─────────────────────────────────────────────────
  const tabTitle = (t: Tab) => {
    const s = surfaces().find((x) => x.id === t.surface);
    if (!s) return t.surface;
    return (t.focus && s.tab?.(feed as Feed, t.focus)) || (t.focus ? `${s.title}: ${t.focus}` : s.title);
  };
  const tabIcon = (t: Tab) => surfaces().find((x) => x.id === t.surface)?.icon;
  const crumbs = $derived(
    showing ? (focus && tabs.list[tabs.active] ? [showing.title, tabTitle(tabs.list[tabs.active])] : [showing.title]) : [],
  );
  const pulse = $derived(
    feed.unauthorised ? "no token" : feed.error ? "not answering" : feed.loaded ? "live" : "connecting",
  );
  const projects = $derived((feed.board as { summary?: { projects?: number } } | null)?.summary?.projects ?? null);
  /// Each surface's own status-bar line, so the bar names none of them.
  const status = $derived(
    surfaces().flatMap((s) => (s.status?.(feed as Feed) ?? []).map((i) => ({ ...i, surface: s.id, title: s.title }))),
  );
  /// The surface a run opens on, for the activity panel's rows.
  const runsAt = $derived(surfaces().find((s) => s.holds === "run")?.id ?? "");
  const keys = $derived(help(current));
  /// Under a transient surface, the page is the active tab's.
  const under = $derived.by(() => {
    if (!showing?.transient) return { s: showing, f: focus };
    const t = tabs.list[tabs.active];
    return { s: (t ? surfaces().find((x) => x.id === t.surface) : undefined) ?? landing(), f: t?.focus ?? "" };
  });
  const page = $derived(under.s);
  const Side = $derived(page?.side);
  /// A surface's props, kept by identity while they say the same thing. A
  /// spread prop is read through the whole object, so a fresh object every
  /// poll would re-run every effect under it — re-fetching, and resetting the
  /// tab a person picked. Each value that did not change keeps its old
  /// object, and when none changed the old props object is handed back.
  function settler() {
    let last: Record<string, unknown> = {};
    return (next: Record<string, unknown>): Record<string, unknown> => {
      const keys = Object.keys(next);
      let changed = keys.length !== Object.keys(last).length;
      const out: Record<string, unknown> = {};
      for (const k of keys) {
        if (k in last && same(last[k], next[k])) out[k] = last[k];
        else {
          out[k] = next[k];
          changed = true;
        }
      }
      if (changed) last = out;
      return last;
    };
  }
  const settlePage = settler();
  const settleOverlay = settler();
  const props = $derived(settlePage(page ? page.select(feed as Feed, under.f) : {}));
  const overlayProps = $derived(settleOverlay(showing?.transient ? showing.select(feed as Feed, focus) : {}));
  function openFocus(f: string, pinIt = false) {
    if (!page) return;
    go(`#${page.id}${f ? `/${encodeURIComponent(f)}` : ""}`);
    if (pinIt) pin();
  }
</script>

{#if showing?.bare}
  <main id="surface" class="bare">
    {#if feed.unauthorised}<p class="problem" role="alert">{feed.error}</p>{/if}
    <showing.component {...props} />
  </main>
{:else}
  <!-- A button, not `href="#surface"`: the address is the route, and a
       fragment link would overwrite it. -->
  <button class="skip" onclick={() => document.getElementById("surface")?.focus()}>Skip to content</button>
  <div class="wb">
    <TitleBar
      {crumbs}
      {sideOpen}
      {panelOpen}
      find={() => run("open-palette", current)}
      create={() => run("new-change", current)}
      toggleSide={() => (sideOpen = !sideOpen)}
      togglePanel={() => (panelOpen = !panelOpen)}
    />
    <div class="mid">
      <ActivityBar items={listed()} feed={feed as Feed} {current} {pick} />
      <Split id="side" size={300} min={200} max={560} collapsed={!sideOpen || !Side}>
        {#snippet pane()}
          <aside class="side" aria-label="{showing?.title ?? ''} list">
            {#if Side}<Side {...props} {focus} open={openFocus} />{/if}
          </aside>
        {/snippet}
        <Split id="panel" axis="y" side="end" size={220} min={120} max={560} collapsed={!panelOpen}>
          {#snippet pane()}
            <Panel board={feed.board as never} error={feed.error} open={runsAt ? (id) => go(`#${runsAt}/${encodeURIComponent(id)}`) : null} />
          {/snippet}
          {#if tabs.list.length > 0}
            <EditorTabs
              list={tabs.list}
              active={tabs.active}
              title={tabTitle}
              icon={tabIcon}
              pick={pickTab}
              close={closeTab}
              pin={(i) => {
                pickTab(i);
                pin();
              }}
            />
          {/if}
          <main id="surface" class="editor" tabindex="-1" aria-label={page?.heading}>
            {#if feed.unauthorised}
              <p class="problem" role="alert">{feed.error}</p>
            {:else if feed.error}
              <Stale error={feed.error} stale_since={feed.stale_since} />
            {/if}
            {#if notice}<p class="notice" role="status">{notice}</p>{/if}
            {#if page}
              <page.component {...props} focus={under.f} open={openFocus} />
            {:else}
              <p>No surface is registered.</p>
            {/if}
          </main>
        </Split>
      </Split>
    </div>
    <StatusBar
      items={status}
      {projects}
      {pulse}
      bad={!!feed.error || feed.unauthorised}
      theme={themeState.choice === "system" ? "auto" : themeState.choice}
      cycleTheme={cycle}
      help={() => (helpOpen = !helpOpen)}
      go={(h) => go(h)}
    />
  </div>

  {#if showing?.transient}
    <div class="scrim" role="presentation" onclick={() => run("leave", current)}></div>
    <div class="overlay" role="dialog" aria-modal="true" aria-label={showing.title} use:trap>
      <showing.component {...overlayProps} />
    </div>
  {/if}

  {#if helpOpen}
    <!-- Generated from the bindings: every row is a binding with its label. -->
    <div class="scrim" role="presentation" onclick={() => (helpOpen = false)}></div>
    <div class="help" role="dialog" aria-modal="true" aria-label="keys bound here" use:trap>
      <header><Icon name="keyboard" size={16} /> <h2>Keys bound here</h2></header>
      <dl>
        {#each keys as k (k.surface + k.combo)}
          <dt><kbd>{spell(k.combo)}</kbd></dt>
          <dd>{k.label}{#if k.surface === "global"}<span class="dim"> · everywhere</span>{/if}</dd>
        {/each}
      </dl>
    </div>
  {/if}
{/if}

<style>
  :global(html),
  :global(body) {
    height: 100%;
    overflow: hidden;
  }
  .skip {
    color: var(--ink);
    font: inherit;
    cursor: pointer;
    position: absolute;
    left: -9999px;
    top: var(--s-2);
    padding: var(--s-2) var(--s-3);
    background: var(--panel);
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    z-index: 30;
  }
  .skip:focus {
    left: var(--s-4);
  }
  .wb {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .mid {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    background: var(--side);
    overflow: hidden;
  }
  /* A surface reflows to its pane's width, never the viewport's. */
  .editor {
    container-type: inline-size;
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--bg);
    outline: none;
  }
  main.bare {
    padding: var(--s-2);
  }
  .problem,
  .notice {
    margin: var(--s-3) var(--s-5) 0;
    padding: var(--s-2) var(--s-3);
    border-radius: var(--radius);
    border: 1px solid var(--line);
    font-size: var(--t-sm);
  }
  .problem {
    color: var(--fail);
  }
  .notice {
    color: var(--dim);
  }
  .scrim {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 20;
  }
  .overlay {
    position: fixed;
    top: 9vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(44rem, calc(100vw - 2rem));
    max-height: 72vh;
    overflow: auto;
    z-index: 21;
    background: var(--panel);
    border: 1px solid var(--edge);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgb(0 0 0 / 0.45);
  }
  .help {
    position: fixed;
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(40rem, calc(100vw - 2rem));
    max-height: 70vh;
    overflow: auto;
    z-index: 21;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: 0 18px 48px rgb(0 0 0 / 0.35);
    padding: var(--s-4) var(--s-5);
  }
  .help header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin-bottom: var(--s-3);
  }
  .help h2 {
    font-size: var(--t-md);
    margin: 0;
  }
  .help dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--s-2) var(--s-4);
    margin: 0;
    font-size: var(--t-sm);
  }
  .help dd {
    margin: 0;
  }
  .dim {
    color: var(--faint);
  }
  kbd {
    font-family: var(--mono);
    font-size: var(--t-xs);
    border: 1px solid var(--line);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 0 0.35rem;
    background: var(--bg);
  }
</style>
