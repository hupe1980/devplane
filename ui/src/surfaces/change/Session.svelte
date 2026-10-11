<script lang="ts" module>
  /// What a driven run's record says about its session, beside the run.
  export type SessionFacts = {
    notices?: Array<{ severity: string; title: string; description?: string | null; at: string }>;
    session_closed?: boolean | null;
    tools_not_offered?: boolean;
    sign_in?: Array<{ id: string; name: string; description?: string | null; kind: string }> | null;
  };
</script>

<script lang="ts">
  // The session as the protocol reported it: the agent's notices (its own
  // words, quoted), how the session ended, whether Devplane's tools reached it,
  // and — when the agent needs signing in — its own sign-in methods. A
  // terminal method is a command the person runs; an agent method is asked for
  // only when the person presses it. Nothing here takes a credential.
  import { api } from "../../lib/api";
  import { failure } from "../../lib/resource.svelte";

  let { facts, change, reload }: { facts: SessionFacts | null | undefined; change: string; reload: () => void } = $props();
  let said = $state("");
  let busy = $state(false);

  async function signIn(method: string) {
    busy = true;
    try {
      await api(`/api/changes/${encodeURIComponent(change)}/login`, { method: "POST", body: JSON.stringify({ method }) });
      said = "Signed in; the change started again.";
      reload();
    } catch (e) {
      said = `That did not land: ${failure(e).says}`;
    } finally {
      busy = false;
    }
  }
</script>

{#if facts}
  {#if facts.sign_in && facts.sign_in.length > 0}
    <h2>Needs signing in</h2>
    <ul class="signin">
      {#each facts.sign_in as m (m.id)}
        <li>
          <q>{m.name}</q>{#if m.description}<span class="quiet"> — <q>{m.description}</q></span>{/if}
          {#if m.kind === "terminal"}
            <code>devplane change login {change} --method {m.id}</code>
          {:else if m.kind === "agent"}
            <button type="button" disabled={busy} onclick={() => signIn(m.id)}>Sign in with <q>{m.name}</q></button>
          {:else}
            <span class="quiet">a kind of sign-in Devplane does not act on</span>
          {/if}
        </li>
      {/each}
    </ul>
    {#if said}<p class="quiet" role="status">{said}</p>{/if}
  {/if}
  {#if (facts.notices ?? []).length > 0}
    <h2>The agent says <span>{facts.notices?.length}</span></h2>
    <ul>
      {#each facts.notices ?? [] as n, i (i)}
        <li class="notice {n.severity}"><b>{n.severity}</b> <q>{n.title}</q>{#if n.description}<span class="quiet"> — <q>{n.description}</q></span>{/if}</li>
      {/each}
    </ul>
  {/if}
  {#if facts.tools_not_offered}<p class="quiet">The agent was not given Devplane's tools.</p>{/if}
  {#if facts.session_closed === true}<p class="quiet">The session was closed by the protocol.</p>
  {:else if facts.session_closed === false}<p class="quiet">The session ended without a close.</p>{/if}
{/if}

<style>
  h2 {
    margin: var(--s-3) 0 var(--s-1);
    font-size: var(--t-sm);
  }
  h2 span {
    color: var(--faint);
    font-weight: normal;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
    font-size: var(--t-sm);
  }
  .signin li {
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
  }
  .notice.warning b {
    color: var(--wait);
  }
  .notice.error b {
    color: var(--fail);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
    user-select: all;
  }
  .quiet {
    color: var(--faint);
    font-size: var(--t-xs);
  }
</style>
