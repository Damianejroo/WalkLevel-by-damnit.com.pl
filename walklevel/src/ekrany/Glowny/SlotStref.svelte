<script lang="ts">
  // Slot pod wskaźnikiem o stałej wysokości 56 w każdym trybie (docs/wyglad.md 8.14, uwaga f):
  // żetony stref (Klawisze, Pad, Chodzenie - tylko informacja) albo pasek Treningu (4 poziomy
  // + Stop). Zaznaczenie neutralne: kwadrat w kolorze strefy, ramka, pogrubienie - bez wypełnień.
  import * as api from '../../lib/api';
  import { liczba } from '../../lib/format';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';
  import type { Zeton } from './strefy';

  interface Props {
    trening: boolean;
    zetony: Zeton[];
    /** Aktywna strefa (żetony) - z telemetrii. */
    aktywna: number;
  }
  let { trening, zetony, aktywna }: Props = $props();

  const POZIOMY = [0, 1, 2, 3] as const;
  const poziomy = $derived(
    POZIOMY.map((i) => {
      const kmh = liczba(stan.settings.trainingLevels[i], 1);
      const [numer, predkosc = ''] = t('glowny.trening.poziom', { poziom: i + 1, kmh }).split('\n');
      // jednostka osobno: w wąskim pasku zostaje sama liczba (km/h jest pod liczbą wskaźnika)
      const podzial = predkosc.indexOf(kmh) + kmh.length;
      return { i, numer, liczba: predkosc.slice(0, podzial), jednostka: predkosc.slice(podzial), opis: t('glowny.trening.poziom_dostepnosc', { poziom: i + 1, kmh }) };
    }),
  );
  const wybrany = $derived(stan.control.enabled ? stan.settings.trainingLevel : -1);
</script>

<div class="slot">
  {#if trening}
    <div class="trening">
      {#each poziomy as p (p.i)}
        <button
          type="button"
          class="poziom"
          aria-pressed={p.i === wybrany}
          aria-label={p.opis}
          title={p.opis}
          onclick={() => wykonaj(api.trainingLevelSet(p.i))}
        >
          <span class="numer liczba">{p.numer}</span>
          <span class="predkosc jedna-linia">{p.liczba}<span class="jednostka">{p.jednostka}</span></span>
        </button>
      {/each}
      <button
        type="button"
        class="stop"
        aria-label={t('glowny.trening.stop_dostepnosc')}
        title={t('glowny.trening.stop_dostepnosc')}
        onclick={() => wykonaj(api.trainingStop())}
      >
        <span class="jedna-linia">{t('glowny.trening.stop')}</span>
      </button>
    </div>
  {:else}
    <ul class="zetony" style:--n={zetony.length}>
      {#each zetony as z, i (i)}
        <li class="zeton k-{z.kolor}" class:aktywny={i === aktywna} title="{z.nazwa}: {z.wartosc}">
          <span class="nazwa jedna-linia">
            {#if i === aktywna}<span class="kwadrat" aria-hidden="true"></span>{/if}{z.nazwa}
          </span>
          <span class="wartosc jedna-linia">{z.wartosc}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .slot { height: var(--row-h); min-width: 0; container-type: inline-size; }

  .zetony {
    display: grid;
    grid-template-columns: repeat(var(--n, 3), minmax(0, 1fr));
    gap: 6px;
    height: 100%;
  }
  .zeton {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 1px;
    min-width: 0;
    padding: 0 6px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--fs-sm);
    line-height: var(--lh-tight);
    text-align: center;
  }
  .nazwa { max-width: 100%; font-weight: var(--fw-semibold); }
  .wartosc { max-width: 100%; font-variant-numeric: tabular-nums; }
  .aktywny { border-color: var(--control); color: var(--text); }
  .aktywny .nazwa,
  .aktywny .wartosc { font-weight: var(--fw-bold); }
  .kwadrat {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-inline-end: 6px;
    vertical-align: 0.05em;
    background: var(--kolor);
  }
  .k-stop { --kolor: var(--zone-stop); }
  .k-walk { --kolor: var(--zone-walk); }
  .k-run { --kolor: var(--zone-run); }

  .trening {
    display: grid;
    grid-template-columns: repeat(4, minmax(52px, 1fr)) minmax(72px, auto);
    gap: 6px;
    height: 100%;
  }
  .poziom,
  .stop {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    min-width: 0;
    padding: 0 6px;
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear,
      color var(--t-color) linear;
  }
  .poziom:hover { background: var(--surface-2); }
  .poziom:active { background: var(--surface-3); transition-duration: var(--t-press); }
  .poziom[aria-pressed='true'] {
    background: var(--sel-bg);
    border-color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--ink); /* ramka 2 px bez przesunięcia treści */
  }
  .numer { font-size: var(--fs-base); line-height: 1.1; }
  .predkosc { max-width: 100%; font-size: var(--fs-sm); color: var(--text-2); font-variant-numeric: tabular-nums; line-height: var(--lh-tight); }
  .poziom[aria-pressed='true'] .predkosc { color: var(--text); }
  @container (max-width: 419px) {
    .jednostka { display: none; }
  }
  .stop {
    padding: 0 12px;
    border-color: var(--danger);
    color: var(--danger);
    font-weight: var(--fw-bold);
  }
  .stop > span { max-width: 100%; }
  .stop:hover { background: var(--danger); color: var(--bg); }
  .stop:active { background: color-mix(in srgb, var(--danger) 80%, var(--text)); color: var(--bg); }
</style>
