<script lang="ts">
  // Commands the host handed back for the person to run, shown whole and
  // copyable. "Copied" is said only when the clipboard took them; otherwise
  // the text stays selectable on screen.
  import { copyText } from "./resource.svelte";

  let { commands }: { commands: string[] } = $props();
  let copied = $state<"" | "yes" | "no">("");
  async function copy() {
    copied = (await copyText(commands.join("\n"))) ? "yes" : "no";
  }
</script>

<div class="commands">
  <pre>{commands.join("\n")}</pre>
  <button type="button" onclick={copy}>Copy</button>
  {#if copied === "yes"}<span class="note" role="status">Copied.</span>{/if}
  {#if copied === "no"}<span class="note" role="status">Nothing was copied — this page has no clipboard; select the text.</span>{/if}
</div>

<style>
  .commands {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--s-2);
    margin: var(--s-2) 0;
  }
  pre {
    flex: 1 1 24rem;
    margin: 0;
    padding: var(--s-2) var(--s-3);
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-family: var(--mono);
    font-size: var(--t-xs);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: all;
  }
  .note {
    flex-basis: 100%;
    font-size: var(--t-xs);
    color: var(--dim);
  }
</style>
