<script lang="ts">
  // What needs a person, and answering it.
  //
  // **This is the surface the product is named for.** A board that shows a
  // question and cannot take the answer is the thing every other tool in this
  // category already is — so the answer path is the port, and the list around
  // it is scaffolding.
  import { api } from "../../lib/api";
  import { ago, clip, plural } from "../../lib/text";

  type Choice = { id: string | null; label: string };
  type Item = {
    id: string;
    kind: string;
    level: string;
    title: string;
    detail?: string | null;
    project_name?: string | null;
    run_id?: string | null;
    ask?: string | null;
    request_id?: string | null;
    options?: Choice[];
    actions?: string[];
    /// The rule to paste so this is never asked again, and where it goes.
    ///
    /// **Composed by the daemon and printed, never written.** The rules are
    /// committed files reviewed like code, and an agent here runs as the same
    /// user — so a surface that wrote one would be reachable by the party the
    /// file exists to bound.
    offer?: { rule: string; file: string; section: string; covers: number; more?: boolean } | null;
    work_id?: string | null;
    project_id?: string | null;
    new_to_you?: boolean;
    since?: string;
  };
  type Summary = { kind: string; project: string | null; count: number; level: string };
  type Inhibited = { cause: string; count: number; because: string };
  type Close = {
    since_last_look?: string | null;
    clear?: boolean;
    quiet?: boolean;
    sentences?: string[];
    next?: string | null;
    keeps_running?: string | null;
  };

  let {
    items = [],
    folded = [],
    inhibited = [],
    close = null,
  }: {
    items?: Item[];
    folded?: Summary[];
    inhibited?: Inhibited[];
    close?: Close | null;
  } = $props();

  /// What the last answer did, so a refusal is never silent.
  let said = $state("");

  /// The one action on this surface that can be taken back, and the route that
  /// takes it back.
  ///
  /// **Offered only while it can actually be delivered.** A control that says
  /// *undo* and cannot is worse than none: the person believes the thing is
  /// reverted and stops thinking about it. So this is set by the action that is
  /// reversible and cleared by every action that is not — it is never a
  /// permanent button that hopes there is something behind it.
  let undo = $state<{ says: string; where: string } | null>(null);

  // **Answering is a POST to the ask, and the page composes no verdict.**
  // There is no allow rule here and no policy route — the button carries the
  // person's decision and the daemon records who made it.
  async function answer(item: Item, choice: string) {
    const ask = item.ask ?? item.request_id;
    if (!ask) {
      said = "this one cannot be answered from here";
      return;
    }
    try {
      await api(`/api/asks/${encodeURIComponent(ask)}/answer`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ choice }),
      });
      // **No undo, and the sentence says why rather than staying quiet about
      // it.** The answer has left: it is on the record and in the agent's
      // hands, and nothing this page can send would recall it. Where an action
      // cannot be taken back, saying so at the moment it happens is the honest
      // substitute for a control that could not keep its promise.
      undo = null;
      said = "answered — on the record and on its way to the agent. That cannot be taken back.";
    } catch (e) {
      // **Never silent.** An answer that did not land and says nothing is the
      // worst failure this surface has: the person believes they replied.
      undo = null;
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  /// A free-text answer, where the agent asked for one rather than offering
  /// a list.
  let typed = $state<Record<string, string>>({});

  async function snooze(item: Item) {
    // The route is the thing the item is *about*: work, run, or the project
    // for a row the forge produced, which has neither.
    // **A whole route, never a base to append to.** Building one in pieces
    // means neither a reader nor a guard can see what is actually called —
    // and a surface reaching a route the daemon does not serve is a button
    // that cannot keep its promise.
    const where = item.work_id
      ? `/api/work/${encodeURIComponent(item.work_id)}/snooze`
      : item.run_id
        ? `/api/runs/${encodeURIComponent(item.run_id)}/snooze`
        : item.project_id
          ? `/api/projects/${encodeURIComponent(item.project_id)}/snooze`
          : null;
    if (!where) {
      said = "there is nothing to snooze this against";
      return;
    }
    try {
      await api(`${where}?minutes=60`, { method: "POST" });
      said = "hidden for an hour";
      // **`minutes=0` is the way back**, and the daemon has always accepted it.
      // No surface offered it until 2026-09-21, so the only thing on this page
      // that *could* be undone was the one thing a person could not undo — they
      // waited out the hour, or restarted the daemon.
      undo = { says: "put it back", where: `${where}?minutes=0` };
    } catch (e) {
      undo = null;
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  /// Takes back the last snooze.
  async function takeBack() {
    if (!undo) return;
    try {
      await api(undo.where, { method: "POST" });
      said = "back in the list";
    } catch (e) {
      said = `that did not land: ${e instanceof Error ? e.message : String(e)}`;
    }
    // **Cleared either way.** A failed undo that leaves the button offering
    // itself again invites somebody to press it until the message changes.
    undo = null;
  }

  /// **A convenience, never the only way to get the text.** The rule is on
  /// screen as selectable text, which is what makes this work on a browser
  /// that withholds the clipboard — over a tunnel, say.
  async function copyRule(rule: string) {
    try {
      await navigator.clipboard?.writeText(rule);
      said = `copied: ${rule}`;
    } catch {
      said = "no clipboard here — select the rule above";
    }
  }

  const nothingRaised = $derived(
    items.length === 0 && folded.length === 0 && inhibited.length === 0,
  );
</script>

<section aria-labelledby="inbox-head">
  <h2 id="inbox-head">What needs you</h2>

  {#if close?.since_last_look}
    <p class="hairline">since you last looked · {close.since_last_look}</p>
  {/if}

  <p class="said" role="status" aria-live="polite">
    {said}
    {#if undo}
      <button class="undo" onclick={takeBack}>{undo.says}</button>
    {/if}
  </p>

  {#if nothingRaised}
    <!-- **The close.** Every board in this category renders an empty list as
         an absence; this is the moment it has the best thing it will ever have
         to say. -->
    <div class="close">
      <p><b>Clear.</b></p>
      {#if close?.quiet}
        <p class="dim">Nothing needed you, and nothing was decided for you.</p>
      {:else}
        {#each close?.sentences ?? [] as s (s)}<p class="dim">{s}</p>{/each}
      {/if}
      {#if close?.next}<p class="dim">next · {close.next}</p>{/if}
      {#if close?.keeps_running}<p class="dim">{close.keeps_running}</p>{/if}
    </div>
  {:else}
    <ul role="list">
      {#each items as i (i.id)}
        <li class="item {i.level}">
          <div class="t">
            {#if i.level === "critical" || i.level === "high"}
              <span class="lvl">{i.level}</span>
            {:else}
              <span class="sr">{i.level}</span>
            {/if}
            <b>{i.title}</b>
            <span class="kind">{i.kind.replace(/_/g, " ")}</span>
            <!-- The word, not the colour: red and amber cannot be separated
                 under deuteranopia at any usable lightness. -->
            {#if i.new_to_you}<span class="unseen">new</span>{/if}
          </div>
          <div class="meta">
            {#if i.project_name}<span>{i.project_name}</span>{/if}
            {#if i.since}<span>{ago(Math.max(0, (Date.now() - Date.parse(i.since)) / 1000))}</span>{/if}
          </div>
          {#if i.detail}<p class="d">{clip(i.detail, 400)}</p>{/if}

          <!-- **The answer path.** The agent named what it will take, so those
               are the buttons.

               They were numbered until the keyboard model was removed, and the
               numbers went with it: `1`–`9` picked an option, and a leading
               digit on a button nobody can press is an affordance that is not
               there. A number that means nothing is worse than no number,
               because somebody will try it. -->
          {#if (i.options ?? []).length > 0 && (i.actions ?? []).includes("choose")}
            <div class="opts">
              {#each i.options ?? [] as o (o.label)}
                {#if o.id}
                  <button class="choice" onclick={() => answer(i, o.id!)}>{o.label}</button>
                {:else}
                  <!-- No id: the provider owns this dialog and only its own
                       window can answer. Shown, because reading them is still
                       worth something, but never dressed up as a button. -->
                  <span class="dead">{o.label}</span>
                {/if}
              {/each}
            </div>
          {/if}
          <!-- **A free-text answer, where the agent asked for one.** Some
               questions have no list: an option set that does not contain the
               real answer is worse than a box. -->
          {#if (i.actions ?? []).includes("reply")}
            <div class="reply">
              <input
                type="text"
                bind:value={typed[i.id]}
                placeholder="your answer"
                aria-label="your answer to: {i.title}"
              />
              <button onclick={() => answer(i, typed[i.id] ?? "")}>reply</button>
            </div>
          {/if}

          <!-- The rule to paste so this is never asked again, and where it
               goes. Printed, never written. -->
          {#if i.offer}
            <p class="offer">
              <span class="dim">never asked again:</span>
              <code>{i.offer.rule}</code>
              <button onclick={() => copyRule(i.offer!.rule)}>copy</button>
              <span class="dim"
                >paste into {i.offer.file} {i.offer.section} · covers {i.offer.covers}{i.offer
                  .more
                  ? "+"
                  : ""} like it</span
              >
            </p>
          {/if}

          <div class="acts">
            {#if (i.actions ?? []).includes("allow")}
              <button onclick={() => answer(i, "allow")}>allow</button>
            {/if}
            {#if (i.actions ?? []).includes("deny")}
              <button onclick={() => answer(i, "deny")}>deny</button>
            {/if}
            {#if (i.actions ?? []).includes("snooze")}
              <button onclick={() => snooze(i)}>snooze 1h</button>
            {/if}
          </div>
        </li>
      {/each}

      <!-- Counted, never hidden. -->
      {#each folded as f (f.kind + (f.project ?? ""))}
        <li class="item folded">
          <div class="t"><b>{f.count} × {f.kind}</b></div>
          <div class="meta"><span>{f.project ?? "across projects"}</span><span>folded — the list is long</span></div>
        </li>
      {/each}
      {#each inhibited as s (s.cause)}
        <li class="item inhibited">
          <div class="t">{s.count} more {plural(s.count, "item", "items")} counted here</div>
          <div class="meta"><span>{s.because}</span></div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  h2 { font-size: var(--t-lg); margin: 0 0 var(--s-1); }
  .hairline, .said { color: var(--dim); font-size: var(--t-xs); margin: 0; }
  .said { display: flex; align-items: center; gap: var(--s-2); min-height: 1.6em; }
  .undo { font-size: var(--t-xs); padding: 1px var(--s-2); }

  ul { list-style: none; margin: var(--s-4) 0 0; padding: 0; display: grid; gap: var(--s-2); }

  /* **A card per item, with the level on its edge.** The list this replaces was
     hairline-separated rows, which is the right density for a table you scan
     and the wrong one for a queue you act on: every row here carries a
     question, a detail and up to six buttons, and without a boundary the
     buttons of one item read as belonging to the one above. */
  .item {
    border: 1px solid var(--line);
    border-left: 3px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
    padding: var(--s-3) var(--s-4);
  }
  /* Colour on the edge, never alone: the word is in `.lvl` beside the title and
     is what a screen reader and a greyscale display get. */
  .item.critical { border-left-color: var(--fail); }
  .item.high { border-left-color: var(--wait); }

  .t { display: flex; gap: var(--s-2); align-items: baseline; flex-wrap: wrap; }
  .t b { font-size: var(--t-md); font-weight: 600; }

  /* The level as a word, in the flow, rather than a glyph needing a legend. */
  .lvl { font-size: var(--t-xs); text-transform: uppercase; letter-spacing: .04em; color: var(--dim); }
  .item.critical .lvl { color: var(--fail); font-weight: 700; }
  .item.high .lvl { color: var(--wait); font-weight: 600; }

  .kind {
    font-size: var(--t-xs);
    color: var(--dim);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 0 var(--s-2);
  }

  .unseen {
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--done);
    border: 1px solid currentColor;
    border-radius: 999px;
    padding: 0 var(--s-2);
  }

  .meta { display: flex; gap: var(--s-3); color: var(--dim); font-size: var(--t-xs); margin-top: var(--s-1); }
  .dim { color: var(--dim); }
  .d { margin: var(--s-2) 0 0; max-width: 78ch; }

  .opts, .acts { display: flex; gap: var(--s-2); flex-wrap: wrap; margin-top: var(--s-3); }
  /* An answer the agent is waiting for is the one thing on this page worth a
     filled button. Everything else stays quiet so this does not have to shout. */
  .choice { border-color: var(--edge); font-weight: 500; }
  .dead { color: var(--dim); font-size: var(--t-sm); align-self: center; }

  .reply { display: flex; gap: var(--s-2); margin-top: var(--s-3); }
  .reply input { flex: 1; max-width: 28rem; }

  .offer {
    margin: var(--s-3) 0 0;
    padding-top: var(--s-3);
    border-top: 1px dashed var(--line);
    display: flex;
    gap: var(--s-2);
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--t-sm);
  }
  .offer code {
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0 var(--s-2);
  }

  /* **The close is the best thing this page ever gets to say**, so it is given
     room rather than rendered as the absence of a list. */
  .close {
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    padding: var(--s-6) var(--s-5);
    margin-top: var(--s-4);
    text-align: center;
  }
  .close p { margin: var(--s-1) 0; }
  .close p:first-child { font-size: var(--t-lg); }

  .sr {
    position: absolute; width: 1px; height: 1px;
    overflow: hidden; clip-path: inset(50%); white-space: nowrap;
  }
</style>
