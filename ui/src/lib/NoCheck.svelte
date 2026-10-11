<script lang="ts" module>
  /// Whether a change's standing says its project declares no gates: the host
  /// writes `no gates declared` (`standing_says`) or the standing `no_gates_declared`.
  export function declaresNoCheck(change: { standing?: { standing?: string } | null; standing_says?: string | null } | null | undefined): boolean {
    if (!change) return false;
    return change.standing?.standing === "no_gates_declared" || /^no gates declared/i.test(change.standing_says ?? "");
  }
</script>

<script lang="ts">
  // What a change in a project with no `check` offers instead of *Run gates*:
  // the fact that it cannot become verified, and the one flow that fixes it.
  // Nothing here runs or writes; *Declare a check* opens Setup's flow, where
  // the change to devplane.toml is shown as a diff and confirmed.
  import Icon from "./ui/Icon.svelte";
  import { surfaces } from "./surfaces";

  let {
    project,
    compact = false,
  }: {
    /// The project's id (its root), as the change carries it.
    project: string;
    /// One line, for a header; otherwise a sentence and the action.
    compact?: boolean;
  } = $props();
  /// Whichever surface holds a project's gates; the registry names it.
  const at = $derived(surfaces().find((s) => s.holds === "gates")?.id ?? "");
  const href = $derived(at ? `#${at}/declare/${encodeURIComponent(project)}` : "");
</script>

<p class="nocheck" class:compact role="note">
  <Icon name="gate" size={13} />
  <span>This change cannot become verified until a check is declared.</span>
  {#if href}<a class="declare" {href}>Declare a check</a>{/if}
</p>

<style>
  .nocheck {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    margin: 0;
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .nocheck.compact {
    font-size: var(--t-xs);
  }
  .declare {
    padding: 0.2rem 0.6rem;
    border: 1px solid var(--accent);
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
    text-decoration: none;
  }
</style>
