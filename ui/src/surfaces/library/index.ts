import { register } from "../../lib/surfaces";
import Library from "./Library.svelte";

register({
  id: "library",
  title: "Library",
  heading: "What is installed where",
  band: "project",
  order: 1,
  ports: ["library"],
  // The matrix is a walk of every project's files; it is read when somebody
  // opens this, not on a two-second poll.
  reads: ["/api/library"],
  select: () => ({}),
  component: Library,
});
