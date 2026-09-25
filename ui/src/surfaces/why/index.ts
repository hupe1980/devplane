import { bind, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { register } from "../../lib/surfaces";
import Why from "./Why.svelte";

bind({ surface: "global", combo: "g l", action: "go-ledger", label: "go to the ledger" });
onAction("go-ledger", () => {
  go("#why");
  return true;
});

register({
  id: "why",
  icon: "ledger",
  title: "Ledger",
  heading: "Ledger",
  band: "happening",
  order: 4,
  // `devplane://run/<id>` lands here on that run's decisions.
  link: "run",
  // The focus is the run this ledger is about.
  select: (_feed, focus) => ({ about: focus }),
  component: Why,
});
