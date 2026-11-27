<script lang="ts">
  // Oś stopni pada (StepLadder, docs/design.md 8.14): wysokość 96, odcinki stopni od wychylenia
  // progu do następnego, kółka progów do przeciągania, „35%” nad osią, prędkości pod nią,
  // trójkąt - gałka teraz. Neutralnie (w Ustawieniach bez akcentu): odcinki coraz ciemniejsze.
  // Strzałki ±1%, Tab - następny próg. Zmiana idzie do rdzenia po puszczeniu kółka.
  import type { PadStep } from '../../lib/contract';

  interface Props {
    kroki: PadStep[];
    /** Napisy prędkości pod progami (pierwszy - gałka puszczona). */
    predkosci: string[];
    /** Wychylenie gałki 0..1 albo null, gdy pada nie widać. */
    galka: number | null;
    minOdstep: number;
    etykieta: string;
    opisKroku: (indeks: number, procent: number) => string;
    onprog: (indeks: number, procent: number) => void;
  }
  let { kroki, predkosci, galka, minOdstep, etykieta, opisKroku, onprog }: Props = $props();

  let tor: HTMLDivElement;
  let ciagniety = $state<{ i: number; at: number } | null>(null);
  const progi = $derived(kroki.map(([at], i) => (ciagniety?.i === i ? ciagniety.at : at)));
  const ile = $derived(progi.length);

  function granice(i: number): [number, number] {
    const dol = (progi[i - 1] ?? 0) + minOdstep;
    const gora = i === ile - 1 ? 100 : progi[i + 1] - minOdstep;
    return [dol, gora];
  }
  const przytnij = (i: number, v: number) => {
    const [dol, gora] = granice(i);
    return Math.round(Math.min(gora, Math.max(dol, v)));
  };

  function zPozycji(i: number, e: PointerEvent) {
    const r = tor.getBoundingClientRect();
    return przytnij(i, ((e.clientX - r.left) / r.width) * 100);
  }
  function chwyc(i: number, e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    ciagniety = { i, at: progi[i] };
  }
  function ciagnij(i: number, e: PointerEvent) {
    if (ciagniety?.i === i) ciagniety = { i, at: zPozycji(i, e) };
  }
  function pusc(i: number) {
    if (ciagniety?.i !== i) return;
    const at = ciagniety.at;
    ciagniety = null;
    if (at !== kroki[i][0]) onprog(i, at);
  }
  function klawisz(i: number, e: KeyboardEvent) {
    const krok = { ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1, PageUp: 5, PageDown: -5 }[e.key];
    if (!krok) return;
    e.preventDefault();
    const at = przytnij(i, progi[i] + krok);
    if (at !== progi[i]) onprog(i, at);
  }

  /** Napis przy krawędzi nie wychodzi poza oś: 0% do lewej, 100% do prawej. */
  const kotwica = (at: number) => (at <= 4 ? '0%' : at >= 96 ? '-100%' : '-50%');

  // Progi mogą stać 5% od siebie: napis, który nachodziłby na poprzedni widoczny, jest ukryty
  // (wartości są też w wierszach prędkości pod osią). Szerokości mierzone, bez zgadywania.
  let szerToru = $state(0);
  const szerProcentow = $state<number[]>([]);
  const szerPredkosci = $state<number[]>([]);
  function widoczne(szerokosci: number[]): boolean[] {
    let prawy = -Infinity;
    return progi.map((at, i) => {
      const x = (at / 100) * szerToru;
      const w = szerokosci[i] ?? 0;
      const k = kotwica(at);
      const lewy = k === '0%' ? x : k === '-100%' ? x - w : x - w / 2;
      const ok = i === 0 || lewy >= prawy + 6;
      if (ok) prawy = lewy + w;
      return ok;
    });
  }
  const pokazProcent = $derived(widoczne(szerProcentow));
  const pokazPredkosc = $derived(widoczne(szerPredkosci));
</script>

<div class="os" role="group" aria-label={etykieta}>
  <div class="tor" bind:this={tor} bind:clientWidth={szerToru}>
    {#each progi as at, i (i)}
      {@const koniec = progi[i + 1] ?? 100}
      <span
        class="odcinek"
        style:left="{at}%"
        style:width="calc({koniec - at}% - 2px)"
        style:opacity={0.3 + (0.7 * i) / Math.max(1, ile - 1)}
      ></span>
    {/each}
    {#each progi as at, i (i)}
      <span
        class="procent liczba"
        class:ukryty={!pokazProcent[i]}
        style:left="{at}%"
        style:translate="{kotwica(at)} 0"
        bind:offsetWidth={szerProcentow[i]}>{at}%</span
      >
      <span
        class="predkosc liczba jedna-linia"
        class:ukryty={!pokazPredkosc[i]}
        style:left="{at}%"
        style:translate="{kotwica(at)} 0"
        bind:offsetWidth={szerPredkosci[i]}>{predkosci[i] ?? ''}</span
      >
      {#if i > 0}
        <button
          type="button"
          class="kolko"
          class:ciagniety={ciagniety?.i === i}
          style:left="{at}%"
          role="slider"
          aria-label={opisKroku(i, at)}
          aria-valuemin={granice(i)[0]}
          aria-valuemax={granice(i)[1]}
          aria-valuenow={at}
          aria-valuetext="{at}%"
          onpointerdown={(e) => chwyc(i, e)}
          onpointermove={(e) => ciagnij(i, e)}
          onpointerup={() => pusc(i)}
          onpointercancel={() => pusc(i)}
          onkeydown={(e) => klawisz(i, e)}
        ><span class="znak"></span></button>
      {/if}
    {/each}
    {#if galka !== null}
      <!-- telemetria 10-20 Hz: tylko transform (szerokość toru = 100cqw osi - 2 x 18 px) -->
      <span
        class="galka"
        aria-hidden="true"
        style:transform="translateX(calc((100cqw - 36px) * {Math.min(1, Math.max(0, galka))} - 6px))"
      ></span>
    {/if}
  </div>
</div>

<style>
  .os {
    position: relative;
    height: 96px;
    padding-inline: 18px;
    container-type: inline-size;
  }
  .tor {
    position: absolute;
    inset-inline: 18px;
    top: 38px;
    height: 14px;
  }
  .odcinek {
    position: absolute;
    top: 0;
    height: 14px;
    margin-inline-start: 1px;
    border-radius: 4px;
    background: var(--text);
  }
  .procent,
  .predkosc {
    position: absolute;
    font-size: var(--fs-cap);
    line-height: 1;
    white-space: nowrap;
  }
  .procent { top: -24px; color: var(--text-2); font-weight: var(--fw-regular); }
  .predkosc { top: 30px; max-width: 96px; color: var(--text); }
  .kolko {
    position: absolute;
    top: 50%;
    display: grid;
    place-items: center;
    width: 32px;
    height: 44px;
    translate: -50% -50%;
    border-radius: var(--r-md);
    cursor: ew-resize;
    touch-action: none;
  }
  .znak {
    width: 18px;
    height: 18px;
    border: 2px solid var(--text);
    border-radius: var(--r-round);
    background: var(--surface);
    transition: transform var(--t-color) linear;
  }
  .kolko:hover .znak,
  .ciagniety .znak { transform: scale(1.15); }
  .ukryty { visibility: hidden; }
  .galka {
    position: absolute;
    top: 18px;
    left: 0;
    width: 0;
    height: 0;
    border-inline: 6px solid transparent;
    border-bottom: 8px solid var(--text);
    transition: transform var(--t-color) linear;
  }
</style>
