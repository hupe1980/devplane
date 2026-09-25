<script lang="ts">
  // The shape of what is coming, and no words: before the first poll a surface
  // claims nothing. The rows do not animate.
  let { rows = 3 }: { rows?: number } = $props();
</script>

<div class="skeleton" aria-busy="true" aria-label="reading">
  {#each { length: rows } as _, i (i)}
    <div class="row"><span class="bar" style:width="{40 + ((i * 23) % 40)}%"></span></div>
  {/each}
</div>

<style>
  .skeleton { display: grid; margin-top: var(--s-4); }
  .row {
    height: 28px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--line);
  }
  .bar {
    display: block;
    height: 0.6em;
    border-radius: 2px;
    background: var(--line);
  }
</style>
