// Renders the board's script against a stub DOM, so "the page works" is a test
// rather than a hope.
//
// Every other check on this page reads it as *text*: the fields it names, the
// buttons it gates, the roles it declares. None of them notices a template that
// throws at run time, and none of them can prove that a title carrying
// `<script>` reaches the document as text. This runs the thing.
//
// The DOM here is the smallest one the page actually uses. Anything it reaches
// for and does not find fails loudly rather than returning undefined, because a
// silently absent element is how a render becomes a blank section.
const fs = require("fs");
const vm = require("vm");

const page = fs.readFileSync(process.argv[2], "utf8");
const script = page.slice(page.indexOf("<script>") + 8, page.lastIndexOf("</script>"));

const made = new Map();
function el(id) {
  if (made.has(id)) return made.get(id);
  const node = {
    id,
    innerHTML: "",
    textContent: "",
    value: "",
    hidden: false,
    dataset: {},
    style: {},
    classList: { add() {}, remove() {}, contains: () => false, toggle() {} },
    setAttribute() {},
    removeAttribute() {},
    focus() {},
    blur() {},
    addEventListener() {},
    closest: () => null,
    querySelector: () => null,
    querySelectorAll: () => [],
    scrollHeight: 0,
    scrollTop: 0,
    clientHeight: 0,
  };
  made.set(id, node);
  return node;
}

const document = {
  getElementById: (id) => el(id),
  querySelector: () => el("main"),
  querySelectorAll: () => [],
  addEventListener() {},
  contains: () => false,
  activeElement: null,
  body: el("body"),
};

const context = {
  document,
  console,
  location: { href: "http://127.0.0.1:1/?token=t", search: "?token=t", hash: "" },
  // The theme toggle asks the system what it prefers, and reads a stored
  // choice. Stubbed rather than left undefined so the load path this harness
  // exercises is the one a browser runs.
  matchMedia: () => ({ matches: false, addEventListener() {} }),
  // The page listens for `hashchange` to switch surfaces, on the global the
  // way a browser does.
  addEventListener() {},
  localStorage: { getItem: () => null, setItem() {} },
  history: { replaceState() {} },
  sessionStorage: { getItem: () => "t", setItem() {}, removeItem() {} },
  navigator: { clipboard: { writeText() {} } },
  URL,
  EventSource: class {
    addEventListener() {}
    close() {}
  },
  fetch: async () => ({ ok: true, status: 200, json: async () => ({}) }),
  setTimeout: () => 0,
  clearTimeout() {},
  setInterval: () => 0,
  alert() {},
  prompt: () => null,
  Math,
  JSON,
  Date,
  Symbol,
  Array,
  Object,
  String,
  Number,
  encodeURIComponent,
};
context.window = context;
context.globalThis = context;
vm.createContext(context);
// `let state` lives in the script's declarative scope, not on the global, so
// the page exports the two handles this test drives it by.
const epilogue = `\n;globalThis.__ui = { render, renderGithub, setState: (v) => { state = v; }, setGh: (v) => { gh = v; } };`;
vm.runInContext(script + epilogue, context, { filename: "ui/legacy.html" });

// ── The fixtures carry an attack in every field a person reads ──────────────
const NASTY = `<img src=x onerror="alert(1)">`;
const run = {
  id: "r1",
  project: "p1",
  project_name: NASTY,
  agent: "claude",
  mode: "observed",
  state: "working",
  cwd: "/repo",
  worktree: null,
  name: NASTY,
  summary: NASTY,
  entrypoint: "cli",
  cost_usd: 1.5,
  context_percent: 90,
  idle_seconds: 42,
  reporting: true,
  plan: [],
  plan_done: 0,
  plan_total: 0,
};
const item = {
  id: "a1",
  kind: "permission",
  level: "high",
  run_id: "r1",
  project_id: "p1",
  title: NASTY,
  detail: NASTY,
  options: [{ id: "o1", label: NASTY }],
  actions: ["allow", "deny", "snooze"],
  request_id: "q1",
  url: null,
  launch: null,
  work_id: null,
  // Every field of the offer is somebody else's bytes: the rule is composed
  // from an agent's command and the path is a repository's.
  offer: {
    rule: NASTY,
    basis: "family",
    covers: 6,
    file: NASTY,
    section: "permissions.allow",
  },
  no_offer: null,
  since: "2026-09-15T00:00:00Z",
  // The project name comes from a repository, so it is somebody else's bytes.
  project_name: NASTY,
};
const work = {
  id: "w1",
  title: NASTY,
  phase: "implement",
  branch: NASTY,
  cost_usd: 0,
  gate: null,
  pull_request: null,
  pipeline: null,
  can_retry: false,
  stopped_summary: null,
  snoozed: {},
};

const board_ = {
  summary: {
    projects: 1,
    runs: 3,
    live: 1,
    working: 1,
    needs_you: 1,
    idle: 1,
    failed: 0,
    dormant: 1,
    cost_usd: 1.5,
    open_issues: 2,
    open_prs: 1,
    forge_needs_you: 1,
  },
  runs: [run],
  projects: [],
  forge: { p1: { issues: 2, pull_requests: 1, needs_you: 1 } },
};
context.__ui.setState({ board: board_, inbox: [item], work: [work], sel: 0, showAll: false });
context.__ui.render();

const fail = (m) => {
  console.error("ui_render: " + m);
  process.exit(1);
};

// ── A question renders the whole question, and promises no deadline ─────────
//
// Two properties, and both were got wrong once before being measured.
//
// The **free-text box** exists because the agent's form may carry one beside
// the buttons: a page rendering only the buttons shows a smaller question than
// was asked, which the first design did, having modelled the payload from
// documentation rather than from the wire.
//
// The **absence of a countdown** is the other half. An earlier draft specified
// `answerable for 29d`, written against a mechanism where the vendor keeps a
// paused session on disk under a retention sweep. A question is held on a live
// connection with no timeout — it waits as long as its run and not one second
// less — so any deadline on this surface is a promise the channel cannot keep.
{
  const asked = {
    ...item,
    kind: "question",
    title: "Keep the legacy /v1/login route?",
    request_id: "req-q",
    options: [
      { id: "Keep it", label: "Keep it" },
      { id: "Drop it", label: "Drop it" },
    ],
    actions: ["choose", "open"],
    form: [
      {
        field: "question_0",
        title: "/v1/login",
        options: [
          { value: "Keep it", label: "Keep it", detail: "Retain it as-is." },
          { value: "Drop it", label: "Drop it", detail: "Remove it." },
        ],
        custom_field: "question_0_custom",
      },
    ],
  };
  context.__ui.setState({ board: board_, inbox: [asked], work: [work], sel: 0, showAll: false });
  context.__ui.render();
  const q = el("inbox").innerHTML;

  if (!q.includes("askbox")) {
    fail("a question offering a free-text answer rendered only its buttons");
  }
  // **Found by rendering a real question and reading it**, after every test
  // above passed: the agent writes a sentence per option explaining the
  // trade-off, the form carries it, and the page showed only the labels. The
  // difference between "JWT" and "JWT — stateless, but cannot be revoked before
  // expiry" is the entire reason a person is being asked rather than told.
  // Checked as the **visible** span, not merely as text present somewhere in
  // the markup: the first version of this assertion passed on a rendering where
  // the description survived only in a `title` tooltip — present in the HTML,
  // invisible to the person, and indistinguishable from working.
  for (const why of ["Retain it as-is.", "Remove it."]) {
    if (!q.includes(`<span class="odetail">${why}</span>`)) {
      fail(`an option's own description is not rendered (${why}); the person sees a label where the agent wrote a reason`);
    }
  }
  if (!q.includes('data-field="question_0"')) {
    fail("the free-text answer would go back under no field, or the wrong one");
  }
  for (const banned of ["answerable", "expires", "countdown", "days left"]) {
    if (q.toLowerCase().includes(banned)) {
      fail(`a question promised a deadline (${banned}); this channel has none`);
    }
  }

  // A question with no free-text field must not grow an empty box.
  const plain = { ...asked, form: [{ ...asked.form[0], custom_field: null }] };
  context.__ui.setState({ board: board_, inbox: [plain], work: [work], sel: 0, showAll: false });
  context.__ui.render();
  if (el("inbox").innerHTML.includes("askbox")) {
    fail("a question the agent gave no free-text field grew one anyway");
  }

  // Put the fixture back for everything below.
  context.__ui.setState({ board: board_, inbox: [item], work: [work], sel: 0, showAll: false });
  context.__ui.render();
}

const board = el("board").innerHTML;
const inbox = el("inbox").innerHTML;
const counts = el("counts").innerHTML;
const workHtml = el("work").innerHTML;

// It rendered at all.
if (!board.includes("card row")) fail("the board rendered no rows");
if (!inbox.includes("card item")) fail("the inbox rendered no items");
if (!workHtml.includes("card row")) fail("the work section rendered no rows");

// Nothing a stranger wrote became markup.
for (const [name, html] of [["board", board], ["inbox", inbox], ["work", workHtml]]) {
  if (html.includes("<img src=x")) fail(`${name}: untrusted text reached the document as markup`);
  if (!html.includes("&lt;img src=x")) fail(`${name}: the untrusted text is not there at all`);
}

// The counts line names every column, and the live region speaks.
for (const want of ["1</b> working", "1</b> need you", "1</b> idle", "2</b> issues"]) {
  if (!counts.includes(want)) fail(`the counts line is missing ${want}`);
}
if (!counts.includes("1 quiet")) fail("the counts line does not report quiet sessions");
if (el("announce").textContent !== "1 item needs you") {
  fail(`the live region said "${el("announce").textContent}"`);
}

// The counts are the affordance: both the header total and the per-project
// heading open the forge view, and both are real buttons so a keyboard gets
// there without a tabindex.
if (!counts.includes('<button id="ghopen"')) {
  fail("the header's issue count is not a button that opens the forge view");
}
if (!board.includes('<button class="linkish ghopen"')) {
  fail("a project heading's issue count is not a button that opens the forge view");
}

// A project heading whose counts are old says so, where the counts are.
const setBoard = (forge) => {
  context.__ui.setState({
    board: { ...board_, forge }, inbox: [item], work: [work], sel: 0, showAll: false,
  });
  context.__ui.render();
};
setBoard({ p1: { issues: 2, pull_requests: 1, needs_you: 1, stale: "gh: boom" } });
if (!el("board").innerHTML.includes(">stale<")) {
  fail("a project heading shows stale counts as though they were fresh");
}
setBoard({ p1: { issues: 2, pull_requests: 1, needs_you: 1 } });
if (el("board").innerHTML.includes(">stale<")) fail("a good poll is marked stale");

// ── The GitHub view renders, and its rows are escaped too ──────────────────
//
// It is behind a key, so nothing above this line touches it — which is exactly
// how a render path goes untested until somebody presses the key.
const ghIssue = {
  project: "p1", project_name: NASTY, number: 7, title: NASTY,
  url: "https://example.com/i/7", labels: [NASTY], assigned_to_me: true, updated_at: null,
};
const ghPr = {
  project: "p1", project_name: NASTY, number: 9, title: NASTY,
  url: "https://example.com/p/9", status: "failing", draft: false, mine: true,
  review_requested: false, head_ref: NASTY, updated_at: null,
};
context.__ui.setGh({
  tab: "issues", loading: false, error: null,
  data: { viewer: NASTY, error: null, issues: [ghIssue], pull_requests: [ghPr] },
});
context.__ui.renderGithub();
let ghHtml = el("ghbody").innerHTML;
if (!ghHtml.includes("ghrow")) fail("the github view rendered no issue rows");
if (ghHtml.includes("<img src=x")) fail("github issues: untrusted text reached the document as markup");
if (!ghHtml.includes("assigned to you")) fail("an assigned issue does not say so");
if (el("ghwho").textContent !== `as ${NASTY}`) fail("the viewer is not named, or not as text");

context.__ui.setGh({
  tab: "prs", loading: false, error: null,
  data: { viewer: null, error: null, issues: [ghIssue], pull_requests: [ghPr] },
});
context.__ui.renderGithub();
ghHtml = el("ghbody").innerHTML;
if (!ghHtml.includes("checks red")) fail("a red PR of mine does not say why it is listed");
if (ghHtml.includes("<img src=x")) fail("github PRs: untrusted text reached the document as markup");

// A draft of your own is listed, and says nothing needs you — the same rule
// the inbox applies, so the two never describe one thing two ways.
context.__ui.setGh({
  tab: "prs", loading: false, error: null,
  data: { viewer: null, error: null, issues: [], pull_requests: [{ ...ghPr, draft: true }] },
});
context.__ui.renderGithub();
ghHtml = el("ghbody").innerHTML;
if (!ghHtml.includes("ghrow")) fail("a draft PR is not listed at all");
if (ghHtml.includes("checks red")) fail("a draft of your own is claimed to need you");

// A project whose poll failed is named in the view, with when it last worked.
context.__ui.setGh({
  tab: "issues", loading: false, error: null,
  data: {
    viewer: null, error: null, issues: [ghIssue], pull_requests: [],
    stale: [{ project: "p1", project_name: NASTY, error: "gh: connection refused",
              last_good: "2026-09-15T10:42:00Z" }],
  },
});
context.__ui.renderGithub();
ghHtml = el("ghbody").innerHTML;
if (!ghHtml.includes("gh: connection refused")) fail("a project that could not be read is not named");
if (!ghHtml.includes("2026-09-15T10:42:00Z")) fail("the view does not say how old the numbers are");
if (ghHtml.includes("<img src=x")) fail("stale notice: untrusted text reached the document as markup");
if (!ghHtml.includes("ghrow")) fail("a failed poll emptied the list instead of annotating it");

// A failed poll is shown rather than rendered as an empty forge.
context.__ui.setGh({
  tab: "issues", loading: false, error: null,
  data: { viewer: null, error: "gh: not logged in", issues: [], pull_requests: [] },
});
context.__ui.renderGithub();
if (!el("ghbody").innerHTML.includes("gh: not logged in")) {
  fail("a failed poll is reported as an empty forge");
}

// A state glyph reads as a word too.
if (!board.includes('class="sr">working<')) fail("the board's state glyph has no word beside it");

// ── The waiting row says where it came from and how long ───────────────────
//
// Both are what make the list readable without opening a row, and the project
// name is repository-provided text, so it is in the escaping sweep above too.
if (!inbox.includes("where")) fail("a waiting row does not name its project");
if (!inbox.includes("waiting ")) fail("a waiting row does not say how long it has waited");

// A row whose project the world no longer has renders without it rather than
// rendering a raw id at a person.
context.__ui.setState({
  board: board_, inbox: [{ ...item, project_name: null }], work: [work], sel: 0, showAll: false,
});
context.__ui.render();
const anon = el("inbox").innerHTML;
if (!anon.includes("card item")) fail("a row with no project name vanished");
if (anon.includes("p1")) fail("a row with no project name fell back to showing the raw id");

// An unparseable timestamp yields no age rather than `NaN`.
context.__ui.setState({
  board: board_, inbox: [{ ...item, since: "not a date" }], work: [work], sel: 0, showAll: false,
});
context.__ui.render();
if (el("inbox").innerHTML.includes("NaN")) fail("an unreadable timestamp rendered as NaN");

// ── The three silences ─────────────────────────────────────────────────────
//
// Three different mornings, and the third used to render as the first — which
// is the reassuring direction to be wrong in and therefore the worst.
const silences = new Set();

// 1. Nothing needs you, everything readable.
context.__ui.setState({ board: { ...board_, coverage: { projects: 3, unreadable: [] } },
                        inbox: [], work: [], sel: 0, showAll: false, checkedAt: Date.now() });
context.__ui.render();
silences.add(el("inbox").innerHTML);
if (!el("inbox").innerHTML.includes("Nothing needs you")) fail("a quiet machine does not say so");
if (el("cover").innerHTML !== "") fail("a fully-readable board warned about coverage");

// 2. Some projects could not be read — and they are named.
context.__ui.setState({
  board: { ...board_, coverage: { projects: 3, unreadable: [{ name: NASTY, why: "gh: not logged in" }] } },
  inbox: [], work: [], sel: 0, showAll: false, checkedAt: Date.now(),
});
context.__ui.render();
silences.add(el("inbox").innerHTML);
const cover = el("cover").innerHTML;
if (!cover.includes("could not be read")) fail("an unreadable project is not reported");
if (!cover.includes("gh: not logged in")) fail("an unreadable project does not say why");
if (cover.includes("<img src=x")) fail("coverage: untrusted text reached the document as markup");
if (el("inbox").innerHTML.includes("Nothing needs you.</b>"))
  fail("an incomplete list claimed the reassuring silence");

// 3. The daemon has not answered recently.
context.__ui.setState({ board: { ...board_, coverage: { projects: 3, unreadable: [] } },
                        inbox: [], work: [], sel: 0, showAll: false,
                        checkedAt: Date.now() - 600000 });
context.__ui.render();
silences.add(el("inbox").innerHTML);
if (!el("inbox").innerHTML.includes("has not answered recently"))
  fail("a stale page does not say the daemon is quiet");
if (el("inbox").innerHTML.includes("Nothing needs you"))
  fail("a stale page claimed nothing needs you, which is the failure this exists to prevent");
if (!el("fresh").innerHTML.includes("out of date")) fail("a stale list is not marked stale");

if (silences.size !== 3) fail(`the three silences produced ${silences.size} distinct renderings`);

// Coverage warns even when the list is not empty: a busy list can be just as
// incomplete as an empty one.
context.__ui.setState({
  board: { ...board_, coverage: { projects: 3, unreadable: [{ name: "p", why: "gone" }] } },
  inbox: [item], work: [work], sel: 0, showAll: false, checkedAt: Date.now(),
});
context.__ui.render();
if (!el("cover").innerHTML.includes("could not be read"))
  fail("a non-empty list stopped reporting what it could not see");

// Never loaded is not an age of nought.
context.__ui.setState({ board: board_, inbox: [], work: [], sel: 0, showAll: false, checkedAt: null });
context.__ui.render();
if (!el("fresh").innerHTML.includes("not loaded yet")) fail("a page that never loaded reported an age");

// The offered rule must be valid **where it is going**. It emitted TOML into a
// JSON settings file after the destination moved, and nothing failed.
context.__ui.setState({ board: board_, inbox: [item], work: [work], sel: 0, showAll: false,
                        checkedAt: Date.now() });
context.__ui.render();
const offered = el("inbox").innerHTML;
if (!offered.includes("permissions")) fail("the offered rule does not name where it goes");
if (offered.includes("auto_allow"))
  fail("the offered rule is TOML for a key that no longer exists");

// **The supervision badge shows exactly the two states worth showing.**
//
// The first version of this rendered "unknown mode" for every session that had
// simply not reported one yet, which on a real machine described sixteen idle
// editors as running something exotic. Silence is not a finding.
{
  const row = (extra) => ({ ...board_.runs[0], ...extra });
  const show = (r) => {
    context.__ui.setState({ board: { ...board_, runs: [r] }, inbox: [], work: [],
                            sel: 0, showAll: false, checkedAt: Date.now() });
    context.__ui.render();
    return el("board").innerHTML;
  };

  const nobody = show(row({ permission_mode: "auto", asks_a_person: false }));
  if (!nobody.includes("pmode nobody"))
    fail("a session nobody supervises carries no badge");
  if (!nobody.includes("auto")) fail("the badge does not name the mode");

  const asks = show(row({ permission_mode: "manual", asks_a_person: true }));
  if (asks.includes("pmode"))
    fail("a supervised session was badged; then every row is decorated and none stands out");

  const strange = show(row({ permission_mode: "hypervigilant", asks_a_person: null }));
  if (!strange.includes("pmode unsure"))
    fail("an unreadable mode must read as a question, not as an alarm");
  if (strange.includes("pmode nobody"))
    fail("an unreadable mode was rendered as unsupervised");

  const silent = show(row({ permission_mode: null, asks_a_person: null }));
  if (silent.includes("pmode"))
    fail("a session that has not reported a mode was badged as though it had");
}

// **The seat's own number, and the four rules it may not break.**
//
// This is the only line on the page that can say the product is not working, so
// the failure that matters is it rendering as a grade. Each rule below is one
// the CLI already enforces, asserted here because the page is a second reader of
// the same daemon field and a second reader is where a rule stops being one.
{
  const over = (oversight, agents) => {
    context.__ui.setState({
      board: board_, inbox: [], work: [], sel: 0, showAll: false,
      checkedAt: Date.now(),
      attention: oversight === null ? null : { oversight, agents: agents || [] },
    });
    context.__ui.render();
    return el("oversight").innerHTML;
  };

  // Nought of a non-zero total is the one case worth the contrast.
  const zero = over({ sentence: "You answered 0 of the 49 decisions taken in your name.",
                      answered: 0, total: 49, asked: 0, unattended: 49 });
  if (!zero.includes("count none"))
    fail("none of a non-zero total must carry the contrast; it is the whole finding");
  if (!zero.includes("2607.28317"))
    fail("the citation is missing, so the count reads as a verdict this product did not make");
  if (!zero.includes("devplane modes"))
    fail("nothing was put to the person and the line that explains why is absent");

  // A low ratio is not a failing grade — this product does not grade.
  const some = over({ sentence: "You answered 3 of the 49 decisions taken in your name.",
                      answered: 3, total: 49, asked: 6, unattended: 43 });
  if (some.includes("count none"))
    fail("a low ratio was painted as an alarm; that is grading");
  if (some.includes("devplane modes"))
    fail("the 'not one of them' line fired while six were asked");

  // A caveat shown always is a caveat nobody reads.
  const one = over({ sentence: "s", answered: 1, total: 2, asked: 2, unattended: 0 }, ["claude"]);
  if (one.includes("compare the ratios"))
    fail("the multi-vendor caveat fired on a single-vendor machine");
  const two = over({ sentence: "s", answered: 1, total: 2, asked: 2, unattended: 0 },
                   ["claude", "copilot"]);
  if (!two.includes("compare the ratios"))
    fail("two vendors in the window and nothing says the denominators differ");

  // Absent rather than nought: a quiet week is not a finding.
  if (over({ sentence: null, answered: 0, total: 0, asked: 0, unattended: 0 }) !== "")
    fail("a week where nothing happened must say nothing at all");
  if (over(null) !== "")
    fail("an unreachable measurement must leave the line absent, never render a zero");
}

console.log("ui_render: ok");
