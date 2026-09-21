<script lang="ts">
  // The shell. **It knows the registry and not the surfaces** — no import here
  // names one, which is the property the rebuild exists for: the page it
  // replaces was one 2,700-line file and every feature landed in the middle of
  // it.
  //
  // Its whole job is the frame: which surface is showing, whether the daemon is
  // answering, and the theme. Everything else belongs to a surface.
  import { surfaces, landing, BANDS } from "./lib/surfaces";
  import { loadSurfaces } from "./lib/load";
  import { live } from "./lib/live.svelte";
  import { theme } from "./lib/theme.svelte";

  loadSurfaces();

  const { state: feed, start } = live();
  $effect(start);

  const { state: themeState, restore, cycle } = theme();
  $effect(restore);

  let current = $state(landing()?.id ?? "");
  /// What the current surface was opened *about*. Opaque here, on purpose.
  let focus = $state("");
  const showing = $derived(surfaces().find((s) => s.id === current));

  // **The address bar is the record of where you are.** Without it, reloading a
  // page somebody has open on the work view drops them back on the board, and a
  // link to a surface cannot be sent to anybody — including to yourself, on the
  // phone. The hash rather than a path because this is one static document
  // served from a binary, with no server-side router to teach about routes.
  /// `#<surface>` or `#<surface>/<focus>`.
  ///
  /// The focus is everything after the first slash, undecoded beyond the URL's
  /// own encoding — ids here contain no slash, and a surface that wants one
  /// can encode it.
  function read() {
    const raw = location.hash.slice(1);
    const cut = raw.indexOf("/");
    const id = cut === -1 ? raw : raw.slice(0, cut);
    if (!surfaces().some((s) => s.id === id)) return;
    current = id;
    focus = cut === -1 ? "" : decodeURIComponent(raw.slice(cut + 1));
  }

  $effect(() => {
    read();
    addEventListener("hashchange", read);
    return () => removeEventListener("hashchange", read);
  });

  function go(id: string) {
    current = id;
    focus = "";
    // `replaceState` rather than assigning `location.hash`: assigning pushes a
    // history entry per click, so Back walks through every surface somebody
    // glanced at instead of leaving the page.
    history.replaceState({}, "", `#${id}`);
  }

  /// Keeps the current tab on screen.
  ///
  /// **The nav scrolls sideways on a phone**, and the surface you are on is
  /// routinely past the right edge — so the one thing the nav exists to tell
  /// you, *where am I*, is the one thing it cannot. Arriving by link or by
  /// reload lands you there with no way to know it short of scrolling.
  ///
  /// `nearest` rather than `center`: it moves only when the tab is actually
  /// out of view, so switching between two adjacent tabs does not slide the
  /// whole nav under the pointer.
  function keepInView(node: HTMLElement, isCurrent: boolean) {
    const show = (on: boolean) => {
      if (!on) return;
      node.scrollIntoView({ block: "nearest", inline: "nearest" });
    };
    show(isCurrent);
    return { update: show };
  }

  /// The nav, grouped into its bands. A flat list of nine is a menu.
  const banded = $derived(
    BANDS.map((band) => ({ band, items: surfaces().filter((s) => s.band === band) })).filter(
      (g) => g.items.length > 0,
    ),
  );

  const themeLabel = $derived(
    themeState.choice === "system" ? "following your system" : `${themeState.choice} theme`,
  );
</script>

<a class="skip" href="#surface">Skip to content</a>

<div class="app">
  <aside>
    <div class="mark">
      <b>Devplane</b>
      <!-- **What this page is, where somebody looks when they do not know.**
           The first-run problem is not that the board is hard to read; it is
           that nothing on it says what it is for. -->
      <span class="what">who decided, when nobody asked you</span>
    </div>

    <nav aria-label="surfaces">
      {#each banded as g (g.band)}
        <ul role="list">
          {#each g.items as s (s.id)}
            <li>
              <button
                class="tab"
                use:keepInView={s.id === current}
                onclick={() => go(s.id)}
                aria-current={s.id === current ? "page" : undefined}>{s.title}</button
              >
            </li>
          {/each}
        </ul>
      {/each}
    </nav>

    <div class="foot">
      <!-- **The connection, said out loud.** A board that has silently stopped
           refreshing looks like a quiet machine, which is the worst way for
           this page to be wrong: it is wrong in the reassuring direction. -->
      <span class="pulse" class:bad={!!feed.error || feed.unauthorised} role="status">
        <span class="dot" aria-hidden="true"></span>
        {feed.unauthorised ? "no token" : feed.error ? "not answering" : "live"}
      </span>
      <button class="theme" onclick={cycle} title="Theme: {themeLabel}" aria-label="Theme: {themeLabel}">
        {themeState.choice === "system" ? "auto" : themeState.choice}
      </button>
    </div>
  </aside>

  <main id="surface">
    {#if feed.unauthorised}
      <p class="problem" role="alert">
        This tab has no token. Run <code>devplane open</code> again to get a fresh link.
      </p>
    {:else if feed.error}
      <p class="problem" role="status">Devplane could not be reached: {feed.error}</p>
    {/if}

    {#if showing}
      <!-- The surface renders itself; the shell passes nothing it knows about. -->
      <showing.component {...showing.select(feed, focus)} />
    {:else}
      <p>No surface is registered.</p>
    {/if}
  </main>
</div>

<style>
  /* **First in the tab order and invisible until focused.** With no keyboard
     shortcuts, tabbing is the entire keyboard story here, and without this
     every visit starts by tabbing through the whole nav. */
  .skip {
    position: absolute;
    left: -9999px;
    top: var(--s-2);
    padding: var(--s-2) var(--s-3);
    background: var(--panel);
    border: 1px solid var(--edge);
    border-radius: var(--radius);
    z-index: 3;
  }
  .skip:focus { left: var(--s-4); }

  /* **A sidebar, not a strip of tabs.** Nine surfaces whose names are
     sentences — *what needs you*, *is this actually done* — do not fit across
     the top without becoming a menu bar you read left to right. Down the side
     they are a list you scan, and the sentences stay, because they are what
     tells a first-time reader what the thing is for. */
  .app {
    display: grid;
    grid-template-columns: 15rem 1fr;
    /* **Rows are named, not implied.** Two implicit rows under a `min-height`
       share the viewport between them, which on a phone gave the nav strip a
       third of the screen and the surface the rest. */
    grid-template-rows: 1fr;
    min-height: 100vh;
  }

  aside {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
    padding: var(--s-4);
    border-right: 1px solid var(--line);
    position: sticky;
    top: 0;
    height: 100vh;
    overflow-y: auto;
  }

  .mark { display: flex; flex-direction: column; gap: 2px; }
  .mark b { font-size: var(--t-md); letter-spacing: -0.02em; }
  .what { color: var(--dim); font-size: var(--t-xs); line-height: 1.35; }

  nav { display: flex; flex-direction: column; gap: var(--s-4); min-width: 0; }
  nav ul { list-style: none; display: flex; flex-direction: column; gap: 1px; }

  /* The current surface is marked by weight, a ground and a bar — so it
     survives being read in greyscale, which colour alone would not. */
  .tab {
    width: 100%;
    justify-content: flex-start;
    text-align: left;
    border: 0;
    border-left: 2px solid transparent;
    border-radius: var(--radius);
    background: none;
    padding: var(--s-2) var(--s-3);
    color: var(--dim);
    line-height: 1.3;
  }
  .tab:hover:not([aria-current]) { color: var(--ink); background: var(--panel); }
  .tab[aria-current="page"] {
    color: var(--ink);
    font-weight: 600;
    background: var(--panel);
    border-left-color: var(--accent);
  }

  .foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-2);
  }
  .pulse { display: inline-flex; align-items: center; gap: var(--s-2); color: var(--dim); font-size: var(--t-xs); }
  .pulse .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--done); }
  /* **The word changes, not only the dot.** Colour may never be the only thing
     carrying a distinction — this one matters most to the people least able to
     see it. */
  .pulse.bad { color: var(--fail); }
  .pulse.bad .dot { background: var(--fail); }
  .theme { font-size: var(--t-xs); padding: 1px var(--s-2); }

  main { padding: var(--s-6) var(--s-6) var(--s-7); max-width: 72rem; min-width: 0; }

  .problem {
    color: var(--fail);
    border: 1px solid currentColor;
    border-radius: var(--radius);
    padding: var(--s-3) var(--s-4);
    margin-bottom: var(--s-4);
  }

  /* **The phone is a different errand, not a narrower desktop.** Read the
     queue, answer one thing, close it — so the sidebar becomes a scrolling
     strip at the top and gives up its subtitle, which is a first-run aid and
     not worth a fifth of a phone screen. */
  @media (max-width: 52rem) {
    .app { grid-template-columns: 1fr; grid-template-rows: auto 1fr; }
    aside {
      position: static;
      height: auto;
      flex-direction: row;
      align-items: center;
      gap: var(--s-3);
      border-right: 0;
      border-bottom: 1px solid var(--line);
      padding: var(--s-3) var(--s-4);
      overflow-x: auto;
    }
    .what { display: none; }
    .mark { flex: none; }
    /* The nav is the only thing that scrolls sideways; the mark and the
       status sit outside it, or a scrolled tab slides under them. */
    nav { flex-direction: row; gap: var(--s-2); min-width: 0; overflow-x: auto; scrollbar-width: none; }
    nav::-webkit-scrollbar { display: none; }
    nav ul { flex-direction: row; gap: var(--s-1); }
    .tab { white-space: nowrap; border-left: 0; border-bottom: 2px solid transparent; border-radius: 0; }
    .tab[aria-current="page"] { border-left-color: transparent; border-bottom-color: var(--accent); }
    /* **A ground of its own.** The nav scrolls sideways underneath it, and
       without a background a scrolled tab reads as text printed through the
       status. */
    .foot {
      margin-top: 0;
      flex: none;
      background: var(--bg);
      padding-left: var(--s-3);
      box-shadow: -8px 0 8px -4px var(--bg);
    }
    main { padding: var(--s-4); }
  }

  /* **No state is carried by motion**, so removing it costs nothing. This is
     the contract rather than a preference: every interaction completes with
     animation disabled, because nothing here animates to communicate. */
  @media (prefers-reduced-motion: reduce) {
    :global(*) { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
  }
</style>
