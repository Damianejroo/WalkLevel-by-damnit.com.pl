<script lang="ts">
  // Pasek XP / postępu (docs/design.md 8.9): płaskie wypełnienie scaleX, podziałka 10 odcinków
  // (przerwy w kolorze toru), bez połysku i gradientu. Kolor: --ink (karta poziomu) albo rzadkość.
  interface Props {
    /** 0..1 */
    wartosc: number;
    wysokosc?: 8 | 12;
    /** Liczba odcinków podziałki (0 - bez podziałki). */
    odcinki?: number;
    /** Kolor wypełnienia (CSS), domyślnie --ink. */
    kolor?: string;
    /** Opis dla czytnika ekranu, np. „1 250 z 2 000 XP”. */
    opis: string;
  }
  let { wartosc, wysokosc = 12, odcinki = 10, kolor = 'var(--ink)', opis }: Props = $props();
  const v = $derived(Math.min(1, Math.max(0, Number.isFinite(wartosc) ? wartosc : 0)));
</script>

<div
  class="pasek"
  style:height="{wysokosc}px"
  role="progressbar"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(v * 100)}
  aria-valuetext={opis}
>
  <span class="wypelnienie" style:background={kolor} style:transform="scaleX({v})"></span>
  {#if odcinki > 1}
    <span class="podzialka" aria-hidden="true" style:--odcinki={odcinki}>
      {#each { length: odcinki - 1 } as _, i (i)}<span></span>{/each}
    </span>
  {/if}
</div>

<style>
  .pasek {
    position: relative;
    min-width: 0;
    border: 1px solid var(--control);
    border-radius: var(--r-xs);
    background: var(--bg);
    overflow: hidden;
  }
  .wypelnienie {
    position: absolute;
    inset: 0;
    transform-origin: left;
    transition: transform var(--t-gauge) var(--ease-enter);
  }
  .podzialka {
    position: absolute;
    inset: 0;
    display: flex;
    justify-content: space-evenly;
  }
  .podzialka > span { width: 2px; background: var(--bg); }
</style>
