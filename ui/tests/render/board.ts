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

// ── The chips are the host's numbers: every session counted once ─────────
{
  const runs = feed.runs;
  const out = html(Board, { loaded: true, runs, summary: feed.summary, thresholds: feed.thresholds, watching: feed.watching });
  const chips = [...out.matchAll(/<button[^>]*>([a-z ]+) <span[^>]*>(\d+)<\/span><\/button>/g)];
  const all = chips.find((m) => m[1] === "all");
  if (!all || Number(all[2]) !== feed.summary.runs) fail("the board's total is not the host's count of sessions");
  const sum = chips.filter((m) => m[1] !== "all").reduce((n, m) => n + Number(m[2]), 0);
  if (sum !== feed.summary.runs) fail(`the state chips count ${sum} sessions of ${feed.summary.runs}; one is hidden or counted twice`);
  const waiting = chips.find((m) => m[1] === "waiting");
  if (!waiting || Number(waiting[2]) !== feed.summary.needs_you) fail("the waiting chip is not the host's number");
  for (const r of runs) if (r.project_name && !out.includes(r.project_name)) fail(`the board drops ${r.project_name}'s session`);

  // No chip, and so no count, before the host has answered.
  const early = html(Board, { runs: [], summary: null });
  if (/all <span/.test(early)) fail("the board prints counts before the feed has arrived");

  const odd = html(Board, { loaded: true, runs: [run({ state: "hibernating", reporting: true })],
    summary: { runs: 1, working: 0, needs_you: 0, idle: 1, failed: 0, dormant: 0 } });
  if (/>done <span/.test(odd)) fail("the board invents a done bucket the host does not count");
  // The word the host sent is on the row, whatever bucket it fell in.
  if (!odd.includes("hibernating")) fail("the board's row does not carry the state's word");
}
