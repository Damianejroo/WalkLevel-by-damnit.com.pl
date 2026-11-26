<script lang="ts">
  // Rząd przycisków, który się zawija (docs/wyglad.md 7.2 pkt 2): dzieci `flex: 1 1 160px`,
  // a gdy napis któregoś się nie mieści - przechodzą pod spód (zamiast ucinać napis).
  // Dziecko z klasą `wasko` ma swoją szerokość.
  import type { Snippet } from 'svelte';

  let { children }: { children: Snippet } = $props();
</script>

<div class="rzad">{@render children()}</div>

<style>
  .rzad { display: flex; flex-wrap: wrap; gap: var(--sp-2); min-width: 0; }
  .rzad.rzad > :global(*) { flex: 1 1 160px; min-width: fit-content; }
  .rzad.rzad > :global(.wasko) { flex: 0 0 auto; }
</style>
