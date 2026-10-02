<script lang="ts">
  // The app was opened while a host it does not own already serves this
  // home. One sentence and one link; it reads no API, and the window it is in
  // started nothing, so closing it quits.
  let { port }: { port: number } = $props();
</script>

<main class="elsewhere">
  <p>A host is already serving this home on port {port}.</p>
  <p class="quiet">It was started outside this app — <code>devplane up</code> in a terminal, say. Its page has everything this window would.</p>
  <p><a class="open" href="http://127.0.0.1:{port}/">Open that page</a></p>
</main>

<style>
  .elsewhere {
    padding: var(--s-4) var(--s-5);
    display: grid;
    gap: var(--s-2);
    font-size: var(--t-sm);
  }
  p {
    margin: 0;
  }
  .quiet {
    color: var(--dim);
  }
  .open {
    font-weight: 600;
  }
</style>
