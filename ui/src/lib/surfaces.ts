// The surface registry: a surface registers itself, and nothing enumerates them.
// `import.meta.glob` resolves `ui/src/surfaces/<id>/index.ts` at build time, so
// adding that file is the whole of adding a surface — no central import list.

import type { Component } from "svelte";

// Keys live in `keys.ts`: a surface binds its own through `bind()`, and a
// collision fails the build.

/// What the host currently says, and whether it has said anything yet.
/// Structural rather than imported, so registry and store do not depend on
/// each other.
///
/// The phase travels with the data: a `null` board (not loaded) and an empty
/// board (nothing there) are different facts. The phase fields are optional so
/// a harness can pass a bare feed; absent means *not loaded*.
export type Feed = {
  board: unknown | null;
  inbox: unknown | null;
  loaded?: boolean;
  stale_since?: string | null;
  error?: string | null;
};

/// The phase a surface renders: the fields above, defaulted.
export type Phase = { loaded: boolean; stale_since: string | null; error: string | null };

export function phase(feed: Feed): Phase {
  return {
    loaded: feed.loaded ?? false,
    stale_since: feed.stale_since ?? null,
    error: feed.error ?? null,
  };
}

/// Which band of the navigation a surface sits in: the errand people arrive
/// with, as in the CLI's command groups. The band carries the errand, the item
/// the noun. No band id may also be a surface id.
export type Band = "attention" | "happening" | "steering" | "project";

export const BANDS: Band[] = ["attention", "happening", "steering", "project"];

/// Each band's on-screen label: four of `COMMAND_GROUPS`' headings (held by a
/// guard), cut at the first comma where needed — never paraphrased. The host's
/// own group has no surface.
export const BAND_LABELS: Record<Band, string> = {
  attention: "What needs you",
  happening: "See what is happening",
  steering: "Start and steer work",
  project: "Set up a project",
};

export type Surface = {
  /// Stable, lowercase, and the id the docs use.
  id: string;
  /// The nav label: a noun, three words at most, first word unique, not a
  /// question. A guard enforces all three.
  title: string;
  /// The page's own `<h2>`, which may be a sentence. The surface renders it
  /// (its `id` is the `aria-labelledby` target); a guard keeps both copies equal.
  heading: string;
  /// Which band, and where within it. Nav order is declared, and the landing
  /// surface is its first entry.
  band: Band;
  order: number;
  /// Whether the nav lists it (default true). An unlisted surface — search —
  /// stays routable and owes the chrome a way in; a guard checks it has one.
  nav?: boolean;
  /// Whether the chrome's search field hands its query to this surface. The
  /// shell may not name a surface, so it looks for whichever claims this.
  takesQuery?: boolean;
  /// How many things are in it, for the nav to show; `null` where a count is
  /// meaningless. The surface counts itself so the shell stays ignorant of it.
  count?: (feed: Feed) => number | null;
  /// Whether opening this surface counts as looking. A poll or a tab becoming
  /// visible is not a look: only the surface that draws the hairline advances it.
  marksLook?: boolean;

  /// Picks this surface's props out of the feed, so the shell never knows what
  /// any surface wants. A surface with no data returns `{}`.
  ///
  /// `focus` is what the surface was opened about — a run id, a change id — an
  /// opaque string the shell reads from `#<surface>/<focus>` and hands over.
  select: (feed: Feed, focus: string) => Record<string, unknown>;

  /// The routes this surface fetches for itself. A surface gets data from the
  /// feed (`select`) or from here; a guard fails one that does neither, and
  /// checks every route is one the host serves.
  reads?: string[];

  /// Which deep link this surface answers: `change`, `ask` or `run`.
  /// `devplane://change/<id>` arrives as `?change=<id>`; the value becomes the
  /// focus.
  link?: string;
  /// How the link's value becomes this surface's focus, where the focus is
  /// not simply the id — the inbox narrows by project and a link names a row.
  linkFocus?: (value: string) => string;

  /// No frame: the shell draws neither nav nor foot (the 480×320 answer window).
  bare?: boolean;

  /// Floats over the page instead of replacing it (the palette); never a tab.
  transient?: boolean;

  /// The icon it carries in the activity bar — a name from `lib/ui/Icon`.
  /// A listed surface with no icon is shown by its initial.
  icon?: string;

  /// The sidebar list for a collection surface, given the same props plus
  /// `focus` and `open(focus, pin)`; the pick opens as an editor tab. Without
  /// one the surface fills the editor.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  side?: Component<any>;

  /// What a tab showing this surface on `focus` is called — a change's title
  /// rather than its id. Defaults to the surface's title.
  tab?: (feed: Feed, focus: string) => string;

  /// What this surface says in the status bar; each item opens it. Absent from
  /// an unread feed, never a zero.
  status?: (feed: Feed) => StatusItem[];

  /// The kind of object a shared region opens this surface on (e.g. `run`), so
  /// the region asks the registry instead of naming a surface.
  holds?: string;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  component: Component<any>;
};

/// One fact in the status bar.
export type StatusItem = {
  n: number;
  word: string;
  icon?: string;
  /// `wait` for something that needs a person, `fail` for something broken;
  /// never the verified green.
  tone?: "wait" | "fail";
  /// Which end of the bar: the machine's state on the left, what needs you on
  /// the right.
  end?: boolean;
};

const registry = new Map<string, Surface>();

/// Registers one surface. Called from the surface's own `index.ts` and nowhere
/// else.
export function register(surface: Surface): void {
  if (registry.has(surface.id)) {
    // Otherwise the loser would be whichever the glob resolved first.
    throw new Error(`two surfaces claim the id "${surface.id}"`);
  }
  registry.set(surface.id, surface);
}

/// The surfaces the nav lists, in its order. Routing uses [`surfaces`].
export function listed(): Surface[] {
  return surfaces().filter((s) => s.nav !== false);
}

/// Band, then order, then id so ties sort the same on every build.
export function surfaces(): Surface[] {
  return [...registry.values()].sort(
    (a, b) =>
      BANDS.indexOf(a.band) - BANDS.indexOf(b.band) ||
      a.order - b.order ||
      a.id.localeCompare(b.id),
  );
}

/// The surface the shell opens on: the first in navigation order, not a flag.
export function landing(): Surface | undefined {
  return surfaces()[0];
}

export function surface(id: string): Surface | undefined {
  return registry.get(id);
}

