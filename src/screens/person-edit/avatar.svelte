<script lang="ts">
  // Awatar osoby: koło w kolorze osoby (odcień „d” - dane, nie akcent) z pierwszą literą nazwy.
  import { stan } from '../../lib/state.svelte';
  import { probka } from '../../lib/theme';
  import type { ColorName } from '../../lib/contract';

  interface Props {
    nazwa: string;
    kolor: ColorName;
    rozmiar?: number;
  }
  let { nazwa, kolor, rozmiar = 52 }: Props = $props();
  const litera = $derived(Array.from(nazwa.trim())[0]?.toLocaleUpperCase('pl-PL') ?? '?');
</script>

<span
  class="awatar"
  style:--rozmiar="{rozmiar}px"
  style:background={probka(kolor, stan.statics.palette)}
  aria-hidden="true">{litera}</span>

<style>
  .awatar {
    display: grid;
    place-items: center;
    flex: none;
    width: var(--rozmiar);
    height: var(--rozmiar);
    border-radius: var(--r-round);
    /* litera na próbce w odcieniu „d” - ciemna w obu motywach (design.md 8.14) */
    color: #0f1f1a;
    font-size: calc(var(--rozmiar) * 0.42);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
</style>
