<script lang="ts" module>
  import type { Rarity } from '../typy';

  /** Heksagon ostrzem w górę, start w górnym wierzchołku, zgodnie z zegarem. */
  function heksagon(r: number): string {
    const p = Array.from({ length: 6 }, (_, i) => {
      const a = ((-90 + 60 * i) * Math.PI) / 180;
      return `${(r * Math.cos(a)).toFixed(4)} ${(r * Math.sin(a)).toFixed(4)}`;
    });
    return `M${p.join('L')}Z`;
  }
  const ZEWN = heksagon(0.8);
  const WEWN = heksagon(0.8 * 0.82);

  /** Poziom -> rzadkość odznaki poziomu (rank_rarity z 0.5.x). */
  export function rzadkoscPoziomu(poziom: number): Rarity {
    return poziom < 10 ? 'pospolite' : poziom < 25 ? 'rzadkie' : poziom < 50 ? 'epickie' : poziom < 80 ? 'heroiczne' : 'legendarne';
  }
</script>

<script lang="ts">
  // Odznaka rzadkości - heksagon (docs/wyglad.md 8.8): płaskie wypełnienie w kolorze rzadkości,
  // jasny obrys i ozdobna linia wewnątrz; znak albo liczba w --on-rarity. Zablokowana: ciemna,
  // postęp jako obrys po obwodzie. Bez gradientu, poświaty i cienia (wyglad.md 1, 8.8, 10).
  import { znak as rysunekZnaku } from './znaki';

  interface Props {
    rzadkosc: Rarity;
    /** Nazwa znaku z definicji osiągnięć (stopy, korona, wieniec42…). */
    znak?: string;
    /** Liczba zamiast znaku (poziom). */
    liczba?: number | string;
    rozmiar?: 34 | 40 | 44 | 58 | 62 | 64 | 70 | number;
    zablokowana?: boolean;
    /** 0..1 - postęp zablokowanej odznaki. */
    postep?: number;
    /** Nazwa dla czytnika ekranu, np. „Epickie, zdobyte”; bez niej odznaka jest ozdobą. */
    etykieta?: string;
  }
  let { rzadkosc, znak, liczba, rozmiar = 58, zablokowana = false, postep = 0, etykieta }: Props = $props();

  const tekstLiczby = $derived(liczba === undefined ? '' : String(liczba));
  const grubosc = $derived(Math.max(4 / rozmiar, 0.1));
  const obrysZablokowanej = $derived(Math.max(3 / rozmiar, 0.07));
  const tresc = $derived(znak && !tekstLiczby ? rysunekZnaku(znak) : '');
</script>

<svg
  class="odznaka"
  class:zablokowana
  viewBox="-1 -1 2 2"
  width={rozmiar}
  height={rozmiar}
  role={etykieta ? 'img' : undefined}
  aria-label={etykieta}
  aria-hidden={etykieta ? undefined : 'true'}
  style:--r="var(--rarity-{rzadkosc})"
  style:--rt="var(--rarity-{rzadkosc}-text)"
>
  {#if zablokowana}
    <path class="tlo" d={ZEWN} stroke-width={obrysZablokowanej} />
    {#if postep > 0}
      <path class="postep" d={ZEWN} pathLength="1" stroke-dasharray="{Math.min(1, postep)} 1" stroke-width={grubosc} />
    {/if}
  {:else}
    <path class="tlo" d={ZEWN} />
    <path class="ozdoba" d={WEWN} />
  {/if}
  {#if tekstLiczby}
    <text class="liczba-odznaki" x="0" y="0.03" font-size={tekstLiczby.length < 3 ? 0.76 : 0.6} text-anchor="middle" dominant-baseline="central">{tekstLiczby}</text>
  {:else if tresc}
    <g class="znak" transform="scale(0.416)">{@html tresc}</g>
  {/if}
</svg>

<style>
  .odznaka {
    flex: none;
    overflow: visible;
    --wyp: var(--r);
    --atrament: var(--on-rarity);
  }
  .tlo { fill: var(--r); stroke: color-mix(in srgb, var(--r) 65%, white); stroke-width: 0.05; }
  :global([data-theme='light']) .tlo { stroke: var(--rt); }
  .ozdoba { fill: none; stroke: rgb(255 255 255 / 0.28); stroke-width: 1px; vector-effect: non-scaling-stroke; }

  .zablokowana { --wyp: var(--surface-2); --atrament: var(--text-2); }
  .zablokowana .tlo,
  :global([data-theme='light']) .zablokowana .tlo { fill: var(--surface-2); stroke: var(--control); }
  .zablokowana .znak,
  .zablokowana .liczba-odznaki { opacity: 0.6; }
  .postep { fill: none; stroke: var(--r); stroke-linecap: round; stroke-linejoin: round; }

  .liczba-odznaki {
    fill: var(--atrament);
    font-family: var(--font-num);
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
  }

  /* Znaki z znaki.ts (wstawiane jako SVG) */
  .znak {
    color: var(--atrament);
    fill: none;
    stroke: currentColor;
    stroke-width: 0.16;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .znak :global(.f) { fill: currentColor; stroke: none; }
  .znak :global(.fs) { fill: currentColor; }
  .znak :global(.d) { fill: var(--wyp); stroke: none; }
  .znak :global(.dl) { stroke: var(--wyp); stroke-width: 0.08; }
  .znak :global(.c) { stroke-width: 0.1; }
  .znak :global(.gr) { stroke-width: 0.22; }
  .znak :global(.kreski) { stroke-width: 0.14; stroke-dasharray: 0.25 0.22; }
  .znak :global(.lawa) { fill: #ff6a2a; stroke: none; }
  .znak :global(.tx) { fill: currentColor; stroke: none; font-family: var(--font-num); font-weight: var(--fw-bold); }
</style>
