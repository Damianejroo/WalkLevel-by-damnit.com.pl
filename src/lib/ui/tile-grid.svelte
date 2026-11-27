<script lang="ts">
  // Siatka kafelków statystyk (docs/design.md 7.3): 4 w rzędzie od 4 x 110 + 3 x 8 px kontenera,
  // węziej 2 x 2 (wysokość kafelka bez zmian).
  import type { Snippet } from 'svelte';
  let { children }: { children: Snippet } = $props();
</script>

<div class="kontener"><div class="siatka">{@render children()}</div></div>

<style>
  .kontener { container-type: inline-size; min-width: 0; }
  .siatka { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--sp-2); }
  @container (min-width: 464px) {
    .siatka { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  }
</style>
