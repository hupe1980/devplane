<script lang="ts">
  // Update checks: off until the person answers, and the answer is one line
  // of their own app.toml, shown before it is written. An offer names the
  // version and its notes — plain text, never markup — and installs only on
  // *Install and restart*, after saying what a restart stops. Where this
  // install cannot replace itself, it says where to get the new version and
  // offers no install.
  import { api, Refused } from "../../lib/api";
  import { resource } from "../../lib/resource.svelte";
  import PendingWrite, { type Pending } from "../../lib/PendingWrite.svelte";
  import Failed from "../../lib/Failed.svelte";
  import type { UpdateState } from "../../wire/UpdateState";

  let {
    shown,
    onchanged,
  }: {
    /// A state to render instead of the host's, for the render checks.
    shown?: UpdateState;
    /// The preference was written (the first-run checklist re-reads).
    onchanged?: () => void;
  } = $props();

  const read = resource<UpdateState>(() => (shown ? null : "/api/app/update"), { tell: () => "devplane doctor" });
  const s = $derived(shown ?? read.data);

  let pending = $state<{ pref: "on" | "off"; p: Pending } | null>(null);
  let stops = $state<string>("");
  let said = $state("");
  let busy = $state(false);

  async function preview(pref: "on" | "off") {
    said = "";
    try {
      pending = { pref, p: await api<Pending>("/api/app/updates", { method: "POST", body: JSON.stringify({ updates: pref, confirm: false }) }) };
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
    }
  }

  async function install(version: string, confirm = false) {
    if (busy) return;
    busy = true;
    said = "";
    try {
      await api("/api/app/update/install", { method: "POST", body: JSON.stringify({ version, confirm }) });
      stops = "";
    } catch (e) {
      const b = e instanceof Refused ? (e.body as { refused?: string; says?: string; why?: string } | null) : null;
      if (b?.refused === "stops" && b.says) stops = b.says;
      else said = b?.why ?? (e instanceof Error ? e.message : String(e));
    } finally {
      busy = false;
      await read.reload();
    }
  }

  async function dismiss(version: string) {
    said = "";
    try {
      await api("/api/app/update/dismiss", { method: "POST", body: JSON.stringify({ version }) });
    } catch (e) {
      said = e instanceof Error ? e.message : String(e);
    }
    await read.reload();
  }
</script>

<div class="updates" data-update-state={s?.state ?? ""}>
  {#if read.phase === "failed" && read.failure}
    <Failed what="the update check" failure={read.failure} />
  {:else if !s}
    <p class="quiet">Reading…</p>
  {:else if s.state === "not_this_build"}
    <p class="quiet">Update checks are in the desktop app. This page is served by <code>devplane up</code>, which never checks.</p>
  {:else}
    {#if s.state === "not_asked"}
      <p>May Devplane check for a newer version when the app starts?</p>
      <p class="quiet">
        It fetches <code>{s.fetches}</code> once per launch — never on a timer — and sends {s.sends}. Nothing is
        downloaded until you choose <b>Install and restart</b>. Until you answer, nothing is checked.
      </p>
    {:else if s.state === "off"}
      <p>Update checks off. Releases are published at <code>{s.releases}</code>.</p>
    {:else if s.state === "unavailable"}
      <p>{s.why}. Releases are published at <code>{s.releases}</code>.</p>
    {:else if s.state === "checking"}
      <p class="quiet">Checking for a newer version…</p>
    {:else if s.state === "current"}
      <p>Update checks on. Version {s.running} is the newest.</p>
    {:else if s.state === "check_failed"}
      <p>Update checks on. {s.why}; nothing was downloaded.</p>
    {:else if s.state === "untrusted"}
      <p class="fail">The last check found nothing it could trust: {s.why}. Nothing was downloaded.</p>
    {:else if s.state === "offer" || s.state === "dismissed" || s.state === "failed"}
      <p>
        <b>Devplane {s.version}</b> is available{#if s.state === "offer"} (you run {s.running}){/if}. Installing it restarts the app.
      </p>
      {#if s.state === "offer" && s.notes}
        <pre class="notes" aria-label="release notes">{s.notes}</pre>
      {/if}
      {#if s.state === "failed"}<p class="fail">{s.why}</p>{/if}
      {#if stops}
        <pre class="stops" role="alert">{stops}</pre>
        <div class="actions">
          <button type="button" class="primary" onclick={() => install(s.version, true)} disabled={busy}>Stop them, install and restart</button>
          <button type="button" onclick={() => (stops = "")} disabled={busy}>Keep running</button>
        </div>
      {:else}
        <div class="actions">
          <button type="button" class="primary" onclick={() => install(s.version)} disabled={busy}>Install and restart</button>
          {#if s.state === "offer"}<button type="button" onclick={() => dismiss(s.version)}>Not now</button>{/if}
        </div>
      {/if}
    {:else if s.state === "cannot_replace"}
      <p>
        <b>Devplane {s.version}</b> is available, but this copy cannot replace itself: {s.why}. Get it from
        <code>{s.where}</code>.
      </p>
    {:else if s.state === "installing"}
      <p>Downloading and verifying {s.version}. Nothing is replaced unless its signature verifies.</p>
    {/if}

    {#if pending}
      <PendingWrite
        pending={pending.p}
        url="/api/app/updates"
        body={{ updates: pending.pref }}
        confirmLabel={pending.pref === "on" ? "Turn update checks on" : "Turn update checks off"}
        oncancel={() => (pending = null)}
        ondone={() => {
          pending = null;
          void read.reload();
          onchanged?.();
        }}
      />
    {:else if s.state !== "unavailable" && s.state !== "installing"}
      <div class="actions">
        {#if s.state === "not_asked" || s.state === "off"}
          <button type="button" onclick={() => preview("on")}>{s.state === "not_asked" ? "Yes, check — show the change" : "Turn checks on — show the change"}</button>
        {/if}
        {#if s.state !== "off"}
          <button type="button" onclick={() => preview("off")}>{s.state === "not_asked" ? "No — show the change" : "Turn checks off — show the change"}</button>
        {/if}
      </div>
    {/if}
    {#if said}<p class="fail" role="alert">{said}</p>{/if}
  {/if}
</div>

<style>
  .updates {
    display: grid;
    gap: var(--s-2);
  }
  p {
    margin: 0;
  }
  .quiet {
    color: var(--dim);
  }
  .fail {
    color: var(--fail);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .notes,
  .stops {
    margin: 0;
    padding: var(--s-2);
    max-height: 14rem;
    overflow: auto;
    white-space: pre-wrap;
    font-size: var(--t-xs);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
</style>
