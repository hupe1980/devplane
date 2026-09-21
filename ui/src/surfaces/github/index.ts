import { register } from "../../lib/surfaces";
import Github from "./Github.svelte";

register({
  id: "github",
  title: "Issues and pull requests",
  band: "doing",
  order: 2,
  ports: ["github"],
  select: () => ({}),
  component: Github,
});
