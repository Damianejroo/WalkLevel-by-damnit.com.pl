<script lang="ts">
  // Statystyki z prawej w dwóch liniach (docs/design.md 8.11, 9.4): „12 345” krojem liczb +
  // „kroków” mniejszym, pod spodem „12,3 km” (albo inna druga linia). Do prawej, bez zawijania
  // liczby w pół.
  interface Props {
    liczba: string;
    jednostka: string;
    dol?: string;
  }
  let { liczba, jednostka, dol = '' }: Props = $props();
</script>

<span class="statystyki">
  <span class="gora"><span class="wartosc liczba">{liczba}</span> <span class="jednostka">{jednostka}</span></span>
  {#if dol}<span class="dol">{dol}</span>{/if}
</span>

<style>
  .statystyki { display: flex; flex-direction: column; align-items: flex-end; min-width: 0; text-align: end; }
  .gora { line-height: var(--lh-tight); }
  .wartosc { font-size: var(--fs-base); white-space: nowrap; }
  .jednostka,
  .dol { font-size: var(--fs-sm); color: var(--text-2); white-space: nowrap; }
  .dol { line-height: var(--lh-tight); }
</style>
