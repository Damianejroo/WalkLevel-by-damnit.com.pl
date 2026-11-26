<script lang="ts">
  // Przycisk na ekranie (bańka 56 × 56, docs/wyglad.md 9.7, kontrakt 7.3): koło; wyłączone -
  // tło powierzchni i ramka kontrolki; włączone - wypełnienie akcentem (bieg: kolor biegu),
  // ikona postaci i pod nią prędkość taśmy, gdy dane płyną. Jedyny element widoku = jedyny akcent.
  // Tryb Trening: obok kółka kolumna + / numer poziomu / − i pod spodem Stop (okno 88 × 84,
  // rozmiar ustawia rdzeń - okna.rs BUBBLE_TRAINING_*); małe przyciski neutralne, Stop jak 8.1 „stop”.
  // Dane: wl:telemetria (enabled, mode, speedKmh, live) i wycinek „bubble” (motyw, kolor profilu,
  // tryb Trening i poziom).
  import { onMount } from 'svelte';
  import type { BubbleView, GaugeMode, PaletteColor } from '../lib/typy';
  import * as api from '../lib/api';
  import { t } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import { ustawAkcent, ustawMotyw } from '../lib/wyglad';
  import { IKONY } from '../lib/ui/Ikona.svelte';

  interface Props {
    /** Przesunięcie okna przez system (w Tauri; w podglądzie nic nie robi). */
    przeciagnijOkno: () => Promise<void>;
  }
  let { przeciagnijOkno }: Props = $props();

  let wlaczone = $state(false);
  let tryb = $state<GaugeMode>('offline');
  let predkosc = $state(0);
  let naZywo = $state(false);
  let wyglad = $state<BubbleView | null>(null);
  let paleta = $state.raw<PaletteColor[]>([]);
  let ciemnySystem = $state(matchMedia('(prefers-color-scheme: dark)').matches);
  let uruchomione = $state(false);

  const pokazPredkosc = $derived(wlaczone && naZywo);
  const tekstPredkosci = $derived(liczba(predkosc, 1));
  const stanSlowo = $derived(t(wlaczone ? 'dymek.stan.wl' : 'dymek.stan.wyl'));
  const trening = $derived(wyglad?.training ?? false);
  const poziom = $derived(wyglad?.trainingLevel ?? 0);
  const POZIOMY = 4;
  const tekstWyzej = $derived(t('dymek.trening.wyzej', { klawisz_szybciej: wyglad?.fasterLabel ?? '' }));
  const tekstNizej = $derived(t('dymek.trening.nizej', { klawisz_wolniej: wyglad?.slowerLabel ?? '' }));
  const tekstPoziomu = $derived(t('dymek.trening.poziom', { poziom: String(poziom + 1), liczba: String(POZIOMY) }));

  /** Błąd komendy z przycisku nie ma gdzie się pokazać - tylko do konsoli (dziennik okna). */
  const wyslij = (obietnica: Promise<unknown>) =>
    void obietnica.catch((e: unknown) => console.error('Przycisk na ekranie', e));

  function ustawWyglad(b: BubbleView) {
    wyglad = b;
    wlaczone = b.enabled;
  }

  onMount(() => {
    const media = matchMedia('(prefers-color-scheme: dark)');
    const zmianaMotywu = () => (ciemnySystem = media.matches);
    media.addEventListener('change', zmianaMotywu);
    const konce: Array<() => void> = [];
    let zamkniete = false;
    let revWygladu = -1;
    (async () => {
      konce.push(
        await api.sluchaj('wl:telemetria', (tel) => {
          wlaczone = tel.enabled;
          tryb = tel.mode;
          predkosc = tel.speedKmh;
          naZywo = tel.live;
        }),
        await api.sluchaj('wl:stan', (ev) => {
          if (ev.slice !== 'bubble' || ev.rev <= revWygladu) return;
          revWygladu = ev.rev;
          ustawWyglad(ev.value as BubbleView);
        }),
      );
      if (zamkniete) return;
      // Stan startowy (kolor profilu potrzebuje palety); zdarzenia nowsze niż migawka wygrywają.
      const migawka = await api.appSnapshot();
      paleta = migawka.statics.palette;
      if (migawka.rev > revWygladu) {
        revWygladu = migawka.rev;
        ustawWyglad(migawka.bubble);
      }
      await api.uiReady('przycisk');
    })()
      .catch((e) => console.error('Start przycisku na ekranie nie powiódł się', e))
      .finally(() => (uruchomione = true));
    return () => {
      zamkniete = true;
      media.removeEventListener('change', zmianaMotywu);
      for (const f of konce) f();
    };
  });

  // Motyw i kolor profilu: tylko zmienne CSS na <html>
  $effect(() => {
    const motyw = wyglad?.theme ?? 'system';
    ustawMotyw(motyw === 'system' ? (ciemnySystem ? 'dark' : 'light') : motyw);
    if (wyglad && paleta.length) ustawAkcent(wyglad.accent, paleta);
    if (uruchomione) document.documentElement.dataset.gotowe = '1';
  });
  // Podgląd w przeglądarce: prostokąt okna jak w Tauri (56 × 56 albo 88 × 84 w Treningu)
  $effect(() => {
    document.documentElement.classList.toggle('trening', trening);
  });

  // ---------------- klik, prawy klik, przeciąganie ----------------
  // Przesunięcie o > 4 px przy wciśniętym lewym przycisku = przeciąganie (jak FloatingToggle);
  // puszczenie bez przesunięcia = klik. W Tauri okno przesuwa system, więc puszczenia przycisku
  // strona może nie dostać - koniec przeciągania to też pierwszy ruch myszy bez wciśniętego przycisku.
  let nacisk: { x: number; y: number } | null = null;
  let przeciaga = $state(false);
  // Podgląd w przeglądarce: przycisk przesuwa się po stronie (w Tauri przesuwa się okno).
  let podgladX = $state(0);
  let podgladY = $state(0);
  let start = { x: 0, y: 0 };

  function nacisnij(e: PointerEvent) {
    if (e.button !== 0) return;
    // Poprzednie przeciąganie w Tauri mogło się skończyć bez żadnego zdarzenia (puszczenie
    // i ponowny klik bez ruchu myszy) - zamknij je, zanim zacznie się nowy klik.
    zakonczPrzeciaganie();
    nacisk = { x: e.screenX, y: e.screenY };
    start = { x: podgladX, y: podgladY };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function rusz(e: PointerEvent) {
    if (!nacisk) return;
    const dx = e.screenX - nacisk.x;
    const dy = e.screenY - nacisk.y;
    if (przeciaga) {
      if (api.wTauri) {
        if (e.buttons === 0) zakonczPrzeciaganie();
      } else {
        podgladX = start.x + dx;
        podgladY = start.y + dy;
      }
      return;
    }
    if (Math.abs(dx) + Math.abs(dy) <= 4) return;
    przeciaga = true;
    wyslij(api.bubbleDragStart());
    wyslij(przeciagnijOkno());
  }

  function pusc(e: PointerEvent) {
    if (e.button !== 0 || !nacisk) return;
    if (przeciaga) return zakonczPrzeciaganie();
    nacisk = null;
    wyslij(api.bubbleClick());
  }

  function zakonczPrzeciaganie() {
    nacisk = null;
    if (!przeciaga) return;
    przeciaga = false;
    wyslij(api.bubbleDragEnd());
  }

  function menu(e: MouseEvent) {
    e.preventDefault();
    wyslij(api.bubbleOpen());
  }

  // Klawiatura (Enter/Spacja) daje click bez wskaźnika (detail 0); klik myszą obsługuje pusc().
  function klik(e: MouseEvent) {
    if (e.detail === 0) wyslij(api.bubbleClick());
  }
</script>

<div class="uklad" class:trening style:transform={podgladX || podgladY ? `translate(${podgladX}px, ${podgladY}px)` : undefined}>
<button
  type="button"
  class="przycisk"
  class:wlaczone
  class:bieg={wlaczone && tryb === 'run'}
  class:przeciaga
  aria-pressed={wlaczone}
  aria-label={t('dymek.dostepnosc', { stan: stanSlowo })}
  title={t(wlaczone ? 'dymek.podpowiedz_wl' : 'dymek.podpowiedz_wyl')}
  onpointerdown={nacisnij}
  onpointermove={rusz}
  onpointerup={pusc}
  onpointercancel={zakonczPrzeciaganie}
  oncontextmenu={menu}
  onclick={klik}
>
  <svg viewBox="0 0 56 56" width="56" height="56" aria-hidden="true">
    <circle class="kolo" cx="28" cy="28" r="25" />
    <g class="srodek" class:wyzej={pokazPredkosc}>
      <text class="ikona" x="28" y="28" text-anchor="middle" dominant-baseline="central">{IKONY.chod}</text>
    </g>
    <text class="predkosc" class:widoczna={pokazPredkosc} x="28" y="41" text-anchor="middle" dominant-baseline="central"
      >{tekstPredkosci}</text
    >
  </svg>
</button>
{#if trening}
  <div class="kolumna">
    <button
      type="button"
      class="maly"
      disabled={poziom >= POZIOMY - 1}
      aria-label={tekstWyzej}
      title={tekstWyzej}
      onclick={() => wyslij(api.trainingLevelStep(1))}
    ><span class="glif" aria-hidden="true">{IKONY.dodaj}</span></button>
    <span class="poziom" aria-label={tekstPoziomu} title={tekstPoziomu}>{poziom + 1}</span>
    <button
      type="button"
      class="maly"
      disabled={poziom <= 0}
      aria-label={tekstNizej}
      title={tekstNizej}
      onclick={() => wyslij(api.trainingLevelStep(-1))}
    ><span class="glif" aria-hidden="true">{IKONY.minus}</span></button>
  </div>
  <button
    type="button"
    class="stop"
    aria-label={t('dymek.trening.stop_dostepnosc')}
    title={t('dymek.trening.stop_dostepnosc')}
    onclick={() => wyslij(api.trainingStop())}
  >{t('dymek.trening.stop')}</button>
{/if}
</div>

<style>
  /* Okno przezroczyste: widać tylko koło */
  :global(html.okno-przezroczyste),
  :global(html.okno-przezroczyste body) { background: transparent; }

  /* Podgląd w przeglądarce (?demo=1): przycisk na środku strony, tło okna motywu (na tle
     --control znikała ramka wyłączonego przycisku) */
  :global(html.podglad-okna body) {
    display: grid;
    place-items: center;
    background: var(--bg);
  }
  :global(html.podglad-okna #app) { width: 56px; height: 56px; }
  :global(html.podglad-okna.trening #app) { width: 88px; height: 84px; }

  /* Trening: kółko 56 + kolumna 28 (+, poziom, −), pod spodem Stop 24; odstępy 4 px */
  .uklad { display: grid; grid-template-columns: 56px; gap: 4px; width: max-content; }
  .uklad.trening { grid-template-columns: 56px 28px; grid-template-rows: 56px 24px; }

  .przycisk {
    display: block;
    width: 56px;
    height: 56px;
    border-radius: var(--r-round);
    touch-action: none;
    user-select: none;
    --wyp: var(--surface);
    --obrys: var(--control);
    --atrament: var(--text-2);
  }
  .przycisk:hover { --wyp: var(--surface-2); }
  .przycisk.wlaczone { --wyp: var(--accent); --obrys: var(--accent); --atrament: var(--on-accent); }
  .przycisk.bieg { --wyp: var(--run); --obrys: var(--run); }
  .przycisk.wlaczone:hover { --obrys: var(--text); }
  .przycisk.przeciaga { cursor: grabbing; }
  .przycisk:focus-visible { outline: none; }
  .przycisk:focus-visible .kolo { stroke: var(--focus); }

  /* Własna warstwa: napisy wygładzane w odcieniach szarości (bez kolorowych obwódek ClearType
     na przezroczystym oknie - w 0.5.x dlatego rysowano je jako ścieżki). */
  svg { will-change: transform; }
  .kolo {
    fill: var(--wyp);
    stroke: var(--obrys);
    stroke-width: 2;
    transition: fill var(--t-color) linear, stroke var(--t-color) linear;
  }
  .srodek {
    transition: transform var(--t-switch) var(--ease-smooth);
  }
  .srodek.wyzej { transform: translateY(-6px); }
  .ikona {
    fill: var(--atrament);
    font-family: var(--font-icon);
    font-size: 22px;
    transition: fill var(--t-color) linear;
  }
  .predkosc {
    fill: var(--atrament);
    font-family: var(--font-num);
    font-size: 12px;
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
    opacity: 0;
    transition: opacity var(--t-switch) var(--ease-smooth);
  }
  .predkosc.widoczna { opacity: 1; }

  /* Kolumna jak krokownik: jedna ramka, + i − bez własnych ramek, numer poziomu pośrodku
     (tło powierzchni - czytelne nad każdą grą) */
  .kolumna {
    display: grid;
    grid-template-rows: 20px 1fr 20px;
    min-width: 0;
    border: 1px solid var(--control);
    border-radius: var(--r-sm);
    background: var(--surface);
    overflow: hidden;
  }
  .maly,
  .stop {
    display: grid;
    place-items: center;
    color: var(--text);
    user-select: none;
    transition: background-color var(--t-color) linear, color var(--t-color) linear;
  }
  .maly:hover:not(:disabled) { background: var(--surface-2); }
  .maly:active:not(:disabled) { background: var(--surface-3); transition-duration: var(--t-press); }
  .maly:disabled { color: var(--text-2); }
  .glif { font-family: var(--font-icon); font-size: 12px; line-height: 1; }
  .poziom {
    display: grid;
    place-items: center;
    font-family: var(--font-num);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    font-variant-numeric: tabular-nums;
    line-height: 1;
    color: var(--text);
  }
  .stop {
    grid-column: 1 / -1;
    border: 1px solid var(--danger);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--danger);
    font-size: var(--fs-xs);
    font-weight: var(--fw-bold);
  }
  .stop:hover { background: var(--danger); color: var(--bg); }
  .maly:focus-visible,
  .stop:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
</style>
