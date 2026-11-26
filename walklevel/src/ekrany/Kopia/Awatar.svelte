<script lang="ts">
  // Awatar osoby: koło w kolorze próbki osoby (kolor danych, wyglad.md 3) z pierwszą literą.
  // Napis na próbce: #0f1f1a jak znacznik na próbce palety (wyglad.md 2.4, 8.14).
  import type { ColorName } from '../../lib/typy';
  import { stan } from '../../lib/stan.svelte';
  import { probka } from '../../lib/wyglad';

  let { nazwa, kolor, rozmiar = 32 }: { nazwa: string; kolor: ColorName; rozmiar?: number } = $props();
  const litera = $derived(nazwa.trim().charAt(0).toUpperCase() || '?');
  const tlo = $derived(probka(kolor, stan.statics?.palette ?? []));
</script>

<span class="awatar" style:width="{rozmiar}px" style:height="{rozmiar}px" style:background={tlo} aria-hidden="true">{litera}</span>

<style>
  .awatar {
    display: grid;
    place-items: center;
    flex: none;
    border-radius: var(--r-round);
    color: #0f1f1a;
    font-size: var(--fs-md);
    font-weight: var(--fw-bold);
    line-height: 1;
    user-select: none;
  }
</style>
