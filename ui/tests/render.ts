import { readFileSync, readdirSync } from "node:fs";
// Renders every surface with Svelte's server renderer and asserts what came
// out.
//
// **No test runner, and that is deliberate.** The alternatives are vitest plus
// a DOM shim plus a testing library — three dependencies, in a repository that
// counts them — to assert on strings that `render()` already returns. Svelte
// ships a server renderer; `vite build --ssr` produces a module node can run;
// the assertions are `if` statements. The same shape as `tests/ui_render.js`,
// which does this for the page being replaced.
//
// What this cannot see is what a browser does: focus, motion, the phone
// breakpoint. Those are named in the specification as tasks a person performs,
// and nothing here pretends otherwise.

import { render } from "svelte/server";
import Board from "../src/surfaces/board/Board.svelte";
import Changes from "../src/surfaces/changes/Changes.svelte";
import Dispatch from "../src/surfaces/dispatch/Dispatch.svelte";
import Work from "../src/surfaces/work/Work.svelte";
import Certificate from "../src/surfaces/work/Certificate.svelte";
import WorkSurface from "../src/surfaces/work/Work.svelte";
import Inbox from "../src/surfaces/inbox/Inbox.svelte";
import GithubList from "../src/surfaces/github/GithubList.svelte";
import LibraryTable from "../src/surfaces/library/LibraryTable.svelte";
import { surfaces, landing, BANDS, BAND_LABELS } from "../src/lib/surfaces";
import { loadSurfaces } from "../src/lib/load";

let failures = 0;
const fail = (m: string) => {
  console.error("ui_surfaces: " + m);
  failures += 1;
};

const html = (c: Parameters<typeof render>[0], props: Record<string, unknown>) =>
  render(c, { props }).body;

/// A surface's source, **with its comments removed**.
///
/// **The only way this file reads a source, and that is the point.** A check
/// that greps a file for the construct it forbids finds the sentence explaining
/// why the construct is forbidden, and passes for ever afterwards. That has now
/// happened five times in this repository — twice in the hour these two blocks
/// were written — so it is no longer something to remember. There is no
/// unstripped read to reach for.
///
/// Spans, not line prefixes: the mention is usually a continuation line, so
/// filtering lines that *start* with `//` leaves it behind.
function source(rel: string): string {
  return readFileSync(new URL(rel, import.meta.url), "utf8")
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\/\/\/?[^\n]*/g, "");
}

// ── An empty state is a result, and it has to be the *right* result ───────
//
// It said "No agent session is running on this machine", which is a claim about
// the machine rather than about what Devplane can see. On a machine running
// three Codex sessions it was false, in the reassuring direction, on the surface
// people trust to tell them nothing needs them. `devplane ls` had always said
// "no **Claude Code** sessions are running".
{
  const watching = {
    watched: ["Claude Code"],
    unproved: ["GitHub Copilot"],
    driven_only: ["Codex", "OpenCode", "Gemini CLI"],
  };
  const out = html(Board, { runs: [], watching });

  if (!/Nothing is running that Devplane can see/.test(out))
    fail("the empty board renders a blank instead of an answer");
  if (/No agent session is running on this machine/.test(out))
    fail("the empty board still claims the machine is quiet rather than that it cannot see");
  if (!/start one and it\s+appears here/.test(out))
    fail("the empty state does not say what happens next");

  // **The vendors it cannot see, named.** This is the gap a person has no other
  // way to discover: a session opened in Codex never appears, and an empty
  // board that does not say so is the whole failure this block exists for.
  for (const v of ["Codex", "OpenCode", "Gemini CLI"])
    if (!out.includes(v)) fail(`the empty board does not say it cannot see ${v}`);
  if (!/not on this board/.test(out))
    fail("the unwatched vendors are listed without saying what that means");

  // Unproved is its own sentence: folding it into the watched list would be the
  // same overstatement in a smaller font.
  if (!/has not been proved/.test(out))
    fail("a vendor whose channels have never been run is presented as watched");

  // **And it still answers when the daemon said nothing.** A board that renders
  // a blank because one field was missing is the original defect with an extra
  // step.
  const bare = html(Board, { runs: [] });
  if (!/Nothing is running that Devplane can see/.test(bare))
    fail("an empty board with no coverage data renders a blank");
}

// ── Outside values reach the document as text ──────────────────────────────
//
// The property the old page needed an `esc()` at every interpolation for. Here
// it is structural, and this is the check that says so out loud.
{
  const nasty = `<script>alert(1)</script>`;
  const out = html(Board, {
    runs: [
      {
        id: "r1",
        project_name: nasty,
        agent: nasty,
        state: "working",
        summary: nasty,
        cost_usd: 0,
        context_percent: null,
        idle_seconds: 5,
      },
    ],
  });
  if (out.includes("<script>alert(1)</script>"))
    fail("an agent's output reached the document as markup");
  // Svelte escapes `<` and `&` and leaves `>`, which is correct: a bare `>`
  // in text content opens nothing. The assertion is that the value survived as
  // text, not that every character was entity-encoded.
  if (!out.includes("&lt;script")) fail("the value was dropped rather than escaped");
}

// ── Three supervision states, and only two of them render ─────────────────
{
  const run = (mode: string | null, asks: boolean | null) => ({
    id: "r1", project_name: "p", agent: "claude", state: "working",
    summary: null, cost_usd: 0, context_percent: null, idle_seconds: 1,
    permission_mode: mode, asks_a_person: asks,
  });

  const nobody = html(Board, { runs: [run("bypassPermissions", false)] });
  if (!nobody.includes("bypassPermissions"))
    fail("a session that decides without you carries no badge");

  const unsure = html(Board, { runs: [run("somethingNew", null)] });
  if (!/somethingNew\s*\?/.test(unsure))
    fail("a mode this build cannot read is not marked as a question");

  // The ordinary case gets no badge: a mark on every row is a mark nobody reads.
  const ordinary = html(Board, { runs: [run("default", true)] });
  if (ordinary.includes("pm "))
    fail("a supervised session grew a badge, so every row has one");

  // Nothing said is not the same as nobody asked.
  const silent = html(Board, { runs: [run(null, null)] });
  if (silent.includes("pm "))
    fail("a session that has not reported a mode was rendered as one that has");
}

// ── Counts: every session is in exactly one bucket ────────────────────────
{
  const out = html(Board, {
    runs: [],
    summary: {
      projects: 3, runs: 9, working: 4, needs_you: 2, idle: 3, failed: 0,
      dormant: 5, cost_usd: 4.18, open_issues: 0, open_prs: 0,
      forge_needs_you: 0, asks_waiting: 1,
    },
  });
  for (const want of ["3</b> projects", "9</b> sessions", "4</b> working", "2</b> need you", "$4.18"]) {
    if (!out.includes(want)) fail(`the header is missing ${want}`);
  }
  // A bucket with nothing in it is absent, not a zero: naming three of five
  // above a total reads as a breakdown and is not one.
  if (/>0<\/b> failed/.test(out)) fail("an empty bucket rendered as a zero");
}

// ── Every surface maps the feed to its own props ─────────────────────────
//
// **The shell knows the registry and not the surfaces**, which only holds if
// each surface does its own picking. A `select` that throws on an empty feed
// is a board that is blank until the first poll lands.
{
  loadSurfaces();
  for (const s of surfaces()) {
    let picked: Record<string, unknown> | undefined;
    try {
      picked = s.select({ board: null, inbox: null });
    } catch (e) {
      fail(`${s.id}.select threw on an empty feed: ${String(e)}`);
      continue;
    }
    if (picked === undefined || typeof picked !== "object")
      fail(`${s.id}.select did not return props for an empty feed`);
  }
  // And on a feed shaped like the real one.
  const feed = { board: { runs: [], summary: undefined, projects: [] }, inbox: { items: [] } };
  for (const s of surfaces()) {
    try {
      s.select(feed);
    } catch (e) {
      fail(`${s.id}.select threw on a populated feed: ${String(e)}`);
    }
  }
}

// ── Every surface is reachable and names itself ─────────────────────────
//
// **The keyboard model was removed on 2026-09-21**, so this is what replaced
// its guards: a surface is reachable because the shell lists it, and it says
// what it is. There is no second way to reach anything and therefore no second
// thing to keep true.
{
  loadSurfaces();
  if (surfaces().length === 0) fail("no surface registered itself");
  const ids = new Set<string>();
  for (const s of surfaces()) {
    if (!s.id.trim()) fail("a surface registered with no id");
    if (!s.title.trim()) fail(`${s.id} has no title, so the nav cannot name it`);
    if (ids.has(s.id)) fail(`two surfaces claim the id ${s.id}`);
    ids.add(s.id);
  }
}

// ── Coverage: an incomplete list never claims the reassuring silence ─────
//
// **The worst way this page can be wrong is in the reassuring direction.** A
// board whose promise is *across everything* and which silently covers four
// projects out of six reads as good news.
{
  const partial = html(Board, {
    runs: [],
    coverage: { projects: 6, unreadable: [{ name: "saas", why: "its forge poll failed" }] },
  });
  if (!partial.includes("5 of 6 projects"))
    fail("an incomplete list claimed the reassuring silence");
  if (!partial.includes("saas")) fail("an unreadable project is not named");
  if (!partial.includes("forge poll failed")) fail("an unreadable project does not say why");

  const nasty = html(Board, {
    runs: [],
    coverage: { projects: 2, unreadable: [{ name: "<img src=x onerror=1>", why: "<b>no</b>" }] },
  });
  if (nasty.includes("<img src=x")) fail("coverage: untrusted text reached the document as markup");

  const whole = html(Board, { runs: [], coverage: { projects: 6, unreadable: [] } });
  if (whole.includes("of 6 projects")) fail("a fully-readable board warned about coverage");
}

// ── A row says where it came from and how long, or says nothing ──────────
{
  const row = (over: Record<string, unknown>) =>
    html(Board, {
      runs: [{ id: "abcdef123456", project_name: "saas", agent: "claude", state: "working",
               summary: null, cost_usd: 0, context_percent: null, idle_seconds: 3600, ...over }],
    });

  if (!row({}).includes("saas")) fail("a waiting row does not name its project");
  if (!row({}).includes("1h")) fail("a waiting row does not say how long it has waited");

  // A row with no project neither vanishes nor falls back to the raw id.
  const anon = row({ project_name: null });
  if (!anon.includes("abcdef1")) fail("a row with no project name vanished");
  if (!anon.includes("(no project)"))
    fail("a row with no project name fell back to nothing rather than saying so");
  // **Visible text, not the markup.** The full id belongs in the row's `href` —
  // that is how the decision log is reached — and the claim here is about what
  // a person reads. Checking the raw HTML conflated the two and failed the
  // moment the row became a link.
  const visible = (html: string) => html.replace(/<[^>]*>/g, "");
  if (visible(anon).includes("abcdef123456"))
    fail("a row shows the whole id rather than a short one");
  // And the link carries the whole id, or it reaches the wrong thing.
  if (!anon.includes("#why/abcdef123456"))
    fail("a row does not link to its decision log, so the why surface is unreachable");

  // An unreadable duration is not rendered as NaN.
  const bad = row({ idle_seconds: Number.NaN });
  if (/NaN/.test(bad)) fail("an unreadable timestamp rendered as NaN");
}

// ── The forge list ───────────────────────────────────────────────────────
{
  const gh = (over: Record<string, unknown>) =>
    html(GithubList, {
      issues: [{ title: "a bug", url: "https://x/1", project_name: "saas", needs_you: true }],
      pulls: [{ title: "a change", url: "https://x/2", project_name: "core", needs_you: false }],
      ...over,
    });

  const issues = gh({});
  if (!issues.includes("a bug")) fail("the github view rendered no issue rows");
  if (!issues.includes("needs you")) fail("an assigned issue does not say so");
  if (!issues.includes("https://x/1")) fail("an issue row is not a link to the issue");

  const hostile = gh({
    issues: [{ title: "<script>x</script>", url: "https://x/1", project_name: "p", needs_you: false }],
  });
  if (hostile.includes("<script>x</script>"))
    fail("github issues: untrusted text reached the document as markup");

  const none = gh({ issues: [], pulls: [] });
  if (!none.includes("Nothing open")) fail("an empty forge list is a blank rather than an answer");
  if (!none.includes("gh")) fail("the empty forge state does not say how the forge is read");
}

// ── The counts line, and what it refuses to say ─────────────────────────
{
  const c = (over: Record<string, unknown>) =>
    html(Board, { runs: [], summary: {
      projects: 2, runs: 4, working: 2, needs_you: 0, idle: 2, failed: 0, dormant: 7,
      cost_usd: 0, open_issues: 0, open_prs: 0, forge_needs_you: 0, asks_waiting: 0, ...over } });

  if (!c({}).includes("7 quiet"))
    fail("the counts line does not report quiet sessions");
  // Asks that outlived their runs are their own number: the buckets above are
  // a breakdown of sessions and this is not one.
  if (!c({ asks_waiting: 3 }).includes("waiting on you"))
    fail("asks that outlived their sessions are folded into a session count");
  if (c({ cost_usd: 0 }).includes("$0.00"))
    fail("a machine with no recorded cost shows a zero rather than nothing");
  if (!c({ cost_usd: 4.18 }).includes("$4.18")) fail("the cost is not reported");
}

// ── The board reads its thresholds from the daemon ──────────────────────
//
// **The last guard the old page had that the rebuild owed.** A session close to
// compaction is the one thing on this board worth catching early, and the
// number that decides "close" is configurable — so a figure written into the
// page would disagree with the one `devplane ls` uses the moment somebody
// changed it, and the two surfaces would call the same session crowded and
// fine.
{
  const run = {
    id: "r1", project_name: "saas", agent: "claude", state: "working",
    summary: null, cost_usd: 0, idle_seconds: 1,
  };

  const over = html(Board, {
    runs: [{ ...run, context_percent: 89 }],
    thresholds: { context_high_percent: 85 },
  });
  if (!/class="ctx[^"]*crowded/.test(over))
    fail("a context window past the daemon's threshold is not marked");
  if (!over.includes("context nearly full"))
    fail("the crowded mark is colour alone, with no word beside it");

  const under = html(Board, {
    runs: [{ ...run, context_percent: 40 }],
    thresholds: { context_high_percent: 85 },
  });
  if (/crowded/.test(under))
    fail("a context window under the threshold is marked anyway");

  // **No threshold is no opinion.** A default invented in the page would be
  // this surface deciding what "crowded" means on its own — which is the whole
  // failure the guard exists to prevent, arriving by the back door.
  const silent = html(Board, { runs: [{ ...run, context_percent: 99 }], thresholds: null });
  if (/crowded/.test(silent))
    fail("the page marked a session crowded with no threshold from the daemon");
}


// ── The diff surface renders the change set, never HTML ────────────────────
//
// **A diff is the densest concentration of somebody else's text this product
// renders** — file names, commit content, whatever an agent wrote — so it is
// the worst possible place to reach for `{@html}`. The daemon used to send a
// rendered `html` field beside the structured set, for a page that no longer
// exists; this surface reads the parsed shape.
{
  const set = {
    base: "main",
    files: [
      {
        path: "src/<script>.ts",
        status: "modified",
        added: 2,
        removed: 1,
        body: {
          hunks: [
            {
              header: "@@ -1,3 +1,4 @@",
              lines: [
                ["context", "const a = 1;"],
                ["removed", "const b = <img onerror=alert(1)>;"],
                ["added", "const b = 2;"],
              ],
            },
          ],
        },
      },
      { path: "logo.png", status: "added", added: 0, removed: 0, body: { binary: { bytes: 4096 } } },
      {
        path: "huge.lock",
        status: "modified",
        added: 9000,
        removed: 9000,
        body: { skipped: { why: "9000 lines, over the limit" } },
      },
    ],
    truncated: { files_shown: 3, files_total: 40, command: "git diff main..." },
  };

  // Rendered directly, because the surface fetches on choice and the harness
  // has no network: the props are what `load()` would have set.
  const out = html(Changes, { works: [{ id: "w1", title: "fix login" }] });
  if (!out.includes("Pick a Work"))
    fail("the changes surface with nothing chosen renders a blank instead of an instruction");
  if (!out.includes("fix login"))
    fail("the changes surface does not offer the works it was handed");

  // **No `{@html}`, asserted over the source** — the rendered output cannot
  // prove the absence of a construct, only this can.
  const src = source("../src/surfaces/changes/Changes.svelte");
  if (src.includes("@html"))
    fail("the diff surface reaches for {@html}, on the densest outside text in the product");
  // And it must not reach for the daemon's rendered HTML either.
  if (/body\.html/.test(src)) fail("the diff surface reads a server-rendered html field");

  // Every distinction the change set draws has to survive rendering: *nothing
  // to show* and *we chose not to show it* are different sentences.
  for (const needed of ["binary", "not shown", "Showing"])
    if (!src.includes(needed)) fail(`the diff surface never renders \`${needed}\``);

  // A branch that changed nothing is a **finding**, not an empty state: a gate
  // that passed over no change verified nothing.
  if (!src.includes("changed nothing"))
    fail("a branch with no changes renders as an empty state rather than as a finding");
  // The sign carries the line kind, because red and green are the one pair that
  // cannot be separated under deuteranopia.
  if (!src.includes('"+"') || !src.includes('"−"'))
    fail("diff lines are marked by colour alone");
  void set;
}


// ── Undo where it works; a plain sentence where it cannot ─────────────────
//
// **A control that says *undo* and cannot deliver is worse than none**: the
// person believes the thing is reverted and stops thinking about it. So the
// affordance is offered by the one action that is reversible and cleared by
// every action that is not — and where an action cannot be taken back, the page
// says so at the moment it happens rather than staying quiet about it.
{
  const code = source("../src/surfaces/inbox/Inbox.svelte");

  // **The way back exists in the daemon and did not exist on the page.**
  // `minutes=0` un-snoozes and has always been accepted; no surface offered it.
  if (!code.includes("minutes=0"))
    fail("nothing on the inbox un-snoozes, so the one reversible action cannot be reversed");

  // Offered only while it can be delivered: set by the reversible action,
  // cleared by the ones that are not.
  if (!/undo = \{/.test(code))
    fail("the undo affordance is never offered");
  const clears = (code.match(/undo = null/g) ?? []).length;
  if (clears < 3)
    fail(
      `undo is cleared in ${clears} places; it must be cleared by every path that is not ` +
        "reversible, or a stale button offers to take back something else",
    );

  // And the irreversible path says so rather than staying quiet.
  //
  // **Read from `code`, not `src`.** Checking the raw file finds the comment
  // explaining the rule and passes whether or not the sentence reaches a
  // person — which it did, on the first run of this very check.
  if (!/cannot be taken back/.test(code))
    fail("answering says nothing about being final, so the absence of undo reads as an oversight");

  // **The button is not permanent.** A control rendered unconditionally is one
  // that cannot deliver whenever there is nothing behind it.
  if (!/\{#if undo\}/.test(code))
    fail("the undo control renders unconditionally, so it can be pressed with nothing behind it");
}


// ── This harness reads sources one way only ───────────────────────────────
//
// **The trap that has caught this repository five times**, and twice in the
// hour the two blocks above were written: a check greps a source for the
// construct it forbids, finds the comment explaining why the construct is
// forbidden, and passes for ever after — including when the thing it guards has
// been deleted.
//
// `source()` strips comments and is the only reader. This is the guard that
// keeps it the only reader, because remembering has demonstrably not worked.
{
  //  is the **bundled** module at runtime, in `.ssr/`, not this
  // file — which is why every other path here starts `../src/`. This one has to
  // reach back into the sources the same way.
  // `import.meta.url` is the **bundled** module at runtime, in `.ssr/`, not this
  // file — which is why every other path here begins `../src/`. This one has to
  // reach back into the sources the same way.
  const self = readFileSync(new URL("../tests/render.ts", import.meta.url), "utf8");
  const body = self.replace(/\/\/[^\n]*/g, "");

  // **The rule is that a *surface* is read only through `source()`.** Two reads
  // belong in this file: the one inside `source()`, and this block reading
  // itself. What must never appear is a third that opens a surface directly,
  // because that one comes back with its comments and the next check written
  // against it passes on the repository's own prose.
  const direct = (body.match(/readFileSync\([^)]*\.\.\/src\//g) ?? []).length;
  if (direct !== 0)
    fail(
      `${direct} check(s) read a surface with readFileSync instead of source(). ` +
        "A direct read returns comments, and a check that greps them passes on the " +
        "sentence explaining the rule rather than on the code.",
    );
  if (!/function source\(/.test(body))
    fail("source() is gone, so nothing strips comments before a check greps a surface");
}


// ── The shell opens on what needs you ─────────────────────────────────────
//
// **The product's whole claim, and nothing asserted it.** The shell took
// `surfaces()[0]` from a list sorted by id, so it opened on `board` — the
// session list, the half every watcher in this category already ships. The
// ledger over the page being replaced recorded
// `the_landing_surface_is_what_needs_you` as *carried*, and it was not.
//
// Nav order is declared now and the landing surface is the first of it, so
// this checks the order rather than a separate flag: if the inbox is not
// first, the navigation is wrong and so is the landing.
{
  const order = surfaces();
  if (order.length < 2) fail("the registry resolved fewer than two surfaces");
  if (landing()?.id !== "inbox")
    fail(`the shell opens on ${landing()?.id}, not on what needs you`);
  if (order[0]?.id !== "inbox")
    fail("the first surface in the nav is not the inbox, so the landing is an accident");

  // Bands are the shape of the nav; a surface outside them cannot be placed.
  for (const s of order)
    if (!BANDS.includes(s.band)) fail(`${s.id} is in no band, so the nav cannot place it`);

  // Within the nav, order is deliberate rather than alphabetical — the defect
  // was exactly that it was alphabetical and nobody had chosen.
  const ids = order.map((s) => s.id);
  const alpha = [...ids].sort();
  if (ids.join() === alpha.join())
    fail("nav order is still alphabetical, so no one has chosen what comes first");
}


// ── A surface opened about something can be reached ───────────────────────
//
// **Three surfaces could not be opened about anything.** *Why this is here*
// rendered "open a row and this shows what was decided" on every visit, with no
// row to open and nothing able to give it one; the work view read whichever
// Work happened to be first and fetched nothing. They were not empty states —
// they were surfaces with no way in.
//
// The shell reads `#<surface>/<focus>` and hands the second half to `select`,
// which is the only thing that makes a detail surface addressable.
{
  // **Two, or the check cannot tell *find* from *first*.** With one Work in the
  // list, ignoring the focus entirely still returns the right id — which is how
  // the first version of this block passed against a surface that ignored it.
  const feed = {
    board: {
      work: [
        { id: "w1", title: "most recent", phase: "running" },
        { id: "w2", title: "second", phase: "done" },
      ],
    },
    inbox: null,
  };

  const why = surfaces().find((s) => s.id === "why");
  if (!why) fail("the why surface is not registered");
  if (why && !("about" in why.select(feed, "run-7")))
    fail("the why surface cannot be opened about a run, so it can never show a decision log");
  if (why && why.select(feed, "run-7").about !== "run-7")
    fail("the why surface ignores what it was opened about");

  const work = surfaces().find((s) => s.id === "work");
  if (work && work.select(feed, "w2").id !== "w2")
    fail("the work surface ignores which Work it was opened about");
  // With no focus it still shows something rather than nothing: the board's
  // list is newest first.
  if (work && work.select(feed, "").id !== "w1")
    fail("the work surface with no focus shows nothing rather than the most recent");

  // **Every surface's select survives a focus it does not understand.** The
  // shell hands over whatever is in the address bar.
  for (const s of surfaces()) {
    try {
      s.select({ board: null, inbox: null }, "nonsense/../..");
    } catch (e) {
      fail(`${s.id}.select threw on an unfamiliar focus: ${String(e)}`);
    }
  }
}

// ── The board's state, in a word and not only a glyph ───────────────────
{
  const out = html(Board, {
    runs: [{ id: "r1", project_name: "p", agent: "claude", state: "waiting",
             summary: null, cost_usd: 0, context_percent: null, idle_seconds: 1 }],
  });
  if (!out.includes("waiting")) fail("the board's state glyph has no word beside it");
  // Context is absent rather than nought when nothing has reported it: a
  // session at 0 % and a session that has not said are different facts.
  if (out.includes("0%")) fail("an unreported context percentage rendered as zero");
}

// ── A free-text box exists where the agent asked for one, and nowhere else ─
{
  const withList = html(Inbox, {
    items: [{ id: "q", kind: "question", level: "normal", title: "which?", ask: "a",
              options: [{ id: "o1", label: "this" }], actions: ["choose"] }],
  });
  if (withList.includes("your answer"))
    fail("a question the agent gave no free-text field grew one anyway");

  const withBox = html(Inbox, {
    items: [{ id: "q", kind: "question", level: "normal", title: "how long?", ask: "a",
              options: [], actions: ["reply"] }],
  });
  if (!withBox.includes("your answer"))
    fail("a question offering a free-text answer rendered only its buttons");
  // The box is bound per item, so two open questions cannot share one answer.
  if (!/aria-label="your answer to: how long\?"/.test(withBox))
    fail("the free-text answer would go back under no field, or the wrong one");
}

// ── A draft of your own is not something waiting on you ─────────────────
//
// The forge list's promise is *what is waiting*, and a draft you opened
// yourself is a thing you already said was unfinished. Counting it is how a
// list of nine becomes a list of four things and five reminders.
{
  const out = html(GithubList, {
    issues: [],
    pulls: [{ title: "wip: the thing", url: "https://x/9", project_name: "p", needs_you: false }],
    tab: "pulls",
  });
  if (!out.includes("wip: the thing")) fail("a draft PR is not listed at all");
  if (out.includes("needs you")) fail("a draft of your own is claimed to need you");
}

// ── Accessibility, asserted rather than documented ───────────────────────
//
// **This product once shipped a documented live region the page did not
// have.** The section described roles, a live region and a glyph-plus-word
// that were not there, and it had been false for every pass since it was
// written. *True but unchecked* is recorded here as equivalent to removed, so
// every property below is asserted against rendered output.
{
  loadSurfaces();

  // Every surface is a landmark with a name. A screen reader announcing
  // "section" four times is a page with no structure.
  for (const s of surfaces()) {
    const out = html(s.component, s.select({ board: null, inbox: null }));
    if (!/aria-labelledby="/.test(out))
      fail(`${s.id} renders no named landmark, so it is unreachable by structure`);
    const id = out.match(/aria-labelledby="([^"]+)"/)?.[1];
    if (id && !new RegExp(`id="${id}"`).test(out))
      fail(`${s.id} points aria-labelledby at "${id}", which it does not render`);
  }

  // **A glyph never carries meaning alone.** `✓` and `✗` are hidden from
  // assistive technology and the word beside them is not — the same rule the
  // board's colour follows, because a mark that only a sighted reader can
  // resolve is a mark half the readers do not get.
  const done = html(Certificate, {
    page: {
      finished: true, basis: "gates passed", checked: true,
      evidence: {
        "gen_ai.evidence.origin": "externally_observed", commit: null, no_commit: null,
        commands: [{ command: "cargo test", shown: "cargo test", truncated: false,
                     outcome: "exit 0", passed: true }],
      },
    },
  });
  if (!/aria-hidden="true"[^>]*>✓/.test(done))
    fail("the outcome glyph is announced, so a screen reader reads a tick as a character");
  if (!done.includes(">met<"))
    fail("the glyph carries the outcome alone — there is no word beside it");

  // **The way off the page, which the docs promised and nothing built.** The
  // daemon has served the certificate's markdown beside its page since the
  // feature shipped; no surface read it, so the one sentence this is sold on —
  // *the others hand you a verdict and this one hands you the commands* — ended
  // at the screen. A test that asserts what a surface *says* cannot see a
  // missing control, which is why this asserts what it can *do*.
  const exportable = html(Certificate, {
    page: { finished: true, basis: "gates passed", checked: true, evidence: {
      "gen_ai.evidence.origin": "externally_observed", commit: null, no_commit: null, commands: [] } },
    markdown: "# done\n\n    cargo test  exit 0\n",
    oncopy: () => {},
  });
  if (!/<button[^>]*>copy this certificate<\/button>/.test(exportable))
    fail("a finished certificate offers no way to get it off the page");

  // And it is not offered where there is nothing to copy: a button that cannot
  // keep its promise is the one failure a control plane cannot afford.
  const nothing = html(Certificate, {
    page: { finished: false, unfinished: "it is still working" },
  });
  if (nothing.includes("copy this certificate"))
    fail("unfinished work offers a certificate to copy");

  // A live region exists where the page reports something changing under the
  // person, and it is polite rather than assertive: none of this interrupts.
  const dispatch = html(Dispatch, { projects: [], preflight: [] });
  if (!dispatch.includes('aria-live="polite"'))
    fail("dispatch changes what it says under the person and announces nothing");

}

// ── The inbox can be answered, which is the whole product ────────────────
{
  const question = {
    id: "a1", kind: "question", level: "high",
    title: "Keep the legacy /v1/login route?",
    project_name: "saas", ask: "ask-1",
    options: [{ id: "o1", label: "keep it" }, { id: "o2", label: "remove it" }],
    actions: ["choose"],
    new_to_you: true,
  };

  const out = html(Inbox, { items: [question], close: { since_last_look: "16h" } });

  // **A board that shows a question and cannot take the answer is every other
  // tool in this category.**
  if (!out.includes("keep it") || !out.includes("remove it"))
    fail("the options the agent offered are not rendered");
  // **Each option is a button that answers**, and the label is the whole of it.
  // They carried a leading `1`–`9` while the keyboard model existed; the keys
  // are gone and so are the numbers, because a digit on a button nobody can
  // press is an affordance that is not there.
  if (!/<button[^>]*>keep it<\/button>/.test(out))
    fail("the options are not answerable buttons");
  if (/<span class="num/.test(out))
    fail("an option still carries a key number, and there are no keys to press");
  if (!out.includes("since you last looked · 16h"))
    fail("the boundary is not rendered");
  if (!out.includes(">new<")) fail("an item raised since the last look is not marked");

  // **An option with no id is shown and never dressed up as a button** — the
  // provider owns that dialog and only its own window can answer.
  const dead = html(Inbox, {
    items: [{ ...question, options: [{ id: null, label: "only in the terminal" }] }],
  });
  if (/<button[^>]*>[^<]*only in the terminal/.test(dead))
    fail("an unanswerable option was rendered as a button that cannot keep its promise");
  if (!dead.includes("only in the terminal"))
    fail("an unanswerable option was hidden rather than shown");

  // Permission items carry allow and deny.
  const perm = html(Inbox, {
    items: [{ id: "p1", kind: "permission", level: "critical", title: "Bash: rm -rf node_modules",
              ask: "ask-2", actions: ["allow", "deny"], options: [] }],
  });
  if (!perm.includes(">allow<") || !perm.includes(">deny<"))
    fail("a permission cannot be answered from the inbox");

  // **The level is a word on the screen, not a glyph with a word hidden behind
  // it.** It rendered as `!!` and `!` with the word in a screen-reader-only
  // span, which satisfies *a glyph never carries meaning alone* by the letter
  // and leaves everybody else reading punctuation that needs a legend.
  if (!/class="lvl[^"]*"[^>]*>critical</.test(perm))
    fail("the level is not shown as a word");
  if (perm.includes(">!!<") || perm.includes(">!<"))
    fail("the level is still a bare glyph");

  // A normal item gets no badge — every row carrying one is every row shouting
  // — but the word stays available to a screen reader, which has no colour and
  // no card edge to read it from.
  const normal = html(Inbox, {
    items: [{ id: "n1", kind: "note", level: "normal", title: "nothing urgent" }],
  });
  if (/class="lvl[^"]*"/.test(normal))
    fail("an ordinary item is badged with its level, so the badge means nothing");
  if (!normal.includes(">normal<"))
    fail("an ordinary item does not carry its level for a screen reader");

  // The kind is a chip a person reads, so it is not a machine identifier.
  if (normal.includes("[note]")) fail("the kind chip still carries its brackets");
  const under = html(Inbox, {
    items: [{ id: "c1", kind: "context_high", level: "high", title: "Context 89% full" }],
  });
  if (!under.includes(">context high<"))
    fail("a kind with an underscore is printed as a machine name");

  // **The close, where nothing was raised.** Every board in this category
  // renders an empty list as an absence.
  const clear = html(Inbox, {
    items: [], folded: [], inhibited: [],
    close: { clear: true, quiet: false, sentences: ["14 decisions taken in your name today."],
             next: "a question in 7c has a deadline of 40m",
             keeps_running: "The daemon keeps watching." },
  });
  if (!clear.includes("Clear.")) fail("an empty inbox renders a blank");
  if (!clear.includes("14 decisions")) fail("the day's tally is not rendered");
  if (!clear.includes("keeps watching")) fail("nothing says what continues while you are away");

  // **A free-text answer, where the agent asked for one.** Some questions
  // have no list, and an option set that does not contain the real answer is
  // worse than a box.
  const free = html(Inbox, {
    items: [{ id: "r1", kind: "question", level: "normal", title: "What should the timeout be?",
              ask: "ask-3", actions: ["reply"], options: [] }],
  });
  if (!free.includes(">reply<")) fail("a question with no list cannot be answered at all");
  if (!/aria-label="your answer to: What should the timeout be\?"/.test(free))
    fail("the free-text box is unlabelled, or labelled without saying which question");

  // The rule to paste, and where — printed, never written.
  const offered = html(Inbox, {
    items: [{ id: "o1", kind: "permission", level: "high", title: "Bash: cargo test",
              ask: "a", actions: ["allow", "deny"], options: [],
              offer: { rule: "Bash(cargo test *)", file: "settings.json",
                       section: "permissions.allow", covers: 12, more: true } }],
  });
  if (!offered.includes("Bash(cargo test *)")) fail("the rule to paste is not shown");
  if (!offered.includes("settings.json")) fail("the rule does not say where it goes");
  if (!offered.includes("12+")) fail("the rule does not say how much it covers");

  // A folded screen is not an empty one.
  const allFolded = html(Inbox, {
    items: [], folded: [{ kind: "ci_red", project: "p1", count: 9, level: "high" }],
    inhibited: [], close: {},
  });
  if (allFolded.includes("Clear.")) fail("the close appeared over nine folded items");
  if (!allFolded.includes("9 × ci_red")) fail("the folded rows are not rendered");
}

// ── Every Work is reachable, so "two clicks" is true of more than one ───────
//
// The registry computed the list of Works and the surface never read it, so it
// showed the one named in the address or the most recent — and any other was
// reachable only by editing the URL. The claim this feature is measured on is
// *two clicks from a finished Work to the clipboard*, and it was true of
// exactly one Work.
{
  const works = [
    { id: "w-1", title: "fix the flaky login test", phase: "done" },
    { id: "w-2", title: "bump deps", phase: "working" },
  ];
  const out = html(WorkSurface, { id: "w-2", title: "bump deps", phase: "working", all: works });

  for (const w of works) {
    if (!out.includes(`#work/${w.id}`))
      fail(`the work surface offers no way to reach ${w.id} — only the address bar does`);
  }
  // An anchor rather than a handler: it has to work from the keyboard, open in
  // a new tab and survive a reload.
  if (!/<a[^>]+href="#work\/w-1"/.test(out))
    fail("a work is reached by a click handler rather than a link");
  if (!/aria-current="page"/.test(out))
    fail("nothing says which work is on screen");

  // One Work is not a choice, and a picker over it is noise.
  const single = html(WorkSurface, {
    id: "w-1",
    title: "only one",
    phase: "done",
    all: [works[0]],
  });
  if (single.includes('aria-label="which work"'))
    fail("a picker was rendered where there is nothing to pick");
}

// ── The library matrix: a word per cell, never a tick ───────────────────────
//
// **`copy_moved` and `library_moved` are the same boolean and opposite
// instructions** — one says this repository has an edit the library does not,
// the other says the library moved on without it. Any rendering that reduces
// the six values to two has lost the only fact a person acts on.
{
  const drifts = [
    "unchanged",
    "copy_moved",
    "library_moved",
    "both_moved",
    "missing",
    "unrecorded",
  ] as const;

  const out = html(LibraryTable, {
    artefacts: [
      {
        name: "review",
        digest: "abc",
        origin: "anthropics/skills",
        copies: drifts.map((d, i) => ({ project: `p${i}`, drift: d, present: d !== "missing" })),
      },
    ],
  });

  const said = new Set<string>();
  for (const d of drifts) {
    // Every value renders as its own sentence, and no two are the same one.
    // Svelte appends a scoped class to every styled element, so the attribute
    // is never the single word this was first written against.
    const words = out.match(/<td[^>]*>([^<]*)<\/td>/g) ?? [];
    if (words.length !== drifts.length) {
      fail(`the library matrix rendered ${words.length} cells for ${drifts.length} copies`);
      break;
    }
    for (const w of words) said.add(w);
  }
  if (said.size !== drifts.length) {
    fail(
      `the library matrix renders ${said.size} distinct cells for six drift values — ` +
        `two of them read alike, and the pair that does is the one a person acts on differently`,
    );
  }
  if (out.includes("origin") && !out.includes("anthropics/skills"))
    fail("an artefact's provenance is not shown");

  const empty = html(LibraryTable, { artefacts: [] });
  if (!empty.includes("Nothing in the library yet"))
    fail("an empty library is a blank rather than an answer");
}

// ── Every surface gets its data from somewhere, and something checks which ──
//
// **Three surfaces shipped wired to nothing.** `select: () => ({})` and no
// fetch means the component renders its own defaults for ever — *what is
// configured* said "nothing is configured yet" on a machine with hooks
// installed and gates declared, *issues and pull requests* rendered two empty
// tabs, and *start work* had no control that started work. Every test passed
// throughout, because the tests above hand props to a component directly: they
// prove the component and never the wiring.
//
// The two checks below are the wiring. A surface takes props from the feed, or
// it names what it reads, and a route it names is one the daemon serves.
{
  // A feed with something in every bucket a `select` could reach for. It does
  // not have to be realistic — it has to be non-empty, because the question is
  // whether `select` reaches into it at all.
  const populated = {
    board: {
      runs: [{ id: "r1" }],
      work: [{ id: "w1", title: "t", phase: "working" }],
      projects: [{ id: "p1", name: "saas" }],
      summary: {},
      forge: {},
    },
    inbox: { items: [{ id: "i1" }], folded: [], inhibited: [], close: {} },
  };

  for (const s of surfaces()) {
    const picked = s.select(populated, "focus");
    const takesProps = Object.keys(picked).length > 0;
    const declaresReads = (s.reads ?? []).length > 0;
    if (!takesProps && !declaresReads) {
      fail(
        `surface "${s.id}" selects nothing from the feed and reads nothing, so it renders its ` +
          `component's defaults for ever. Give it a select, or name the route it fetches in "reads".`,
      );
    }
  }

  // **A route a surface names must be one the daemon serves.** The failure this
  // catches is a rename: the handler moves, the surface goes on fetching the
  // old path, and the only symptom is a surface that is empty in production and
  // green in every test.
  const api = source("../../src/api.rs");
  const served = new Set([...api.matchAll(/\.route\("([^"]+)"/g)].map((m) => m[1]));
  if (served.size === 0) fail("no routes were extracted from src/api.rs — this check is inert");
  for (const s of surfaces()) {
    for (const route of s.reads ?? []) {
      // `{id}` segments are axum's; compare on the shape rather than the value.
      const shape = route.replace(/\/[^/]*\$\{[^}]*\}/g, "/{id}");
      if (!served.has(shape) && !served.has(route)) {
        fail(`surface "${s.id}" reads ${route}, which src/api.rs does not serve`);
      }
    }
  }

  // And the other direction, for the surfaces that were found broken: a
  // surface that names a route has to actually call it, or the declaration is
  // the same empty promise `ports` turned out to be.
  for (const s of surfaces()) {
    for (const route of s.reads ?? []) {
      const dir = `../src/surfaces/${s.id}/`;
      const files = ["index.ts"];
      let found = false;
      for (const f of [...files, `${s.id[0].toUpperCase()}${s.id.slice(1)}.svelte`]) {
        try {
          if (source(dir + f).includes(route)) found = true;
        } catch {
          /* a surface need not have every file */
        }
      }
      if (!found) {
        fail(`surface "${s.id}" declares it reads ${route} and no file in its directory names it`);
      }
    }
  }
}

// ── The nav is scanned, and four of ten items began with the same word ────
//
// **This whole block exists because the harness watched every surface get
// retitled and said nothing.** The only check on a title was that it was not
// empty — so *What is happening*, *What is configured* and *What is installed
// where* sat in one column, three items whose first two words were identical and
// whose distinguishing noun arrived at word three or four. Scanning is a
// left-to-right operation on the first word; a shared prefix spends the one
// fixation that was going to do the work.
//
// The errand belongs to the band heading, which is read once, and the noun
// belongs to the item. That is `devplane --help`'s shape, and the board had it
// exactly backwards: the errand in every item, and the band rendered as nothing
// at all.
{
  const titles = surfaces().map((s) => s.title);
  const firstWord = (s: string) => s.trim().split(/\s+/)[0].toLowerCase().replace(/[^a-z]/g, "");

  // **A shared first word.** Case-insensitive, because the defect is what the
  // eye lands on and not what the source says.
  const seen = new Map<string, string>();
  for (const s of surfaces()) {
    const w = firstWord(s.title);
    const taken = seen.get(w);
    if (taken) {
      fail(
        `"${s.title}" and "${taken}" both begin with "${w}", so the nav's first word ` +
          `discriminates nothing between them — the errand belongs to the band heading`,
      );
    }
    seen.set(w, s.title);
  }

  // **An interrogative opening.** A nav label is a noun; a question is what the
  // band above it asks. These four words are how the defect looked.
  for (const s of surfaces()) {
    if (/^(what|why|is|how|which|where|who)\b/i.test(s.title.trim())) {
      fail(`"${s.title}" opens as a question, which is the band heading's job, not a nav item's`);
    }
  }

  // **And a nav label stays short enough to scan**, which is the reason the
  // question could not stay. Three words is the ceiling; a sidebar is 15rem.
  for (const s of surfaces()) {
    const words = s.title.trim().split(/\s+/).length;
    if (words > 3) fail(`"${s.title}" is ${words} words; a nav label is at most three`);
  }

  if (titles.length < 5) fail("too few surfaces to compare titles — this check is inert");
}

// ── Every band is named, and the names are the CLI's errands ──────────────
//
// **The bands rendered as a one-pixel gap.** No heading, no label, no accessible
// name: a sighted reader saw an unexplained break and a screen reader heard
// anonymous lists. The names existed in the published documentation — the
// quickstart describes the bands in prose — so the one place they were missing
// was the product.
//
// They are the CLI's own group headings, read out of the Rust rather than
// copied, because two hand-kept vocabularies for one product's errands is the
// drift this corpus keeps recording.
{
  const cli = source("../../src/cli/mod.rs");
  const block = (cli.split("COMMAND_GROUPS")[1] ?? "").split("\n];")[0];
  // **Uppercase-initial literals only, and not anchored to a line.** A group
  // name starts with a capital and every command name is lowercase, so the case
  // does the separating. The first version anchored on `^\s*"` and therefore
  // missed `("The daemon", &["serve", "stop"])`, which `rustfmt` keeps on one
  // line — an extraction that depended on how the formatter had broken the
  // literal, and which would have silently stopped seeing any group the next
  // `cargo fmt` decided to inline.
  const groups = new Set([...block.matchAll(/"([A-Z][^"]+)"/g)].map((m) => m[1]));
  if (groups.size < 5) {
    fail(
      `only ${groups.size} command groups were extracted from src/cli/mod.rs — ` +
        `this check is inert or the grouping has shrunk`,
    );
  }
  for (const band of BANDS) {
    const label = BAND_LABELS[band];
    if (!label?.trim()) fail(`band "${band}" has no label, so the nav cannot name its group`);
    // **Or that group cut at its first comma.** Eighty columns against fifteen
    // rems: the longest group wraps to three lines in a sidebar. A clause
    // boundary keeps it the same sentence; anything looser would let a
    // paraphrase through, which is the drift this checks for.
    else if (!groups.has(label) && ![...groups].some((g) => g.startsWith(label + ","))) {
      fail(
        `band "${band}" is called "${label}", which is neither one of the CLI's command groups ` +
          `nor one cut at its first comma — the two surfaces would be naming one product's ` +
          `errands two different ways`,
      );
    }
  }
  // Every band must hold something. An empty band is a heading over nothing,
  // which is worse than no heading.
  for (const band of BANDS) {
    if (!surfaces().some((s) => s.band === band)) fail(`band "${band}" holds no surface`);
  }
}

// ── The page heading is declared and the page renders that string ─────────
//
// **It was written twice per surface with nothing comparing them**: once in the
// registry and once as a hard-coded `<h2>` inside the component. So the nav
// could be retitled end to end and every page keep its old wording, which is
// exactly what happened on the first attempt at this change.
//
// The two are allowed to differ — a nav label is scanned in a column and a page
// heading is read once with the surface under it — but both are declared, and
// the one the page shows has to be the one the registry claims.
//
// **The first version of this guard was vacuous and passed its own mutation.**
// It searched the surface's whole directory, `index.ts` included — and
// `index.ts` is where the heading is *declared*, so every surface satisfied it
// by the declaration alone. Changing a registry heading to a string no page
// rendered came back green. It searches the **components only**, with comments
// stripped, because a guard that finds its subject in the sentence explaining
// its subject is the failure this file has recorded six times.
{
  for (const s of surfaces()) {
    if (!s.heading?.trim()) {
      fail(`${s.id} declares no heading, so nothing holds its <h2> to anything`);
      continue;
    }
    const dir = `../src/surfaces/${s.id}/`;
    let components = 0;
    let found = false;
    for (const f of readdirSync(new URL(dir, import.meta.url))) {
      if (!f.endsWith(".svelte")) continue;
      components += 1;
      if (source(dir + f).includes(s.heading)) found = true;
    }
    if (components === 0) {
      fail(`surface "${s.id}" has no component, so this check is inert for it`);
    } else if (!found) {
      fail(
        `surface "${s.id}" declares the heading "${s.heading}" and no component in its ` +
          `directory renders that string`,
      );
    }
  }
}

// ── A shortened command says where the whole of it is ────────────────────
//
// **Reported from a real inbox row on 2026-09-22**: a stalled shell command cut
// mid-word with a horizontal ellipsis and no way to read the rest. The cause was
// a clip in the **reducer** — eighty characters, applied before any surface
// existed — so the remainder was not hidden, it was gone.
//
// The whole string reaches the surface now, which makes shortening this layer's
// job and therefore makes a route to the rest this layer's debt. The
// certificate's commands have always done it this way: show what fits, carry the
// verbatim text in the title.
{
  const long = "Bash: " + "cargo test --quiet ".repeat(40);
  const out = html(Inbox, {
    items: [
      {
        id: "s1",
        kind: "stalled",
        level: "normal",
        title: "No activity for 65 min",
        detail: long,
      },
    ],
  });
  // **Matched on the detail paragraph, with the scoped class allowed for.**
  // Svelte appends its own class — `class="d svelte-ytvk4v"` — so the first
  // version of this read `class="d"` exactly, matched nothing, and took its
  // `title=` from some other element in the document. It reported a pass while
  // checking nothing, and the mutation that put a title on every detail went
  // green. This repository has been caught by that exact scoped class before.
  const para = (markup: string) =>
    /<p class="d(?: [^"]*)?"([^>]*)>([\s\S]*?)<\/p>/.exec(markup);

  const hit = para(out);
  if (!hit) {
    fail("no detail paragraph was rendered at all, so this check is inert");
  } else {
    const attrs = hit[1];
    const shown = hit[2].trim();
    const inTitle = /title="([^"]*)"/.exec(attrs)?.[1] ?? "";
    if (!inTitle) {
      fail("a shortened detail carries no title, so the rest of the command is unreachable");
    } else if (inTitle.length <= shown.length) {
      // The title has to hold **more** than the row shows, or it is decoration.
      fail(
        `the detail's title (${inTitle.length} chars) is no longer than what the row shows ` +
          `(${shown.length}) — a shortened command must carry its whole self somewhere`,
      );
    }
  }

  // And a detail short enough to render whole gets no title, because a tooltip
  // repeating what is already on screen is noise.
  const brief = para(
    html(Inbox, {
      items: [
        { id: "s2", kind: "stalled", level: "normal", title: "No activity", detail: "Bash: ls" },
      ],
    }),
  );
  if (!brief) fail("the short-detail case rendered no paragraph, so this check is inert");
  else if (/title="/.test(brief[1])) {
    fail("a detail that fits is also being put in a title, which is a tooltip saying nothing");
  }
}

// ── Every field a surface posts is a field the route reads ───────────────
//
// **Pressing *allow* on the board denied the call.** Every control on the inbox
// posted `{ choice }` and `/api/asks/{id}/answer` reads `decision`, `option`,
// `custom` and `field` — there is no `choice`. So the body was valid JSON in
// which nobody had said anything, `Decision::parse(None, None)` answered `Deny`,
// and the surface reported *"answered — on the record and on its way to the
// agent"*. A wrong decision, under the person's name, on the product whose whole
// claim is recording who decided what.
//
// **The mirror image of a CLI reading a key the API never sent**, which this
// repository also shipped. One direction leaves a surface blank; this one
// records the opposite of what somebody chose, which is why it is worse. Both
// are invisible to a test that hands a component its props.
{
  const api = source("../../src/api.rs");
  // Each `#[derive(Deserialize)] struct XBody { … }` and the keys it accepts.
  const bodies = new Map<string, Set<string>>();
  for (const m of api.matchAll(/struct (\w*Body)\s*\{([\s\S]*?)\n\}/g)) {
    const keys = new Set(
      [...m[2].matchAll(/^\s*(?:pub\s+)?([a-z_][a-z0-9_]*)\s*:/gm)].map((k) => k[1]),
    );
    bodies.set(m[1], keys);
  }
  if (!bodies.has("AnswerBody")) {
    fail("no AnswerBody was extracted from src/api.rs — this check is inert");
  } else {
    const accepted = bodies.get("AnswerBody")!;
    // What the inbox surface posts to the answer route: the object literals
    // handed to `answer(...)`, plus anything merged in alongside them.
    const inbox = source("../src/surfaces/inbox/Inbox.svelte");
    const posted = new Set<string>();
    for (const m of inbox.matchAll(/answer\(\s*\w+\s*,\s*\{([^}]*)\}/g)) {
      for (const k of m[1].matchAll(/([a-z_][a-z0-9_]*)\s*:/g)) posted.add(k[1]);
    }
    // `{ ...what, from: "board" }` — the keys spread in are the ones above; the
    // literal ones beside it are read here.
    for (const m of inbox.matchAll(/body: JSON\.stringify\(\{([^}]*)\}/g)) {
      for (const k of m[1].matchAll(/([a-z_][a-z0-9_]*)\s*:/g)) posted.add(k[1]);
    }
    if (posted.size === 0) {
      fail("no answer body fields were extracted from the inbox surface — this check is inert");
    }
    for (const key of posted) {
      if (!accepted.has(key)) {
        fail(
          `the inbox posts "${key}" to /api/asks/{id}/answer and AnswerBody has no such field — ` +
            `the route would ignore it, and an ignored permission answer used to mean deny`,
        );
      }
    }
    // And the route must refuse what it cannot act on, which is the half that
    // turned this from a broken button into a wrong decision.
    if (!/deny_unknown_fields/.test(api.slice(0, api.indexOf("struct AnswerBody")).slice(-400))) {
      fail(
        "AnswerBody does not deny unknown fields, so a surface can post a key nobody reads and " +
          "the route will answer as though nothing was said",
      );
    }
  }
}

// ── The sidebar says what the product says ───────────────────────────────
//
// Ten places carry this sentence; the sidebar was the one that dropped its verb,
// and without it the fragment reads as a riddle. One figure, one home.
{
  const app = source("../src/App.svelte");
  const about = /about = "([^"]+)"/.exec(source("../../src/cli/mod.rs"))?.[1] ?? "";
  if (!about) fail("no `about` was read from src/cli/mod.rs — this check is inert");
  else {
    const claim = about.replace(/^Records/, "records");
    if (!app.includes(claim)) {
      fail(`the sidebar does not say "${claim}", which is what the CLI's --help says it does`);
    }
  }
}

// ── Every action the daemon can offer has a home ──────────────────────────
//
// **The daemon offered thirteen and the inbox rendered five.** `focus`, `open`,
// `open_pr`, `open_issue`, `approve`, `retry` and `resume` had no control, while
// every route behind them already existed — so a permission on a session
// Devplane only *watches* arrived at level `high` offering `focus`, `attach` and
// `open`, and the row showed no buttons at all. Reported from a real inbox on
// 2026-09-22.
//
// The variants are read out of the Rust, so an action added there without a home
// here fails rather than going quietly missing.
{
  const att = source("../../src/core/attention.rs");
  const arms = att.split("impl Action")[1]?.split("}")[0] ?? "";
  const actions = [...arms.matchAll(/Action::\w+ => "([a-z_]+)"/g)].map((m) => m[1]);
  if (actions.length < 10) {
    fail(`only ${actions.length} actions were read from core::attention — this check is inert`);
  }
  const inbox = source("../src/surfaces/inbox/Inbox.svelte");
  // **What gates a control is the surface branching on the action**, not the
  // name appearing somewhere in the file. The first version of this searched the
  // whole source, and `path: "focus"` in the route table satisfied it — so
  // deleting the button left the check green. A route table is a declaration;
  // `includes("focus")` is what decides whether anything renders.
  const branched = new Set(
    [...inbox.matchAll(/includes\("([a-z_]+)"\)/g)].map((m) => m[1]),
  );
  if (branched.size === 0) {
    fail("no action branches were found in the inbox surface — this check is inert");
  }
  // A browser cannot attach a terminal to a process. Named rather than offered,
  // which the surface does with the command itself.
  const terminalOnly = new Set(["attach"]);
  for (const a of actions) {
    if (!branched.has(a)) {
      fail(
        `the daemon can offer "${a}" and the inbox surface never branches on it — ` +
          `an item whose only actions are unhandled ones renders with nothing to act on`,
      );
      continue;
    }
    if (terminalOnly.has(a) && !inbox.includes(`devplane ${a}`)) {
      fail(`"${a}" cannot be done from a browser and the surface does not name the command`);
    }
  }
}

if (failures > 0) {
  console.error(`ui_surfaces: ${failures} failure(s)`);
  process.exit(1);
}
console.log("ui_surfaces: ok");
