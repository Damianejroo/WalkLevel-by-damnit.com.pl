<script lang="ts">
  // Przycisk na ekranie (bańka 56 × 56, docs/wyglad.md 9.7, kontrakt 7.3): koło; wyłączone -
  // tło powierzchni i ramka kontrolki; włączone - wypełnienie akcentem (bieg: kolor biegu),
  // ikona postaci i pod nią prędkość taśmy, gdy dane płyną. Jedyny element widoku = jedyny akcent.
  // Dane: wl:telemetria (enabled, mode, speedKmh, live) i wycinek „bubble” (motyw, kolor profilu).
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

<button
  type="button"
  class="przycisk"
  class:wlaczone
  class:bieg={wlaczone && tryb === 'run'}
  class:przeciaga
  style:transform={podgladX || podgladY ? `translate(${podgladX}px, ${podgladY}px)` : undefined}
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

<style>
  /* Okno przezroczyste: widać tylko koło */
  :global(html.okno-przezroczyste),
  :global(html.okno-przezroczyste body) { background: transparent; }

  /* Podgląd w przeglądarce (?demo=1): przycisk na środku strony */
  :global(html.podglad-okna body) {
    display: grid;
    place-items: center;
    background: var(--control);
  }
  :global(html.podglad-okna #app) { width: 56px; height: 56px; }

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
</style>
