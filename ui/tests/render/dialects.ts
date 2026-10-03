// Dialects on the Specifications surface, over a payload built here in the
// host's shape (`GET /api/specs`): a GSD phase shows no count of any kind and
// says why; an OpenSpec project whose changes live in a store says where,
// instead of an empty list or "no specification layout".
import PlanList from "../../src/surfaces/plan/List.svelte";
import PlanDoc from "../../src/surfaces/plan/Doc.svelte";
import { specs, key } from "../../src/surfaces/plan/store.svelte";
import { bare, fail, html } from "../harness";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Json = any;

const NO_TASK_FILE = "this layout keeps no task file, so Devplane claims no progress for it";
const STORE = "this project's OpenSpec changes live in store `team-specs`, outside this project";
const NO_LAYOUT = "this project has no specification layout; Devplane recognises Spec Kit";

const phase = (path: string): Json => ({
  change_id: null,
  title: null,
  state: null,
  contradicts_done: false,
  drifted: null,
  plan: {
    path,
    present: true,
    files: 2,
    progress: null,
    no_progress: NO_TASK_FILE,
    truncated: null,
    questions: [],
    open_questions: 0,
    outline: [{ level: 1, text: "Phase plan" }],
    fingerprint: "f",
  },
  trace: { edges: [], orphan_requirements: [], tasks_citing_nothing: [], unrecognised: true },
});

const gsd: Json = {
  project_id: "p-gsd",
  project: "planner",
  root: "/repo/planner",
  declares_markers: false,
  declares_plans: false,
  layouts: [{ name: "GSD", root: ".planning/phases", task_file: null, detected_by: ".planning/STATE.md", elsewhere: null }],
  no_layout: null,
  plans: ["01-auth", "02-billing", "02.1-hotfix", "ABC-03-setup"].map((n) => phase(`.planning/phases/${n}`)),
};
const store: Json = {
  project_id: "p-store",
  project: "satellite",
  root: "/repo/satellite",
  declares_markers: false,
  declares_plans: false,
  layouts: [{ name: "OpenSpec", root: "openspec/changes", task_file: "tasks.md", detected_by: "openspec/config.yaml", elsewhere: STORE }],
  no_layout: null,
  plans: [],
};
const none: Json = {
  project_id: "p-none",
  project: "plain",
  root: "/repo/plain",
  declares_markers: false,
  declares_plans: false,
  layouts: [],
  no_layout: `${NO_LAYOUT} (\`.specify/\`), OpenSpec (\`openspec/config.yaml\`), Kiro (\`.kiro/specs/\`) and GSD (\`.planning/STATE.md\`); \`[spec] plans\` declares another`,
  plans: [],
};

{
  const before = specs.projects;
  specs.projects = [gsd, store, none];
  const text = (m: string) => bare(m).replace(/<[^>]*>/g, " ").replace(/\s+/g, " ");

  const list = text(html(PlanList, { open: () => {} }));
  for (const n of ["01-auth", "02-billing", "02.1-hotfix", "ABC-03-setup"])
    if (!list.includes(n)) fail(`the Specifications list drops the GSD phase ${n}`);
  for (const counted of [/\d+ tasks/, /\d+ ticked/, /0 of 0/, /\d+%/])
    if (counted.test(list)) fail(`a GSD phase renders a count (${counted}) on the Specifications list`);
  if (!list.includes("no task file")) fail("a GSD phase does not say it has no task file");

  if (!list.includes("team-specs")) fail("a store-only OpenSpec project does not name its store");
  const unlaid = list.slice(list.indexOf("no specification layout"));
  if (unlaid.includes("satellite") || /2 projects have no specification layout/.test(list))
    fail("a store-only OpenSpec project is listed as having no layout");
  if (!list.includes("1 project has no specification layout")) fail("the project with no layout is not counted alone");

  const doc = html(PlanDoc, { focus: key(gsd, gsd.plans[0]) });
  const said = text(doc);
  if (!said.includes(NO_TASK_FILE)) fail("the GSD phase page does not say why there is no count");
  if (/\d+ boxes/.test(said) || /\d+ ticked/.test(said)) fail("the GSD phase page renders a count");
  if (/<progress|role="progressbar"|class="bar/.test(doc)) fail("the GSD phase page renders a bar");

  specs.projects = before;
}
