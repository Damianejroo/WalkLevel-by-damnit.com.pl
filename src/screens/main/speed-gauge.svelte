<script lang="ts">
  // Wskaźnik prędkości (docs/design.md 8.14): łuk 240° w SVG o stałym rozmiarze 280 x 186, pasma
  // stref na zewnątrz, kreski i etykiety progów, liczba Bahnschrift w środku. Telemetria (10-20 Hz)
  // zmienia tylko liczbę i długość łuku wartości (stroke-dasharray) - wartość dochodzi do celu
  // w 420 ms w requestAnimationFrame (wyjątek z 6.2: rysowanie w elemencie o stałym rozmiarze).
  import { onDestroy, untrack } from 'svelte';
  import type { GaugeMode } from '../../lib/contract';
  import { liczba } from '../../lib/format';
  import { t } from '../../lib/texts';
  import type { UkladStref } from './zones';

  interface Props {
    uklad: UkladStref;
    /** Prędkość taśmy z telemetrii (km/h). */
    predkosc: number;
    tryb: GaugeMode;
    /** Indeks aktywnej strefy (-1 brak). */
    aktywna: number;
    /** Podpis pod wskaźnikiem - do opisu dla czytnika ekranu. */
    podpis: string;
  }
  let { uklad, predkosc, tryb, aktywna, podpis }: Props = $props();

  const START = 210;
  const ZAKRES = 240;
  const R = 78;
  const CX = 140;
  const CY = 124;

  const kat = (v: number) => START - ZAKRES * Math.max(0, Math.min(1, v / uklad.vmax));
  const punkt = (r: number, deg: number) => {
    const a = (deg * Math.PI) / 180;
    return [CX + r * Math.cos(a), CY - r * Math.sin(a)] as const;
  };
  /** Łuk od kąta a1 do a2 (a1 > a2) zgodnie z ruchem wskazówek zegara. */
  function luk(r: number, a1: number, a2: number): string {
    const [x1, y1] = punkt(r, a1);
    const [x2, y2] = punkt(r, a2);
    const duzy = a1 - a2 > 180 ? 1 : 0;
    return `M${x1.toFixed(2)} ${y1.toFixed(2)}A${r} ${r} 0 ${duzy} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
  }

  const TOR = luk(R, START, START - ZAKRES);
  const pasma = $derived(
    uklad.strefy.map((s, i) => ({ d: luk(R + 14, kat(s.od) - (i ? 1.5 : 0), kat(s.do)), kolor: s.kolor })),
  );
  const progi = $derived(
    uklad.progi.map((p) => {
      const a = kat(p.v);
      const [x1, y1] = punkt(R + 20, a);
      const [x2, y2] = punkt(R + 27, a);
      const [tx, ty] = punkt(R + 40, a);
      return { ...p, x1, y1, x2, y2, tx, ty };
    }),
  );

  // --- wartość animowana w rAF ---
  let wartosc = $state(0);
  let od = 0;
  let cel = 0;
  let poczatek = 0;
  let klatka = 0;
  const CZAS = 420;
  const spokojnie = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  // przybliżenie --ease-enter (szybki start, łagodne dojście)
  const wygladz = (x: number) => 1 - Math.pow(1 - x, 3);

  function krok(teraz: number) {
    const x = Math.min(1, (teraz - poczatek) / CZAS);
    wartosc = od + (cel - od) * wygladz(x);
    klatka = x < 1 ? requestAnimationFrame(krok) : 0;
  }

  $effect(() => {
    const nowa = tryb === 'offline' ? 0 : Math.max(0, predkosc);
    if (Math.abs(nowa - cel) < 0.05 && (nowa !== 0 || cel === 0)) return;
    cel = nowa;
    if (spokojnie) {
      wartosc = nowa;
      return;
    }
    od = untrack(() => wartosc);
    poczatek = performance.now();
    if (!klatka) klatka = requestAnimationFrame(krok);
  });
  onDestroy(() => cancelAnimationFrame(klatka));

  const offline = $derived(tryb === 'offline');
  const ulamek = $derived(Math.max(0, Math.min(1, wartosc / uklad.vmax)));
  const kolorWartosci = $derived(uklad.strefy[aktywna]?.kolor ?? 'stop');
  const opis = $derived(t('glowny.wskaznik.opis_dostepnosc', { kmh: liczba(offline ? 0 : predkosc, 1), podpis }));
</script>

<svg class="wskaznik" class:offline viewBox="0 0 280 186" role="img" aria-label={opis}>
  <title>{t('glowny.wskaznik.dostepnosc')}</title>
  <path class="tor" d={TOR} />
  {#each pasma as p, i (i)}
    <path class="pasmo k-{p.kolor}" class:aktywne={i === aktywna} d={p.d} />
  {/each}
  {#if !offline && wartosc > 0.05}
    <path class="wartosc k-{kolorWartosci}" d={TOR} pathLength="1" stroke-dasharray="{ulamek} 2" />
  {/if}
  {#each progi as p (p.strefa)}
    <line class="kreska" class:aktywne={p.strefa === aktywna} x1={p.x1} y1={p.y1} x2={p.x2} y2={p.y2} />
    <text
      class="etykieta k-{uklad.strefy[p.strefa]?.kolor ?? 'stop'}"
      class:aktywne={p.strefa === aktywna}
      x={p.tx}
      y={p.ty}
      text-anchor="middle"
      dominant-baseline="central">{p.tekst}</text>
  {/each}
  <text class="liczba-wskaznika" x={CX} y={CY - 4} text-anchor="middle" dominant-baseline="central">{liczba(wartosc, 1)}</text>
  <text class="jednostka" x={CX} y={CY + 30} text-anchor="middle" dominant-baseline="central">{t('glowny.wskaznik.jednostka')}</text>
</svg>

<style>
  .wskaznik {
    width: 100%;
    min-width: 0;
    height: 186px;
    overflow: visible;
  }
  /* bez połączenia wskaźnik jest szary: pasma bez koloru stref */
  .offline {
    --zone-walk: var(--text-2);
    --zone-run: var(--text-2);
  }
  .k-stop { --kolor: var(--zone-stop); }
  .k-walk { --kolor: var(--zone-walk); }
  .k-run { --kolor: var(--zone-run); }

  .tor { fill: none; stroke: var(--surface-2); stroke-width: 14; }
  .pasmo { fill: none; stroke: var(--kolor); stroke-width: 5; opacity: 0.32; }
  .pasmo.aktywne { opacity: 1; }
  .wartosc { fill: none; stroke: var(--kolor); stroke-width: 14; stroke-linecap: round; }
  .kreska { stroke: var(--text-2); stroke-width: 1.5; }
  .kreska.aktywne { stroke: var(--text); }
  .etykieta { fill: var(--text-2); font-size: 12px; font-family: var(--font-ui); }
  .etykieta.aktywne { fill: var(--kolor); font-weight: var(--fw-bold); }
  .liczba-wskaznika {
    fill: var(--text);
    font-family: var(--font-num);
    font-size: 44px;
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
  }
  .offline .liczba-wskaznika { fill: var(--text-2); }
  .jednostka { fill: var(--text-2); font-size: 13px; font-family: var(--font-ui); }
</style>
