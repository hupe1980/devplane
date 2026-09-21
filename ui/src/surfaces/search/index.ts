import { register } from "../../lib/surfaces";
import Search from "./Search.svelte";

register({
  id: "search",
  title: "Search",
  band: "doing",
  order: 3,
  ports: ["search"],
  select: () => ({}),
  component: Search,
});
