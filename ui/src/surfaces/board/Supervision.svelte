<script lang="ts">
  // Who is supervising this session.
  //
  // **Three states and only two of them render.** `asks_a_person === true` is
  // the ordinary case and gets no badge; `false` is the one worth seeing;
  // `null` *with a mode* is a mode this build cannot read, which is a question
  // rather than an alarm. No mode at all renders nothing — eleven hook events
  // carry one and the event that fires on every tool call does not, so a busy
  // session may simply not have said yet.
  //
  // The word carries it and the colour helps: red and amber cannot be
  // separated under deuteranopia at any usable lightness.
  let {
    mode = null,
    asksAPerson = null,
  }: { mode?: string | null; asksAPerson?: boolean | null } = $props();
</script>

{#if mode !== null && asksAPerson === false}
  <span class="pm nobody" title="This session decides without you. Devplane reads the mode and never sets it."
    >{mode}</span
  >
{:else if mode !== null && asksAPerson === null}
  <span class="pm unsure" title="A permission mode this build does not recognise, so whether anybody is asked cannot be said."
    >{mode} ?</span
  >
{/if}

<style>
  .pm { font-size: .78rem; padding: 0 .3rem; border-radius: 3px; border: 1px solid var(--line); }
  .nobody { color: var(--fail); font-weight: 700; }
  .unsure { color: var(--wait); }
</style>
