<script lang="ts">
  // Connecting an agent: per vendor, the exact change to its user settings,
  // written only on confirm, and the way back (Disconnect) in the same place.
  // A hook whose binary is gone reads *gate off*, by name. Skipping leaves
  // Devplane a session viewer and installs nothing.
  import { api, Refused } from "../../lib/api";
  import PendingWrite, { type Pending } from "../../lib/PendingWrite.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Inline from "../../lib/ui/Inline.svelte";
  import type { Connection } from "./checklist";

  let {
    agents,
    onchanged,
    onskip,
    firstRun = false,
  }: {
    agents: Connection[];
    onchanged?: () => void;
    onskip?: () => void;
    firstRun?: boolean;
  } = $props();

  /// The two routes, spelled whole so each can be checked against the host.
  const route = (verb: "connect" | "disconnect", agent: string) =>
    verb === "connect" ? `/api/connect/${encodeURIComponent(agent)}` : `/api/disconnect/${encodeURIComponent(agent)}`;

  let open = $state<{ agent: string; verb: "connect" | "disconnect"; pending: Pending } | null>(null);
  let said = $state("");
  let asking = $state("");
  async function preview(agent: string, verb: "connect" | "disconnect") {
    if (asking) return;
    asking = agent;
    said = "";
    try {
      const pending = await api<Pending>(route(verb, agent), { method: "POST", body: JSON.stringify({ confirm: false }) });
      open = { agent, verb, pending };
    } catch (e) {
      const b = e instanceof Refused ? (e.body as { refused?: string; says?: string } | null) : null;
      said = b?.refused === "unstable_path" && b.says ? b.says : e instanceof Error ? e.message : String(e);
    } finally {
      asking = "";
    }
  }
  const shown = $derived(agents.filter((a) => a.present || a.connected));
</script>

<div class="connect">
  {#if shown.length === 0}
    <p class="quiet">
      No agent found on this machine. Looked for {agents.map((a) => `${a.name} (${a.settings_path})`).join(", ")}. Install one — Claude Code with
      <code>npm install -g @anthropic-ai/claude-code</code> — and come back; until then Devplane watches nothing and gates nothing.
    </p>
  {/if}
  {#each shown as a (a.agent)}
    <div class="agent">
      <p class="who">
        <Icon name="agent" size={13} /> <b>{a.name}</b>
        {#if a.gate_off.length}
          <span class="off">gate off for {a.name} — the binary its hooks run is gone ({a.gate_off.join(", ")})</span>
        {:else if a.connected}
          <span class="on">connected</span>
        {:else}
          <span class="quiet">not connected</span>
        {/if}
        <code class="path">{a.settings_path}</code>
      </p>
      {#if open && open.agent === a.agent}
        <PendingWrite
          pending={open.pending}
          url={route(open.verb, a.agent)}
          body={{}}
          confirmLabel={open.verb === "connect" ? `Connect ${a.name}` : `Disconnect ${a.name}`}
          oncancel={() => (open = null)}
          ondone={() => {
            open = null;
            onchanged?.();
          }}
        />
      {:else}
        <div class="actions">
          {#if !a.connected || a.gate_off.length}
            <button type="button" onclick={() => preview(a.agent, "connect")} disabled={!!asking}>
              {a.gate_off.length ? "Connect again — show the change" : "Connect — show the change"}
            </button>
          {/if}
          {#if a.connected}
            <button type="button" onclick={() => preview(a.agent, "disconnect")} disabled={!!asking}>Disconnect — show the change</button>
          {/if}
        </div>
      {/if}
    </div>
  {/each}
  {#if said}<p class="fail" role="alert"><Inline text={said} /></p>{/if}
  {#if onskip}
    <div>
      <button type="button" onclick={onskip} data-first-run-step={firstRun ? "skip-connect" : undefined}>Skip — watch sessions only</button>
    </div>
  {/if}
</div>

<style>
  .connect {
    display: grid;
    gap: var(--s-2);
  }
  .agent {
    display: grid;
    gap: var(--s-2);
    padding: var(--s-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
  p {
    margin: 0;
  }
  .who {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  .path,
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
    color: var(--faint);
    overflow-wrap: anywhere;
  }
  .on {
    color: var(--ink);
  }
  .off {
    color: var(--fail);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-sm);
  }
  .fail {
    color: var(--fail);
    font-size: var(--t-sm);
  }
</style>
