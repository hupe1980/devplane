<script lang="ts" module>
  export type Row = { title: string; url: string; project_name: string; needs_you: boolean; number?: number; author?: string };
  export type Coverage = { projects: number; configured: number };
  /// One GitHub host's sign-in, as `/api/forge` says it. Never a token.
  export type SignIn = {
    state: "signed_out" | "pending" | "signed_in" | "expired" | "rate_limited" | "unreachable" | "not_github" | "unknown";
    host?: string;
    login?: string;
    until?: string;
    why?: string;
    since?: string;
    said?: string;
    user_code?: string;
    verification_uri?: string;
    last_read?: string | null;
  };
  /// A project's GitHub, as `/api/forge` lists it.
  export type ProjectState = {
    project: string;
    project_name: string;
    github: SignIn;
    stale?: string | null;
    read_at?: string | null;
    issues_more?: number;
    pull_requests_more?: number;
  };

  /// The sentence and the one action for a state that is not data. `null`
  /// for signed in: then the lists are the answer.
  export function stateSays(g: SignIn | null | undefined, now = Date.now()): { title: string; body: string; action: "sign_in" | "wait" | "doctor" | "remote"; blocks: boolean } | null {
    if (!g) return null;
    switch (g.state) {
      case "signed_out":
        return { title: "Not signed in to GitHub", body: g.said ? g.said : "Nothing is read from GitHub until you sign in; the token is kept only in this machine's credential store.", action: "sign_in", blocks: true };
      case "pending":
        return { title: "Signing in to GitHub", body: `Enter the code ${g.user_code ?? ""} at ${g.verification_uri ?? "GitHub's device page"}.`, action: "sign_in", blocks: true };
      case "expired":
        return { title: "GitHub sign-in expired", body: "GitHub no longer accepts the token, so it was removed from this machine.", action: "sign_in", blocks: true };
      case "rate_limited":
        return { title: "GitHub's rate limit is spent", body: `It resets at ${clock(g.until)}; Devplane asks again then.`, action: "wait", blocks: false };
      case "unreachable":
        return { title: "GitHub unreachable", body: `${g.why ? `${g.why}. ` : ""}${g.since ? `Since ${clock(g.since)}.` : ""}`.trim(), action: "doctor", blocks: false };
      case "not_github":
        return { title: "Not a GitHub repository", body: g.why ?? "Its git remote is not on a GitHub host this machine signs in to.", action: "remote", blocks: true };
      default:
        void now;
        return null;
    }
  }

  /// `14:02`, in the reader's own time.
  export function clock(at: string | null | undefined): string {
    if (!at) return "a time GitHub did not say";
    const d = new Date(at);
    if (Number.isNaN(d.getTime())) return at;
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }
</script>

<script lang="ts">
  // Every open issue and pull request, what needs you first, read by
  // Devplane's own GitHub sign-in. A state that is not data — not signed in,
  // expired, rate limited, unreachable, not a GitHub repository — is said as
  // its own sentence with its one action, never as a count or an empty list.
  import Grid, { type Column } from "../../lib/ui/Grid.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Empty from "../../lib/ui/Empty.svelte";
  import { safeHref } from "../../lib/href";
  import { ago, listed, plural } from "../../lib/text";

  let {
    issues = [],
    pulls = [],
    loaded = false,
    failed = "",
    tab = "issues",
    coverage = null,
    github = null,
    projects = [],
    readAt = null,
  }: {
    issues?: Row[];
    pulls?: Row[];
    loaded?: boolean;
    failed?: string;
    tab?: "issues" | "pulls";
    coverage?: Coverage | null;
    /// The configured host's sign-in.
    github?: SignIn | null;
    projects?: ProjectState[];
    /// When the host last read GitHub.
    readAt?: string | null;
  } = $props();

  let showing = $state<string>("issues");
  $effect(() => {
    showing = tab;
  });
  const shown = $derived(showing === "issues" ? issues : pulls);
  const absent = $derived(coverage ? Math.max(0, coverage.projects - coverage.configured) : 0);
  const says = $derived(stateSays(github));
  /// Projects whose own state is not data: said by name, never counted in.
  const elsewhere = $derived(projects.filter((p) => p.github?.state === "not_github"));
  const onGithub = $derived(projects.filter((p) => p.github?.state !== "not_github"));
  /// Every project is elsewhere: the page says so rather than *nothing open*.
  const allElsewhere = $derived(projects.length > 0 && onGithub.length === 0);
  /// A state that replaces the lists entirely.
  const blocking = $derived(!!says?.blocks);
  /// The lists are from before the latest attempt failed.
  const stale = $derived(!!says && !says.blocks);
  const more = $derived(projects.reduce((n, p) => n + ((showing === "issues" ? p.issues_more : p.pull_requests_more) ?? 0), 0));
  const age = $derived(readAt ? ago((Date.now() - new Date(readAt).getTime()) / 1000) : "");
  /// Counts only for data: a failure state has none.
  const counted = $derived(loaded && !failed && !blocking && !allElsewhere && !stale);
  let selected = $state<string | null>(null);
  const columns: Column<Row>[] = [
    { key: "you", label: "", width: 36 },
    { key: "title", label: "Title", width: 520, sort: (r) => r.title },
    { key: "project", label: "Project", width: 160, sort: (r) => r.project_name },
    { key: "open", label: "", align: "end" },
  ];
</script>

{#snippet act(kind: "sign_in" | "wait" | "doctor" | "remote")}
  {#if kind === "sign_in"}<a class="act" href="#setup"><Icon name="person" size={13} /> Sign in</a>
  {:else if kind === "wait"}<span class="quiet">nothing to do — it resumes on its own</span>
  {:else if kind === "doctor"}<span class="quiet">check the network, then <code>devplane doctor</code></span>
  {:else}<span class="quiet">add a GitHub remote: <code>git remote add origin git@github.com:owner/name.git</code></span>{/if}
{/snippet}

<div class="page">
  <header class="head">
    <h1>Issues and pull requests</h1>
    {#if coverage && !blocking}<span class="cov">{coverage.configured} of {coverage.projects} projects are on GitHub{absent ? ` — ${absent} not seen here` : ""}</span>{/if}
  </header>
  <Tabs
    tabs={[
      { id: "issues", label: "Issues", icon: "dot", count: counted ? issues.length : null },
      { id: "pulls", label: "Pull requests", icon: "forge", count: counted ? pulls.length : null },
    ]}
    bind:active={showing}
    label="what to show"
  />
  {#if !loaded}
    <p class="quiet">Asking GitHub…</p>
  {:else if failed}
    <Empty icon="alert" title="The forge could not be read" body={failed} />
  {:else if says && blocking}
    <div class="state" data-state={github?.state}>
      <Empty icon="alert" title={says.title} body={says.body}>
        {#snippet action()}{@render act(says.action)}{/snippet}
      </Empty>
    </div>
  {:else if allElsewhere}
    <div class="state" data-state="not_github">
      <Empty icon="forge" title="Not a GitHub repository" body={`${listed(elsewhere.map((p) => p.project_name))} ${plural(elsewhere.length, "has", "have")} no remote on a GitHub host this machine signs in to.`}>
        {#snippet action()}{@render act("remote")}{/snippet}
      </Empty>
    </div>
  {:else}
    {#if says && stale}
      <p class="banner" data-state={github?.state}>
        <Icon name="alert" size={13} /> <b>{says.title}</b> — {says.body}
        {#if age}<span class="stale">stale, read {age} ago</span>{/if}
        {@render act(says.action)}
      </p>
    {/if}
    {#if elsewhere.length}
      <p class="quiet">{listed(elsewhere.map((p) => p.project_name))}: not a GitHub repository.</p>
    {/if}
    {#if shown.length === 0 && stale}
      <p class="quiet">Nothing was read before GitHub stopped answering, so there is no list to show.</p>
    {:else if shown.length === 0}
      <Empty icon="forge" title={showing === "issues" ? "No open issue" : "No open pull request"} body="Across every registered project that has a GitHub remote." limit={absent ? `${absent} projects have no GitHub remote and are not counted.` : ""} />
    {:else}
      <div class="frame">
        <Grid id="forge" {columns} rows={shown} key={(r) => r.url} group={(r) => (r.needs_you ? "Needs you" : "Everything else")} bind:selected open={(r) => {
          const to = safeHref(r.url);
          if (to) window.open(to, "_blank", "noopener");
        }} label={showing}>
          {#snippet cell(r, c)}
            {#if c.key === "you"}{#if r.needs_you}<span class="you" title="needs you"><Icon name="person" size={13} /></span>{/if}
            {:else if c.key === "title"}<span class="t">{r.title}</span>
            {:else if c.key === "project"}<span class="dim">{r.project_name}</span>
            {:else if safeHref(r.url)}<a href={safeHref(r.url)} target="_blank" rel="noopener noreferrer" class="open"><Icon name="external" size={13} /> open</a>{/if}
          {/snippet}
        </Grid>
      </div>
      {#if more > 0}
        <p class="quiet more">{more} more open {showing === "issues" ? plural(more, "issue", "issues") : plural(more, "pull request", "pull requests")} on GitHub — only the newest are read each time.</p>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: var(--s-4) var(--s-5);
    gap: var(--s-3);
    box-sizing: border-box;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: var(--s-4);
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .cov {
    font-size: var(--t-xs);
    color: var(--faint);
  }
  .frame {
    flex: 1;
    min-height: 0;
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
  }
  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--wait);
    font-size: var(--t-sm);
  }
  .stale {
    color: var(--faint);
  }
  .act {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--accent);
    text-decoration: none;
  }
  .you {
    color: var(--wait);
  }
  .t {
    color: var(--ink);
  }
  .dim {
    color: var(--faint);
  }
  .open {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--accent);
    text-decoration: none;
    font-size: var(--t-xs);
  }
  .quiet {
    color: var(--faint);
    margin: 0;
  }
  .more {
    font-size: var(--t-xs);
  }
  code {
    font-family: var(--mono);
  }
</style>
