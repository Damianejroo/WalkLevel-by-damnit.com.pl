<script lang="ts">
  // Ikona profilu gry (IconRef): obrazek z gry albo znak Segoe Fluent w kolorze profilu na
  // neutralnym kwadracie (kolor profilu to dane - nie akcent). Bez ikony: awatar osoby -
  // koło w kolorze osoby z pierwszą literą imienia.
  import type { ColorName, IconRef } from '../../lib/contract';
  import { stan } from '../../lib/state.svelte';

  interface Props {
    ikona?: IconRef | null;
    /** Awatar osoby (gdy brak ikony profilu). */
    osoba?: { name: string; color: ColorName } | null;
    rozmiar?: number;
  }
  let { ikona = null, osoba = null, rozmiar = 36 }: Props = $props();

  const kolor = (nazwa: ColorName) =>
    stan.statics.palette.find((k) => k.name === nazwa) ?? stan.statics.palette.find((k) => k.name === stan.statics.defaultColor);
  const k = $derived(kolor(ikona?.color ?? osoba?.color ?? stan.statics.defaultColor));
</script>

{#if ikona?.url}
  <img class="obrazek" src={ikona.url} alt="" width={rozmiar} height={rozmiar} style:--r="{rozmiar}px" />
{:else if ikona}
  <span
    class="znak"
    aria-hidden="true"
    style:--r="{rozmiar}px"
    style:--k-d={k?.dark}
    style:--k-l={k?.light}>{ikona.glyph ?? stan.statics.defaultGlyph}</span
  >
{:else if osoba}
  <span class="awatar" aria-hidden="true" style:--r="{rozmiar}px" style:background={k?.dark}>
    {(osoba.name.trim()[0] ?? '?').toUpperCase()}
  </span>
{/if}

<style>
  .obrazek,
  .znak,
  .awatar {
    display: grid;
    place-items: center;
    width: var(--r);
    height: var(--r);
    flex: none;
  }
  .obrazek { border-radius: calc(var(--r) * 0.24); object-fit: cover; }
  .znak {
    border-radius: calc(var(--r) * 0.24);
    background: var(--surface-2);
    color: var(--k-d);
    font-family: var(--font-icon);
    font-size: calc(var(--r) * 0.5);
    line-height: 1;
  }
  :global(:root[data-theme='light']) .znak { color: var(--k-l); }
  .awatar {
    border-radius: var(--r-round);
    color: var(--on-rarity); /* ciemna litera na żywym kolorze (jak w 0.5.x) */
    font-size: calc(var(--r) * 0.44);
    font-weight: var(--fw-bold);
    line-height: 1;
  }
</style>
