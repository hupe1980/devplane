// The first run and the honest states: Declare a check offers what the
// repository has as quoted text and writes nothing before a shown diff; every
// write is that diff with Confirm and Cancel; a failure says what it means and
// offers a retry; a project with no check is offered Declare, never Run gates;
// the forge never says *No open issue* while signed out or unread.
import Declare from "../../src/surfaces/setup/Declare.svelte";
import type { Candidates } from "../../src/surfaces/setup/Declare.svelte";
import { incomplete } from "../../src/surfaces/setup/checklist";
import PendingWrite from "../../src/lib/PendingWrite.svelte";
import Failed from "../../src/lib/Failed.svelte";
import Skeleton from "../../src/lib/Skeleton.svelte";
import NoCheck, { declaresNoCheck } from "../../src/lib/NoCheck.svelte";
import GithubList from "../../src/surfaces/github/GithubList.svelte";
import { unseen } from "../../src/lib/sight";
import { surfaces, landing } from "../../src/lib/surfaces";
import { recorded } from "../fixtures";
import { fail, html, source, visible } from "../harness";

const text = (markup: string) => visible(markup).replace(/\s+/g, " ");

// ── Declare a check: offered whole, attributed, OpenSpec apart, no write yet ─
{
  const long = "cargo test --locked --workspace --all-features -- --include-ignored --test-threads 1 --nocapture";
  const offered: Candidates = {
    project: { id: "/home/you/code/core-lib", name: "core-lib", root: "/home/you/code/core-lib" },
    current: { check: [], named: {} },
    candidates: [
      { command: long, source: ".github/workflows/ci.yml", line: 12, joins: "check" },
      { command: "npm test", source: "package.json", line: 5, joins: "check" },
      { command: "openspec validate --all --strict", source: "openspec/", line: null, joins: "named:openspec" },
    ],
    looked_for: ["Cargo.toml", "package.json scripts.test", "justfile recipes", ".github/workflows/*.yml run:", "openspec/"],
    config: { path: "/home/you/code/core-lib/devplane.toml", exists: false, uncommitted: false, parses: true },
  };
  const out = html(Declare, { project: offered.project.id, candidates: offered });
  const t = text(out);
  if (!t.includes(long)) fail("Declare a check cuts an offered command short");
  if (!t.includes("found: .github/workflows/ci.yml:12")) fail("an offered command does not say the file and line it came from");
  if (!t.includes("found: package.json:5")) fail("the test script does not say where it came from");
  if (!/data-quoted[^>]*>[^<]*npm test/.test(out)) fail("an offered command is not rendered as quoted repository text");
  if (!t.includes("no gates declared — changes here cannot become verified")) fail("a project with no check does not say it cannot become verified");
  if (!/a named gate — evidence on the change, never part of check/i.test(t)) fail("the OpenSpec candidate is not offered apart, as a named gate");
  if (/checked/.test(out.split("openspec validate")[0].split("npm test")[1] ?? "")) fail("a candidate is pre-selected");
  if (/Write devplane\.toml|Confirm/.test(t)) fail("Declare offers a write before the diff was shown");
  if (!t.includes("Continue — show the change to devplane.toml")) fail("Declare has no way to the diff");

  const nothing = html(Declare, { project: "p", candidates: { ...offered, candidates: [] } });
  if (!/Found nothing to offer — looked for: Cargo\.toml/.test(text(nothing))) fail("an empty offer does not say what it looked for");
  if (!/add a command to check/.test(nothing)) fail("an empty offer leaves nowhere to type a command");

  const broken = html(Declare, { project: "p", candidates: { ...offered, config: { ...offered.config, exists: true, parses: false, line: 3, error: "expected `]`" } } });
  if (!/does not parse at line 3/.test(text(broken))) fail("an unparseable devplane.toml is not shown with its line");
  if (/Continue — show the change/.test(text(broken))) fail("an unparseable devplane.toml still offers to write");

  const walk = html(Declare, { project: "p", candidates: offered, firstRun: true });
  for (const step of ["pick-check", "show-diff"]) if (!walk.includes(`data-first-run-step="${step}"`)) fail(`the checklist's declare step has no ${step} control`);
}

// ── Every write: the file, the diff verbatim, Confirm and Cancel ──────────
{
  const diff = '+ # gates declared from the Devplane window; edit freely\n+ [gates]\n+ check = ["npm test"]\n';
  const out = html(PendingWrite, { pending: { path: "/r/devplane.toml", diff, writes: true, uncommitted: true }, url: "/api/projects/p/gates", body: {} });
  const t = text(out);
  if (!t.includes("/r/devplane.toml")) fail("a pending write does not name its file");
  if (!t.includes('+ check = ["npm test"]') && !t.includes("+ check = [&quot;npm test&quot;]")) fail("a pending write does not show the diff verbatim");
  if (!/Confirm/.test(t) || !/Cancel — write nothing/.test(t)) fail("a pending write lacks Confirm or Cancel");
  if (!/uncommitted changes/.test(t)) fail("a pending write hides that the file has uncommitted changes");
}

// ── A failed read: what it means, the command, a retry, the raw error ─────
{
  const out = html(Failed, {
    what: "the changes",
    failure: { says: "bars: planted failure", tell: "devplane doctor", raw: "/api/changes → 500 {\"error\":\"x\"}", retry: async () => {} },
  });
  const t = text(out);
  if (!/Nothing on this page is current/.test(t)) fail("a failed read does not say what it means for the page");
  if (!/devplane doctor/.test(t)) fail("a failed read does not name devplane doctor");
  if (!/>Retry</.test(out)) fail("a failed read offers no retry");
  if (!/<details[\s\S]*raw error[\s\S]*\/api\/changes → 500/.test(out)) fail("the raw error is not behind a disclosure");
  if (!/aria-busy="true"/.test(html(Skeleton, {}))) fail("a loading skeleton is not marked busy");
  if (/aria-busy/.test(html(Skeleton, { failed: true }))) fail("a skeleton stays beside a failed read");
}

// ── No check: Declare, never Run gates ─────────────────────────────────────
{
  const noGates = recorded("change", "Forty files");
  if (!declaresNoCheck(noGates)) fail("a recorded change in a project with no gates is not recognised as such");
  const gated = recorded("change", "Rate-limit the login route");
  if (declaresNoCheck(gated)) fail("a gated change is read as having no check");
  const out = html(NoCheck, { project: "/home/you/code/core-lib" });
  if (/Run gates/.test(out)) fail("the no-check offer says Run gates");
  if (!out.includes('href="#setup/declare/%2Fhome%2Fyou%2Fcode%2Fcore-lib"')) fail("Declare a check does not open Setup's flow for that project");
  if (!/cannot become verified until a check is declared/.test(text(out))) fail("the no-check offer does not say the change cannot become verified");
}

// ── The forge: signed out offers Sign in; unread is not *No open issue* ─────
{
  const signedOut = html(GithubList, { loaded: true, failed: "not signed in to GitHub — run `devplane auth login`", readAt: null });
  if (!/Sign in/.test(signedOut)) fail("signed out of GitHub, the forge does not offer Sign in");
  if (/No open issue/.test(signedOut)) fail("signed out of GitHub, the forge says No open issue");
  const unread = html(GithubList, { loaded: true, readAt: null });
  if (/No open issue/.test(unread)) fail("before the first read, the forge says No open issue");
  if (!/Not read yet/.test(unread)) fail("before the first read, the forge does not say it has not read");
  const oneFailed = html(GithubList, {
    loaded: true,
    readAt: "2026-10-02T08:00:00Z",
    projects: [{ project: "p", project_name: "saas", github: { state: "signed_in" }, stale: "GitHub said 404 for owner/saas" }],
  });
  if (!/saas<\/b> could not be read — GitHub said 404/.test(oneFailed)) fail("a project whose read failed is not named with the host's words");
  if (/No open issue/.test(oneFailed)) fail("with a project unread, the forge says No open issue");
}

// ── Nothing visible here never reads as nothing here ───────────────────────
{
  const fresh = { runs: [], summary: { runs: 0 }, watching: { driven_only: ["Gemini CLI"], unproved: ["Codex"] } };
  const said = unseen(fresh);
  if (!/No session has reported/.test(said) || !/devplane doctor/.test(said)) fail("a host no session reported to does not say what it cannot see");
  if (unseen({ ...fresh, summary: { runs: 1 } })) fail("a host that has seen a session still says it has seen none");
  if (unseen(null)) fail("an unread board is taken as a fact about sight");
}

// ── Opening on the checklist without the shell naming Setup ────────────────
{
  if (landing()?.id !== "inbox") fail("the landing surface is no longer the inbox");
  if (!surfaces().find((s) => s.id === "setup")?.opensFirst) fail("Setup does not ask to open first while the checklist is incomplete");
  const app = source("../src/App.svelte");
  if (/["'`#]setup\b/.test(app)) fail("the shell names the setup surface");
  const steps = [
    { id: "sessions", state: "done" as const, says: "", advances_without_input: true },
    { id: "trust", state: "next" as const, says: "", advances_without_input: false },
  ];
  if (!incomplete(steps, [])) fail("a step neither done nor skipped does not keep the checklist open");
  if (incomplete(steps, ["trust"])) fail("a skipped step keeps the checklist open");
}
