<script lang="ts">
  // One inbox item and every control it offers. The actions themselves
  // (`answer`, `act`, `snooze`) belong to the list, which owns the one
  // result sentence and the one undo.
  import State from "../../lib/State.svelte";
  import { clip } from "../../lib/text";
  import Qualifier from "../../lib/Qualifier.svelte";
  import { unreadSays } from "../change/review/unread.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import { safeHref } from "../../lib/href";
  import { spell } from "../../lib/keys";
  import { answerItemKeys } from "./keys";
  import { copyCommand, untilActivity } from "./actions";
  import type { ReadyFacts } from "../../wire/ReadyFacts";

  /// How much of a detail the row shows; the rest is in the `title`.
  const DETAIL_CHARS = 400;

  export type Choice = { id: string | null; label: string };
  /// One question of a form, as `core::question` sends it.
  export type FormQuestion = {
    field: string;
    title: string;
    options: Array<{ value: string; label: string; detail: string | null }>;
    custom_field: string | null;
  };
  export type Item = {
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
    /// Printed, never written: rules are reviewed files, and an agent runs as
    /// the same user.
    offer?: { rule: string; file: string; section: string; covers: number; more?: boolean } | null;
    /// Why there is no rule to offer, on a permission that has none.
    no_offer?: { reason: string; sentence: string } | null;
    /// The whole form behind a question; `options` is only the first
    /// question's buttons.
    form?: FormQuestion[] | null;
    /// A deep link that opens an agent with a prompt typed and not sent.
    launch?: string | null;
    change_id?: string | null;
    project_id?: string | null;
    /// Where `open_pr` and `open_issue` go.
    url?: string | null;
    /// Why there is no yes-or-no, when there is not.
    answer_in?: string | null;
    new_to_you?: boolean;
    since?: string;
    /// On a *ready to decide* row: what its review leads with.
    facts?: ReadyFacts | null;
    /// On a dialog the vendor owns: the command that resumes its session.
    resume_command?: string | null;
    /// On a dialog the vendor owns, where the vendor documents one: a link
    /// that opens the session.
    session_link?: string | null;
  };

  /// The bodies the answer route accepts. A body with none of these fields
  /// would be recorded as a deny.
  export type Answer =
    | { decision: "allow" | "deny" }
    | { option: string; field?: string }
    | { custom: string; field?: string }
    | { answers: { field: string; option?: string; custom?: string }[] };

  let {
    item,
    /// Whether the list's cursor is on this row: a margin bar, not colour alone.
    current = false,
    /// How long it has waited, from the list's clock; absolute time on hover.
    age = "",
    answer,
    act,
    snooze,
    snoozeHour = snooze,
    copyRule,
    say,
    keys = "",
  }: {
    item: Item;
    current?: boolean;
    age?: string;
    answer: (item: Item, what: Answer) => Promise<void>;
    act: (item: Item, action: string) => Promise<void>;
    snooze: (item: Item) => Promise<void>;
    /// The keyless snooze beside `s`: for an hour.
    snoozeHour?: (item: Item) => Promise<void>;
    copyRule: (rule: string) => Promise<void>;
    /// Reports a refusal this row makes on its own, such as an empty reply.
    say: (sentence: string) => void;
    /// The surface whose item keys this row answers (`inbox`, `answer`);
    /// empty for a row that takes no keys. The keys are printed on the
    /// controls either way.
    keys?: string;
  } = $props();

  const acts = $derived(item.actions ?? []);
  /// Hunks a person marked seen, from the host's marks — the count the
  /// review, the CLI and the offer read. Absent in an older recording.
  const markedHunks = $derived(item.facts ? Math.min(item.facts.hunks_seen ?? 0, item.facts.hunks) : 0);
  const has = (a: string) => acts.includes(a);
  /// The host's links, only with a scheme `lib/href` allows.
  const url = $derived(safeHref(item.url));
  const launch = $derived(safeHref(item.launch));

  /// Critical and high carry the *needs you* diamond; the rest the idle ring,
  /// with the level word kept for screen readers.
  const loud = $derived(item.level === "critical" || item.level === "high");

  /// Free-text answers: one box per row and per field.
  let typed = $state("");
  let custom = $state<Record<string, string>>({});

  /// An empty reply is refused here rather than sent.
  function reply(field?: string) {
    const words = (field ? (custom[field] ?? "") : typed).trim();
    if (!words) {
      say("type your answer first — an empty reply is not an answer");
      return;
    }
    if (field) {
      pick(field, { custom: words });
    } else {
      void answer(item, { custom: words });
    }
  }

  /// The form, when there is one, replaces the flat option list.
  const form = $derived((item.form ?? []).length > 0 ? (item.form ?? []) : null);

  /// A multi-question form is answered once: picks are kept until every field
  /// has one, then sent in one body. A one-field form sends on the pick.
  let picked = $state<Record<string, { option?: string; custom?: string }>>({});
  function pick(field: string, what: { option?: string; custom?: string }) {
    const fields = form ?? [];
    if (fields.length <= 1) {
      void answer(item, what.custom !== undefined ? { custom: what.custom, field } : { option: what.option!, field });
      return;
    }
    picked = { ...picked, [field]: what };
    // The last field picked sends the whole form: one key per field, and the
    // hold still lets the answer be taken back.
    if (fields.every((q) => picked[q.field])) send();
  }
  const outstanding = $derived((form ?? []).filter((q) => !picked[q.field]).length);
  /// The field `1`–`9` answer: the first one not yet picked.
  const keyedField = $derived((form ?? []).find((q) => !picked[q.field])?.field ?? null);
  /// The numbers printed on the flat list's options: only the ones that can
  /// be chosen are counted, so `2` is the second button.
  const optionKey = (index: number) => {
    const n = (item.options ?? []).slice(0, index + 1).filter((o) => o.id).length;
    return n <= 9 ? String(n) : null;
  };
  const answerable = $derived((item.options ?? []).filter((o) => o.id).length);
  /// An option's number among the ones that can be chosen — its row action.
  const optionNumber = (index: number) => (item.options ?? []).slice(0, index + 1).filter((o) => o.id).length;

  /// `w`: the vendor's documented link where there is one, else the resume
  /// command copied — and the control says which.
  const sessionLink = $derived(safeHref(item.session_link));
  const copyAttach = async () => say(await copyCommand(`devplane run attach ${item.run_id}`, ""));
  const copyResume = async () => item.resume_command && say(await copyCommand(item.resume_command, " — run it in a terminal to answer there"));

  // The item keys press the control they are printed on, so a key and its
  // button can never do two different things; a control that is not here
  // makes the key say why, in place.
  let root = $state<HTMLLIElement | undefined>();
  $effect(() => (keys ? answerItemKeys(keys, () => root, () => item, say) : undefined));
  function send() {
    void answer(item, {
      answers: (form ?? []).map((q) => ({ field: q.field, ...picked[q.field] })),
    });
  }
</script>

<li bind:this={root} class="item" class:loud class:current id={item.ask ? `row-${item.ask}` : undefined} aria-current={current ? "true" : undefined}>
  <span class="lvl">
    {#if loud}
      <State state="needs you" word={item.level} />
    {:else}
      <State state="idle" word={item.level} quiet />
    {/if}
  </span>

  <div class="body">
    <div class="t">
      <b>{item.title}</b>
      <span class="kind">{item.kind.replace(/_/g, " ")}</span>
      <!-- Raised since the last look: a word, not a colour. -->
      {#if item.new_to_you}<span class="unseen">new</span>{/if}
      <span class="meta">
        {#if item.project_name}<span>{item.project_name}</span>{/if}
        {#if age}<span title={item.since}>{age}</span>{/if}
      </span>
    </div>
    <!-- Detail is clipped, whole in the title. -->
    {#if item.detail}
      <p class="d" title={item.detail.length > DETAIL_CHARS ? item.detail : undefined}>
        <Inline text={clip(item.detail, DETAIL_CHARS)} />
      </p>
    {/if}

    <!-- A change ready to decide: what its review leads with, as numbers with
         their words. The primary action opens the review; there is no offer. -->
    {#if item.facts}
      <p class="facts">
        <Qualifier q={item.facts.qualifier} />
        <span class="unread" title="hunks nobody marked seen, and weakened rows nobody has read">· {unreadSays({ hunks: item.facts.hunks - markedHunks, weakened: item.facts.qualifier.unseen })}</span>
        {#each item.facts.says as s (s)}<span>· <Inline text={s} /></span>{/each}
      </p>
    {/if}

    <!-- The answers the agent named are the buttons; a form renders every
         question under its own field. -->
    {#if form && (has("choose") || has("reply"))}
      {#each form as q (q.field)}
        <fieldset class="q">
          <legend>{q.title}</legend>
          <div class="opts">
            <!-- Keyed by position: two options may share a label. -->
            {#each q.options as o, oi (oi)}
              {@const k = q.field === keyedField && oi < 9 ? String(oi + 1) : null}
              <span class="keyed"
                ><button
                  class="choice"
                  class:picked={picked[q.field]?.option === o.value}
                  title={o.detail ?? undefined}
                  data-key={k}
                  data-action="option:{q.field}:{oi}"
                  onclick={() => pick(q.field, { option: o.value })}>{o.label}</button
                >{#if k}<kbd>{k}</kbd>{/if}</span
              >
            {/each}
          </div>
          {#if q.custom_field}
            <div class="reply">
              <input
                type="text"
                bind:value={custom[q.field]}
                placeholder="your answer"
                aria-label="your answer to: {q.title}"
                data-key={q.field === keyedField ? "t" : null}
                data-action="reply:{q.field}"
              />
              <button data-send onclick={() => reply(q.field)}>reply</button><kbd title="sends from inside the field">{spell("Mod+Enter")}</kbd>
              {#if q.field === keyedField}<kbd title="puts you in this field">t</kbd>{/if}
            </div>
          {/if}
        </fieldset>
      {/each}
      {#if form.length > 1}
        <div class="reply">
          <button data-action="send_form" onclick={send} disabled={outstanding > 0}>
            {outstanding > 0 ? `${outstanding} still to answer` : `send ${form.length} answers`}
          </button>
        </div>
      {/if}
    {:else}
      {#if (item.options ?? []).length > 0 && has("choose")}
        <div class="opts">
          {#each item.options ?? [] as o, oi (oi)}
            {#if o.id}
              {@const k = optionKey(oi)}
              <span class="keyed"
                ><button class="choice" data-key={k} data-action="option:{optionNumber(oi)}" onclick={() => answer(item, { option: o.id! })}>{o.label}</button
                >{#if k}<kbd>{k}</kbd>{/if}</span
              >
            {:else}
              <!-- No id: only the provider's own window can answer, so it is
                   shown but not a button. -->
              <span class="dead">{o.label}</span>
            {/if}
          {/each}
        </div>
        {#if answerable > 9}
          <p class="elsewhere">1–9 choose the first nine; Tab reaches the rest, and Enter chooses one.</p>
        {/if}
      {/if}
      <!-- A free-text answer, where the agent asked for one. -->
      {#if has("reply")}
        <div class="reply">
          <input
            type="text"
            bind:value={typed}
            placeholder="your answer"
            aria-label="your answer to: {item.title}"
            data-key="t"
            data-action="reply"
          />
          <button data-send onclick={() => reply()}>reply</button><kbd title="sends from inside the field">{spell("Mod+Enter")}</kbd>
          <kbd title="puts you in this field">t</kbd>
        </div>
      {/if}
    {/if}

    <!-- The rule to paste, printed, never written; or the host's reason there is none. -->
    {#if item.offer}
      <p class="offer">
        <span class="dim">To stop being asked, paste this rule:</span>
        <code>{item.offer.rule}</code>
        <button data-action="copy_rule" onclick={() => copyRule(item.offer!.rule)}>copy</button>
        <span class="dim"
          >paste into {item.offer.file} {item.offer.section} · covers {item.offer.covers}{item.offer.more
            ? "+"
            : ""} like it</span
        >
      </p>
    {:else if item.no_offer}
      <p class="elsewhere"><Inline text={item.no_offer.sentence} /></p>
    {/if}

    <!-- A watched session has no protocol request, so nothing here can grant it. -->
    {#if item.answer_in}<p class="elsewhere"><Inline text={item.answer_in} /></p>{/if}
    <!-- The way to the vendor's own dialog: a documented link, else the
         command to run; the control says which it does. Never a window
         raised: no documented channel can. -->
    {#if item.answer_in && (sessionLink || item.resume_command)}
      <p class="session">
        {#if sessionLink}
          <span class="keyed"><a class="act primary" data-key="w" data-action="session" href={sessionLink}>open the session in VS Code</a><kbd>w</kbd></span>
        {:else}
          <span class="keyed"><button class="primary" data-key="w" data-action="session" onclick={copyResume}>copy the resume command</button><kbd>w</kbd></span>
          <code class="cmd">{item.resume_command}</code>
        {/if}
      </p>
    {/if}

    <div class="acts">
      {#if has("allow")}
        <span class="keyed"><button data-key="a" data-action="allow" onclick={() => answer(item, { decision: "allow" })}>allow</button><kbd>a</kbd></span>
      {/if}
      {#if has("deny")}
        <span class="keyed"><button data-key="d" data-action="deny" onclick={() => answer(item, { decision: "deny" })}>deny</button><kbd>d</kbd></span>
      {/if}
      {#if has("retry")}
        <button data-action="retry" onclick={() => act(item, "retry")}>retry</button>
      {/if}
      {#if has("resume")}
        <button data-action="resume" onclick={() => act(item, "resume")}>resume</button>
      {/if}
      {#if has("review") && item.change_id}
        <span class="keyed"><a class="act primary" href={`#change/${encodeURIComponent(item.change_id)}/review`} data-key="r" data-action="review">review</a><kbd>r</kbd></span>
      {/if}
      <!-- A drift: tell the run which files moved, or accept it. Either is recorded. -->
      {#if has("tell_run")}
        <button data-action="tell_run" onclick={() => act(item, "tell_run")}>tell the run</button>
      {/if}
      {#if has("accept_drift")}
        <button data-action="accept_drift" onclick={() => act(item, "accept_drift")}>accept the drift</button>
      {/if}
      {#if has("open") && (item.run_id || item.change_id)}
        <span class="keyed"
          ><a
            class="act"
            data-key="o"
            data-action="open"
            href={item.change_id
              ? `#change/${encodeURIComponent(item.change_id)}`
              : `#why/${encodeURIComponent(item.run_id ?? "")}`}>open</a
          ><kbd>o</kbd></span
        >
      {/if}
      {#if (has("open_pr") || has("open_issue")) && url}
        <a class="act" data-action={has("open_pr") ? "open_pr" : "open_issue"} href={url} target="_blank" rel="noreferrer noopener">
          {has("open_pr") ? "open pull request" : "open issue"}
        </a>
      {/if}
      <!-- A link, never a fetch: the vendor's window opens with the prompt typed. -->
      {#if launch}
        <a class="act" data-action="launch" href={launch}>open an agent with this typed</a>
      {/if}
      <!-- Until the next activity (an hour where there is no session), or an hour. -->
      {#if has("snooze")}
        <span class="keyed"><button data-key="s" data-action="snooze" onclick={() => snooze(item)}>{untilActivity(item) ? "snooze until new activity" : "snooze 1h"}</button><kbd>s</kbd></span>
        {#if untilActivity(item)}<button data-action="snooze_hour" onclick={() => snoozeHour(item)}>for an hour</button>{/if}
      {/if}
      <!-- A browser cannot attach, so show the command, and copy it. -->
      {#if has("attach") && item.run_id}
        <code class="cmd">devplane run attach {item.run_id}</code>
        <button data-action="attach" onclick={copyAttach}>copy</button>
      {/if}
    </div>
  </div>
</li>

<style>
  /* One item in full: the title large, the answer with a firm edge, the rest quiet. */
  .item {
    list-style: none;
    display: grid;
    gap: var(--s-3);
  }
  .lvl {
    display: none;
  }
  .body {
    min-width: 0;
    display: grid;
    gap: var(--s-3);
  }
  .t {
    display: flex;
    gap: var(--s-2);
    align-items: baseline;
    flex-wrap: wrap;
  }
  .t b {
    font-size: 1.3rem;
    font-weight: 650;
    letter-spacing: -0.01em;
    color: var(--ink);
  }
  .kind,
  .meta {
    display: none;
  }
  .unseen {
    font-size: 0.625rem;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--accent);
    border: 1px solid var(--accent);
    border-radius: 999px;
    padding: 0 0.4rem;
  }
  .dim {
    color: var(--dim);
  }
  .d {
    margin: 0;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    font-family: var(--mono);
    font-size: var(--t-sm);
    color: var(--ink);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .opts,
  .acts {
    display: flex;
    gap: var(--s-2);
    flex-wrap: wrap;
  }
  .acts:empty {
    display: none;
  }
  .choice {
    height: 2.2rem;
    padding: 0 var(--s-4);
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--select);
    color: var(--ink);
    font-weight: 600;
  }
  .choice:hover {
    background: var(--accent);
    color: var(--chrome);
  }
  .choice.picked {
    background: var(--accent);
    color: var(--chrome);
  }
  .dead {
    color: var(--dim);
    font-size: var(--t-sm);
    align-self: center;
  }
  .q {
    border: 0;
    border-left: 2px solid var(--accent);
    margin: 0;
    padding: 0 0 0 var(--s-3);
    display: grid;
    gap: var(--s-2);
  }
  .q legend {
    color: var(--ink);
    font-weight: 600;
    font-size: var(--t-sm);
    padding: 0;
    margin-bottom: var(--s-2);
  }
  .act {
    display: inline-flex;
    align-items: center;
    height: 1.9rem;
    padding: 0 var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--ink);
    text-decoration: none;
    font-size: var(--t-sm);
    background: var(--panel);
  }
  .act:hover {
    border-color: var(--edge);
  }
  .act.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .facts {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--s-2);
    font-size: var(--t-sm);
    color: var(--dim);
    font-variant-numeric: tabular-nums;
  }
  .elsewhere {
    margin: 0;
    color: var(--dim);
    font-size: var(--t-sm);
    padding-left: var(--s-3);
    border-left: 2px solid var(--line);
  }
  .cmd {
    align-self: center;
    color: var(--dim);
    font-family: var(--mono);
    font-size: var(--t-xs);
    user-select: all;
    padding: 0.2rem 0.5rem;
    border-radius: var(--radius);
    background: var(--chrome);
  }
  .reply {
    display: flex;
    gap: var(--s-2);
    align-items: center;
  }
  /* A control and the key that presses it, side by side. */
  .keyed {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
  }
  kbd {
    font-family: var(--mono);
    font-size: 0.625rem;
    color: var(--faint);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 0.25rem;
  }
  .session {
    margin: 0;
    display: flex;
    gap: var(--s-2);
    align-items: center;
    flex-wrap: wrap;
  }
  .primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--chrome);
    font-weight: 600;
  }
  .reply input {
    flex: 1;
    max-width: 36rem;
  }
  .offer {
    margin: 0;
    display: flex;
    gap: var(--s-2);
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--t-sm);
    padding: var(--s-3);
    border: 1px dashed var(--line);
    border-radius: var(--radius-lg);
  }
  .offer code {
    font-family: var(--mono);
    font-size: var(--t-xs);
    background: var(--chrome);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 0.1rem var(--s-2);
  }
</style>
