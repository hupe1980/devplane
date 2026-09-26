import { isArchived } from "../../lib/State.svelte";
import { register, phase } from "../../lib/surfaces";
import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import List from "./List.svelte";
import Doc from "./Doc.svelte";

type Brief = { id: string; title: string; state: string };

// The changes: the product's home noun. The sidebar lists them by project;
// each opens in the editor as a document with six views.
bind({ surface: "global", combo: "g c", action: "go-changes", label: "go to the changes" });
bind({ surface: "change", combo: "r", action: "review", label: "review the change" });
onAction("go-changes", () => {
  go("#change");
  return true;
});

const briefs = (feed: { board: unknown | null }): Brief[] =>
  (feed.board as { changes?: Brief[] } | null)?.changes ?? [];

register({
  id: "change",
  icon: "change",
  title: "Changes",
  heading: "Changes",
  band: "attention",
  order: 1,
  // `devplane://change/<id>` lands here on that change.
  link: "change",
  // As many as the sidebar lists: archived changes are kept, not counted.
  count: (feed) => {
    const b = feed.board as { changes?: Brief[] } | null;
    return b?.changes ? b.changes.filter((c) => !isArchived(c.state)).length : null;
  },
  tab: (feed, focus) => briefs(feed).find((c) => c.id === focus)?.title ?? "Change",
  select: (feed, focus) => {
    const all = briefs(feed);
    return {
      id: focus,
      all,
      brief: all.find((c) => c.id === focus) ?? null,
      loaded: phase(feed).loaded,
    };
  },
  side: List,
  component: Doc,
});
