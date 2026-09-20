<script lang="ts">
  // Scaffold only. The four existing surfaces are ported behind the gate, and
  // nothing product-shaped belongs here until the surface registry exists.
  let surfaces = $state<string[]>([]);
</script>

<main>
  <h1>Devplane</h1>
  <p>{surfaces.length} surfaces registered</p>
</main>
