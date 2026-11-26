<script lang="ts">
  // Awatar osoby: koło w kolorze osoby (kolor danych) z pierwszą literą imienia.
  import type { ColorName } from '../../lib/typy';
  import { stan } from '../../lib/stan.svelte';
  import { probka } from '../../lib/wyglad';

  let { imie, kolor, rozmiar = 32 }: { imie: string; kolor: ColorName; rozmiar?: number } = $props();
  const litera = $derived(imie.trim().charAt(0).toLocaleUpperCase('pl-PL'));
</script>

<span
  class="awatar"
  aria-hidden="true"
  style:width="{rozmiar}px"
  style:height="{rozmiar}px"
  style:font-size="{Math.round(rozmiar * 0.45)}px"
  style:background={probka(kolor, stan.statics.palette)}>{litera}</span>

<style>
  .awatar {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: var(--r-round);
    color: var(--on-rarity);
    font-family: var(--font-num);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
</style>
