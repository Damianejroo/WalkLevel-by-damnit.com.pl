<script lang="ts" module>
  /** Okrągła podziałka osi Y (1, 2, 5 × 10^n), najwyżej 5 przedziałów, szczyt min. 100 (axis_step z 0.5.x). */
  export function podzialka(szczyt: number): { krok: number; ile: number } {
    const p = Math.max(szczyt, 100);
    for (let baza = 1; ; baza *= 10) {
      for (const m of [1, 2, 5]) {
        const krok = m * baza;
        const ile = Math.ceil(p / krok);
        if (ile <= 5) return { krok, ile: Math.max(ile, 2) };
      }
    }
  }
</script>

<script lang="ts">
  // Wykres „Kroki w ciągu dnia” / „Kroki dziennie” (docs/wyglad.md 8.15): SVG o szerokości
  // kontenera i stałej wysokości 190, słupki ułożone z części w kolorach profili albo osób
  // (dane - nie akcent), legenda zawijana pod spodem. Przerysowanie tylko przy zmianie
  // szerokości albo danych; słupki rosną od osi tylko przy pierwszym pokazaniu.
  import type { HistoryChart } from '../../lib/typy';
  import { t } from '../../lib/teksty';
  import { liczba } from '../../lib/format';
  import { stan } from '../../lib/stan.svelte';

  interface Props {
    wykres: HistoryChart;
    /** Chwila pobrania danych - bieżąca godzina („teraz”) na wykresie godzinowym. */
    teraz: Date;
  }
  let { wykres, teraz }: Props = $props();

  const WYS = 190;
  const GORA = 8;
  const DOL = 22;
  const WYS_SLUPKOW = WYS - GORA - DOL;

  let szer = $state(0);
  let pole: HTMLDivElement;
  /** Rozmiar pisma osi w px (rośnie z „Dużym tekstem”). */
  let pismo = $state(11);
  $effect(() => {
    void szer;
    void stan.settings.largeText;
    pismo = parseFloat(getComputedStyle(pole).fontSize) || 11;
  });
  /** Przybliżona szerokość napisu osi (bez pomiaru DOM przy każdej zmianie). */
  const dlugosc = (s: string) => s.length * pismo * 0.6;

  const sumy = $derived(wykres.bars.map((b) => b.parts.reduce((a, p) => a + p.steps, 0)));
  const os = $derived(podzialka(Math.max(1, ...sumy)));
  const gora = $derived(os.krok * os.ile);

  function etykietaOsi(v: number): string {
    if (v === 0) return '0';
    if (v < 1000) return liczba(v);
    return t('historia.wykres.tys', { liczba: v % 1000 === 0 ? liczba(v / 1000) : liczba(v / 1000, 1) });
  }
  const linie = $derived(
    Array.from({ length: os.ile + 1 }, (_, i) => {
      const v = os.krok * i;
      return { v, y: GORA + WYS_SLUPKOW * (1 - v / gora), tekst: etykietaOsi(v) };
    }),
  );
  const lewa = $derived(Math.max(46, Math.ceil(Math.max(...linie.map((l) => dlugosc(l.tekst)))) + 10));
  const ile = $derived(wykres.bars.length);
  const slot = $derived(ile ? Math.max(0, szer - lewa) / ile : 0);
  const grubosc = $derived(Math.min(30, slot * 0.6));

  const DNI = $derived(t('historia.wykres.dni_tygodnia').split(' '));
  const godzinaTeraz = $derived(teraz.getHours());

  /** Data słupka dziennego „d.m” z etykiety 'YYYY-MM-DD'. */
  function dzienMiesiac(etykieta: string): string {
    const [, m, d] = etykieta.split('-');
    return `${Number(d)}.${Number(m)}`;
  }
  /** Słupek dzienny „dziś” po dacie (0.6.2: ten tydzień i miesiąc kalendarzowo - dni po dziś
   *  są puste, dziś nie musi być ostatnie). */
  const dzisIso = $derived(
    `${teraz.getFullYear()}-${String(teraz.getMonth() + 1).padStart(2, '0')}-${String(teraz.getDate()).padStart(2, '0')}`,
  );
  const jestDzis = (i: number) => (wykres.hourly ? i === godzinaTeraz : wykres.bars[i]?.label === dzisIso);
  const kiedy = (i: number) =>
    wykres.hourly ? `${Number(wykres.bars[i].label)}:00` : dzienMiesiac(wykres.bars[i].label);

  interface Etykieta { i: number; tekst: string; x: number; kotwica: 'start' | 'middle' | 'end'; dzis: boolean }
  /** Środek napisu osi X (do sprawdzania nachodzenia). */
  const srodekNapisu = (x: number, kotwica: Etykieta['kotwica'], w: number) =>
    kotwica === 'end' ? x - w / 2 : kotwica === 'start' ? x + w / 2 : x;

  const etykiety = $derived.by((): Etykieta[] => {
    if (!slot) return [];
    const kandydaci: Array<{ i: number; tekst: string }> = [];
    const co = ile <= 14 ? 1 : Math.max(1, Math.floor(ile / 7));
    wykres.bars.forEach((b, i) => {
      if (jestDzis(i)) kandydaci.push({ i, tekst: wykres.hourly ? t('historia.wykres.teraz') : t('historia.wykres.dzis') });
      else if (wykres.hourly) {
        if (i % 3 === 0 && Math.abs(i - godzinaTeraz) > 1) kandydaci.push({ i, tekst: 3 * slot < 34 ? String(i) : `${i}:00` });
      } else if (i % co === 0) {
        kandydaci.push({ i, tekst: ile <= 7 && b.weekday !== null ? (DNI[b.weekday] ?? '') : dzienMiesiac(b.label) });
      }
    });
    // „dziś”/„teraz” pierwsze; etykieta nachodząca na już postawioną odpada
    kandydaci.sort((a, b) => Number(jestDzis(b.i)) - Number(jestDzis(a.i)));
    const postawione: Etykieta[] = [];
    for (const k of kandydaci) {
      const w = dlugosc(k.tekst);
      let x = lewa + slot * (k.i + 0.5);
      let kotwica: Etykieta['kotwica'] = 'middle';
      // przy krawędziach napis nie wychodzi poza wykres; z lewej może wejść pod kolumnę osi Y
      // (etykiety osi Y kończą się na linii 0, wyżej) - inaczej druga etykieta odpadałaby
      if (x + w / 2 > szer) {
        x = szer;
        kotwica = 'end';
      } else if (x - w / 2 < 0) {
        x = 0;
        kotwica = 'start';
      }
      const srodek = srodekNapisu(x, kotwica, w);
      const koliduje = postawione.some((p) => {
        const wp = dlugosc(p.tekst);
        return Math.abs(srodekNapisu(p.x, p.kotwica, wp) - srodek) < (w + wp) / 2 + 4;
      });
      if (!koliduje) postawione.push({ ...k, x, kotwica, dzis: jestDzis(k.i) });
    }
    return postawione;
  });

  const slupki = $derived(
    wykres.bars.map((b, i) => {
      const x = lewa + slot * i + (slot - grubosc) / 2;
      let y = GORA + WYS_SLUPKOW;
      const czesci = [...b.parts]
        .sort((a, c) => c.steps - a.steps)
        .map((p) => {
          const h = (WYS_SLUPKOW * p.steps) / gora;
          y -= h;
          return { nazwa: p.name, y, h: Math.max(1, h - 1.5), kolor: wykres.colors[p.name] };
        });
      return { i, x, czesci, podpowiedz: t('historia.wykres.slupek', { kiedy: kiedy(i), kroki: sumy[i] }) };
    }),
  );

  const najwiecej = $derived(sumy.indexOf(Math.max(...sumy)));
  const opis = $derived(
    t('historia.wykres.opis', { kroki: sumy.reduce((a, s) => a + s, 0), kiedy: najwiecej >= 0 ? kiedy(najwiecej) : '' }),
  );

  /** Nazwa w legendzie: najwyżej 14 znaków + „…” (pełna w title). */
  const krotko = (s: string) => (s.length > 15 ? s.slice(0, 14) + '…' : s);
</script>

<div class="pole" bind:this={pole} bind:clientWidth={szer}>
  {#if wykres.empty}
    <p class="pusto">{t('historia.pusto')}</p>
  {:else if szer > 0}
    <svg width={szer} height={WYS} viewBox="0 0 {szer} {WYS}" role="img" aria-label={opis}>
      {#each linie as l (l.v)}
        <line class="siatka" x1={lewa} x2={szer} y1={l.y} y2={l.y} />
        <text class="os" x={lewa - 8} y={l.y} text-anchor="end" dominant-baseline="central">{l.tekst}</text>
      {/each}
      <g class="slupki" style:transform-origin="0 {GORA + WYS_SLUPKOW}px">
        {#each slupki as s (s.i)}
          <g>
            <title>{s.podpowiedz}</title>
            <rect class="trafienie" x={lewa + slot * s.i} y={GORA} width={slot} height={WYS_SLUPKOW} />
            {#each s.czesci as c (c.nazwa)}
              <rect x={s.x} y={c.y} width={grubosc} height={c.h} rx="2" style:fill={c.kolor ?? 'var(--text-2)'} />
            {/each}
          </g>
        {/each}
      </g>
      {#each etykiety as e (e.i)}
        <text class="os" class:dzis={e.dzis} x={e.x} y={WYS - 6} text-anchor={e.kotwica}>{e.tekst}</text>
      {/each}
    </svg>
    <table class="niewidoczne">
      <tbody>
        {#each slupki as s (s.i)}<tr><td>{s.podpowiedz}</td></tr>{/each}
      </tbody>
    </table>
  {/if}
</div>

{#if !wykres.empty && wykres.legend.length}
  <ul class="legenda">
    {#each wykres.legend as nazwa (nazwa)}
      <li class="pozycja" title={nazwa}>
        <span class="kropka" style:background={wykres.colors[nazwa] ?? 'var(--text-2)'}></span>
        <span class="nazwa dane">{krotko(nazwa)}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .pole {
    position: relative;
    height: 190px;
    min-width: 0;
    font-size: var(--fs-cap);
  }
  .pusto {
    display: grid;
    place-items: center;
    height: 100%;
    padding-inline: var(--sp-4);
    color: var(--text-2);
    font-size: var(--fs-md);
    text-align: center;
  }
  .siatka { stroke: var(--line); stroke-width: 1; shape-rendering: crispEdges; }
  .os { fill: var(--text-2); font-size: var(--fs-cap); font-variant-numeric: tabular-nums; }
  .os.dzis { fill: var(--text); font-weight: var(--fw-semibold); }
  .trafienie { fill: transparent; }
  .slupki {
    transform-box: view-box;
    animation: rosnij var(--t-morph) var(--ease-enter);
  }
  @keyframes rosnij {
    from { transform: scaleY(0); }
    to { transform: none; }
  }
  .legenda {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    min-width: 0;
  }
  .pozycja { display: flex; align-items: center; gap: 6px; min-width: 0; max-width: 100%; }
  .kropka { width: 10px; height: 10px; flex: none; border-radius: var(--r-round); }
  .nazwa { min-width: 0; font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
</style>
