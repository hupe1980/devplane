<script lang="ts" module>
  import { LIFE, STATES, type Life } from "../../lib/State.svelte";

  /// What comes after each step of a change's life, as the verb a person
  /// takes: never a promise that it will pass.
  /// In `LIFE`'s order, so the state words stay spelled in the one table.
  const VERBS = ["isolate it", "start an agent", "run the gates", "offer", "archive", ""] as const;
  const NEXT = Object.fromEntries(LIFE.map((l, i) => [l, VERBS[i]])) as Record<Life, string>;

  /// Where a change is and what comes next, as one line: *verified → next:
  /// offer*. A state outside the six (stopped, gates failing) is said in its
  /// own word. While a weakened row is unread, what comes next is reading it;
  /// with no `check` declared, there are no gates to run, so it is declaring one.
  export function where(state: string, archived: boolean, offered: boolean, weakUnread = 0, noCheck = false): { at: string; next: string } {
    const at = archived ? STATES.archived.word : offered ? STATES.offered.word : state;
    const step = (LIFE as readonly string[]).includes(at) ? (at as Life) : null;
    if (weakUnread > 0 && at !== STATES.archived.word) return { at, next: `read ${weakUnread} weakened ${weakUnread === 1 ? "check" : "checks"}` };
    const next = step ? NEXT[step] : "";
    return { at, next: noCheck && next === NEXT[LIFE[2]] ? "declare a check" : next };
  }
</script>

<script lang="ts">
  // Where a change is in its six-state life, in one line. The host computes
  // the state; this only names it and the step after it. The word is never
  // green here: the header's state already carries the one verified green.
  let {
    state,
    archived = false,
    offered = false,
    weakUnread = 0,
    noCheck = false,
    qualified = "",
  }: { state: string; archived?: boolean; offered?: boolean; weakUnread?: number; noCheck?: boolean; qualified?: string } = $props();
  const line = $derived(where(state, archived, offered, weakUnread, noCheck));
  // *Verified* is never said alone where a check was weakened: the qualifier
  // travels with the word on this line too.
  const at = $derived(qualified && line.at === STATES.verified.word ? `${qualified} · ${line.at}` : line.at);
</script>

<p class="stepper" aria-label="where this change is" title="drafted → isolated → in flight → verified → offered → archived">
  <span class="at" aria-current="step">{at}</span>{#if line.next}<span class="arrow" aria-hidden="true">→</span><span class="next">next: {line.next}</span>{/if}
</p>

<style>
  .stepper {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35rem;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--dim);
    white-space: nowrap;
  }
  .at {
    color: var(--ink);
    font-weight: 600;
  }
  .arrow {
    color: var(--faint);
  }
</style>
