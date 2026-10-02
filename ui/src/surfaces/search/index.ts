import { register } from "../../lib/surfaces";
import { bindList } from "../../lib/keys";

// The matches are a list like every other: `j`/`k` move, Enter opens the
// session the match is from.
bindList("search", "open the session the match is from");

register({
  id: "search",
  icon: "search",
  title: "Search",
  heading: "Search every session",
  band: "happening",
  order: 2,
  // Not in the nav: the chrome's field hands its query here, via the address.
  nav: false,
  takesQuery: true,
  reads: ["/api/search"],
  select: (_feed, focus) => ({ q: focus }),
  page: () => import("./Search.svelte"),
});
