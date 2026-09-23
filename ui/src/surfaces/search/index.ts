import { register } from "../../lib/surfaces";
import Search from "./Search.svelte";

register({
  id: "search",
  title: "Search",
  heading: "Search every session",
  band: "happening",
  order: 2,
  // **Not in the nav.** Finding something is not a place you go; the field is in
  // the chrome and this is where its results land.
  nav: false,
  // The chrome's field hands its query here. Declared rather than hard-coded in
  // the shell, which may not name a surface.
  takesQuery: true,
  ports: ["search"],
  // The query arrives in the address, and the surface fetches on demand — so it
  // names the route it reads rather than selecting from the poll feed.
  reads: ["/api/search"],
  select: (_feed, focus) => ({ q: focus }),
  component: Search,
});
