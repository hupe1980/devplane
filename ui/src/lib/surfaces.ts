// The surface registry: a surface registers itself, and nothing enumerates them.
//
// **"Nothing shared enumerates them" is the property, and a barrel file would
// break it.** A list of imports somewhere central is a file every new surface
// has to touch, which is exactly the merge conflict the rebuild exists to
// remove — the hand-written page had one 2,700-line file and every feature
// landed in the middle of it.
//
// `import.meta.glob` with `eager` resolves the directory at build time, so
// adding `ui/src/surfaces/<id>/index.ts` is the whole of adding a surface. No
// registry edit, no import list, and the check that this stays true is a test
// that adds a scratch surface and asserts the diff touches one directory.

import type { Component } from "svelte";

// **The keyboard model was removed on 2026-09-21, deliberately.**
//
// A surface used to declare its shortcuts, and one registry rendered them four
// ways so a key could not be bound without being documented. It was a good
// mechanism for a problem this interface does not have: it is served over
// loopback to a browser, every action is a control on the page, and the CLI is
// the keyboard-first surface — `devplane inbox`, `devplane answer` — for
// anybody who wants one.
//
// What it cost was real: three global keys were claimed by two surfaces each
// and silently did nothing, the hint bar competed for the bottom of every
// screen, and the phone breakpoint had to hide it because `j`/`k` do not exist
// there. **Two ways to do everything is two things to keep true.**

/// What the daemon currently says. Structural rather than imported, so the
/// registry does not depend on the store and the store does not depend on the
/// registry — the cycle that already cost this rebuild one silent bug.
export type Feed = { board: unknown | null; inbox: unknown | null };

/// Which band of the navigation a surface sits in.
///
/// The same idea as the CLI's command groups: not categories, but the errands
/// people arrive with. A nav of nine equal items is a menu; three bands are a
/// shape somebody can learn.
///
/// **None of these may be a surface id.** A band called `work` beside a surface
/// called `work` reads as the same thing twice, and it trips the guard that
/// stops any shared file naming a surface.
export type Band = "attention" | "doing" | "project";

export const BANDS: Band[] = ["attention", "doing", "project"];

export type Surface = {
  /// Stable, lowercase, and the id the docs use.
  id: string;
  /// What the heading says.
  title: string;
  /// Which band, and where within it.
  ///
  /// **Nav order is declared rather than alphabetical, and the landing surface
  /// falls out of it.** It was `id` order, so the shell opened on `board` —
  /// the session list — while the product's whole claim is that it opens on
  /// *what needs you*. Nothing asserted it, and the ledger recorded the
  /// property as carried.
  band: Band;
  order: number;
  /// Which controls of the page being replaced this surface has taken over.
  ///
  /// **Declared, because grepping for the word does not work.** The inventory
  /// first matched a control by looking for its name anywhere in the rebuilt
  /// sources, and `snooze` appearing in a *shortcut label* read as a ported
  /// snooze button. A control is ported when something handles it, and the
  /// only thing that knows is the surface.
  ///
  /// The ids are the page's own: the `data-act` and `data-go` values that
  /// `the_rebuild_loses_no_control_the_page_already_has` extracts.
  ports: string[];

  /// Picks this surface's props out of the feed.
  ///
  /// **This is how the shell stays ignorant of every surface.** Without it the
  /// shell would have to know that the board wants `runs` and the work view
  /// wants a certificate — which is the central file that touches every
  /// feature, back again under a different name.
  ///
  /// A surface with no data returns `{}`. Its component is still typed by its
  /// own props, so the mapping is the one place a shape can be wrong, and it
  /// is next to the surface it is about.
  ///
  /// `focus` is **what the surface was opened about** — a run id, a work id, an
  /// opaque string out of the address bar. The shell does not know what it
  /// means and must not: it reads `#<surface>/<focus>` and hands the second
  /// half over.
  ///
  /// It exists because three surfaces could not be reached about anything.
  /// *Why this is here* rendered *"Open a row and this shows what was decided"*
  /// on every visit, with no row to open and nothing able to give it one, and
  /// the work view read whichever Work happened to be first.
  select: (feed: Feed, focus: string) => Record<string, unknown>;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  component: Component<any>;
};

const registry = new Map<string, Surface>();

/// Registers one surface. Called from the surface's own `index.ts` and nowhere
/// else.
export function register(surface: Surface): void {
  if (registry.has(surface.id)) {
    // Two surfaces under one id is a silent override, and the one that loses
    // is whichever the glob resolved first — which is to say, unpredictable.
    throw new Error(`two surfaces claim the id "${surface.id}"`);
  }
  registry.set(surface.id, surface);
}

/// Every registered surface, in navigation order.
///
/// Band first, then the surface's own order, then id — the last only so two
/// surfaces that forgot to disagree still sort the same way on every build.
export function surfaces(): Surface[] {
  return [...registry.values()].sort(
    (a, b) =>
      BANDS.indexOf(a.band) - BANDS.indexOf(b.band) ||
      a.order - b.order ||
      a.id.localeCompare(b.id),
  );
}

/// The surface the shell opens on: the first in navigation order.
///
/// **Structural rather than a flag.** A `landing: true` somewhere would be a
/// second thing to keep true, and the first item in a deliberate order already
/// is the landing surface — if it is not, the order is wrong.
export function landing(): Surface | undefined {
  return surfaces()[0];
}

export function surface(id: string): Surface | undefined {
  return registry.get(id);
}

