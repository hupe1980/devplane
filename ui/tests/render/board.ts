// The sessions grid: what it claims about the machine, and what it refuses to.
import Board from "../../src/surfaces/board/Board.svelte";
import feed from "../../fixtures/board.json";
import { fail, html, visible } from "../harness";

const run = (over: Record<string, unknown> = {}) => ({
  id: "r1", project_name: "saas", agent: "claude", state: "working",
  summary: null, cost_usd: 0, context_percent: null, idle_seconds: 1, ...over,
});

// ── An empty state claims only what Devplane can see, not the machine ─────
{
  const watching = { watched: ["Claude Code"], unproved: ["GitHub Copilot"], driven_only: ["Codex", "OpenCode", "Gemini CLI"] };
  const out = html(Board, { runs: [], watching, loaded: true });
  if (!/No session is reporting/.test(out)) fail("the empty board renders a blank instead of an answer");
  if (/No agent session is running on this machine/.test(out))
    fail("the empty board claims the machine is quiet rather than that it cannot see");
  for (const v of ["Codex", "OpenCode", "Gemini CLI"])
    if (!out.includes(v)) fail(`the empty board does not say it cannot see ${v}`);
  if (!/appear only when Devplane starts them/.test(out))
    fail("the unwatched vendors are listed without saying what that means");
  // Unproved is its own sentence, never folded into the watched list.
  if (!/GitHub Copilot[^<]*has not been proved/.test(out))
    fail("a vendor whose channels have never been run is presented as watched");

  // Before the feed has arrived it says nothing at all.
  const unread = html(Board, { runs: [] });
  if (/No session/.test(unread)) fail("the board claims no session before the first poll has landed");
  if (!/aria-busy="true"/.test(unread)) fail("the board renders no skeleton before the feed arrives");

  // A host that stopped answering: a gap, not a quiet machine.
  const gone = html(Board, { runs: [], loaded: true, error: "Devplane is not running" });
  if (/No session is reporting/.test(gone)) fail("an unreachable host renders the calm empty state");
  if (!/has not answered since/.test(gone)) fail("an unreachable host with no rows says nothing about why");
}

// ── Outside values reach the document as text ──────────────────────────────
{
  const nasty = `<script>alert(1)</script>`;
  const out = html(Board, { loaded: true, runs: [run({ project_name: nasty, agent: nasty, name: nasty, summary: nasty })] });
  if (out.includes("<script>alert(1)</script>")) fail("an agent's output reached the document as markup");
  if (!out.includes("&lt;script")) fail("the value was dropped rather than escaped");
}

// ── The board reads its thresholds from the host, as `devplane ls` does ───
{
  const over = html(Board, { loaded: true, runs: [run({ context_percent: 89 })], thresholds: { context_high_percent: 85 } });
  if (!/class="ctx[^"]*hot/.test(over)) fail("a context window past the host's threshold is not marked");
  if (!over.includes("context nearly full")) fail("the crowded mark is colour alone, with no word beside it");
  const under = html(Board, { loaded: true, runs: [run({ context_percent: 40 })], thresholds: { context_high_percent: 85 } });
  if (/context nearly full/.test(under)) fail("a context window under the threshold is marked anyway");
  // No threshold is no opinion.
  const silent = html(Board, { loaded: true, runs: [run({ context_percent: 99 })], thresholds: null });
  if (/context nearly full|class="ctx[^"]*hot/.test(silent))
    fail("the page marked a session crowded with no threshold from the host");
  // Context is a measured fact and may be a percentage; absent is not nought.
  if (!/99%/.test(silent)) fail("a reported context percentage is not shown");
  const unsaid = html(Board, { loaded: true, runs: [run()] });
  if (/\b0%/.test(visible(unsaid))) fail("an unreported context percentage rendered as zero");
}

// ── Every session is counted once; an unknown state is *other*, never *done* ─
{
  const runs = feed.runs;
  const out = html(Board, { loaded: true, runs, summary: feed.summary, thresholds: feed.thresholds, watching: feed.watching });
  const chips = [...out.matchAll(/<button[^>]*>([a-z ]+) <span[^>]*>(\d+)<\/span><\/button>/g)];
  const all = chips.find((m) => m[1] === "all");
  if (!all || Number(all[2]) !== runs.length) fail("the board's total is not the number of sessions it was handed");
  const sum = chips.filter((m) => m[1] !== "all").reduce((n, m) => n + Number(m[2]), 0);
  if (sum !== runs.length) fail(`the state chips count ${sum} sessions of ${runs.length}; one is hidden or counted twice`);
  for (const r of runs) if (r.project_name && !out.includes(r.project_name)) fail(`the board drops ${r.project_name}'s session`);

  const odd = html(Board, { loaded: true, runs: [run({ state: "hibernating" })] });
  if (/>done <span[^>]*>1</.test(odd)) fail("a state the board does not know was counted as done");
  if (!/>other <span[^>]*>1</.test(odd)) fail("a state the board does not know is not counted at all");
  // The word the host sent is on the row, whatever bucket it fell in.
  if (!odd.includes("hibernating")) fail("the board's row does not carry the state's word");
}
