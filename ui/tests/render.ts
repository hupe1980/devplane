import { readFileSync } from "node:fs";
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
import Inbox from "../src/surfaces/inbox/Inbox.svelte";
import Github from "../src/surfaces/github/Github.svelte";
import { surfaces, landing, BANDS } from "../src/lib/surfaces";
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
    html(Github, {
      issues: [{ title: "a bug", url: "https://x/1", project: "saas", needs_you: true }],
      pulls: [{ title: "a change", url: "https://x/2", project: "core", needs_you: false }],
      ...over,
    });

  const issues = gh({});
  if (!issues.includes("a bug")) fail("the github view rendered no issue rows");
  if (!issues.includes("needs you")) fail("an assigned issue does not say so");
  if (!issues.includes("https://x/1")) fail("an issue row is not a link to the issue");

  const hostile = gh({
    issues: [{ title: "<script>x</script>", url: "https://x/1", project: "p", needs_you: false }],
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
  const out = html(Github, {
    issues: [],
    pulls: [{ title: "wip: the thing", url: "https://x/9", project: "p", needs_you: false }],
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

if (failures > 0) {
  console.error(`ui_surfaces: ${failures} failure(s)`);
  process.exit(1);
}
console.log("ui_surfaces: ok");
