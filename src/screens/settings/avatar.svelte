<script lang="ts">
  // Awatar osoby: koło w kolorze osoby (kolor danych - nie akcent) z pierwszą literą imienia.
  import type { ColorName } from '../../lib/contract';
  import { stan } from '../../lib/state.svelte';
  import { probka } from '../../lib/theme';

  interface Props {
    imie: string;
    kolor: ColorName;
    rozmiar?: number;
  }
  let { imie, kolor, rozmiar = 38 }: Props = $props();
  const litera = $derived([...imie.trim()][0]?.toLocaleUpperCase('pl') ?? '');
</script>

<span
  class="awatar"
  aria-hidden="true"
  style:width="{rozmiar}px"
  style:height="{rozmiar}px"
  style:background={probka(kolor, stan.statics.palette)}>{litera}</span
>

<style>
  .awatar {
    display: grid;
    place-items: center;
    flex: none;
    border-radius: var(--r-round);
    color: var(--on-rarity);
    font-family: var(--font-num);
    font-size: var(--fs-base);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
</style>
