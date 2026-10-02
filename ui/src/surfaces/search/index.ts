import { register } from "../../lib/surfaces";

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
