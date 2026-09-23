import { register } from "../../lib/surfaces";
import Work from "./Work.svelte";

register({
  id: "work",
  title: "Finished work",
  heading: "Finished work",
  band: "attention",
  order: 2,
  count: (feed) => {
    const b = feed.board as { work?: unknown[] } | null;
    return b?.work?.length ?? null;
  },
  ports: ["approve", "resume", "retry"],
  select: (feed, focus) => {
    // The Work named in the address, or the most recent — the board's list is
    // newest first. The certificate itself is fetched per id by the surface.
    const b = feed.board as { work?: Array<{ id: string; title: string; phase: string }> } | null;
    const all = b?.work ?? [];
    const pick = all.find((w) => w.id === focus) ?? all[0];
    return { id: pick?.id ?? "", title: pick?.title ?? "", phase: pick?.phase ?? "", all };
  },
  component: Work,
});
