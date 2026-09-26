<script lang="ts" module>
  /// One GitHub host's sign-in, as `/api/github` says it. Never a token.
  export type HostSignIn = {
    host: string;
    state: "signed_out" | "pending" | "signed_in" | "expired" | "rate_limited" | "unreachable";
    login?: string;
    scopes?: string[];
    user_code?: string;
    verification_uri?: string;
    expires_at?: string;
    until?: string;
    why?: string;
    said?: string;
    last_read?: string | null;
    /// Whether an OAuth App is registered for this host, so the window can
    /// start a sign-in. Without one only a token signs in.
    device_flow?: boolean;
  };

  /// What a host's row says, in one sentence.
  export function hostSays(h: HostSignIn): string {
    switch (h.state) {
      case "signed_in":
        return `signed in as ${h.login ?? "?"}${h.scopes?.length ? ` · ${h.scopes.join(", ")}` : ""}`;
      case "pending":
        return "waiting for the code to be entered at GitHub";
      case "expired":
        return "sign-in expired — GitHub no longer accepts the token, so it was removed";
      case "rate_limited":
        return `signed in; the rate limit is spent until ${h.until ?? "GitHub's reset"}`;
      case "unreachable":
        return `signed in; GitHub unreachable${h.why ? ` — ${h.why}` : ""}`;
      default:
        return "not signed in";
    }
  }
</script>

<script lang="ts">
  // Sign in to GitHub from the window: the code shown large, GitHub's device
  // page as a link, a copy button — and sign out, which says where to revoke
  // the grant at GitHub, because deleting the local copy does not.
  import Icon from "../../lib/ui/Icon.svelte";
  import { safeHref } from "../../lib/href";

  let {
    hosts = [],
    clientId = true,
    busy = "",
    said = "",
    signIn = () => {},
    signOut = () => {},
    copy = () => {},
  }: {
    hosts?: HostSignIn[];
    /// Whether this build or `app.toml` names a GitHub app; without one only
    /// a token signs in.
    clientId?: boolean;
    busy?: string;
    /// What the last action said: a refusal, or where to revoke the grant.
    said?: string;
    signIn?: (host: string) => void;
    signOut?: (host: string) => void;
    copy?: (code: string) => void;
  } = $props();
</script>

<section class="card wide" id="github">
  <h2><Icon name="forge" size={14} /> GitHub</h2>
  {#each hosts as h (h.host)}
    <div class="host" data-state={h.state}>
      <div class="line">
        <b>{h.host}</b>
        <span class:ok={h.state === "signed_in"} class:warn={h.state !== "signed_in"}>{hostSays(h)}</span>
        {#if h.state === "signed_in" || h.state === "rate_limited" || h.state === "unreachable"}
          <button disabled={!!busy} onclick={() => signOut(h.host)}>Sign out</button>
        {:else if h.state !== "pending"}
          <button disabled={!!busy || !(h.device_flow ?? clientId)} onclick={() => signIn(h.host)}>Sign in</button>
        {/if}
      </div>
      {#if h.said}
        <p class="said">{h.said}</p>
      {/if}
      {#if !(h.device_flow ?? clientId) && h.state !== "signed_in"}
        <p class="quiet">No GitHub app is registered for {h.host}, so the window cannot start a sign-in. In a terminal, <code>gh auth token | devplane login github --with-token{h.host === "github.com" ? "" : ` --host ${h.host}`}</code> signs in with GitHub CLI's token, or pipe a fine-grained personal access token to the same command.</p>
      {/if}
      {#if h.state === "pending" && h.user_code}
        <div class="code">
          <p>Open <a href={safeHref(h.verification_uri) ?? "#setup"} target="_blank" rel="noopener noreferrer">{h.verification_uri}</a> and enter:</p>
          <p class="big">{h.user_code}</p>
          <button onclick={() => copy(h.user_code ?? "")}>Copy code</button>
        </div>
      {/if}
    </div>
  {:else}
    <p class="quiet">Reading the sign-in…</p>
  {/each}
  {#if busy}<p class="quiet">{busy}…</p>{/if}
  {#if said}<p class="said">{said}</p>{/if}
  <p class="quiet">The token is kept only in this machine's credential store — never in a file, a log or this page.</p>
</section>

<style>
  .card {
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--panel);
    display: grid;
    gap: var(--s-2);
    align-content: start;
  }
  .card.wide {
    grid-column: 1 / -1;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--faint);
  }
  .host {
    display: grid;
    gap: var(--s-2);
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-3);
    font-size: var(--t-sm);
  }
  .ok {
    color: var(--ink);
  }
  .warn {
    color: var(--wait);
  }
  .code {
    display: grid;
    gap: var(--s-1);
    justify-items: start;
  }
  .code p {
    margin: 0;
  }
  .big {
    font-family: var(--mono);
    font-size: 1.6rem;
    letter-spacing: 0.12em;
    color: var(--ink);
  }
  .quiet,
  .said {
    color: var(--faint);
    font-size: var(--t-sm);
    margin: 0;
  }
  .said {
    color: var(--ink);
  }
  code {
    font-family: var(--mono);
    font-size: var(--t-xs);
  }
</style>
