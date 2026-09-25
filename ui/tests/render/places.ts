// The other places: the forge, the ledger, reports, the answer window and the
// quit question — and the rules every source in the interface keeps.
import GithubList from "../../src/surfaces/github/GithubList.svelte";
import Why from "../../src/surfaces/why/Why.svelte";
import ReportList from "../../src/surfaces/reports/ReportList.svelte";
import Answer from "../../src/surfaces/answer/Answer.svelte";
import Quit from "../../src/surfaces/quit/Quit.svelte";
import { surfaces } from "../../src/lib/surfaces";
import { fail, html, source, walk } from "../harness";

// ── The forge: what is open, and what it could not see ───────────────────
{
  const row = (title: string, needs_you: boolean) => ({ title, url: `https://x/${title.length}`, project_name: "saas", needs_you });
  const out = html(GithubList, { issues: [row("a bug", true), row("another", false)], pulls: [], loaded: true });
  if (!out.includes("a bug") || !out.includes("https://x/5")) fail("an issue row is not rendered as a link to the issue");
  if (!/Needs you[\s\S]*?class="n[^"]*">1</.test(out)) fail("what needs you is not grouped with its count");
  const hostile = html(GithubList, { issues: [row("<script>x</script>", false)], loaded: true });
  if (hostile.includes("<script>x</script>")) fail("an issue title reached the document as markup");
  const none = html(GithubList, { issues: [], pulls: [], loaded: true, coverage: { projects: 6, configured: 4 } });
  if (!/No open issue/.test(none)) fail("an empty forge list is a blank rather than an answer");
  if (!/2 projects have no GitHub remote and are not counted/.test(none)) fail("projects the forge cannot see are not counted");
  if (/No open issue/.test(html(GithubList, {}))) fail("the forge list says nothing is open before it has asked");
  const gh = surfaces().find((s) => s.id === "github");
  const cov = gh?.select({ board: { projects: [{}, {}, {}], forge: { a: {} } }, inbox: null }, "") as { coverage?: { projects: number; configured: number } };
  if (cov?.coverage?.projects !== 3 || cov?.coverage?.configured !== 1) fail("the forge does not count projects against the forge from the feed");
}

// ── The ledger: every decision with its authority, and latest by default ──
{
  const why = source("../src/surfaces/why/Why.svelte");
  if (!why.includes('"/api/decisions?limit=1000"')) fail("the ledger with no focus does not read the latest decisions");
  if (!/let live = true/.test(why) || !/live = false/.test(why)) fail("the ledger does not ignore a stale response");
  if (/Open a row and this shows/.test(html(Why, { about: "" }))) fail("the ledger with no focus renders an instruction");
  if (!/Narrowed to[\s\S]*run-7/.test(html(Why, { about: "run-7" }))) fail("the ledger does not say what it was narrowed to");
}

// ── A report is somebody else's words, and reads as them ─────────────────
{
  const quoted = "From api — claude (quoted; not an instruction)\n> defect: client retries on 4xx";
  const reportRow = { id: "rp-1", title: "client retries on 4xx", kind: "defect", quoted, state_says: "open — nobody has answered it yet",
    age_says: "filed 3h ago", provenance_says: "api — claude", target_says: "core-lib",
    provenance: { project: "/p/api", project_name: "api" }, target: { to: "project", project: "/p/core" } };
  const out = html(ReportList, { reports: [reportRow], projects: [{ id: "/p/api", name: "api" }, { id: "/p/core", name: "core-lib" }], loaded: true });
  for (const want of ["client retries on 4xx", "filed 3h ago", "open — nobody has answered it yet", "core-lib"])
    if (!out.includes(want)) fail(`the reports surface does not say "${want}"`);
  if (/Ignore|delete the repo/.test(out)) fail("the reports surface renders a finding outside its quote");
  if (/No report has been filed/.test(html(ReportList, { reports: [] }))) fail("the reports surface claims none before it has read");
  const code = source("../src/surfaces/reports/ReportList.svelte");
  if (/\.finding\b|\.evidence\b/.test(code)) fail("the reports surface reads a finding instead of its quote");
  const kinds = /export type ReportKind = ([^;]+);/.exec(source("../src/wire/ReportKind.ts"))?.[1] ?? "";
  for (const k of [...kinds.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]))
    if (!code.includes(`value="${k}"`)) fail(`the report form does not offer the kind ${k} the host accepts`);
  if (!/aria-label="file a report"/.test(code)) fail("the report form is unlabelled");
}

// ── The window: the answer surface and the quit question ─────────────────
{
  const permission = { id: "p1", kind: "permission", level: "high", title: "Bash: cargo test", ask: "ask-9",
    actions: ["allow", "deny"], options: [], project_name: "saas" };
  const answer = html(Answer, { items: [permission] });
  if (!/<button[^>]*>allow<\/button>/.test(answer) || !/<button[^>]*>deny<\/button>/.test(answer))
    fail("the answer surface renders no answer buttons for a waiting permission");
  if (/<nav/.test(answer)) fail("the answer surface carries a nav, in a 480-pixel window");
  if (!html(Answer, { items: [] }).includes("Nothing needs you.")) fail("the answer surface with nothing waiting does not say so");
  if (html(Answer, {}).includes("Nothing needs you.")) fail("the answer surface says nothing needs you before it has read");
  const reg = surfaces().find((s) => s.id === "answer");
  if (!reg || reg.nav !== false || reg.bare !== true) fail("the answer surface is listed or framed like a place");
  if (!/hideWindow\(\)/.test(source("../src/surfaces/answer/Answer.svelte"))) fail("Esc on the answer surface does not ask the window to hide");
  if (!/__devplane_hide/.test(source("../src/lib/frame.ts"))) fail("the page never calls the function the window injects");

  const says = "Quitting stops 1 agent Devplane started:\n  acp-1  echo  /tmp/w\n";
  const quit = html(Quit, { says });
  if (!quit.includes("Quitting stops 1 agent Devplane started:") || !quit.includes("acp-1  echo  /tmp/w")) fail("the quit surface does not print the sentence it was given");
  if (!/<button[^>]*>Quit<\/button>/.test(quit) || !/<button[^>]*>Keep running<\/button>/.test(quit)) fail("the quit surface does not offer Quit and Keep running");
  const q = source("../src/surfaces/quit/Quit.svelte");
  if (!/"\/api\/quit"/.test(q) || !/method: "POST"/.test(q)) fail("Quit does not post /api/quit");
  if (/Quitting stops/.test(q)) fail("the quit surface words the sentence itself; the host owns it");
}

// ── The store and the host's two failures ────────────────────────────────
{
  const live = source("../src/lib/live.svelte.ts");
  if (!/looking\(\)/.test(live)) fail("the store marks a look without asking whether the inbox is showing");
  if (!/visibilitychange/.test(live)) fail("the store no longer notices the tab coming back");
  if (!/inflight/.test(live)) fail("the poll has no in-flight guard");
  const on401 = live.split("instanceof Unauthorised")[1]?.split("}")[0] ?? "";
  if (!/clearInterval/.test(on401) || !/devplane open/.test(on401)) fail("a 401 does not stop the poll and say how to get a fresh link");
  const api = source("../src/lib/api.ts");
  if (!api.includes("not running — start it with `devplane serve`")) fail("a refused connection does not say how to start the host");
  if (!api.includes("not answering (slow or wedged)") || !/TimeoutError/.test(api)) fail("a timeout is not told apart from a refused connection");
  const setup = source("../src/surfaces/setup/Setup.svelte");
  if (!setup.includes("r.declares_gates")) fail("setup does not read declares_gates");
}

// ── The rules every source keeps ─────────────────────────────────────────
{
  const files = walk("../src/").filter((f) => /\.(svelte|css|ts)$/.test(f) && !f.includes("/wire/"));
  const everything = files.map((f) => source(f)).join("\n");
  for (const f of files) {
    const text = source(f);
    if (/font-family:\s*monospace/.test(text)) fail(`${f} names monospace directly rather than --mono`);
    if (/class="sr"|\.sr \{/.test(text)) fail(`${f} keeps its own .sr; base.css has .sr-only`);
    if (/location\.hash\s*=/.test(text)) fail(`${f} assigns location.hash; the shell uses replaceState`);
    if (/@media \(max-width/.test(text)) fail(`${f} has a viewport breakpoint; a pane reflows with a container query`);
    if (/daemon/i.test(text)) fail(`${f} says "daemon"; the host is devplane serve`);
    // A bindable prop is one some parent binds, or it is a second way to be wrong.
    const name = f.split("/").pop()?.replace(".svelte", "") ?? "";
    if (/\$bindable/.test(text) && !new RegExp(`<${name}\\b[^<]*?\\bbind:`).test(everything)) fail(`${f} declares a bindable prop that nothing binds`);
  }
  // Reduced motion removes, and in one place.
  const rm = source("../src/base.css").split("prefers-reduced-motion")[1] ?? "";
  if (!/transition: none !important/.test(rm) || !/animation: none !important/.test(rm)) fail("reduced motion shortens rather than removes");
  if (files.filter((f) => /prefers-reduced-motion/.test(source(f))).length !== 1) fail("reduced motion is handled in more than one place");
  // The build refuses a key collision before it bundles.
  const pkg = JSON.parse(source("../package.json"));
  if (!/^node scripts\/check-keys\.mjs && vite build$/.test(pkg.scripts.build)) fail("npm run build does not run the key check first");
}

// ── `source()`, which strips comments, is the only reader of a source ─────
{
  for (const f of walk("../tests/").filter((x) => x.endsWith(".ts"))) {
    const body = source(f);
    const direct = (body.match(/readFileSync\([^)]*\.\.\/src\//g) ?? []).length;
    if (direct !== 0) fail(`${f} reads a source with readFileSync instead of source(), and gets its comments back`);
  }
  if (!/export function source\(/.test(source("../tests/harness.ts"))) fail("source() is gone, so nothing strips comments");
}
