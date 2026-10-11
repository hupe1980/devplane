import { unseen } from "../../lib/sight";
import { bind, bindList, onAction } from "../../lib/keys";
import { go } from "../../lib/route";
import { register } from "../../lib/surfaces";

bind({ surface: "global", combo: "g l", action: "go-ledger", label: "go to the ledger" });
bindList("why", "open the change the decision is about");
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
  select: (feed, focus) => ({ about: focus, blind: unseen(feed.board) }),
  page: () => import("./Why.svelte"),
});
